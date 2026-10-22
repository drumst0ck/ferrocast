//! Client for the `ferrocast-capture` Swift helper (NDJSON over stdin/stdout, see
//! `ferrocast_core::capture::protocol`). Text and paths never travel in argv.

use anyhow::{anyhow, bail, Result};
use ferrocast_core::capture::protocol::{self, Command, ErrorCode, Hello, Line, Request, Shareable};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

pub const HELPER_NAME: &str = "ferrocast-capture";

/// Where the helper lives: next to the running executable. Pass the exe already resolved through
/// `instance::resolve_exe` (that function lives in the app crate).
pub fn helper_path(exe: &Path) -> PathBuf {
    exe.with_file_name(HELPER_NAME)
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Screen capture is offered on macOS 14+ with the helper present.
pub fn available(exe: &Path) -> bool {
    crate::system::os_at_least(14) && is_executable(&helper_path(exe))
}

/// Per-command reply timeout.
pub fn timeout_for(cmd: &Command) -> Duration {
    Duration::from_secs(match cmd {
        Command::Hello => 5,
        Command::Shareable => 10,
    })
}

/// A request that did not produce an `ok` reply.
#[derive(Debug)]
pub enum CallError {
    Helper { code: ErrorCode, message: String },
    Timeout,
    Closed,
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Helper { code, message } => write!(f, "capture helper error {code:?}: {message}"),
            CallError::Timeout => write!(f, "capture helper timed out"),
            CallError::Closed => write!(f, "capture helper closed"),
        }
    }
}
impl std::error::Error for CallError {}

pub struct Helper {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Line>,
    next_id: u64,
}

impl Helper {
    /// Start the helper; every stderr line goes to `log` (the app appends it to `ferrocast.log`).
    pub fn spawn(path: &Path, log: impl Fn(&str) + Send + 'static) -> Result<Helper> {
        let mut child = std::process::Command::new(path).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for l in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
                match protocol::decode_line(&l) {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(e) => eprintln!("capture helper: bad line ({e}): {l}"),
                }
            }
        });
        std::thread::spawn(move || {
            for l in BufReader::new(stderr).lines().map_while(|l| l.ok()) {
                log(&l);
            }
        });
        Ok(Helper { child, stdin, lines, next_id: 1 })
    }

    /// Send one command and wait for its reply body; kills the helper on timeout.
    pub fn call(&mut self, command: Command, timeout: Duration) -> std::result::Result<protocol::Value, CallError> {
        let id = self.next_id;
        self.next_id += 1;
        let line = protocol::encode_request(&Request { id, command });
        if writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).is_err() {
            return Err(CallError::Closed);
        }
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(Line::Ok { id: rid, body }) if rid == id => return Ok(body),
                Ok(Line::Error { id: rid, code, message }) if rid.is_none_or(|r| r == id) => {
                    return Err(CallError::Helper { code, message })
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {
                    self.kill();
                    return Err(CallError::Timeout);
                }
                Err(RecvTimeoutError::Disconnected) => return Err(CallError::Closed),
            }
        }
    }

    pub fn hello(&mut self) -> Result<Hello> {
        let body = self.call(Command::Hello, timeout_for(&Command::Hello))?;
        protocol::parse_body(&body).map_err(|e| anyhow!(e))
    }

    pub fn shareable(&mut self) -> Result<Shareable> {
        let body = self.call(Command::Shareable, timeout_for(&Command::Shareable))?;
        protocol::parse_body(&body).map_err(|e| anyhow!(e))
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Run `f` against a short-lived helper.
pub fn with_helper<T>(exe: &Path, log: impl Fn(&str) + Send + 'static, f: impl FnOnce(&mut Helper) -> Result<T>) -> Result<T> {
    if !available(exe) {
        bail!("screen capture is not available");
    }
    f(&mut Helper::spawn(&helper_path(exe), log)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};

    fn fake(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fc-capture-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("helper.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn helper_path_is_next_to_exe() {
        assert_eq!(
            helper_path(Path::new("/A/Ferrocast.app/Contents/MacOS/ferrocast")),
            PathBuf::from("/A/Ferrocast.app/Contents/MacOS/ferrocast-capture")
        );
        assert!(!available(Path::new("/nonexistent/ferrocast")));
    }

    #[test]
    fn frames_replies_and_logs_stderr() {
        let p = fake(
            "frame",
            r#"while read l; do echo noise >&2; echo '{"event":"x","id":1}'; echo '{"id":1,"ok":true,"protocol":1,"os":"14.0.0","version":"t","sck":true,"metal":true}'; done"#,
        );
        let logs = Arc::new(Mutex::new(Vec::new()));
        let l2 = logs.clone();
        let mut h = Helper::spawn(&p, move |s| l2.lock().unwrap().push(s.to_string())).unwrap();
        let hello = h.hello().unwrap();
        assert_eq!(hello.os, "14.0.0");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(logs.lock().unwrap().as_slice(), ["noise"]);
    }

    #[test]
    fn helper_error_is_structured() {
        let p = fake("err", r#"read l; echo '{"id":1,"error":"permission-screen","message":"off"}'; exit 1"#);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        match h.call(Command::Shareable, Duration::from_secs(3)) {
            Err(CallError::Helper { code, message }) => assert_eq!((code, message.as_str()), (ErrorCode::PermissionScreen, "off")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn timeout_kills_the_helper() {
        let p = fake("hang", "read l; sleep 30");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let t = Instant::now();
        assert!(matches!(h.call(Command::Hello, Duration::from_millis(300)), Err(CallError::Timeout)));
        assert!(t.elapsed() < Duration::from_secs(5));
        assert!(h.child.try_wait().unwrap().is_some());
    }

    #[test]
    fn closed_helper_reports_closed() {
        let p = fake("exit", "exit 0");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert!(matches!(h.call(Command::Hello, Duration::from_secs(2)), Err(CallError::Closed)));
    }
}
