//! System commands (lock, sleep, volume, appearance, trash…).

use std::process::{Command, Stdio};

/// True when System Settings > Accessibility > Display > Reduce motion is on
/// (`NSWorkspace.accessibilityDisplayShouldReduceMotion`). Cheap, but callers should still read it
/// once per use (e.g. when a wheel opens), not every frame.
pub fn reduce_motion() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// Major macOS version from `sw_vers -productVersion` output (`"14.5"` -> 14).
pub fn parse_major(version: &str) -> Option<u32> {
    version.trim().split('.').next()?.parse().ok()
}

/// True when this Mac runs macOS `major` or newer. Cached; false when the version is unknown.
pub fn os_at_least(major: u32) -> bool {
    static MAJOR: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    let cur = *MAJOR.get_or_init(|| {
        let out = Command::new("/usr/bin/sw_vers").arg("-productVersion").output().ok()?;
        parse_major(&String::from_utf8_lossy(&out.stdout))
    });
    cur.is_some_and(|c| c >= major)
}

/// Run AppleScript, returning trimmed stdout.
pub fn osascript(script: &str) -> anyhow::Result<String> {
    let out = Command::new("/usr/bin/osascript").arg("-e").arg(script).output()?;
    if !out.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Like `osascript`, but gives up (and kills the process) after `secs` seconds, so a pending
/// Automation prompt cannot hang the caller forever.
pub fn osascript_timeout(script: &str, secs: u64) -> anyhow::Result<String> {
    osascript_raw(script, secs, None).map(|o| o.trim().to_string())
}

/// Like `osascript_timeout`, but returns stdout verbatim (leading/trailing whitespace and the final
/// newline `osascript` adds are kept) and stops early, killing the process, once `cancel` is set.
pub fn osascript_raw(script: &str, secs: u64, cancel: Option<&std::sync::atomic::AtomicBool>) -> anyhow::Result<String> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = Command::new("/usr/bin/osascript").arg("-e").arg(script).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let (mut so, mut se) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::SeqCst)) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("cancelled");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("timed out after {secs} s (is a macOS Automation permission prompt waiting?)");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    anyhow::ensure!(status.success(), "{}", err.trim());
    Ok(out)
}

