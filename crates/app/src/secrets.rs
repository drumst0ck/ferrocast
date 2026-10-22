//! Keychain secrets through `/usr/bin/security`, never through argv (`ps` shows argv).
//!
//! Writes go through `security -i`, which reads its commands from stdin. Reads use
//! `find-generic-password -w` (the secret comes back on stdout). The account is always `ferrocast`.

use std::io::Write;
use std::process::{Command, Stdio};

const SECURITY: &str = "/usr/bin/security";
const ACCOUNT: &str = "ferrocast";
/// `security -i` fails once a line passes about 4,050 bytes, and it still stores a truncated value
/// (measured on macOS: a 4,000 char value works, 4,050 stores 4,019 chars and exits 1). Stay well under.
const MAX_SCRIPT: usize = 3_900;

/// Result of one `security` run.
pub struct Run {
    pub ok: bool,
    pub stdout: Vec<u8>,
}

/// How `security` is spawned: argv and an optional stdin script. Injected so tests never touch the Keychain.
pub type Runner = fn(&[&str], Option<&str>) -> Option<Run>;

fn real_runner(args: &[&str], stdin: Option<&str>) -> Option<Run> {
    let mut child = Command::new(SECURITY)
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let Some(s) = stdin {
        // dropping the handle closes stdin, which ends `security -i`
        child.stdin.take()?.write_all(s.as_bytes()).ok()?;
    }
    let out = child.wait_with_output().ok()?;
    Some(Run { ok: out.status.success(), stdout: out.stdout })
}

