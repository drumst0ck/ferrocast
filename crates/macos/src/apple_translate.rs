//! Runs `ferrocast-translate`, the Swift helper in the app bundle that calls Apple's Translation
//! framework (see `ferrocast_core::apple_translate` for the protocol). Blocking: call it off the
//! UI thread.

use ferrocast_core::apple_translate::{self as proto, Status, TranslateError};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Name of the helper next to the app's executable.
pub const HELPER: &str = "ferrocast-translate";

/// Longest a translation or a status check may take before the helper is killed.
const TIMEOUT_SECS: u64 = 60;

/// The helper next to the running executable (`Contents/MacOS/ferrocast-translate`).
pub fn helper_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(HELPER))
}

/// Whether the running macOS is at least `major` (26 for Apple Translation's headless API).
pub fn os_at_least(major: isize) -> bool {
    objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion().majorVersion >= major
}

/// Apple Translation can be used: macOS 26 or later and the helper is in the bundle.
pub fn available() -> bool {
    os_at_least(26) && helper_path().is_some_and(|p| is_executable(&p))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Run `program` with `input` on stdin and return its stdout; kill it after `secs`.
/// Exit codes other than 0 still return stdout when there is some (the helper explains errors in
/// JSON); with no output the error is its stderr or the exit code.
pub fn run_process(program: &Path, input: &str, secs: u64) -> Result<String, String> {
    let mut child =
        Command::new(program).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let data = input.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(data.as_bytes());
    });
    let (mut so, mut se) = (child.stdout.take().ok_or("no stdout")?, child.stderr.take().ok_or("no stderr")?);
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = so.read_to_string(&mut s);
        s
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = se.read_to_string(&mut s);
        s
    });
    let deadline = Instant::now() + Duration::from_secs(secs);
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("timed out after {secs} s"));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = writer.join();
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    if out.trim().is_empty() {
        return Err(if err.trim().is_empty() { format!("exited with {status}") } else { err.trim().to_string() });
    }
    Ok(out)
}

fn helper() -> Result<PathBuf, TranslateError> {
    if !os_at_least(26) {
        return Err(TranslateError::UnsupportedOs);
    }
    helper_path().filter(|p| is_executable(p)).ok_or_else(|| TranslateError::Other("the translation helper is missing from the app".into()))
}

/// Whether `source` (the system language when `None`) to `target` is downloaded, downloadable or
/// not translatable.
pub fn status(source: Option<&str>, target: &str) -> Result<Status, TranslateError> {
    let out = run_process(&helper()?, &proto::status_request(source, target), TIMEOUT_SECS).map_err(TranslateError::Other)?;
    proto::parse_status(&out)
}

/// Translate `text` into `target`; without `source` the helper detects the language of the text.
pub fn translate(source: Option<&str>, target: &str, text: &str) -> Result<String, TranslateError> {
    let out = run_process(&helper()?, &proto::translate_request(source, target, text), TIMEOUT_SECS).map_err(TranslateError::Other)?;
    proto::parse_translation(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("fc-apple-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("h{}.sh", body.len()));
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn the_request_reaches_stdin_and_stdout_comes_back() {
        let out = run_process(Path::new("/bin/cat"), "{\"cmd\":\"status\"}\nsecond", 5).unwrap();
        assert_eq!(out, "{\"cmd\":\"status\"}\nsecond");
    }

    #[test]
    fn a_hung_helper_is_killed_after_the_timeout() {
        let p = script("sleep 30");
        let t = Instant::now();
        let e = run_process(&p, "x", 1).unwrap_err();
        assert!(e.contains("timed out"), "{e}");
        assert!(t.elapsed() < Duration::from_secs(5), "killed promptly");
    }

    #[test]
    fn json_on_stdout_wins_over_a_failing_exit_code_and_silence_reports_stderr() {
        let p = script("echo '{\"error\":\"needs-download\",\"language\":\"de\"}'; exit 1");
        let out = run_process(&p, "x", 5).unwrap();
        assert_eq!(proto::parse_translation(&out), Err(TranslateError::NeedsDownload("de".into())));
        let p = script("echo boom >&2; exit 3");
        assert_eq!(run_process(&p, "x", 5), Err("boom".to_string()));
        let p = script("exit 4");
        assert!(run_process(&p, "x", 5).unwrap_err().contains("exit"));
    }

    #[test]
    fn a_missing_program_is_an_error_not_a_panic() {
        assert!(run_process(Path::new("/nonexistent/ferrocast-translate"), "x", 1).is_err());
    }

    /// Against the real helper: `FERROCAST_HELPER=dist/Ferrocast.app/Contents/MacOS/ferrocast-translate
    /// cargo test -p ferrocast-macos -- --ignored real_helper`. Needs macOS 26 and the English and
    /// Spanish packs. A pack that is not downloaded must be checked by hand: the answer is
    /// `needs-download` and the app shows the System Settings instructions.
    #[test]
    #[ignore = "needs macOS 26, the built helper (FERROCAST_HELPER) and the English and Spanish packs"]
    fn real_helper_answers_the_protocol() {
        let helper = PathBuf::from(std::env::var("FERROCAST_HELPER").expect("set FERROCAST_HELPER to the built helper"));
        let ask = |req: String| run_process(&helper, &req, 60).unwrap();
        assert_eq!(proto::parse_status(&ask(proto::status_request(Some("en"), "es"))), Ok(Status::Installed));
        assert_eq!(proto::parse_status(&ask(proto::status_request(Some("en"), "xx"))), Ok(Status::Unsupported));
        let es = proto::parse_translation(&ask(proto::translate_request(Some("en"), "es", "Good morning"))).unwrap();
        assert!(es.to_lowercase().contains("buenos"), "{es}");
        let multi = proto::parse_translation(&ask(proto::translate_request(None, "de", "Line one.\n\"Line two\""))).unwrap();
        assert!(multi.contains('\n') && multi.contains('"'), "line breaks and quotes survive: {multi:?}");
        assert_eq!(
            proto::parse_translation(&ask(proto::translate_request(Some("en"), "xx", "Hello"))),
            Err(TranslateError::Unsupported("xx".into()))
        );
    }

    #[test]
    fn this_mac_reports_its_version() {
        assert!(os_at_least(1));
        assert!(!os_at_least(10_000));
    }
}