fn sh(cmd: &str, args: &[&str]) -> anyhow::Result<()> {
    let st = Command::new(cmd).args(args).status()?;
    anyhow::ensure!(st.success(), "{cmd} failed");
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemCommand {
    LockScreen,
    Sleep,
    SleepDisplays,
    Restart,
    ShutDown,
    LogOut,
    ScreenSaver,
    PlayPause,
    NextTrack,
    PreviousTrack,
    ToggleMute,
    VolumeUp,
    VolumeDown,
    Volume(u8),
    ShowDesktop,
    ToggleAppearance,
    OpenTrash,
    EmptyTrash,
    EjectAll,
    ToggleHiddenFiles,
    HideAllExceptFrontmost,
    UnhideAll,
    QuitAllApps,
    DismissNotifications,
    ToggleWifi,
    /// Keep the Mac awake (caffeinate) until toggled again.
    KeepAwake,
}

impl SystemCommand {
    pub const ALL: &'static [SystemCommand] = &[
        SystemCommand::LockScreen,
        SystemCommand::Sleep,
        SystemCommand::SleepDisplays,
        SystemCommand::Restart,
        SystemCommand::ShutDown,
        SystemCommand::LogOut,
        SystemCommand::ScreenSaver,
        SystemCommand::PlayPause,
        SystemCommand::NextTrack,
        SystemCommand::PreviousTrack,
        SystemCommand::ToggleMute,
        SystemCommand::VolumeUp,
        SystemCommand::VolumeDown,
        SystemCommand::Volume(0),
        SystemCommand::Volume(25),
        SystemCommand::Volume(50),
        SystemCommand::Volume(75),
        SystemCommand::Volume(100),
        SystemCommand::ShowDesktop,
        SystemCommand::ToggleAppearance,
        SystemCommand::OpenTrash,
        SystemCommand::EmptyTrash,
        SystemCommand::EjectAll,
        SystemCommand::ToggleHiddenFiles,
        SystemCommand::HideAllExceptFrontmost,
        SystemCommand::UnhideAll,
        SystemCommand::QuitAllApps,
        SystemCommand::DismissNotifications,
        SystemCommand::ToggleWifi,
        SystemCommand::KeepAwake,
    ];

    pub fn id(&self) -> String {
        match self {
            SystemCommand::Volume(v) => format!("system:volume-{v}"),
            other => format!("system:{}", other.title().to_lowercase().replace([' ', '/'], "-")),
        }
    }

    pub fn title(&self) -> String {
        match self {
            SystemCommand::LockScreen => "Lock Screen".into(),
            SystemCommand::Sleep => "Sleep".into(),
            SystemCommand::SleepDisplays => "Sleep Displays".into(),
            SystemCommand::Restart => "Restart".into(),
            SystemCommand::ShutDown => "Shut Down".into(),
            SystemCommand::LogOut => "Log Out".into(),
            SystemCommand::ScreenSaver => "Show Screen Saver".into(),
            SystemCommand::PlayPause => "Play / Pause".into(),
            SystemCommand::NextTrack => "Next Track".into(),
            SystemCommand::PreviousTrack => "Previous Track".into(),
            SystemCommand::ToggleMute => "Toggle Mute".into(),
            SystemCommand::VolumeUp => "Turn Volume Up".into(),
            SystemCommand::VolumeDown => "Turn Volume Down".into(),
            SystemCommand::Volume(v) => format!("Set Volume to {v}%"),
            SystemCommand::ShowDesktop => "Show Desktop".into(),
            SystemCommand::ToggleAppearance => "Toggle System Appearance".into(),
            SystemCommand::OpenTrash => "Open Trash".into(),
            SystemCommand::EmptyTrash => "Empty Trash".into(),
            SystemCommand::EjectAll => "Eject All Disks".into(),
            SystemCommand::ToggleHiddenFiles => "Toggle Hidden Files".into(),
            SystemCommand::HideAllExceptFrontmost => "Hide All Apps Except Frontmost".into(),
            SystemCommand::UnhideAll => "Unhide All Hidden Apps".into(),
            SystemCommand::QuitAllApps => "Quit All Apps".into(),
            SystemCommand::DismissNotifications => "Dismiss Notifications".into(),
            SystemCommand::ToggleWifi => "Toggle Wi-Fi".into(),
            SystemCommand::KeepAwake => "Toggle Keep Awake".into(),
        }
    }

    pub fn keywords(&self) -> &'static [&'static str] {
        match self {
            SystemCommand::LockScreen => &["lock"],
            SystemCommand::ToggleAppearance => &["dark mode", "light mode", "theme"],
            SystemCommand::ToggleMute => &["mute", "sound", "audio"],
            SystemCommand::VolumeUp | SystemCommand::VolumeDown | SystemCommand::Volume(_) => &["sound", "audio"],
            SystemCommand::ShutDown => &["power off", "turn off"],
            SystemCommand::Restart => &["reboot"],
            SystemCommand::EjectAll => &["unmount"],
            SystemCommand::ToggleWifi => &["wifi", "wireless", "network", "airport"],
            SystemCommand::KeepAwake => &["caffeinate", "coffee", "awake", "sleep", "insomnia"],
            _ => &[],
        }
    }

    /// Commands that should ask for confirmation.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            SystemCommand::Restart
                | SystemCommand::ShutDown
                | SystemCommand::LogOut
                | SystemCommand::EmptyTrash
                | SystemCommand::QuitAllApps
        )
    }

    /// Execute; returns a HUD message.
    pub fn run(&self, quit_excluded: &[String]) -> anyhow::Result<String> {
        use SystemCommand::*;
        let key = |code: u8| format!("tell application \"System Events\" to key code {code}");
        match self {
            LockScreen => {
                // ctrl+cmd+Q
                osascript("tell application \"System Events\" to keystroke \"q\" using {control down, command down}")?;
            }
            Sleep => sh("/usr/bin/pmset", &["sleepnow"])?,
            SleepDisplays => sh("/usr/bin/pmset", &["displaysleepnow"])?,
            Restart => {
                osascript("tell application \"System Events\" to restart")?;
            }
            ShutDown => {
                osascript("tell application \"System Events\" to shut down")?;
            }
            LogOut => {
                osascript("tell application \"System Events\" to log out")?;
            }
            ScreenSaver => sh("/usr/bin/open", &["-a", "ScreenSaverEngine"])?,
            PlayPause => crate::keyboard::media_key(crate::keyboard::MediaKey::PlayPause),
            NextTrack => crate::keyboard::media_key(crate::keyboard::MediaKey::Next),
            PreviousTrack => crate::keyboard::media_key(crate::keyboard::MediaKey::Previous),
            ToggleMute => {
                let muted = osascript("output muted of (get volume settings)")? == "true";
                osascript(&format!("set volume output muted {}", !muted))?;
                return Ok(if muted { "Unmuted".into() } else { "Muted".into() });
            }
            VolumeUp | VolumeDown => {
                let cur: i32 = osascript("output volume of (get volume settings)")?.parse().unwrap_or(50);
                let v = (cur + if *self == VolumeUp { 6 } else { -6 }).clamp(0, 100);
                osascript(&format!("set volume output volume {v}"))?;
                return Ok(format!("Volume {v}%"));
            }
            Volume(v) => {
                osascript(&format!("set volume output volume {v}"))?;
                return Ok(format!("Volume {v}%"));
            }
            ShowDesktop => {
                // F11 (Mission Control "Show Desktop")
                osascript(&key(103))?;
            }
            ToggleAppearance => {
                osascript("tell application \"System Events\" to tell appearance preferences to set dark mode to not dark mode")?;
            }
            OpenTrash => {
                let t = dirs::home_dir().unwrap_or_default().join(".Trash");
                sh("/usr/bin/open", &[&t.to_string_lossy()])?;
            }
            EmptyTrash => {
                osascript("tell application \"Finder\" to empty trash")?;
                return Ok("Trash emptied".into());
            }
            EjectAll => {
                osascript("tell application \"Finder\" to eject (every disk whose ejectable is true)")?;
                return Ok("Ejected all disks".into());
            }
            ToggleHiddenFiles => {
                let cur = Command::new("/usr/bin/defaults").args(["read", "com.apple.finder", "AppleShowAllFiles"]).output()?;
                let on = String::from_utf8_lossy(&cur.stdout).trim().eq_ignore_ascii_case("1")
                    || String::from_utf8_lossy(&cur.stdout).trim().eq_ignore_ascii_case("true");
                sh("/usr/bin/defaults", &["write", "com.apple.finder", "AppleShowAllFiles", "-bool", if on { "false" } else { "true" }])?;
                sh("/usr/bin/killall", &["Finder"])?;
                return Ok(if on { "Hidden files hidden".into() } else { "Hidden files shown".into() });
            }
            HideAllExceptFrontmost => {
                osascript("tell application \"System Events\" to set visible of (every process whose visible is true and frontmost is false) to false")?;
            }
            UnhideAll => {
                osascript("tell application \"System Events\" to set visible of every process to true")?;
            }
            QuitAllApps => {
                let excluded: Vec<String> = quit_excluded.iter().map(|s| format!("\"{}\"", s.replace('"', ""))).collect();
                let script = format!(
                    "set keep to {{{}}}\ntell application \"System Events\" to set names to name of every process whose background only is false\nrepeat with n in names\nif keep does not contain (n as text) and (n as text) is not \"Ferrocast\" then\ntry\ntell application (n as text) to quit\nend try\nend if\nend repeat",
                    excluded.join(", ")
                );
                osascript(&script)?;
                return Ok("Quit all apps".into());
            }
            ToggleWifi => {
                let dev = wifi_device().ok_or_else(|| anyhow::anyhow!("no Wi-Fi interface"))?;
                let st = Command::new("/usr/sbin/networksetup").args(["-getairportpower", &dev]).output()?;
                let on = String::from_utf8_lossy(&st.stdout).trim_end().ends_with("On");
                sh("/usr/sbin/networksetup", &["-setairportpower", &dev, if on { "off" } else { "on" }])?;
                return Ok(if on { "Wi-Fi off".into() } else { "Wi-Fi on".into() });
            }
            KeepAwake => {
                if let Some(pid) = awake_pid() {
                    sh("/bin/kill", &[&pid.to_string()])?;
                    return Ok("Sleep allowed again".into());
                }
                // -d keeps the display awake, -i the system; detached so it outlives this call.
                Command::new("/usr/bin/caffeinate")
                    .args(["-di"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?;
                return Ok("Keeping the Mac awake".into());
            }
            DismissNotifications => {
                osascript(
                    "tell application \"System Events\" to tell process \"NotificationCenter\"\ntry\nrepeat with w in windows\nrepeat with g in (groups of UI element 1 of scroll area 1 of group 1 of w)\ntry\nperform (first action of g whose description is \"Close\")\nend try\nend repeat\nend repeat\nend try\nend tell",
                )?;
            }
        }
        Ok(self.title())
    }
}

/// The hardware port named Wi-Fi (`en0` on most Macs, but not all).
fn wifi_device() -> Option<String> {
    let out = Command::new("/usr/sbin/networksetup").arg("-listallhardwareports").output().ok()?;
    parse_wifi_device(&String::from_utf8_lossy(&out.stdout))
}

fn parse_wifi_device(listing: &str) -> Option<String> {
    let mut lines = listing.lines();
    while let Some(l) = lines.next() {
        if l.starts_with("Hardware Port:") && (l.contains("Wi-Fi") || l.contains("AirPort")) {
            return lines.next()?.strip_prefix("Device:").map(|d| d.trim().to_string());
        }
    }
    None
}

/// PID of a `caffeinate` we (or the user) started, if one is running.
fn awake_pid() -> Option<i32> {
    let out = Command::new("/usr/bin/pgrep").args(["-x", "caffeinate"]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().next()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_the_wifi_device() {
        let l = "\nHardware Port: Ethernet Adapter (en4)\nDevice: en4\nEthernet Address: a\n\nHardware Port: Wi-Fi\nDevice: en0\nEthernet Address: b\n";
        assert_eq!(parse_wifi_device(l).as_deref(), Some("en0"));
        assert_eq!(parse_wifi_device("Hardware Port: Ethernet\nDevice: en1\n"), None);
    }

    #[test]
    fn ids_unique() {
        let mut ids: Vec<String> = SystemCommand::ALL.iter().map(|c| c.id()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}

#[cfg(test)]
mod timeout_tests {
    use super::{osascript_raw, osascript_timeout, reduce_motion};

    #[test]
    fn raw_keeps_whitespace_while_osascript_timeout_still_trims() {
        let script = "return \"  a b  \"";
        assert_eq!(osascript_raw(script, 5, None).unwrap(), "  a b  \n");
        assert_eq!(osascript_timeout(script, 5).unwrap(), "a b", "other callers keep getting trimmed output");
    }

    #[test]
    fn a_cancelled_script_is_killed_long_before_its_deadline() {
        let cancel = std::sync::atomic::AtomicBool::new(true);
        let t = std::time::Instant::now();
        let e = osascript_raw("delay 30", 60, Some(&cancel)).unwrap_err().to_string();
        assert!(e.contains("cancelled"), "{e}");
        assert!(t.elapsed() < std::time::Duration::from_secs(3));
    }

    #[test]
    fn a_stuck_script_is_killed_at_the_deadline() {
        let t = std::time::Instant::now();
        let e = osascript_timeout("delay 30", 1).unwrap_err().to_string();
        assert!(e.contains("timed out"), "{e}");
        assert!(t.elapsed().as_secs() < 5, "returned long after the deadline");
    }

    #[test]
    fn reduce_motion_reads_the_system_flag_without_panicking() {
        let v = reduce_motion();
        println!("reduce_motion() = {v}");
        assert_eq!(v, reduce_motion(), "stable between calls");
    }

    #[test]
    fn a_quick_script_returns_its_output() {
        assert_eq!(osascript_timeout("return \"ok\"", 10).unwrap(), "ok");
    }
}

#[cfg(test)]
mod os_version_tests {
    #[test]
    fn parses_major() {
        assert_eq!(super::parse_major("14.5\n"), Some(14));
        assert_eq!(super::parse_major("27.0.1"), Some(27));
        assert_eq!(super::parse_major(""), None);
        assert!(super::os_at_least(1));
    }
}