/// Quote a value for the `security -i` line parser: double quotes, with `\` and `"` backslash-escaped.
/// Line breaks and NUL cannot be represented in a one-line command: `None` (refuse, never truncate).
fn quote(v: &str) -> Option<String> {
    if v.contains(['\n', '\r', '\0']) {
        return None;
    }
    let mut s = String::with_capacity(v.len() + 2);
    s.push('"');
    for c in v.chars() {
        if c == '\\' || c == '"' {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('"');
    Some(s)
}

/// `security` prints a value that has control or non-ASCII bytes as bare hex; decode it back.
/// A plain ASCII value (which `security` prints verbatim) is never decoded unless it is valid hex that
/// decodes to UTF-8 with such characters (see risk note in the Log).
fn decode_output(raw: &[u8]) -> Option<String> {
    let s = String::from_utf8_lossy(raw);
    let s = s.strip_suffix('\n').unwrap_or(&s);
    if s.len() >= 2 && s.len().is_multiple_of(2) && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        let bytes: Option<Vec<u8>> = (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect();
        if let Some(Ok(d)) = bytes.map(String::from_utf8) {
            if d.chars().any(|c| c.is_control() || !c.is_ascii()) {
                return Some(d);
            }
        }
    }
    Some(s.to_string())
}

pub fn secret_get(service: &str) -> Option<String> {
    secret_get_with(real_runner, service)
}

/// Store `value` (empty deletes the item). `false` when refused or `security` failed.
pub fn secret_set(service: &str, value: &str) -> bool {
    secret_set_with(real_runner, service, value)
}

pub fn secret_get_with(run: Runner, service: &str) -> Option<String> {
    let out = run(&["find-generic-password", "-s", service, "-w"], None)?;
    if !out.ok {
        return None;
    }
    decode_output(&out.stdout).filter(|v| !v.is_empty())
}

pub fn secret_set_with(run: Runner, service: &str, value: &str) -> bool {
    let (Some(svc), Some(acct)) = (quote(service), quote(ACCOUNT)) else { return false };
    let script = if value.is_empty() {
        format!("delete-generic-password -s {svc}\n")
    } else {
        let Some(v) = quote(value) else { return false };
        format!("add-generic-password -U -s {svc} -a {acct} -w {v}\n")
    };
    if script.len() > MAX_SCRIPT {
        return false;
    }
    run(&["-i"], Some(&script)).is_some_and(|r| r.ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static CALLS: RefCell<Vec<(Vec<String>, Option<String>)>> = const { RefCell::new(Vec::new()) };
        static REPLY: RefCell<Option<(bool, Vec<u8>)>> = const { RefCell::new(None) };
    }
    fn fake(args: &[&str], stdin: Option<&str>) -> Option<Run> {
        CALLS.with(|c| c.borrow_mut().push((args.iter().map(|s| s.to_string()).collect(), stdin.map(str::to_string))));
        REPLY.with(|r| r.borrow().clone()).map(|(ok, stdout)| Run { ok, stdout })
    }
    fn reset(reply: Option<(bool, Vec<u8>)>) {
        CALLS.with(|c| c.borrow_mut().clear());
        REPLY.with(|r| *r.borrow_mut() = reply);
    }
    fn calls() -> Vec<(Vec<String>, Option<String>)> {
        CALLS.with(|c| c.borrow().clone())
    }

    /// Mimics the `security -i` line parser for the subset we emit: words split on spaces, double-quoted
    /// words with backslash escapes.
    fn parse(line: &str) -> Vec<String> {
        let (mut words, mut cur, mut inq, mut has, mut esc) = (vec![], String::new(), false, false, false);
        for c in line.trim_end_matches('\n').chars() {
            if esc {
                cur.push(c);
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                inq = !inq;
                has = true;
            } else if c == ' ' && !inq {
                if has || !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
                has = false;
            } else {
                cur.push(c);
                has = true;
            }
        }
        if has || !cur.is_empty() {
            words.push(cur);
        }
        words
    }

    const AWKWARD: &[&str] = &[
        "plain",
        "has \"quotes\" inside",
        "back\\slash and \\\" mix",
        "ends with backslash\\",
        "  leading and trailing  ",
        "ñandú → 😀 日本語",
        "tab\tinside",
        "{\"access\":\"a\\\"b\",\"n\":1}",
        "-w -s x",
    ];

    #[test]
    fn secret_never_in_argv_and_script_round_trips() {
        for v in AWKWARD {
            reset(Some((true, vec![])));
            assert!(secret_set_with(fake, "svc-x", v), "{v:?}");
            let c = calls();
            assert_eq!(c.len(), 1);
            assert_eq!(c[0].0, vec!["-i".to_string()]);
            for a in &c[0].0 {
                assert!(!a.contains(v), "secret leaked into argv");
            }
            let w = parse(c[0].1.as_deref().unwrap());
            assert_eq!(w, vec!["add-generic-password", "-U", "-s", "svc-x", "-a", "ferrocast", "-w", v], "{v:?}");
            assert_eq!(c[0].1.as_deref().unwrap().matches('\n').count(), 1, "single line script");
        }
    }

    #[test]
    fn empty_value_deletes() {
        reset(Some((true, vec![])));
        assert!(secret_set_with(fake, "svc-x", ""));
        let c = calls();
        assert_eq!(c[0].0, vec!["-i".to_string()]);
        assert_eq!(parse(c[0].1.as_deref().unwrap()), vec!["delete-generic-password", "-s", "svc-x"]);
    }

    #[test]
    fn unrepresentable_value_is_refused_without_spawning() {
        for v in ["a\nb", "a\r\nb", "nul\0x", "\n"] {
            reset(Some((true, vec![])));
            assert!(!secret_set_with(fake, "svc-x", v), "{v:?}");
            assert!(calls().is_empty(), "nothing may be spawned for {v:?}");
        }
    }

    #[test]
    fn too_long_a_script_is_refused_not_truncated() {
        // `security -i` stores a truncated value (and reports failure) once a line passes ~4,050 bytes
        let long = "k".repeat(4000);
        reset(Some((true, vec![])));
        assert!(!secret_set_with(fake, "svc-x", &long));
        assert!(calls().is_empty(), "nothing may be spawned for an oversized value");
        let fits = "k".repeat(3000);
        reset(Some((true, vec![])));
        assert!(secret_set_with(fake, "svc-x", &fits));
        assert_eq!(calls().len(), 1);
    }

    #[test]
    fn failing_security_reports_false() {
        reset(Some((false, vec![])));
        assert!(!secret_set_with(fake, "svc-x", "k"));
        reset(None);
        assert!(!secret_set_with(fake, "svc-x", "k"));
    }

    #[test]
    fn get_reads_stdout_and_never_sends_a_script() {
        reset(Some((true, b"sk-abc\n".to_vec())));
        assert_eq!(secret_get_with(fake, "svc-x").as_deref(), Some("sk-abc"));
        let c = calls();
        assert_eq!(c[0].0, vec!["find-generic-password", "-s", "svc-x", "-w"]);
        assert!(c[0].1.is_none());
        reset(Some((true, b"  sp \n".to_vec())));
        assert_eq!(secret_get_with(fake, "svc-x").as_deref(), Some("  sp "));
        reset(Some((false, b"x".to_vec())));
        assert_eq!(secret_get_with(fake, "svc-x"), None);
        reset(Some((true, b"\n".to_vec())));
        assert_eq!(secret_get_with(fake, "svc-x"), None);
    }

    #[test]
    fn get_decodes_hex_for_non_ascii_values_only() {
        // `security -w` prints "ñ→" as hex
        reset(Some((true, b"c3b1e28692\n".to_vec())));
        assert_eq!(secret_get_with(fake, "s").as_deref(), Some("ñ→"));
        // a hex-looking ASCII key stays as is (decodes to invalid UTF-8 / printable ASCII)
        reset(Some((true, b"deadbeef\n".to_vec())));
        assert_eq!(secret_get_with(fake, "s").as_deref(), Some("deadbeef"));
        reset(Some((true, b"41424344\n".to_vec())));
        assert_eq!(secret_get_with(fake, "s").as_deref(), Some("41424344"));
    }
}
