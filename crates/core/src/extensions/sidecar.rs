//! The Node sidecar process: spawn, line protocol, kill. One process per launched command; it is
//! killed when the `Sidecar` is dropped (back to the root, timeout, or app exit).

use super::protocol::{encode_host, parse_runtime, HostMsg, RuntimeMsg};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug)]
pub enum Event {
    Msg(RuntimeMsg),
    /// A stdout line that is not a protocol message (kept for the log).
    BadLine(String),
    /// One line of stderr.
    Stderr(String),
    /// A stdout line exceeded `MAX_LINE`; the process group was killed.
    TooLong,
    /// stdout closed: the process is gone.
    Closed,
    /// The answer to a runtime `Call` that took a background thread (the host built it off the UI
    /// thread). Never produced by the sidecar itself.
    Reply {
        call: u64,
        result: Result<serde_json::Value, String>,
    },
}

/// Longest stdout line accepted.
pub const MAX_LINE: usize = 16 * 1024 * 1024;
/// Stderr lines forwarded as events; the rest is drained and dropped.
pub const MAX_STDERR_LINES: usize = 500;
/// Host -> runtime messages that may wait for the writer thread before the command is declared stuck.
const WRITE_QUEUE: usize = 256;

/// The environment variables a sidecar receives (everything else of the launcher's is withheld).
pub fn filter_env(vars: impl IntoIterator<Item = (OsString, OsString)>) -> Vec<(OsString, OsString)> {
    const ALLOWED: &[&str] = &[
        "PATH",
        "HOME",
        "LANG",
        "TMPDIR",
        "USER",
        "LOGNAME",
        "SHELL",
        "TZ",
        "TERM",
        "SSH_AUTH_SOCK",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "no_proxy",
        "SSL_CERT_FILE",
        "NODE_EXTRA_CA_CERTS",
        "__CF_USER_TEXT_ENCODING",
    ];
    // names are matched as UTF-8 (a non-Unicode name is never allowed); values pass through untouched
    vars.into_iter()
        .filter(|(k, _)| k.to_str().is_some_and(|k| ALLOWED.contains(&k) || k.starts_with("LC_") || k.starts_with("XDG_")))
        .collect()
}

