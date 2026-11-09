//! Running processes for the Kill Process command: listed through `ps`, ended with `kill`.

use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct ProcInfo {
    pub pid: i32,
    /// Percent of one CPU, as `ps` reports it.
    pub cpu: f32,
    pub mem_mb: f32,
    /// Executable name (last path component).
    pub name: String,
    /// Full executable path.
    pub path: String,
}

impl ProcInfo {
    /// The `.app` bundle this executable lives in, if any, for the icon: the innermost one (Xcode's
    /// Simulator.app shows the Simulator icon, not Xcode's). Helper apps inside a framework or an
    /// XPC service (browser renderers) have no icon of their own and keep the outer app's.
    pub fn bundle(&self) -> Option<String> {
        let mut best = None;
        for (i, _) in self.path.match_indices(".app/") {
            let before = &self.path[..i];
            if before.contains(".app/Contents/Frameworks/") || before.contains(".app/Contents/XPCServices/") {
                continue;
            }
            best = Some(i);
        }
        best.map(|i| self.path[..i + 4].to_string())
    }
}

/// Parse `ps -axo pid=,pcpu=,rss=,comm=` output. `comm` may contain spaces, so it is the
/// remainder of the line.
pub fn parse_ps(out: &str) -> Vec<ProcInfo> {
    out.lines()
        .filter_map(|l| {
            // pid, cpu and rss are single tokens; `comm` is the rest of the line verbatim
            // (collapsing whitespace would corrupt paths holding double spaces).
            let mut rest = l.trim_start();
            let mut field = || {
                let end = rest.find(char::is_whitespace)?;
                let (tok, tail) = rest.split_at(end);
                rest = tail.trim_start();
                Some(tok)
            };
            let pid: i32 = field()?.parse().ok()?;
            let cpu: f32 = field()?.parse().ok()?;
            let rss_kb: f32 = field()?.parse().ok()?;
            let path = rest.trim_end().to_string();
            if path.is_empty() {
                return None;
            }
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            Some(ProcInfo { pid, cpu, mem_mb: rss_kb / 1024.0, name, path })
        })
        .collect()
}

/// `ps` arguments listing only the processes of user `uid`.
fn ps_args(uid: &str) -> Vec<String> {
    ["-U", uid, "-o", "pid=,pcpu=,rss=,comm="].iter().map(|s| s.to_string()).collect()
}

fn current_uid() -> Option<String> {
    let o = Command::new("/usr/bin/id").arg("-u").output().ok()?;
    let uid = String::from_utf8_lossy(&o.stdout).trim().to_string();
    (o.status.success() && !uid.is_empty()).then_some(uid)
}

/// Processes of the current user, busiest first. Kernel tasks and this launcher are omitted.
pub fn list() -> Vec<ProcInfo> {
    let Some(uid) = current_uid() else { return Vec::new() };
    let out = Command::new("/bin/ps").args(ps_args(&uid)).output().ok();
    let me = std::process::id() as i32;
    let mut v: Vec<ProcInfo> = out
        .map(|o| parse_ps(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.pid > 1 && p.pid != me && !p.path.starts_with('[') && !p.path.starts_with("(") && !p.path.starts_with('-'))
        .collect();
    v.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.mem_mb.total_cmp(&a.mem_mb)));
    v
}

/// SIGTERM, or SIGKILL when `force`. Returns whether the signal was delivered.
pub fn kill(pid: i32, force: bool) -> bool {
    Command::new("/bin/kill").args([if force { "-9" } else { "-15" }, &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ps_lines_with_spaces_in_paths() {
        let out =
            "  412   3.2  204800 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome\n  1  0.0 1024 /sbin/launchd\nbad line\n";
        let v = parse_ps(out);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].pid, 412);
        assert_eq!(v[0].name, "Google Chrome");
        assert_eq!(v[0].bundle().as_deref(), Some("/Applications/Google Chrome.app"));
        assert!((v[0].mem_mb - 200.0).abs() < 0.01);
        assert_eq!(v[1].bundle(), None);
    }

    #[test]
    fn nested_apps_use_the_innermost_bundle_unless_it_is_a_framework_helper() {
        let p = |path: &str| ProcInfo { pid: 1, cpu: 0.0, mem_mb: 0.0, name: String::new(), path: path.into() };
        assert_eq!(
            p("/Applications/Xcode.app/Contents/Developer/Applications/Simulator.app/Contents/MacOS/Simulator").bundle().as_deref(),
            Some("/Applications/Xcode.app/Contents/Developer/Applications/Simulator.app")
        );
        assert_eq!(
            p("/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/1/Helpers/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper")
                .bundle()
                .as_deref(),
            Some("/Applications/Google Chrome.app")
        );
        assert_eq!(p("/Applications/Safari.app/Contents/MacOS/Safari").bundle().as_deref(), Some("/Applications/Safari.app"));
    }

    #[test]
    fn keeps_double_spaces_in_paths() {
        let v = parse_ps("  7  0.0 2048 /Applications/My  App.app/Contents/MacOS/My  App\n");
        assert_eq!(v[0].path, "/Applications/My  App.app/Contents/MacOS/My  App");
        assert_eq!(v[0].name, "My  App");
    }

    #[test]
    fn lists_only_the_given_user() {
        let a = ps_args("501");
        assert_eq!(&a[..2], ["-U", "501"]);
        assert!(!a.iter().any(|x| x.contains('a') && x.starts_with('-') && x != "-U"), "no -a (all users): {a:?}");
    }

    #[test]
    fn lists_real_processes() {
        let v = list();
        assert!(!v.is_empty());
        assert!(v.iter().all(|p| p.pid != std::process::id() as i32));
    }
}
