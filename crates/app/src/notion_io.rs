//! Notion HTTP and Keychain access. Every request is a `/usr/bin/curl -K -` subprocess whose config
//! (token, URL, body) goes through stdin, so nothing secret reaches argv. Callers run it off the UI thread.

use ferrocast_core::notion::{self, NotionError};

/// Keychain service of the integration token.
pub const SERVICE: &str = "ferrocast-notion";

#[cfg(test)]
thread_local! {
    /// Test seam: base URL replacing `https://api.notion.com/v1` (a loopback server).
    static BASE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    /// Test seam: the fake Keychain item (`None` = no token), so tests never touch the real Keychain.
    static KEYCHAIN: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Applies the test base-URL override to a URL built by `notion::*_url()`. Call it on the thread
/// that owns the override (the UI thread), then move the result into the worker.
pub fn url(full: String) -> String {
    #[cfg(test)]
    if let Some(base) = BASE.with(|b| b.borrow().clone()) {
        return full.replacen("https://api.notion.com/v1", &base, 1);
    }
    full
}

/// A way to read the token that can move to a worker thread (the Keychain read is a subprocess).
pub type TokenReader = Box<dyn FnOnce() -> Option<String> + Send>;

pub fn token_reader() -> TokenReader {
    #[cfg(test)]
    let reader: TokenReader = {
        let v = KEYCHAIN.with(|k| k.borrow().clone());
        Box::new(move || v)
    };
    #[cfg(not(test))]
    let reader: TokenReader = Box::new(|| crate::secrets::secret_get(SERVICE));
    reader
}

/// Cached-presence check for the UI (one Keychain read).
pub fn has_token() -> bool {
    token_reader()().is_some()
}

/// Store the token (empty removes it). `false` when the Keychain refused.
pub fn set_token(value: &str) -> bool {
    #[cfg(test)]
    let ok = {
        KEYCHAIN.with(|k| *k.borrow_mut() = Some(value.to_string()).filter(|v| !v.is_empty()));
        true
    };
    #[cfg(not(test))]
    let ok = crate::secrets::secret_set(SERVICE, value);
    ok
}

#[cfg(test)]
thread_local! {
    /// Test seam: URLs the app asked the system to open and texts it put on the clipboard.
    static OPENED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    static COPIED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Open a web link with the default handler (Notion desktop takes it when installed).
pub fn open_url(url: &str) {
    #[cfg(test)]
    OPENED.with(|o| o.borrow_mut().push(url.to_string()));
    #[cfg(not(test))]
    {
        let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    }
}

/// Replace the clipboard with plain text (recorded instead in tests, which must not touch the real one).
pub fn write_clipboard(text: &str) {
    #[cfg(test)]
    COPIED.with(|o| o.borrow_mut().push(text.to_string()));
    #[cfg(not(test))]
    ferrocast_macos::clipboard::write_text(text);
}

#[cfg(test)]
pub fn take_opened() -> Vec<String> {
    OPENED.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

#[cfg(test)]
pub fn take_copied() -> Vec<String> {
    COPIED.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

/// "just now", "5m ago", "2h ago", "yesterday", then the date ("Mar 11", with the year when it is not this one).
/// `iso` is the API's RFC 3339 `last_edited_time`; unparsable text yields an empty string.
pub fn relative_time(iso: &str, now: chrono::DateTime<chrono::Utc>) -> String {
    use chrono::{Datelike, Local};
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) else { return String::new() };
    let secs = (now - t.with_timezone(&chrono::Utc)).num_seconds();
    match secs {
        s if s < 60 => "just now".into(),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3600),
        s if s < 2 * 86_400 => "yesterday".into(),
        _ => {
            let (d, n) = (t.with_timezone(&Local), now.with_timezone(&Local));
            if d.year() == n.year() {
                d.format("%b %-d").to_string()
            } else {
                d.format("%b %-d, %Y").to_string()
            }
        }
    }
}

pub fn curl_command() -> std::process::Command {
    let mut c = std::process::Command::new("/usr/bin/curl");
    c.args(["-K", "-"]);
    c
}

/// One request: the body of a 2xx answer, or the mapped error. A spawn failure is `Network`.
pub fn request(token: &str, method: &str, url: &str, body: Option<&str>) -> Result<String, NotionError> {
    request_with(curl_command(), token, method, url, body)
}

pub fn request_with(
    mut cmd: std::process::Command,
    token: &str,
    method: &str,
    url: &str,
    body: Option<&str>,
) -> Result<String, NotionError> {
    use std::io::Write;
    use std::process::Stdio;
    let config = notion::curl_config(token, method, url, body);
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_| NotionError::Network)?;
    if let Some(mut stdin) = child.stdin.take() {
        // dropping the handle closes stdin; curl reads the whole config first
        let _ = stdin.write_all(config.as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| NotionError::Network)?;
    let text = String::from_utf8_lossy(&out.stdout);
    notion::from_output(&text).map(str::to_string)
}

#[cfg(test)]
pub fn set_test_base(base: Option<String>) {
    BASE.with(|b| *b.borrow_mut() = base);
}

#[cfg(test)]
pub fn set_test_token(v: Option<&str>) {
    KEYCHAIN.with(|k| *k.borrow_mut() = v.map(str::to_string));
}

#[cfg(test)]
pub fn test_token() -> Option<String> {
    KEYCHAIN.with(|k| k.borrow().clone())
}

#[cfg(test)]
pub mod testing {
    use std::io::{Read, Write};

    /// Loopback server answering each connection with the next of `answers` (the last one repeats)
    /// until dropped; `requests()` is every raw request received so far, in order.
    pub struct Loopback {
        pub base: String,
        log: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl Loopback {
        pub fn requests(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
    }

    impl Drop for Loopback {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    pub fn serve_seq(answers: Vec<(u16, &'static str)>) -> Loopback {
        use std::sync::atomic::Ordering;
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.set_nonblocking(true).unwrap();
        let base = format!("http://{}/v1", l.local_addr().unwrap());
        let (log, stop) =
            (std::sync::Arc::new(std::sync::Mutex::new(vec![])), std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
        let (log2, stop2) = (log.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut n = 0;
            while !stop2.load(Ordering::SeqCst) {
                let Ok((mut s, _)) = l.accept() else {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                };
                s.set_nonblocking(false).unwrap();
                let req = read_request(&mut s);
                log2.lock().unwrap().push(req);
                let (status, body) = answers[n.min(answers.len() - 1)];
                n += 1;
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(resp.as_bytes());
            }
        });
        Loopback { base, log, stop }
    }

    fn read_request(s: &mut std::net::TcpStream) -> String {
        let mut buf: Vec<u8> = vec![];
        let mut chunk = [0u8; 4096];
        loop {
            let n = s.read(&mut chunk).unwrap_or(0);
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).to_string();
            if let Some(end) = text.find("\r\n\r\n") {
                let len = text[..end]
                    .lines()
                    .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                    .unwrap_or(0);
                if buf.len() >= end + 4 + len {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        String::from_utf8_lossy(&buf).to_string()
    }

    /// One-shot loopback server answering `status` + `body`; yields the raw request it received.
    pub fn serve(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf: Vec<u8> = vec![];
            let mut chunk = [0u8; 4096];
            loop {
                let n = s.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text[..end]
                        .lines()
                        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if buf.len() >= end + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let resp = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            s.write_all(resp.as_bytes()).unwrap();
            String::from_utf8_lossy(&buf).to_string()
        });
        (base, h)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::serve;
    use super::*;

    fn at(iso: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(iso).unwrap().with_timezone(&chrono::Utc)
    }

    #[test]
    fn relative_time_boundaries() {
        let now = at("2026-06-15T12:00:00Z");
        let rel = |iso: &str| relative_time(iso, now);
        assert_eq!(rel("2026-06-15T11:59:30.000Z"), "just now");
        assert_eq!(rel("2026-06-15T12:00:30.000Z"), "just now", "a clock a little ahead is not negative");
        assert_eq!(rel("2026-06-15T11:59:00.000Z"), "1m ago");
        assert_eq!(rel("2026-06-15T11:00:01.000Z"), "59m ago");
        assert_eq!(rel("2026-06-15T11:00:00.000Z"), "1h ago");
        assert_eq!(rel("2026-06-14T12:00:01.000Z"), "23h ago");
        assert_eq!(rel("2026-06-14T12:00:00.000Z"), "yesterday");
        assert_eq!(rel("2026-06-13T12:00:01.000Z"), "yesterday");
        assert_eq!(rel("not a date"), "");
    }

    #[test]
    fn relative_time_older_than_two_days_is_a_date_with_the_year_only_when_it_differs() {
        let now = at("2026-06-15T12:00:00Z");
        assert_eq!(relative_time("2026-03-11T12:00:00.000Z", now), "Mar 11");
        assert_eq!(relative_time("2025-12-24T12:00:00.000Z", now), "Dec 24, 2025");
    }

    #[test]
    fn the_keychain_service_name_is_stable() {
        assert_eq!(SERVICE, "ferrocast-notion");
    }

    #[test]
    fn curl_argv_never_holds_the_token_or_the_body() {
        let c = curl_command();
        let args: Vec<_> = c.get_args().map(|a| a.to_string_lossy().to_string()).collect();
        assert_eq!(args, ["-K", "-"]);
        let cfg = notion::curl_config("secret_tok", "POST", "https://x/v1/search", Some("{\"q\":\"hi\"}"));
        assert!(cfg.contains("secret_tok") && cfg.contains("hi"), "they travel on stdin");
    }

    #[test]
    fn success_returns_the_body_and_sends_token_and_version_on_stdin() {
        let (base, h) = serve(200, r#"{"ok":true}"#);
        let url = format!("{base}/search");
        let body = request("secret_tok", "POST", &url, Some(r#"{"query":"a b"}"#)).unwrap();
        assert_eq!(body, r#"{"ok":true}"#);
        let req = h.join().unwrap();
        assert!(req.starts_with("POST /v1/search"), "{req}");
        assert!(req.to_lowercase().contains("authorization: bearer secret_tok"), "{req}");
        assert!(req.contains("Notion-Version: 2026-03-11"), "{req}");
        assert!(req.contains(r#"{"query":"a b"}"#), "{req}");
    }

    #[test]
    fn a_401_is_the_exact_unauthorized_message() {
        let (base, h) = serve(401, r#"{"object":"error","status":401,"code":"unauthorized","message":"API token is invalid."}"#);
        let err = request("bad", "GET", &format!("{base}/users/me"), None).unwrap_err();
        assert_eq!(err, NotionError::Unauthorized);
        assert_eq!(err.user_message(), NotionError::Unauthorized.user_message());
        assert!(err.user_message().contains("Settings → Integrations"), "{}", err.user_message());
        let _ = h.join();
    }

    #[test]
    fn a_missing_status_marker_is_a_network_error() {
        // nothing listens on this port: curl prints no body, and the marker is 000
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let err = request("t", "GET", &format!("http://127.0.0.1:{port}/v1/users/me"), None).unwrap_err();
        assert_eq!(err, NotionError::Network);
        // a server whose answer carries no marker at all (curl missing): from_output says Network too
        assert_eq!(notion::from_output("{}").unwrap_err(), NotionError::Network);
    }

    #[test]
    fn a_spawn_failure_is_a_network_error() {
        let cmd = std::process::Command::new("/nonexistent/curl");
        assert_eq!(request_with(cmd, "t", "GET", "http://x", None).unwrap_err(), NotionError::Network);
    }

    #[test]
    fn the_test_base_rewrites_only_the_api_prefix() {
        assert_eq!(url(notion::me_url()), "https://api.notion.com/v1/users/me");
        set_test_base(Some("http://127.0.0.1:9/v1".into()));
        assert_eq!(url(notion::search_url()), "http://127.0.0.1:9/v1/search");
        set_test_base(None);
    }
}
