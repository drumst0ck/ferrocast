//! Launch-at-login via a per-user LaunchAgent.

pub const LABEL: &str = "dev.ferrocast.launcher";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Homebrew installs into a versioned `…/Cellar/<name>/<version>/` directory that disappears on
/// upgrade; map it to the stable `…/opt/<name>/` symlink so login items survive `brew upgrade`.
pub fn stable_exe_path(exe: &str) -> String {
    let Some(i) = exe.find("/Cellar/") else { return exe.to_string() };
    let rest = &exe[i + "/Cellar/".len()..];
    let mut parts = rest.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(name), Some(_version), Some(tail)) if !name.is_empty() => format!("{}/opt/{name}/{tail}", &exe[..i]),
        _ => exe.to_string(),
    }
}

/// LaunchAgent plist that starts `exe --hidden` at login (no keep-alive: quitting stays quit).
pub fn plist(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key><array><string>{}</string><string>--hidden</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict></plist>
"#,
        xml_escape(exe)
    )
}

/// Install or remove the agent file in `agents_dir` (normally `~/Library/LaunchAgents`).
pub fn set_enabled(agents_dir: &std::path::Path, exe: &str, enabled: bool) -> std::io::Result<()> {
    let path = agents_dir.join(format!("{LABEL}.plist"));
    if enabled {
        std::fs::create_dir_all(agents_dir)?;
        std::fs::write(path, plist(exe))
    } else {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homebrew_cellar_path_becomes_stable_opt_path() {
        assert_eq!(
            stable_exe_path("/opt/homebrew/Cellar/ferrocast/0.1.0/Ferrocast.app/Contents/MacOS/ferrocast"),
            "/opt/homebrew/opt/ferrocast/Ferrocast.app/Contents/MacOS/ferrocast"
        );
        assert_eq!(stable_exe_path("/usr/local/Cellar/ferrocast/1.2.3_1/bin/ferrocast"), "/usr/local/opt/ferrocast/bin/ferrocast");
        assert_eq!(
            stable_exe_path("/Applications/Ferrocast.app/Contents/MacOS/ferrocast"),
            "/Applications/Ferrocast.app/Contents/MacOS/ferrocast"
        );
        assert_eq!(stable_exe_path("/opt/homebrew/Cellar/ferrocast"), "/opt/homebrew/Cellar/ferrocast", "malformed paths are left alone");
    }

    #[test]
    fn plist_escapes_paths() {
        let p = plist("/Applications/R&D <Tools>/ferrocast");
        assert!(p.contains("/Applications/R&amp;D &lt;Tools&gt;/ferrocast"));
        assert!(p.contains("<string>--hidden</string>"));
        assert!(p.contains("<key>RunAtLoad</key><true/>"));
        assert!(!p.contains("KeepAlive"));
    }

    #[test]
    fn enable_then_disable_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("ferrocast-launch-{}", uuid::Uuid::new_v4()));
        set_enabled(&dir, "/x/ferrocast", true).unwrap();
        let file = dir.join(format!("{LABEL}.plist"));
        assert!(std::fs::read_to_string(&file).unwrap().contains("/x/ferrocast"));
        set_enabled(&dir, "/x/ferrocast", false).unwrap();
        assert!(!file.exists());
        set_enabled(&dir, "/x/ferrocast", false).unwrap(); // removing twice is fine
        let _ = std::fs::remove_dir_all(dir);
    }
}
