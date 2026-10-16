//! Searchable items, sections and actions shown in the launcher.

use egui::{Key, Modifiers};
use ferrocast_core::{calc::CalcResult, color::Rgba};
use ferrocast_macos::{running::WindowInfo, system::SystemCommand, windows::WindowCommand};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    /// Finder icon of a path.
    App(String),
    /// A symbol glyph on a colored rounded square.
    Glyph(&'static str, [u8; 3]),
    /// Colour emoji rendered by AppKit.
    Emoji(String),
    Color(Rgba),
    /// Clipboard image file name.
    ClipImage(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    AskAi,
    Welcome,
    KeyboardShortcuts,
    About,
    ExportData,
    ImportSnippets,
    RevealLog,
    ClipboardHistory,
    SearchSnippets,
    CreateSnippet,
    SearchQuicklinks,
    CreateQuicklink,
    SearchEmoji,
    SearchFiles,
    SwitchWindows,
    KillProcess,
    Timers,
    SearchNotes,
    CreateNote,
    CreateScript,
    OpenScriptsFolder,
    ReloadScripts,
    Settings,
    Quit,
}

impl Builtin {
    pub const ALL: &'static [Builtin] = &[
        Builtin::AskAi,
        Builtin::Welcome,
        Builtin::KeyboardShortcuts,
        Builtin::ExportData,
        Builtin::ImportSnippets,
        Builtin::RevealLog,
        Builtin::About,
        Builtin::ClipboardHistory,
        Builtin::SearchSnippets,
        Builtin::CreateSnippet,
        Builtin::SearchQuicklinks,
        Builtin::CreateQuicklink,
        Builtin::SearchEmoji,
        Builtin::SearchFiles,
        Builtin::SwitchWindows,
        Builtin::KillProcess,
        Builtin::Timers,
        Builtin::SearchNotes,
        Builtin::CreateNote,
        Builtin::CreateScript,
        Builtin::OpenScriptsFolder,
        Builtin::ReloadScripts,
        Builtin::Settings,
        Builtin::Quit,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Builtin::AskAi => "Ask AI",
            Builtin::Welcome => "Welcome & Permissions",
            Builtin::KeyboardShortcuts => "Keyboard Shortcuts",
            Builtin::About => "About Ferrocast",
            Builtin::ExportData => "Export Backup",
            Builtin::ImportSnippets => "Import Snippets",
            Builtin::RevealLog => "Reveal Log File",
            Builtin::ClipboardHistory => "Clipboard History",
            Builtin::SearchSnippets => "Search Snippets",
            Builtin::CreateSnippet => "Create Snippet",
            Builtin::SearchQuicklinks => "Search Quicklinks",
            Builtin::CreateQuicklink => "Create Quicklink",
            Builtin::SearchEmoji => "Search Emoji & Symbols",
            Builtin::SearchFiles => "Search Files",
            Builtin::SwitchWindows => "Switch Windows",
            Builtin::KillProcess => "Kill Process",
            Builtin::Timers => "Timers",
            Builtin::SearchNotes => "Search Notes",
            Builtin::CreateNote => "Create Note",
            Builtin::CreateScript => "Create Script Command",
            Builtin::OpenScriptsFolder => "Open Script Commands Folder",
            Builtin::ReloadScripts => "Reload Script Directories",
            Builtin::Settings => "Ferrocast Settings",
            Builtin::Quit => "Quit Ferrocast",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Builtin::AskAi => "AI",
            Builtin::Welcome | Builtin::KeyboardShortcuts | Builtin::About | Builtin::RevealLog => "Ferrocast",
            Builtin::ExportData | Builtin::ImportSnippets => "Data",
            Builtin::ClipboardHistory => "Clipboard History",
            Builtin::SearchSnippets | Builtin::CreateSnippet => "Snippets",
            Builtin::SearchQuicklinks | Builtin::CreateQuicklink => "Quicklinks",
            Builtin::SearchEmoji => "Emoji & Symbols",
            Builtin::SearchFiles => "File Search",
            Builtin::SwitchWindows => "Navigation",
            Builtin::KillProcess => "System",
            Builtin::Timers => "Productivity",
            Builtin::SearchNotes | Builtin::CreateNote => "Notes",
            Builtin::CreateScript | Builtin::OpenScriptsFolder | Builtin::ReloadScripts => "Script Commands",
            Builtin::Settings | Builtin::Quit => "Ferrocast",
        }
    }

    pub fn glyph(&self) -> (&'static str, [u8; 3]) {
        match self {
            Builtin::AskAi => ("✦", [168, 110, 255]),
            Builtin::Welcome => ("✓", [89, 212, 153]),
            Builtin::KeyboardShortcuts => ("⌘", [90, 90, 105]),
            Builtin::About => ("ⓘ", [86, 194, 255]),
            Builtin::ExportData | Builtin::ImportSnippets => ("⇅", [255, 167, 38]),
            Builtin::RevealLog => ("▤", [120, 120, 130]),
            Builtin::ClipboardHistory => ("⎘", [255, 99, 99]),
            Builtin::SearchSnippets | Builtin::CreateSnippet => ("✂", [255, 167, 38]),
            Builtin::SearchQuicklinks | Builtin::CreateQuicklink => ("↗", [86, 194, 255]),
            Builtin::SearchEmoji => ("☺", [255, 197, 49]),
            Builtin::SearchFiles => ("🔍", [120, 120, 130]),
            Builtin::SwitchWindows => ("⊞", [89, 212, 153]),
            Builtin::KillProcess => ("✕", [240, 83, 95]),
            Builtin::Timers => ("◷", [245, 165, 36]),
            Builtin::SearchNotes | Builtin::CreateNote => ("✎", [255, 197, 49]),
            Builtin::CreateScript | Builtin::OpenScriptsFolder | Builtin::ReloadScripts => ("›_", [60, 60, 70]),
            Builtin::Settings => ("⚙", [120, 120, 130]),
            Builtin::Quit => ("⏻", [177, 36, 36]),
        }
    }

    pub fn keywords(&self) -> Vec<String> {
        let k: &[&str] = match self {
            Builtin::AskAi => &["chatgpt", "llm", "ollama", "assistant", "question"],
            Builtin::Welcome => &["permissions", "accessibility", "setup", "onboarding", "help"],
            Builtin::KeyboardShortcuts => &["help", "keys", "cheat sheet"],
            Builtin::About => &["version", "license"],
            Builtin::ExportData => &["backup", "save"],
            Builtin::ImportSnippets => &["raycast", "restore"],
            Builtin::RevealLog => &["debug", "errors", "support"],
            Builtin::ClipboardHistory => &["paste", "copy", "history"],
            Builtin::SearchEmoji => &["emoji", "symbols", "unicode"],
            Builtin::SearchFiles => &["finder", "find"],
            Builtin::SwitchWindows => &["windows", "switch"],
            Builtin::KillProcess => &["kill", "process", "force quit", "task", "cpu", "memory"],
            Builtin::Timers => &["timer", "countdown", "pomodoro", "alarm"],
            Builtin::Settings => &["preferences", "config"],
            _ => &[],
        };
        k.iter().map(|s| s.to_string()).collect()
    }

    pub fn id(&self) -> String {
        format!("builtin:{}", self.title().to_lowercase().replace([' ', '&'], "-"))
    }
}

