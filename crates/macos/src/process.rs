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
    /// The `.app` bundle this executable lives in, if any, for the icon.
    pub fn bundle(&self) -> Option<String> {
        let i = self.path.find(".app/")?;
        Some(self.path[..i + 4].to_string())
    }
}

/// Parse `ps -axo pid=,pcpu=,rss=,comm=` output. `comm` may contain spaces, so it is the
/// remainder of the line.
pub fn parse_ps(out: &str) -> Vec<ProcInfo> {
    out.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let pid: i32 = it.next()?.parse().ok()?;
            let cpu: f32 = it.next()?.parse().ok()?;
            let rss_kb: f32 = it.next()?.parse().ok()?;
            let path = it.collect::<Vec<_>>().join(" ");
            if path.is_empty() {
                return None;
            }
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            Some(ProcInfo { pid, cpu, mem_mb: rss_kb / 1024.0, name, path })
        })
        .collect()
}

/// Processes of the current user, busiest first. Kernel tasks and this launcher are omitted.
pub fn list() -> Vec<ProcInfo> {
    let out = Command::new("/bin/ps").args(["-axo", "pid=,pcpu=,rss=,comm="]).output().ok();
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
    fn lists_real_processes() {
        let v = list();
        assert!(!v.is_empty());
        assert!(v.iter().all(|p| p.pid != std::process::id() as i32));
    }
}
