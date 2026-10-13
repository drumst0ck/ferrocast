//! System commands (lock, sleep, volume, appearance, trash…).

use std::process::Command;

/// Run AppleScript, returning trimmed stdout.
pub fn osascript(script: &str) -> anyhow::Result<String> {
    let out = Command::new("/usr/bin/osascript").arg("-e").arg(script).output()?;
    if !out.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
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
            _ => &[],
        }
    }

    /// Commands that should ask for confirmation.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            SystemCommand::Restart | SystemCommand::ShutDown | SystemCommand::LogOut | SystemCommand::EmptyTrash | SystemCommand::QuitAllApps
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
            DismissNotifications => {
                osascript(
                    "tell application \"System Events\" to tell process \"NotificationCenter\"\ntry\nrepeat with w in windows\nrepeat with g in (groups of UI element 1 of scroll area 1 of group 1 of w)\ntry\nperform (first action of g whose description is \"Close\")\nend try\nend repeat\nend repeat\nend try\nend tell",
                )?;
            }
        }
        Ok(self.title())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_unique() {
        let mut ids: Vec<String> = SystemCommand::ALL.iter().map(|c| c.id()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
