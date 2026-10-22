//! Persisted user data models.

use crate::fuzzy::Sensitivity;
use crate::ranking::Frecency;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snippet {
    #[serde(default = "new_id")]
    pub id: String,
    pub name: String,
    pub text: String,
    #[serde(default)]
    pub keyword: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Snippet {
    pub fn new(name: &str, text: &str, keyword: &str) -> Self {
        Snippet { id: new_id(), name: name.into(), text: text.into(), keyword: keyword.into(), tags: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quicklink {
    #[serde(default = "new_id")]
    pub id: String,
    pub name: String,
    pub link: String,
    /// Bundle path/name of the app to open with (None = default handler).
    #[serde(default)]
    pub open_with: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub pinned: bool,
}

impl Quicklink {
    pub fn new(name: &str, link: &str) -> Self {
        Quicklink { id: new_id(), name: name.into(), link: link.into(), open_with: None, icon: None, pinned: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    #[serde(default = "new_id")]
    pub id: String,
    pub text: String,
    pub created: i64,
    pub modified: i64,
}

impl Note {
    pub fn title(&self) -> String {
        let t = self.text.lines().find(|l| !l.trim().is_empty()).unwrap_or("Untitled").trim();
        t.trim_start_matches('#').trim().chars().take(80).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClipKind {
    Text,
    Link,
    Color,
    File,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipEntry {
    #[serde(default = "new_id")]
    pub id: String,
    pub kind: ClipKind,
    /// Text content, file path(s) joined by newlines, or image file name.
    pub content: String,
    #[serde(default)]
    pub title: Option<String>,
    pub copied_at: i64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub source_app: Option<String>,
    #[serde(default)]
    pub times_copied: u32,
    /// Image size, if an image.
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
}

impl ClipEntry {
    pub fn classify_text(s: &str) -> ClipKind {
        let t = s.trim();
        if crate::color::Rgba::parse(t).is_some() {
            ClipKind::Color
        } else if (t.starts_with("http://") || t.starts_with("https://")) && !t.contains(char::is_whitespace) {
            ClipKind::Link
        } else {
            ClipKind::Text
        }
    }
    pub fn display_title(&self) -> String {
        if let Some(t) = &self.title {
            return t.clone();
        }
        match self.kind {
            ClipKind::Image => format!("Image ({}×{})", self.width, self.height),
            ClipKind::File => {
                let first = self.content.lines().next().unwrap_or("");
                let name = std::path::Path::new(first).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let n = self.content.lines().count();
                if n > 1 {
                    format!("{name} and {} more", n - 1)
                } else {
                    name
                }
            }
            _ => self.content.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect(),
        }
    }
}

/// Clipboard retention policy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Retention {
    Day,
    Week,
    #[default]
    Month,
    ThreeMonths,
    Year,
    Unlimited,
}

impl Retention {
    pub fn secs(&self) -> Option<i64> {
        Some(match self {
            Retention::Day => 86400,
            Retention::Week => 7 * 86400,
            Retention::Month => 30 * 86400,
            Retention::ThreeMonths => 90 * 86400,
            Retention::Year => 365 * 86400,
            Retention::Unlimited => return None,
        })
    }
    pub const ALL: [Retention; 6] =
        [Retention::Day, Retention::Week, Retention::Month, Retention::ThreeMonths, Retention::Year, Retention::Unlimited];
    pub fn label(&self) -> &'static str {
        match self {
            Retention::Day => "1 Day",
            Retention::Week => "7 Days",
            Retention::Month => "1 Month",
            Retention::ThreeMonths => "3 Months",
            Retention::Year => "1 Year",
            Retention::Unlimited => "Unlimited",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

/// Per-command user configuration (alias, hotkey, enabled).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CommandConfig {
    #[serde(default)]
    pub alias: Option<String>,
    /// Hotkey string like "Alt+Shift+KeyL" (global-hotkey format).
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

/// Hotkeys bound out of the box; the user can change or remove them in Settings.
pub const DEFAULT_HOTKEYS: &[(&str, &str)] = &[("builtin:clipboard-history", "Control+KeyV")];

/// Where Ask AI sends its questions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AiProvider {
    /// Any server speaking the OpenAI chat-completions API: Ollama, LM Studio, OpenAI, OpenRouter…
    #[default]
    OpenAiCompatible,
    /// Anthropic's Messages API with an API key.
    Anthropic,
    /// The Claude Code command-line tool, signed in with the user's own Claude subscription.
    ClaudeCode,
    /// Sign in with ChatGPT: the OAuth flow and backend the Codex CLI uses, with the user's ChatGPT plan.
    ChatGpt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub launcher_hotkey: String,
    pub theme: Theme,
    pub compact_mode: bool,
    pub sensitivity: Sensitivity,
    pub clipboard_enabled: bool,
    pub clipboard_retention: Retention,
    pub clipboard_ignored_apps: Vec<String>,
    pub snippet_expansion: bool,
    pub snippet_disabled_apps: Vec<String>,
    pub script_directories: Vec<String>,
    pub file_search_in_root: bool,
    pub rem_px: f64,
    pub window_gap: f64,
    pub fallback_search_url: String,
    pub favorites: Vec<String>,
    pub commands: HashMap<String, CommandConfig>,
    pub show_menu_bar_icon: bool,
    pub quit_all_excluded: Vec<String>,
    pub ai_provider: AiProvider,
    pub ai_base_url: String,
    pub ai_model: String,
    /// Model for the Anthropic API (the OpenAI-compatible model name lives in `ai_model`).
    pub anthropic_model: String,
    /// Model alias for Claude Code (kept apart from `anthropic_model`, whose names are different).
    pub claude_code_model: String,
    /// Model for Sign in with ChatGPT; empty means the default.
    pub chatgpt_model: String,
    pub launch_at_login: bool,
    pub onboarded: bool,
    /// Tap a key while dragging files to open a wheel of conversions or actions.
    pub drop_wheel: bool,
    /// "Shift", "Control", "Option" or "Command".
    pub wheel_convert_key: String,
    pub wheel_actions_key: String,
    pub wheel_tools_key: String,
    /// Key tapped during a TEXT drag to open the Translate wheel. Not Shift: Shift extends the selection.
    pub wheel_translate_key: String,
    /// Opt-in: Return on a Finder selection opens the Convert wheel (needs Accessibility).
    pub enter_on_finder: bool,
    /// Target languages on the Translate wheel (names the model understands).
    pub translate_languages: Vec<String>,
    pub translate_output: TranslateOutput,
    /// Provider that translates; `None` follows the chat provider.
    pub translate_provider: Option<AiProvider>,
    /// Check GitHub for new stable releases (off = never touch the network).
    pub update_check: bool,
    pub update_install: UpdateInstall,
    /// Version the user chose to skip.
    pub update_skipped: Option<String>,
    /// Unix seconds of the last automatic check.
    pub update_last_check: Option<i64>,
}

/// What happens with a finished translation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TranslateOutput {
    /// Replace the selection in place where possible (editable field, Safari/Chromium page),
    /// otherwise show the Ferrocast window.
    #[default]
    Auto,
    /// Show it in a Ferrocast window (and keep it on the clipboard).
    Show,
    /// Paste it over the selection in the app the text came from.
    Replace,
    /// Only put it on the clipboard.
    Copy,
}

/// How a found update is installed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum UpdateInstall {
    Automatic,
    #[default]
    AskFirst,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            launcher_hotkey: "Alt+Space".into(),
            theme: Theme::System,
            compact_mode: false,
            sensitivity: Sensitivity::Medium,
            clipboard_enabled: true,
            clipboard_retention: Retention::Month,
            clipboard_ignored_apps: vec![
                "com.agilebits.onepassword7".into(),
                "com.1password.1password".into(),
                "com.apple.keychainaccess".into(),
            ],
            snippet_expansion: true,
            snippet_disabled_apps: vec!["com.1password.1password".into(), "com.apple.keychainaccess".into()],
            script_directories: vec![],
            file_search_in_root: true,
            rem_px: 16.0,
            window_gap: 0.0,
            fallback_search_url: "https://www.google.com/search?q={argument name=\"query\"}".into(),
            favorites: vec![],
            commands: HashMap::new(),
            show_menu_bar_icon: true,
            quit_all_excluded: vec!["Finder".into()],
            ai_provider: AiProvider::default(),
            ai_base_url: crate::ai::DEFAULT_BASE_URL.into(),
            ai_model: crate::ai::DEFAULT_MODEL.into(),
            anthropic_model: String::new(),
            claude_code_model: String::new(),
            chatgpt_model: String::new(),
            launch_at_login: false,
            onboarded: false,
            drop_wheel: true,
            wheel_convert_key: "Shift".into(),
            wheel_actions_key: "Control".into(),
            wheel_tools_key: "Command".into(),
            wheel_translate_key: "Command".into(),
            enter_on_finder: false,
            translate_languages: crate::translate::DEFAULT_LANGUAGES.iter().map(|s| s.to_string()).collect(),
            translate_output: TranslateOutput::default(),
            translate_provider: None,
            update_check: true,
            update_install: UpdateInstall::default(),
            update_skipped: None,
            update_last_check: None,
        }
    }
}

impl Settings {
    pub fn alias(&self, id: &str) -> Option<&str> {
        self.commands.get(id).and_then(|c| c.alias.as_deref()).filter(|a| !a.is_empty())
    }
    /// Effective hotkey: the user's choice, else the built-in default. An empty
    /// string means the user removed the hotkey on purpose.
    pub fn hotkey(&self, id: &str) -> Option<&str> {
        match self.commands.get(id).and_then(|c| c.hotkey.as_deref()) {
            Some(h) => Some(h).filter(|a| !a.is_empty()),
            None => DEFAULT_HOTKEYS.iter().find(|(c, _)| *c == id).map(|(_, h)| *h),
        }
    }
    pub fn is_disabled(&self, id: &str) -> bool {
        self.commands.get(id).is_some_and(|c| c.disabled)
    }
    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.iter().any(|f| f == id)
    }
    pub fn toggle_favorite(&mut self, id: &str) {
        if let Some(i) = self.favorites.iter().position(|f| f == id) {
            self.favorites.remove(i);
        } else {
            self.favorites.push(id.to_string());
        }
    }
    /// Move a favorite by `delta` positions.
    pub fn move_favorite(&mut self, id: &str, delta: isize) {
        if let Some(i) = self.favorites.iter().position(|f| f == id) {
            let j = (i as isize + delta).clamp(0, self.favorites.len() as isize - 1) as usize;
            let v = self.favorites.remove(i);
            self.favorites.insert(j, v);
        }
    }
    pub fn command_mut(&mut self, id: &str) -> &mut CommandConfig {
        self.commands.entry(id.to_string()).or_default()
    }
}

/// Everything persisted, grouped by file.
#[derive(Debug, Default)]
pub struct Data {
    pub settings: Settings,
    pub snippets: Vec<Snippet>,
    pub quicklinks: Vec<Quicklink>,
    pub notes: Vec<Note>,
    pub clipboard: Vec<ClipEntry>,
    pub frecency: Frecency,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_on_finder_is_off_by_default_and_old_settings_files_still_load() {
        assert!(!Settings::default().enter_on_finder);
        let old = r#"{"launcher_hotkey":"Alt+Space","compact_mode":true,"drop_wheel":false}"#;
        let s: Settings = serde_json::from_str(old).expect("a settings.json without the field loads");
        assert!(!s.enter_on_finder);
        assert!(s.compact_mode, "other fields are still read");
        let on: Settings = serde_json::from_str(r#"{"enter_on_finder":true}"#).unwrap();
        assert!(on.enter_on_finder);
        let round: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert!(round.enter_on_finder);
    }

    #[test]
    fn favorites() {
        let mut s = Settings::default();
        s.toggle_favorite("a");
        s.toggle_favorite("b");
        s.move_favorite("b", -1);
        assert_eq!(s.favorites, vec!["b", "a"]);
        s.toggle_favorite("b");
        assert_eq!(s.favorites, vec!["a"]);
    }

    #[test]
    fn classify() {
        assert_eq!(ClipEntry::classify_text("https://x.com/a"), ClipKind::Link);
        assert_eq!(ClipEntry::classify_text("#ff0000"), ClipKind::Color);
        assert_eq!(ClipEntry::classify_text("hello world"), ClipKind::Text);
    }

    #[test]
    fn settings_tolerate_missing_fields() {
        let s: Settings = serde_json::from_str("{\"compact_mode\": true}").unwrap();
        assert!(s.compact_mode);
        assert_eq!(s.launcher_hotkey, "Alt+Space");
    }

    #[test]
    fn default_hotkey_can_be_overridden_or_removed() {
        let mut s = Settings::default();
        let id = "builtin:clipboard-history";
        assert_eq!(s.hotkey(id), Some("Control+KeyV"));
        s.command_mut(id).hotkey = Some("Alt+KeyV".into());
        assert_eq!(s.hotkey(id), Some("Alt+KeyV"));
        s.command_mut(id).hotkey = Some(String::new());
        assert_eq!(s.hotkey(id), None);
        assert_eq!(s.hotkey("builtin:other"), None);
    }

    #[test]
    fn note_title() {
        let n = Note { id: "1".into(), text: "\n# Groceries\n- milk".into(), created: 0, modified: 0 };
        assert_eq!(n.title(), "Groceries");
    }
}

/// One line of the calculator history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalcEntry {
    pub question: String,
    pub display: String,
    pub raw: String,
    pub value: f64,
    pub unit: Option<String>,
    pub at: i64,
}