#[derive(Clone, Debug)]
pub enum Kind {
    App {
        path: String,
    },
    System(SystemCommand),
    Window(WindowCommand),
    Builtin(Builtin),
    /// A builtin opened with a prefilled query.
    Prefilled(Builtin, String),
    Quicklink {
        id: String,
        arg: Option<String>,
    },
    Snippet(String),
    Script(usize),
    Calc(CalcResult),
    Color(Rgba),
    ColorFormat(String),
    Url(String),
    File(PathBuf),
    WebSearch(String),
    Clip(String),
    Emoji(&'static str),
    Note(String),
    OpenWindow(WindowInfo),
    Process(ferrocast_macos::process::ProcInfo),
    /// Start a countdown of this many seconds with an optional label.
    Timer(u64, String),
    /// A running timer, by its index in `App::timers`.
    RunningTimer(usize),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: Icon,
    /// Right-aligned type label ("Application", "Command", …).
    pub accessory: String,
    pub keywords: Vec<String>,
    pub kind: Kind,
}

impl Item {
    /// Can this item be configured (favorite, alias, hotkey) in Root Search?
    pub fn is_command(&self) -> bool {
        matches!(
            self.kind,
            Kind::App { .. } | Kind::System(_) | Kind::Window(_) | Kind::Builtin(_) | Kind::Quicklink { .. } | Kind::Script(_)
        )
    }
}

#[derive(Clone)]
pub struct Section {
    pub title: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Act {
    Run,
    CopyText(String),
    PasteText(String),
    Reveal(PathBuf),
    QuitApp(i32),
    ForceQuitApp(i32),
    /// Send SIGTERM (false) or SIGKILL (true) to a process.
    KillProcess(i32, bool),
    CancelTimer(usize),
    HideApp(i32),
    ToggleFavorite,
    MoveFavorite(isize),
    SetAlias,
    SetHotkey,
    ResetRanking,
    DisableCommand,
    EditSnippet(String),
    DuplicateSnippet(String),
    DeleteSnippet(String),
    CreateSnippet,
    EditQuicklink(String),
    DuplicateQuicklink(String),
    DeleteQuicklink(String),
    CreateQuicklink,
    ClipCopy(String),
    ClipPaste(String),
    ClipPin(String),
    ClipDelete(String),
    ClipDeleteAll,
    ClipSaveSnippet(String),
    NoteDelete(String),
    CreateNote,
    Trash(PathBuf),
    EditScript(PathBuf),
    RunSystem(SystemCommand),
    RunScript {
        index: usize,
        args: Vec<String>,
    },
}

#[derive(Clone, Debug)]
pub struct Action {
    pub title: String,
    pub kind: Act,
    pub shortcut: Option<(Modifiers, Key)>,
    pub destructive: bool,
}

impl Action {
    pub fn new(title: impl Into<String>, kind: Act) -> Action {
        Action { title: title.into(), kind, shortcut: None, destructive: false }
    }
    pub fn key(mut self, m: Modifiers, k: Key) -> Action {
        self.shortcut = Some((m, k));
        self
    }
    pub fn danger(mut self) -> Action {
        self.destructive = true;
        self
    }
}

pub const CMD: Modifiers = Modifiers::COMMAND;
pub const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };
pub const CTRL: Modifiers = Modifiers::CTRL;
pub const CTRL_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::CTRL };

/// `⌃⌥⇧⌘K`-style label.
pub fn shortcut_label(m: Modifiers, k: Key) -> String {
    let mut s = String::new();
    if m.ctrl {
        s.push('⌃');
    }
    if m.alt {
        s.push('⌥');
    }
    if m.shift {
        s.push('⇧');
    }
    if m.mac_cmd || m.command {
        s.push('⌘');
    }
    s.push_str(match k {
        Key::Enter => "↵",
        Key::ArrowUp => "↑",
        Key::ArrowDown => "↓",
        Key::Backspace => "⌫",
        Key::Delete => "⌦",
        Key::Period => ".",
        Key::Comma => ",",
        other => other.name(),
    });
    s
}