/// Read one line, keeping at most `cap` bytes. Returns `None` at end of input; the flag is true when
/// the line was longer than `cap` (the rest of it is consumed and dropped).
fn read_line_capped(r: &mut impl BufRead, cap: usize) -> std::io::Result<Option<(Vec<u8>, bool)>> {
    let mut buf = Vec::new();
    let mut over = false;
    let mut any = false;
    loop {
        let chunk = r.fill_buf()?;
        if chunk.is_empty() {
            return Ok(any.then_some((buf, over)));
        }
        any = true;
        let (take, done) = match chunk.iter().position(|b| *b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        let room = cap.saturating_sub(buf.len());
        if take - usize::from(done) > room {
            over = true;
        }
        buf.extend_from_slice(&chunk[..(take - usize::from(done)).min(room)]);
        r.consume(take);
        if done {
            return Ok(Some((buf, over)));
        }
    }
}

/// The sidecar's process group, shared with the reader thread. The id is handed out once: after the
/// first kill it is 0, so nothing signals a pid the system may have reused for another process.
struct Group {
    pgid: std::sync::atomic::AtomicI32,
    signals: std::sync::atomic::AtomicUsize,
}

impl Group {
    fn kill(&self) {
        use std::sync::atomic::Ordering::SeqCst;
        let pgid = self.pgid.swap(0, SeqCst);
        if pgid > 0 {
            self.signals.fetch_add(1, SeqCst);
            // SAFETY: plain signal syscall on the process group we created for the sidecar, once.
            unsafe { libc::killpg(pgid, libc::SIGKILL) };
        }
    }
}

pub struct Sidecar {
    child: Child,
    group: Arc<Group>,
    tx: Option<SyncSender<String>>,
    pub started: Instant,
}

impl Sidecar {
    /// Start `node <runtime_js>` in its own process group; `on_event` is called from reader threads.
    pub fn spawn(node: &Path, runtime_js: &Path, cwd: &Path, on_event: impl Fn(Event) + Send + Sync + 'static) -> std::io::Result<Sidecar> {
        Self::spawn_with_env(node, runtime_js, cwd, std::env::vars_os(), on_event)
    }

    /// Like `spawn`, with the launcher's environment given explicitly (it is filtered by `filter_env`).
    pub fn spawn_with_env(
        node: &Path,
        runtime_js: &Path,
        cwd: &Path,
        vars: impl IntoIterator<Item = (OsString, OsString)>,
        on_event: impl Fn(Event) + Send + Sync + 'static,
    ) -> std::io::Result<Sidecar> {
        use std::os::unix::process::CommandExt;
        let mut child = Command::new(node)
            .arg("--no-warnings")
            .arg(runtime_js)
            .current_dir(cwd)
            .env_clear()
            .envs(filter_env(vars))
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let pgid = child.id() as i32;
        let group = Arc::new(Group { pgid: std::sync::atomic::AtomicI32::new(pgid), signals: Default::default() });
        let g = group.clone();
        let on: Arc<dyn Fn(Event) + Send + Sync> = Arc::new(on_event);
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let o = on.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            while let Ok(Some((line, over))) = read_line_capped(&mut r, MAX_LINE) {
                if over {
                    g.kill();
                    o(Event::TooLong);
                    break;
                }
                let line = String::from_utf8_lossy(&line);
                if line.trim().is_empty() {
                    continue;
                }
                match parse_runtime(&line) {
                    Ok(m) => o(Event::Msg(m)),
                    Err(_) => o(Event::BadLine(line.chars().take(300).collect())),
                }
            }
            o(Event::Closed);
        });
        let o = on;
        std::thread::spawn(move || {
            // keep draining after the cap so the child never writes to a closed pipe
            let mut r = BufReader::new(stderr);
            let mut n = 0usize;
            while let Ok(Some((line, _))) = read_line_capped(&mut r, 500) {
                n += 1;
                if n <= MAX_STDERR_LINES {
                    o(Event::Stderr(String::from_utf8_lossy(&line).into_owned()));
                } else if n == MAX_STDERR_LINES + 1 {
                    o(Event::Stderr("output truncated: further stderr lines are discarded".into()));
                }
            }
        });
        let mut stdin = child.stdin.take().expect("piped");
        let (tx, rx) = sync_channel::<String>(WRITE_QUEUE);
        std::thread::spawn(move || {
            for line in rx {
                if stdin.write_all(line.as_bytes()).and_then(|_| stdin.flush()).is_err() {
                    break;
                }
            }
        });
        Ok(Sidecar { child, group, tx: Some(tx), started: Instant::now() })
    }

    /// Queue a message for the runtime. Never blocks: when the runtime stops reading and the queue
    /// fills up, the command is killed and an error is returned.
    pub fn send(&mut self, m: &HostMsg) -> std::io::Result<()> {
        use std::io::{Error, ErrorKind};
        let tx = self.tx.as_ref().ok_or_else(|| Error::new(ErrorKind::BrokenPipe, "sidecar stopped"))?;
        let mut line = encode_host(m);
        line.push('\n');
        match tx.try_send(line) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.kill();
                Err(Error::new(ErrorKind::WouldBlock, "the extension stopped reading its input and was stopped"))
            }
            Err(TrySendError::Disconnected(_)) => Err(Error::new(ErrorKind::BrokenPipe, "sidecar stdin closed")),
        }
    }

    /// How many process-group signals this sidecar has sent (tests).
    pub fn signals_sent(&self) -> usize {
        self.group.signals.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Kill the process and everything it started (its process group).
    pub fn kill(&mut self) {
        self.tx = None;
        self.group.kill();
        let _ = self.child.kill();
        let _ = self.child.wait();
        // reaped: from here on the pid belongs to the system again
        self.group.pgid.store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::protocol::Environment;
    use crate::extensions::view::{build, ViewModel};
    use serde_json::{json, Map};
    use std::sync::mpsc::{channel, Receiver};
    use std::time::Duration;

    fn next_msg(rx: &Receiver<Event>) -> RuntimeMsg {
        loop {
            match rx.recv_timeout(Duration::from_secs(20)).expect("sidecar went quiet") {
                Event::Msg(m) => return m,
                Event::Stderr(l) => eprintln!("[sidecar stderr] {l}"),
                Event::BadLine(l) => panic!("bad stdout line: {l}"),
                Event::TooLong => panic!("line too long"),
                Event::Closed => panic!("sidecar closed early"),
                Event::Reply { .. } => {}
            }
        }
    }

    /// Launch a fixture through the real sidecar; `None` (test skipped) without Node >= 22.
    fn launch(name: &str) -> Option<(Sidecar, Receiver<Event>, std::path::PathBuf)> {
        launch_mode(name, "view")
    }

    fn launch_mode(name: &str, mode: &str) -> Option<(Sidecar, Receiver<Event>, std::path::PathBuf)> {
        // unique per launch: tests run in parallel and several launch the same fixture
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::temp_dir().join(format!("fc-sidecar-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let Some(node) = crate::extensions::node::find_node(&tmp) else {
            eprintln!("skipped: no Node >= 22 found");
            return None;
        };
        let runtime = crate::extensions::write_runtime(&tmp).unwrap();
        let fixture = format!("{}/src/extensions/fixtures/{name}.js", env!("CARGO_MANIFEST_DIR"));
        let (tx, rx) = channel();
        let tx = std::sync::Mutex::new(tx);
        let mut sc = Sidecar::spawn(&node.path, &runtime, &tmp, move |e| {
            let _ = tx.lock().unwrap().send(e);
        })
        .unwrap();
        sc.send(&HostMsg::Launch {
            entry: fixture,
            extension: "fixture".into(),
            command: "two-items".into(),
            title: "Two items".into(),
            mode: mode.into(),
            arguments: Map::new(),
            preferences: Map::new(),
            environment: Environment {
                extension_name: "fixture".into(),
                command_name: "two-items".into(),
                support_path: tmp.join("support").to_string_lossy().into(),
                assets_path: tmp.to_string_lossy().into(),
                is_development: false,
                raycast_version: "1.0.0".into(),
                launch_type: "userInitiated".into(),
            },
        })
        .unwrap();
        Some((sc, rx, tmp))
    }

    fn next_render(rx: &Receiver<Event>) -> (usize, crate::extensions::protocol::Node) {
        loop {
            if let RuntimeMsg::Render { depth, tree } = next_msg(rx) {
                return (depth, tree);
            }
        }
    }

    /// Runs the fixture List extension through the real Node sidecar. Skipped without Node >= 22.
    fn os(k: &str, v: &str) -> (OsString, OsString) {
        (OsString::from(k), OsString::from(v))
    }

    #[test]
    fn env_allowlist_drops_secrets() {
        let vars = [
            "PATH",
            "HOME",
            "LANG",
            "LC_ALL",
            "TMPDIR",
            "USER",
            "SHELL",
            "HTTPS_PROXY",
            "SSH_AUTH_SOCK",
            "TZ",
            "TERM",
            "XDG_CONFIG_HOME",
            "NODE_OPTIONS",
            "OPENAI_API_KEY",
            "AWS_SECRET_ACCESS_KEY",
            "FERROCAST_DATA_DIR",
            "LCX",
            "XDGX",
        ]
        .map(|k| os(k, "v"));
        let kept: Vec<String> = filter_env(vars).into_iter().map(|(k, _)| k.into_string().unwrap()).collect();
        assert_eq!(
            kept,
            vec![
                "PATH",
                "HOME",
                "LANG",
                "LC_ALL",
                "TMPDIR",
                "USER",
                "SHELL",
                "HTTPS_PROXY",
                "SSH_AUTH_SOCK",
                "TZ",
                "TERM",
                "XDG_CONFIG_HOME"
            ]
        );
    }

    #[test]
    fn env_filter_handles_non_unicode_without_panicking() {
        use std::os::unix::ffi::OsStringExt;
        let bad = OsString::from_vec(vec![b'x', 0xff, 0xfe]);
        let vars = vec![
            (OsString::from("PATH"), bad.clone()),
            (bad.clone(), OsString::from("v")),
            (OsString::from("SECRET"), bad.clone()),
            os("HOME", "/h"),
        ];
        assert_eq!(filter_env(vars), vec![(OsString::from("PATH"), bad), os("HOME", "/h")]);
    }

    #[test]
    fn killing_twice_signals_the_process_group_only_once() {
        let Some((mut sc, _rx, tmp)) = launch("two-items") else { return };
        sc.kill();
        sc.kill();
        assert_eq!(sc.signals_sent(), 1, "a reaped child's pid may be reused: never signal it again");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn fixture_list_extension_renders_and_copies_through_the_real_sidecar() {
        let Some((mut sc, rx, tmp)) = launch("two-items") else { return };
        let (depth, tree) = next_render(&rx);
        assert_eq!(depth, 1);
        let ViewModel::List(list) = build(&tree) else { panic!("expected a list") };
        let items: Vec<_> = list.sections.iter().flat_map(|s| &s.items).collect();
        assert_eq!(items.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), vec!["Alpha", "Beta"]);
        assert_eq!(items[0].accessories, vec!["A"]);
        let copy = &items[0].actions[0];
        assert_eq!((copy.kind.as_str(), copy.title.as_str()), ("Action.CopyToClipboard", "Copy Alpha"));

        sc.send(&HostMsg::Action { id: copy.id.clone(), values: None }).unwrap();
        let (id, params) = loop {
            if let RuntimeMsg::Call { id, method, params } = next_msg(&rx) {
                assert_eq!(method, "clipboard.copy");
                break (id, params);
            }
        };
        assert_eq!(params["text"], json!("alpha-value"));
        sc.send(&HostMsg::Result { id, value: json!(null), error: None }).unwrap();
        loop {
            if let RuntimeMsg::Hud { title } = next_msg(&rx) {
                assert_eq!(title, "Copied to Clipboard");
                break;
            }
        }
        drop(sc);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Search text reaches the extension, Action.Push opens a second screen, Pop returns. Skipped without Node.
    #[test]
    fn fixture_push_pop_and_search_through_the_real_sidecar() {
        let Some((mut sc, rx, tmp)) = launch("push-detail") else { return };
        let (_, tree) = next_render(&rx);
        let ViewModel::List(l) = build(&tree) else { panic!("list") };
        assert!(l.handles_search && !l.filtering);
        assert_eq!(l.sections[0].items[0].title, "Item ");
        sc.send(&HostMsg::Search { text: "xyz".into() }).unwrap();
        let (_, tree) = next_render(&rx);
        let ViewModel::List(l) = build(&tree) else { panic!("list") };
        assert_eq!(l.sections[0].items[0].title, "Item xyz");
        let push = l.sections[0].items[0].actions[0].id.clone();
        sc.send(&HostMsg::Action { id: push, values: None }).unwrap();
        let (depth, tree) = next_render(&rx);
        assert_eq!(depth, 2);
        let ViewModel::Detail(d) = build(&tree) else { panic!("detail") };
        assert!(d.markdown.starts_with("# Pushed"));
        assert_eq!(d.actions[0].kind, "Action.OpenInBrowser");
        sc.send(&HostMsg::Pop).unwrap();
        let (depth, _) = next_render(&rx);
        assert_eq!(depth, 1);
        drop(sc);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A form through the real sidecar: edits reach `onChange`, the controlled value comes back, and
    /// ⌘↵ submits the form's values (as `FormState` holds them). Skipped without Node.
    #[test]
    fn fixture_form_edit_and_submit_through_the_real_sidecar() {
        use crate::extensions::view::FormState;
        let Some((mut sc, rx, tmp)) = launch("form") else { return };
        let (_, tree) = next_render(&rx);
        let ViewModel::Form(f) = build(&tree) else { panic!("form") };
        let mut st = FormState::default();
        st.sync(&f);
        assert_eq!(st.get("name"), json!("ann"));
        st.set("name", json!("bob"));
        st.set("when", json!("2024-05-06"));
        sc.send(&HostMsg::Field { id: "name".into(), value: json!("bob") }).unwrap();
        let (_, tree) = next_render(&rx);
        let ViewModel::Form(f) = build(&tree) else { panic!("form") };
        st.sync(&f);
        assert_eq!(st.get("name"), json!("BOB"), "the controlled value follows the extension");
        assert_eq!(st.get("when"), json!("2024-05-06"), "an uncontrolled edit survives");
        let submit = f.actions[0].id.clone();
        sc.send(&HostMsg::Action { id: submit, values: Some(st.values(&f)) }).unwrap();
        let params = loop {
            if let RuntimeMsg::Call { id, method, params } = next_msg(&rx) {
                if method == "localStorage.setItem" {
                    sc.send(&HostMsg::Result { id, value: json!(null), error: None }).unwrap();
                    break params;
                }
            }
        };
        let got: serde_json::Value = serde_json::from_str(params["value"].as_str().unwrap()).unwrap();
        assert_eq!(got["name"], json!("BOB"));
        assert_eq!(got["ok"], json!(true));
        assert_eq!(got["color"], json!("r"), "a dropdown starts on its first option");
        assert_eq!(got["whenLocal"], json!("2024,5,6,0,0"), "a typed YYYY-MM-DD is local midnight, in any time zone");
        drop(sc);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Collect events for `ms` milliseconds (at most `cap` of them).
    fn collect(rx: &Receiver<Event>, ms: u64, cap: usize) -> Vec<Event> {
        let end = std::time::Instant::now() + Duration::from_millis(ms);
        let mut v = vec![];
        while v.len() < cap {
            let left = end.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(left) {
                Ok(e) => v.push(e),
                Err(_) => break,
            }
        }
        v
    }

    /// B1: a command that prints 5,000 lines must not flood the host, and must stay killable.
    #[test]
    fn chatty_extension_produces_bounded_events_and_can_be_killed() {
        let Some((mut sc, rx, tmp)) = launch("chatty") else { return };
        let events = collect(&rx, 4000, 200_000);
        let toasts = events.iter().filter(|e| matches!(e, Event::Msg(RuntimeMsg::Toast { .. }))).count();
        let stderr = events.iter().filter(|e| matches!(e, Event::Stderr(_))).count();
        let renders = events.iter().filter(|e| matches!(e, Event::Msg(RuntimeMsg::Render { .. }))).count();
        assert!(renders >= 1, "the list still rendered");
        assert!(toasts <= 5, "toasts: {toasts}");
        assert!(stderr <= 600, "stderr events: {stderr}");
        assert!(events.len() < 1000, "events: {}", events.len());
        assert!(!events.iter().any(|e| matches!(e, Event::Closed)), "the extension is still running");
        assert!(sc.send(&HostMsg::Pop).is_ok());
        let t = std::time::Instant::now();
        sc.kill();
        assert!(t.elapsed() < Duration::from_secs(3));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// B1 (runtime side): once stderr is a broken pipe the runtime must exit, not loop on EPIPE.
    #[test]
    fn runtime_exits_instead_of_looping_when_its_stderr_is_broken() {
        let tmp = std::env::temp_dir().join(format!("fc-sidecar-epipe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let Some(node) = crate::extensions::node::find_node(&tmp) else { return };
        let runtime = crate::extensions::write_runtime(&tmp).unwrap();
        let fixture = format!("{}/src/extensions/fixtures/chatty.js", env!("CARGO_MANIFEST_DIR"));
        let mut child =
            Command::new(&node.path).arg(&runtime).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        drop(child.stderr.take()); // read end closed: every stderr write is EPIPE
        let launch = encode_host(&HostMsg::Launch {
            entry: fixture,
            extension: "f".into(),
            command: "c".into(),
            title: "C".into(),
            mode: "view".into(),
            arguments: Map::new(),
            preferences: Map::new(),
            environment: Environment {
                extension_name: "f".into(),
                command_name: "c".into(),
                support_path: tmp.to_string_lossy().into(),
                assets_path: tmp.to_string_lossy().into(),
                is_development: false,
                raycast_version: "1".into(),
                launch_type: "userInitiated".into(),
            },
        });
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{launch}").unwrap();
        let stdout = child.stdout.take().unwrap();
        let reader =
            std::thread::spawn(move || BufReader::new(stdout).lines().map_while(Result::ok).filter(|l| l.contains("\"toast\"")).count());
        let end = std::time::Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(st) = child.try_wait().unwrap() {
                break Some(st);
            }
            if std::time::Instant::now() > end {
                let _ = child.kill();
                break None;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let toasts = reader.join().unwrap();
        assert!(status.is_some(), "the runtime kept running on a broken stderr");
        assert!(toasts <= 5, "toasts: {toasts}");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Fix 2: killing the sidecar kills what the extension started.
    #[test]
    fn killing_the_sidecar_kills_its_process_group() {
        let Some((mut sc, rx, tmp)) = launch("child-spawner") else { return };
        let mut pid = None;
        for e in collect(&rx, 3000, 1000) {
            if let Event::Stderr(l) = e {
                if let Some(p) = l.strip_prefix("CHILD_PID ") {
                    pid = p.trim().parse::<i32>().ok();
                }
            }
        }
        let pid = pid.expect("the fixture reports its child");
        let alive = |pid: i32| Command::new("/bin/kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().unwrap().success();
        assert!(alive(pid));
        sc.kill();
        let end = std::time::Instant::now() + Duration::from_secs(3);
        while alive(pid) && std::time::Instant::now() < end {
            std::thread::sleep(Duration::from_millis(50));
        }
        let still = alive(pid);
        if still {
            let _ = Command::new("/bin/kill").args(["-9", &pid.to_string()]).status();
        }
        assert!(!still, "the extension's child process survived the kill");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Fix 4: a stdout line over 16 MB ends the command with a clear event.
    #[test]
    fn overlong_stdout_line_is_reported_and_the_command_stopped() {
        let Some((mut sc, rx, tmp)) = launch("long-line") else { return };
        let events = collect(&rx, 8000, 100);
        assert!(events.iter().any(|e| matches!(e, Event::TooLong)), "events: {events:?}");
        sc.kill();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Fix 5: writing to a sidecar that no longer reads must not block the caller.
    #[test]
    fn sending_to_a_stuck_sidecar_never_blocks_and_eventually_fails() {
        let Some((mut sc, rx, tmp)) = launch("busy") else { return };
        let _ = collect(&rx, 1200, 100); // render, then the loop starts blocking
        let (tx, done) = channel();
        std::thread::spawn(move || {
            let big = "y".repeat(10_000);
            let mut failed = false;
            for _ in 0..20_000 {
                if sc.send(&HostMsg::Search { text: big.clone() }).is_err() {
                    failed = true;
                    break;
                }
            }
            let _ = tx.send(failed);
        });
        let failed = done.recv_timeout(Duration::from_secs(10)).expect("send() blocked the caller");
        assert!(failed, "an overflowing queue must be reported as an error");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// The sidecar gets an allowlisted environment, not the launcher's (passed explicitly: no set_var).
    #[test]
    fn sidecar_does_not_inherit_secret_environment() {
        let tmp = std::env::temp_dir().join(format!("fc-sidecar-env-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let Some(node) = crate::extensions::node::find_node(&tmp) else { return };
        let runtime = crate::extensions::write_runtime(&tmp).unwrap();
        let fixture = format!("{}/src/extensions/fixtures/env-probe.js", env!("CARGO_MANIFEST_DIR"));
        let (tx, rx) = channel();
        let tx = std::sync::Mutex::new(tx);
        let vars = vec![
            os("PATH", "/usr/bin"),
            os("TZ", "UTC"),
            os("FERROCAST_TEST_SECRET", "s3cret"),
            os("NODE_OPTIONS", "--max-old-space-size=1"),
        ];
        let mut sc = Sidecar::spawn_with_env(&node.path, &runtime, &tmp, vars, move |e| {
            let _ = tx.lock().unwrap().send(e);
        })
        .unwrap();
        sc.send(&HostMsg::Launch {
            entry: fixture,
            extension: "f".into(),
            command: "c".into(),
            title: "C".into(),
            mode: "no-view".into(),
            arguments: Map::new(),
            preferences: Map::new(),
            environment: Environment {
                extension_name: "f".into(),
                command_name: "c".into(),
                support_path: tmp.to_string_lossy().into(),
                assets_path: tmp.to_string_lossy().into(),
                is_development: false,
                raycast_version: "1".into(),
                launch_type: "userInitiated".into(),
            },
        })
        .unwrap();
        let line = collect(&rx, 5000, 50).into_iter().find_map(|e| match e {
            Event::Stderr(l) if l.starts_with("ENV ") => Some(l),
            _ => None,
        });
        let line = line.expect("the probe reports its environment");
        assert!(line.contains("PATH") && line.contains("TZ"), "{line}");
        assert!(!line.contains("FERROCAST_TEST_SECRET") && !line.contains("NODE_OPTIONS"), "{line}");
        sc.kill();
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
