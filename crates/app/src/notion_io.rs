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

/// Plain text on the clipboard (`None`: empty or not text). Tests set it with `set_test_clipboard`.
pub fn read_clipboard() -> Option<String> {
    #[cfg(test)]
    let t = CLIP.with(|c| c.borrow().clone());
    #[cfg(not(test))]
    let t = ferrocast_macos::clipboard::read_text();
    t
}

#[cfg(test)]
thread_local! {
    static CLIP: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_test_clipboard(v: Option<&str>) {
    CLIP.with(|c| *c.borrow_mut() = v.map(str::to_string));
}

/// Maps an API URL like [`url`] does, but can move to a worker thread (it carries the test base URL).
pub type UrlMapper = Box<dyn Fn(String) -> String + Send>;

pub fn url_mapper() -> UrlMapper {
    #[cfg(test)]
    let m: UrlMapper = {
        let base = BASE.with(|b| b.borrow().clone());
        Box::new(move |full| match &base {
            Some(b) => full.replacen("https://api.notion.com/v1", b, 1),
            None => full,
        })
    };
    #[cfg(not(test))]
    let m: UrlMapper = Box::new(|full| full);
    m
}

#[cfg(test)]
pub fn take_opened() -> Vec<String> {
    OPENED.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

#[cfg(test)]
pub fn take_copied() -> Vec<String> {
    COPIED.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

// ---------------------------------------------------------------- capture (Add to Notion)

/// Most characters of markdown sent in one request (the API accepts more; this keeps every call small).
pub const MAX_PART: usize = 100_000;
/// Longest page title taken from the first line of the text.
const TITLE_MAX: usize = 100;

/// What to send and where.
#[derive(Clone, Debug)]
pub struct Capture {
    pub target: ferrocast_core::models::NotionTarget,
    pub title: String,
    pub text: String,
}

/// Title and markdown body of a database page: the given title, else the first non-empty line cut at
/// 100 characters. The line leaves the body only when it fit whole in the title, so a cut never loses text.
pub fn database_title_and_body(title: &str, text: &str) -> (String, String) {
    let title = title.trim();
    if !title.is_empty() {
        return (title.to_string(), text.to_string());
    }
    let mut rest = text;
    loop {
        let (line, after) = rest.split_once('\n').unwrap_or((rest, ""));
        if line.trim().is_empty() {
            if after.is_empty() {
                return (String::new(), String::new());
            }
            rest = after;
            continue;
        }
        let line = line.trim();
        return if line.chars().count() <= TITLE_MAX {
            (line.to_string(), after.trim_start_matches('\n').to_string())
        } else {
            (line.chars().take(TITLE_MAX).collect(), text.to_string())
        };
    }
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// Split markdown into parts of at most `max` characters, cutting only at blank lines outside fenced
/// code blocks. A block longer than `max` is cut by lines (then characters); a cut fence is closed and
/// reopened so every part stays balanced. Concatenating the parts gives back the text (plus fences added).
pub fn split_markdown(text: &str, max: usize) -> Vec<String> {
    let mut blocks: Vec<String> = vec![];
    let (mut cur, mut in_fence, mut prev_blank) = (String::new(), false, false);
    for line in text.split_inclusive('\n') {
        let blank = line.trim().is_empty();
        // a block ends after its trailing blank lines: the next non-blank line outside a fence starts a new one
        if prev_blank && !blank && !in_fence && !cur.is_empty() {
            blocks.push(std::mem::take(&mut cur));
        }
        if is_fence(line) {
            in_fence = !in_fence;
        }
        cur.push_str(line);
        prev_blank = blank && !in_fence;
    }
    if !cur.is_empty() {
        blocks.push(cur);
    }
    let len = |s: &str| s.chars().count();
    let mut parts: Vec<String> = vec![];
    let mut acc = String::new();
    for b in blocks {
        if len(&b) > max {
            if !acc.is_empty() {
                parts.push(std::mem::take(&mut acc));
            }
            parts.extend(cut_block(&b, max));
        } else if !acc.is_empty() && len(&acc) + len(&b) > max {
            parts.push(std::mem::replace(&mut acc, b));
        } else {
            acc.push_str(&b);
        }
    }
    if !acc.is_empty() {
        parts.push(acc);
    }
    parts
}

/// Cut one block longer than `max` by lines; a fence open at the cut is closed and reopened.
fn cut_block(block: &str, max: usize) -> Vec<String> {
    const CLOSE: &str = "```\n";
    let budget = max.saturating_sub(CLOSE.len() + 16).max(1);
    let len = |s: &str| s.chars().count();
    let mut out: Vec<String> = vec![];
    let (mut cur, mut opener): (String, Option<String>) = (String::new(), None);
    let flush = |cur: &mut String, opener: &Option<String>, out: &mut Vec<String>| {
        if cur.is_empty() {
            return;
        }
        let mut piece = std::mem::take(cur);
        if opener.is_some() {
            if !piece.ends_with('\n') {
                piece.push('\n');
            }
            piece.push_str(CLOSE);
        }
        out.push(piece);
        if let Some(o) = opener {
            cur.push_str(o);
        }
    };
    for line in block.split_inclusive('\n') {
        // very long lines are chunked by characters
        let chunks: Vec<String> = if len(line) > budget {
            line.chars().collect::<Vec<_>>().chunks(budget).map(|c| c.iter().collect()).collect()
        } else {
            vec![line.to_string()]
        };
        for piece in chunks {
            if !cur.is_empty() && len(&cur) + len(&piece) > budget {
                flush(&mut cur, &opener, &mut out);
            }
            cur.push_str(&piece);
        }
        if is_fence(line) {
            opener = match opener {
                Some(_) => None,
                None => Some(line.trim_end_matches('\n').to_string() + "\n"),
            };
        }
    }
    if !cur.is_empty() && cur != opener.clone().unwrap_or_default() {
        out.push(cur);
    }
    out
}

/// A request that waits and retries once on 429 (`Retry-After` when known, else 1 s; at most 10 s).
fn request_retry(token: &str, method: &str, url: &str, body: Option<&str>) -> Result<String, NotionError> {
    match request(token, method, url, body) {
        Err(NotionError::RateLimited { retry_after_secs }) => {
            std::thread::sleep(std::time::Duration::from_secs(retry_after_secs.unwrap_or(1).min(10)));
            request(token, method, url, body)
        }
        r => r,
    }
}

/// Add the capture to Notion, sequentially. `Err` is the exact toast text. A database target caches the
/// name of its title property in `schema` (per target id) so only the first send reads it.
pub fn send_capture(
    token: &str,
    c: &Capture,
    map: &dyn Fn(String) -> String,
    schema: &std::sync::Mutex<std::collections::HashMap<String, String>>,
) -> Result<(), String> {
    let is_page = c.target.kind != "data_source";
    let (title, markdown) = if is_page {
        let md = match (c.title.trim(), c.text.is_empty()) {
            ("", _) => c.text.clone(),
            (t, true) => format!("## {t}"),
            (t, false) => format!("## {t}\n\n{}", c.text),
        };
        (String::new(), md)
    } else {
        database_title_and_body(&c.title, &c.text)
    };
    let parts = split_markdown(&markdown, MAX_PART);
    let total = if is_page { parts.len() } else { parts.len().max(1) };
    let fail = |done: usize, e: NotionError| {
        if done == 0 {
            e.user_message()
        } else {
            format!("Added {done} of {total} parts to {}; the rest were not sent. {}", c.target.title, e.user_message())
        }
    };
    let mut done = 0;
    let mut page_id = c.target.id.clone();
    let mut rest = &parts[..];
    if !is_page {
        let cached = schema.lock().ok().and_then(|m| m.get(&c.target.id).cloned());
        let prop = match cached {
            Some(p) => p,
            None => {
                let body = request_retry(token, "GET", &map(notion::data_source_url(&c.target.id)), None).map_err(|e| fail(0, e))?;
                let p = notion::parse_data_source_title_property(&body).unwrap_or_else(|| "Name".into());
                if let Ok(mut m) = schema.lock() {
                    m.insert(c.target.id.clone(), p.clone());
                }
                p
            }
        };
        let first = parts.first().map(String::as_str).unwrap_or("");
        let body = notion::create_page_body(&notion::Parent::DataSource(c.target.id.clone()), &prop, &title, first);
        let out = request_retry(token, "POST", &map(notion::pages_url()), Some(&body)).map_err(|e| fail(0, e))?;
        // the page exists once the POST answered 2xx, even if the answer could not be read
        page_id = match notion::parse_created_page(&out) {
            Ok((id, _)) => id,
            Err(e) => return if total == 1 { Ok(()) } else { Err(fail(1, e)) },
        };
        done = 1;
        rest = parts.get(1..).unwrap_or(&[]);
    }
    for part in rest {
        let body = notion::append_markdown_body(part);
        request_retry(token, "PATCH", &map(notion::page_markdown_url(&page_id)), Some(&body)).map_err(|e| fail(done, e))?;
        done += 1;
    }
    Ok(())
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

#[cfg(test)]
mod capture_tests {
    use super::*;

    fn paras(n: usize, len: usize) -> String {
        (0..n).map(|i| format!("{i:04}{}", "z".repeat(len))).collect::<Vec<_>>().join("\n\n")
    }

    #[test]
    fn short_text_is_one_part_and_empty_text_is_none() {
        assert_eq!(split_markdown("hello\n\nworld", 100), ["hello\n\nworld"]);
        assert!(split_markdown("", 100).is_empty());
    }

    #[test]
    fn parts_split_at_blank_lines_and_rejoin_to_the_original() {
        let text = paras(30, 40);
        let parts = split_markdown(&text, 500);
        assert!(parts.len() > 2);
        assert!(parts.iter().all(|p| p.chars().count() <= 500));
        assert_eq!(parts.concat(), text);
        assert!(parts.iter().all(|p| p.starts_with(|c: char| c.is_ascii_digit())), "cuts fall between paragraphs");
    }

    #[test]
    fn a_fenced_block_with_blank_lines_is_never_split_when_it_fits() {
        let fence = format!("```rust\nfn a() {{}}\n\n\nfn b() {{}}\n{}\n```", "// c\n".repeat(20));
        let text = format!("{}\n\n{}\n\n{}", paras(5, 60), fence, paras(5, 60));
        let parts = split_markdown(&text, 600);
        assert_eq!(parts.concat(), text);
        for p in &parts {
            assert_eq!(p.matches("```").count() % 2, 0, "every part has balanced fences:\n{p}");
        }
    }

    #[test]
    fn a_single_block_over_the_limit_is_cut_by_lines_and_keeps_fences_balanced() {
        let body = "line of code\n".repeat(200);
        let text = format!("```\n{body}```");
        let parts = split_markdown(&text, 500);
        assert!(parts.len() > 3);
        assert!(parts.iter().all(|p| p.chars().count() <= 500), "{:?}", parts.iter().map(|p| p.chars().count()).collect::<Vec<_>>());
        for p in &parts {
            assert_eq!(p.matches("```").count() % 2, 0, "{p}");
        }
        // a long line without breaks is cut on a character boundary
        let long = "é".repeat(1200);
        let parts = split_markdown(&long, 500);
        assert_eq!(parts.concat(), long);
    }

    #[test]
    fn database_title_rules() {
        let (t, b) = database_title_and_body("", "One\ntwo");
        assert_eq!((t.as_str(), b.as_str()), ("One", "two"));
        let (t, b) = database_title_and_body("Given", "One\ntwo");
        assert_eq!((t.as_str(), b.as_str()), ("Given", "One\ntwo"));
        let (t, b) = database_title_and_body("", "   ");
        assert_eq!((t.as_str(), b.as_str()), ("", ""));
    }
}
