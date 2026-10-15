//! Launch-at-login via a per-user LaunchAgent.

pub const LABEL: &str = "dev.ferrocast.launcher";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
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
