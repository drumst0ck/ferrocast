//! Launcher application: state, views, keyboard handling, actions, drawing.

use crate::instance;
use crate::items::*;
use crate::services::{self, now_secs, Bus, Hotkeys, Msg, Shared};
use crate::ui::{self, palette, IconCache, Palette};
use egui::{Align2, Color32, Context, FontId, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};
use ferrocast_core::calc;
use ferrocast_core::color::Rgba;
use ferrocast_core::models::*;
use ferrocast_core::placeholders::{self, Target};
use ferrocast_core::ranking::{self, Frecency, Rankable};
use ferrocast_core::script::{self, Mode, ScriptCommand};
use ferrocast_core::storage::Store;
use ferrocast_macos::{apps, clipboard, keyboard, running, system, windows};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct ArgDef {
    pub name: String,
    pub options: Vec<(String, String)>,
    pub optional: bool,
    pub password: bool,
}

#[derive(Clone, Debug)]
pub enum ArgTarget {
    Quicklink(String),
    Snippet(String),
    Script(usize),
}

/// A countdown started from Root Search.
pub struct RunningTimer {
    pub ends_at: Instant,
    pub secs: u64,
    pub label: String,
    pub cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
pub enum View {
    Root,
    Clipboard,
    Snippets,
    Quicklinks,
    Emoji,
    Files,
    Windows,
    Processes,
    Timers,
    CalcHistory,
    Shelf,
    Convert(Vec<PathBuf>),
    Notes,
    Colors(Rgba),
    SnippetForm {
        id: Option<String>,
        name: String,
        text: String,
        keyword: String,
    },
    QuicklinkForm {
        id: Option<String>,
        name: String,
        link: String,
        open_with: String,
    },
    NoteEdit(String),
    Args {
        target: ArgTarget,
        title: String,
        defs: Vec<ArgDef>,
        values: Vec<String>,
    },
    Output {
        run: u64,
        title: String,
        text: String,
        done: bool,
    },
    Confirm {
        msg: String,
        act: Act,
    },
    Settings,
    Alias {
        id: String,
        name: String,
        value: String,
    },
    Hotkey {
        id: String,
        name: String,
        err: String,
    },
    CreateScript {
        title: String,
        mode: usize,
    },
    Welcome,
    Shortcuts,
    About,
    Ai {
        prompt: String,
        asked: String,
        answer: String,
        history: Vec<(String, String)>,
        busy: bool,
        run: u64,
    },
    /// Radial menu shown under the pointer during a file drag; `center` is in screen points.
    Wheel {
        files: Vec<PathBuf>,
        center: Pos2,
        segs: Vec<WheelSeg>,
        kind: ferrocast_core::wheel::WheelKind,
        /// Segment chosen with the arrow keys.
        sel: Option<usize>,
    },
}

/// One wheel segment: label, SF symbol, and what releasing on it does.
#[derive(Clone)]
pub struct WheelSeg {
    pub label: String,
    pub symbol: &'static str,
    pub pick: WheelPick,
}

#[derive(Clone)]
pub enum WheelPick {
    Convert(ferrocast_core::convert::Target),
    Image(ferrocast_core::convert::ImageOp),
    Act(Act),
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    List,
    Form,
    Single,
}

impl View {
    fn shape(&self) -> Shape {
        match self {
            View::Root
            | View::Clipboard
            | View::Snippets
            | View::Quicklinks
            | View::Emoji
            | View::Files
            | View::Windows
            | View::Processes
            | View::Timers
            | View::CalcHistory
            | View::Shelf
            | View::Convert(_)
            | View::Notes
            | View::Colors(_) => Shape::List,
            View::SnippetForm { .. } | View::QuicklinkForm { .. } | View::NoteEdit(_) | View::Settings => Shape::Form,
            _ => Shape::Single,
        }
    }
    fn title(&self) -> &'static str {
        match self {
            View::Root => "Ferrocast",
            View::Clipboard => "Clipboard History",
            View::Snippets => "Search Snippets",
            View::Quicklinks => "Search Quicklinks",
            View::Emoji => "Search Emoji & Symbols",
            View::Files => "Search Files",
            View::Windows => "Switch Windows",
            View::Processes => "Kill Process",
            View::Timers => "Timers",
            View::CalcHistory => "Calculator History",
            View::Shelf => "File Shelf",
            View::Convert(_) => "Convert",
            View::Notes => "Search Notes",
            View::Colors(_) => "Copy Color as…",
            View::SnippetForm { .. } => "Snippet",
            View::QuicklinkForm { .. } => "Quicklink",
            View::NoteEdit(_) => "Note",
            View::Args { .. } => "Arguments",
            View::Output { .. } => "Output",
            View::Confirm { .. } => "Confirm",
            View::Settings => "Settings",
            View::Alias { .. } => "Set Alias",
            View::Hotkey { .. } => "Set Hotkey",
            View::CreateScript { .. } => "Create Script Command",
            View::Ai { .. } => "Ask AI",
            View::Welcome => "Welcome to Ferrocast",
            View::Shortcuts => "Keyboard Shortcuts",
            View::About => "About Ferrocast",
            View::Wheel { .. } => "Wheel",
        }
    }
    fn placeholder(&self) -> &'static str {
        match self {
            View::Root => "Search apps, commands, files… or calculate",
            View::Clipboard => "Search clipboard history…",
            View::Snippets => "Search snippets…",
            View::Quicklinks => "Search quicklinks…",
            View::Emoji => "Search emoji by name…",
            View::Files => "Search files by name…",
            View::Windows => "Search open windows…",
            View::Processes => "Search processes…",
            View::Timers => "Search timers…",
            View::CalcHistory => "Search past calculations…",
            View::Shelf => "Search the shelf…",
            View::Convert(_) => "Convert to…",
            View::Notes => "Search notes…",
            _ => "Filter…",
        }
    }
}

enum RunKind {
    Output,
    Compact(String),
    Silent(String),
    Inline(PathBuf),
}

struct ActionPanel {
    actions: Vec<Action>,
    query: String,
    sel: usize,
    item: Item,
}

#[derive(Default)]
struct Keys {
    up: bool,
    down: bool,
    enter: bool,
    cmd_enter: bool,
    esc: bool,
    backspace: bool,
    cmd_k: bool,
}

pub struct App {
    ctx: Context,
    shared: Arc<Shared>,
    bus: Bus,
    rx: Receiver<Msg>,
    store: Store,
    settings: Settings,
    snippets: Vec<Snippet>,
    quicklinks: Vec<Quicklink>,
    notes: Vec<Note>,
    frecency: Frecency,
    apps: Vec<apps::AppInfo>,
    apps_at: Instant,
    scripts: Vec<ScriptCommand>,
    base: Vec<Item>,
    hotkeys: Option<Hotkeys>,
    _tray: Option<tray_icon::TrayIcon>,
    icons: IconCache,
    stack: Vec<View>,
    query: String,
    prev_query: String,
    selected: usize,
    scroll_to_sel: bool,
    panel: Option<ActionPanel>,
    toast: Option<(String, Instant)>,
    prev_pid: Option<i32>,
    visible: bool,
    had_focus: bool,
    shown_at: Instant,
    hidden_at: Instant,
    cur_height: f32,
    files: (String, Vec<PathBuf>),
    files_req: String,
    query_changed_at: Instant,
    win_cache: Vec<running::WindowInfo>,
    proc_cache: Vec<ferrocast_macos::process::ProcInfo>,
    /// Running countdowns: when they end, their length, label, and a cancel flag.
    timers: Vec<RunningTimer>,
    calc_history: Vec<ferrocast_core::models::CalcEntry>,
    recent_emoji: Vec<String>,
    /// Files kept on the File Shelf (persisted in shelf.json).
    shelf: Vec<PathBuf>,
    /// The app icon, decoded once for the footer.
    app_icon: Option<egui::TextureHandle>,
    /// Whether the 22 MB Unicode fallback font has been loaded.
    wide_font: bool,
    /// Set by show(); cleared by the first frame drawn afterwards (FERROCAST_PROFILE).
    pub(crate) show_t: Option<Instant>,
    /// Last push/pop/reset, to debounce clicks across view changes.
    pub(crate) nav_at: Instant,
    /// Warm Claude Code process while Ask AI is open (ClaudeCode provider only).
    claude: Option<std::sync::Arc<services::ClaudeSession>>,
    /// Model lists for the Settings picker, by `services::models_key`; `None` while loading.
    /// Optional conversion tools found on this Mac (probed once).
    tools: ferrocast_core::convert::Tools,
    pub(crate) ai_models: std::collections::HashMap<String, Option<Result<Vec<String>, String>>>,
    runs: HashMap<u64, RunKind>,
    next_run: u64,
    inline: HashMap<PathBuf, (String, Instant)>,
    focus_first: bool,
    dark: bool,
    started: bool,
    act_item: Option<Item>,
    scratch: String,
    key_scratch: String,
    rates: ferrocast_core::currency::Rates,
    rates_checked_at: Instant,
    /// Result list for the current query/view, reused across frames until something changes.
    sec_cache: Option<(String, Vec<Section>)>,
    sec_dirty: bool,
}

/// San Francisco (the system face) first, Apple Symbols for ⌘⌥⇧ glyphs. Arial Unicode, the
/// 22 MB fallback for scripts San Francisco lacks (CJK, Arabic, Hebrew, Thai…), costs about
/// 40 MB of memory, so it is only added once such text actually shows up (see `App::ensure_wide_font`).
fn load_fonts(ctx: &Context, wide: bool) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        fonts.font_data.insert("sf-pro".into(), Arc::new(egui::FontData::from_owned(bytes)));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "sf-pro".into());
    }
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNSMono.ttf") {
        fonts.font_data.insert("sf-mono".into(), Arc::new(egui::FontData::from_owned(bytes)));
        fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "sf-mono".into());
    }
    let mut extra = vec![("apple-symbols", "/System/Library/Fonts/Apple Symbols.ttf")];
    if wide {
        extra.push(("arial-unicode", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"));
    }
    for (name, path) in extra {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().push(name.into());
            }
        }
    }
    ctx.set_fonts(fonts);
}

/// Scripts outside what San Francisco and Apple Symbols cover.
pub(crate) fn needs_wide_font(text: &str) -> bool {
    text.chars().any(|c| {
        let u = c as u32;
        (0x0590..0x2000).contains(&u) || (0x2E80..0x1F000).contains(&u)
    })
}

/// SF Symbol for each system command (white on the grey tile).
fn system_symbol(c: &system::SystemCommand) -> &'static str {
    use system::SystemCommand::*;
    match c {
        LockScreen => "sf:lock",
        Sleep => "sf:moon.zzz",
        SleepDisplays => "sf:display",
        Restart => "sf:arrow.counterclockwise",
        ShutDown => "sf:power",
        LogOut => "sf:rectangle.portrait.and.arrow.right",
        ScreenSaver => "sf:sparkles.tv",
        PlayPause => "sf:playpause",
        NextTrack => "sf:forward",
        PreviousTrack => "sf:backward",
        ToggleMute => "sf:speaker.slash",
        VolumeUp => "sf:speaker.wave.3",
        VolumeDown => "sf:speaker.wave.1",
        Volume(_) => "sf:speaker.wave.2",
        ShowDesktop => "sf:menubar.dock.rectangle",
        ToggleAppearance => "sf:circle.lefthalf.filled",
        OpenTrash | EmptyTrash => "sf:trash",
        EjectAll => "sf:eject",
        ToggleHiddenFiles | UnhideAll => "sf:eye",
        HideAllExceptFrontmost => "sf:eye.slash",
        QuitAllApps => "sf:xmark.circle",
        DismissNotifications => "sf:bell.slash",
        ToggleWifi => "sf:wifi",
        KeepAwake => "sf:cup.and.saucer",
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, hidden: bool) -> App {
        let ctx = cc.egui_ctx.clone();
        let prof = std::env::var_os("FERROCAST_PROFILE").is_some();
        let t = Instant::now();
        load_fonts(&ctx, false);
        if prof {
            eprintln!("fonts {:?}", t.elapsed());
        }
        let t = Instant::now();
        let store = Store::default_location();
        let calc_history = store.load("calc-history.json");
        let recent_emoji = store.load("recent-emoji.json");
        let shelf: Vec<PathBuf> = store.load("shelf.json");
        let data = store.load_all();
        if prof {
            eprintln!("data {:?}", t.elapsed());
        }
        let t = Instant::now();
        let (tx, rx) = std::sync::mpsc::channel();
        let bus = Bus { tx, ctx: ctx.clone() };
        services::ensure_scripts_dir(&store);
        let shared = Arc::new(Shared {
            store: store.clone(),
            clipboard: Mutex::new(data.clipboard.clone()),
            snippets: RwLock::new(data.snippets.clone()),
            settings: RwLock::new(data.settings.clone()),
            ignored_clipboard_count: Default::default(),
            visible: Default::default(),
            keywords: Default::default(),
        });
        services::start_clipboard(shared.clone(), bus.clone());
        services::start_expansion(shared.clone());
        services::start_wheel(shared.clone(), bus.clone());
        {
            // the floating panel reports its edits; the app persists them
            let b = bus.clone();
            ferrocast_macos::shelf::set_on_change(Box::new(move |v| b.send(Msg::ShelfChanged(v))));
        }
        if prof {
            eprintln!("services {:?}", t.elapsed());
        }
        let t = Instant::now();
        let mut hotkeys = Hotkeys::new(bus.clone());
        let mut toast = None;
        if let Some(h) = hotkeys.as_mut() {
            let errs = h.apply(&data.settings);
            if let Some(e) = errs.first() {
                toast = Some((format!("Hotkey error: {e}"), Instant::now()));
            }
        }
        let tray = if data.settings.show_menu_bar_icon { services::make_tray(bus.clone()) } else { None };
        let b2 = bus.clone();
        if prof {
            eprintln!("hotkeys {:?}", t.elapsed());
        }
        std::thread::spawn(move || {
            let t = Instant::now();
            let a = apps::list_apps();
            if std::env::var_os("FERROCAST_PROFILE").is_some() {
                eprintln!("apps scan {:?} ({} apps)", t.elapsed(), a.len());
            }
            b2.send(Msg::Apps(a))
        });
        let store_rates: ferrocast_core::currency::Rates = store.load("rates.json");
        if store_rates.is_stale(now_secs()) {
            services::refresh_rates(bus.clone());
        }
        let mut app = App {
            icons: IconCache::new(store.images_dir()),
            ctx,
            shared,
            bus,
            rx,
            store,
            settings: data.settings,
            snippets: data.snippets,
            quicklinks: data.quicklinks,
            notes: data.notes,
            frecency: data.frecency,
            apps: vec![],
            apps_at: Instant::now(),
            scripts: vec![],
            base: vec![],
            hotkeys,
            _tray: tray,
            stack: vec![View::Root],
            query: String::new(),
            prev_query: String::new(),
            selected: 0,
            scroll_to_sel: false,
            panel: None,
            toast,
            prev_pid: None,
            visible: !hidden,
            had_focus: false,
            shown_at: Instant::now(),
            hidden_at: Instant::now(),
            cur_height: ui::HEIGHT,
            files: (String::new(), vec![]),
            files_req: String::new(),
            query_changed_at: Instant::now(),
            win_cache: vec![],
            proc_cache: vec![],
            timers: vec![],
            calc_history,
            recent_emoji,
            shelf,
            app_icon: None,
            wide_font: false,
            show_t: None,
            nav_at: Instant::now(),
            claude: None,
            ai_models: Default::default(),
            tools: ferrocast_core::convert::Tools::detect(),
            runs: HashMap::new(),
            next_run: 1,
            inline: HashMap::new(),
            focus_first: false,
            dark: true,
            started: false,
            act_item: None,
            scratch: String::new(),
            key_scratch: String::new(),
            rates: store_rates,
            rates_checked_at: Instant::now(),
            sec_cache: None,
            sec_dirty: true,
        };
        app.shared.visible.store(app.visible, Ordering::Relaxed);
        if !app.settings.onboarded && !hidden {
            app.settings.onboarded = true;
            let _ = app.store.save("settings.json", &app.settings);
            app.stack.push(View::Welcome);
        }
        app.reload_scripts();
        app.set_theme();
        app.rebuild();
        app
    }

    // ------------------------------------------------------------------ data

    fn set_theme(&mut self) {
        let pref = match self.settings.theme {
            Theme::System => egui::ThemePreference::System,
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::Light => egui::ThemePreference::Light,
        };
        self.ctx.set_theme(pref);
    }

    fn script_dirs(&self) -> Vec<PathBuf> {
        let mut v = vec![self.store.scripts_dir()];
        v.extend(self.settings.script_directories.iter().map(|d| {
            if let Some(r) = d.strip_prefix("~/") {
                dirs::home_dir().unwrap_or_default().join(r)
            } else {
                PathBuf::from(d)
            }
        }));
        v
    }

    fn reload_scripts(&mut self) {
        self.scripts = services::load_scripts(&self.script_dirs());
        self.rebuild();
    }

    fn save_settings(&mut self) {
        let v = self.settings.clone();
        self.persist("settings.json", &v);
        *self.shared.settings.write().unwrap() = self.settings.clone();
        self.rebuild();
    }
    fn save_snippets(&mut self) {
        let v = self.snippets.clone();
        self.persist("snippets.json", &v);
        *self.shared.snippets.write().unwrap() = self.snippets.clone();
        self.shared.refresh_keywords();
    }
    fn save_quicklinks(&mut self) {
        let v = self.quicklinks.clone();
        self.persist("quicklinks.json", &v);
        self.rebuild();
    }
    fn send_ai(&mut self) {
        let run = self.next_run;
        self.next_run += 1;
        let (history, question) = match self.stack.last_mut() {
            Some(View::Ai { prompt, asked, answer, history, busy, run: r }) if !prompt.trim().is_empty() && !*busy => {
                // The previous turn becomes context unless it failed.
                if !asked.is_empty() && !answer.is_empty() && !answer.contains("⚠ ") {
                    history.push((std::mem::take(asked), std::mem::take(answer)));
                }
                *busy = true;
                *r = run;
                *asked = std::mem::take(prompt);
                answer.clear();
                (history.clone(), asked.clone())
            }
            _ => return,
        };
        if self.settings.ai_provider == ferrocast_core::models::AiProvider::ClaudeCode {
            // The warm session already holds the conversation; send only the new turn.
            if self.claude.as_ref().is_none_or(|c| !c.is_alive()) {
                match services::ClaudeSession::start(&self.settings, self.bus.clone()) {
                    Ok(c) => self.claude = Some(c),
                    Err(e) => {
                        self.bus.send(Msg::AiDone { run, error: Some(e) });
                        return;
                    }
                }
            }
            if let Err(e) = self.claude.as_ref().unwrap().ask(run, &question) {
                self.bus.send(Msg::AiDone { run, error: Some(e) });
            }
            return;
        }
        services::ask_ai(&self.settings, history, question, run, self.bus.clone());
    }

    /// Start Claude Code in the background so the first answer does not pay its boot time.
    fn warm_claude(&mut self) {
        if self.settings.ai_provider == ferrocast_core::models::AiProvider::ClaudeCode && self.claude.as_ref().is_none_or(|c| !c.is_alive())
        {
            if let Ok(c) = services::ClaudeSession::start(&self.settings, self.bus.clone()) {
                self.claude = Some(c);
            }
        }
    }

    /// Drop the warm session once Ask AI is no longer on the stack.
    fn cool_claude(&mut self) {
        if !self.stack.iter().any(|v| matches!(v, View::Ai { .. })) {
            if let Some(c) = self.claude.take() {
                c.kill();
            }
        }
    }

    fn save_notes(&mut self) {
        let v = self.notes.clone();
        self.persist("notes.json", &v);
    }
    fn save_frecency(&self) {
        if let Err(e) = self.store.save("frecency.json", &self.frecency) {
            instance::log(&self.store.root, &format!("saving frecency.json failed: {e:#}"));
        }
    }
    fn apply_hotkeys(&mut self) {
        if let Some(h) = self.hotkeys.as_mut() {
            if let Some(e) = h.apply(&self.settings).first() {
                self.toast = Some((format!("Hotkey error: {e}"), Instant::now()));
            }
        }
    }

    /// Rebuild the cached list of root-searchable items.
    /// Load the Unicode fallback font the first time non-Latin text needs it.
    pub(crate) fn ensure_wide_font(&mut self, text: &str) {
        if !self.wide_font && needs_wide_font(text) {
            self.wide_font = true;
            load_fonts(&self.ctx, true);
        }
    }

    fn rebuild(&mut self) {
        self.sec_dirty = true;
        let mut v = vec![];
        for a in &self.apps {
            v.push(Item {
                id: format!("app:{}", a.bundle_id.clone().unwrap_or_else(|| a.path.to_string_lossy().to_string())),
                title: a.name.clone(),
                subtitle: String::new(),
                icon: Icon::App(a.path.to_string_lossy().to_string()),
                accessory: "Application".into(),
                keywords: a.keywords.clone(),
                kind: Kind::App { path: a.path.to_string_lossy().to_string() },
            });
        }
        for b in Builtin::ALL {
            let (g, c) = b.glyph();
            v.push(Item {
                id: b.id(),
                title: b.title().into(),
                subtitle: if b.extension() == b.title() { String::new() } else { b.extension().into() },
                icon: Icon::Glyph(g, c),
                accessory: "Command".into(),
                keywords: b.keywords(),
                kind: Kind::Builtin(*b),
            });
        }
        for c in windows::WindowCommand::ALL {
            v.push(Item {
                id: c.id(),
                title: c.title().into(),
                subtitle: "Window Management".into(),
                icon: Icon::Glyph("sf:rectangle.lefthalf.filled", [89, 212, 153]),
                accessory: "Command".into(),
                keywords: vec!["window".into(), "resize".into(), "move".into()],
                kind: Kind::Window(*c),
            });
        }
        for c in system::SystemCommand::ALL {
            v.push(Item {
                id: c.id(),
                title: c.title(),
                subtitle: "System".into(),
                icon: Icon::Glyph(system_symbol(c), [90, 90, 105]),
                accessory: "Command".into(),
                keywords: c.keywords().iter().map(|s| s.to_string()).collect(),
                kind: Kind::System(*c),
            });
        }
        for q in &self.quicklinks {
            v.push(Item {
                id: format!("quicklink:{}", q.id),
                title: q.name.clone(),
                subtitle: q.link.clone(),
                icon: Icon::Glyph("sf:link", [86, 194, 255]),
                accessory: "Quicklink".into(),
                keywords: vec![],
                kind: Kind::Quicklink { id: q.id.clone(), arg: None },
            });
        }
        for (i, s) in self.scripts.iter().enumerate() {
            let sub = self.inline.get(&s.path).map(|(t, _)| t.clone()).or_else(|| s.package_name.clone()).unwrap_or_default();
            v.push(Item {
                id: s.id(),
                title: s.title.clone(),
                subtitle: sub,
                icon: match s.icon.clone().filter(|i| i.chars().count() <= 2 && !i.is_ascii()) {
                    Some(e) => Icon::Emoji(e),
                    None => Icon::Glyph("sf:terminal", [60, 60, 70]),
                },
                accessory: "Script Command".into(),
                keywords: s.description.iter().cloned().collect(),
                kind: Kind::Script(i),
            });
        }
        v.retain(|i| !self.settings.is_disabled(&i.id));
        self.base = v;
    }

    // ------------------------------------------------------------- show/hide

    fn show(&mut self) {
        self.sec_dirty = true;
        if self.hidden_at.elapsed() > Duration::from_secs(90) {
            self.stack = vec![View::Root];
            self.query.clear();
        }
        self.prev_pid = running::frontmost().map(|f| f.pid).filter(|p| *p != std::process::id() as i32);
        self.visible = true;
        self.had_focus = false;
        self.shown_at = Instant::now();
        self.show_t = Some(Instant::now());
        self.panel = None;
        self.shared.visible.store(true, Ordering::Relaxed);
        if let Some(p) = launcher_origin() {
            self.ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(p));
        }
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            app.unhide(None);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        }
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        if self.apps_at.elapsed() > Duration::from_secs(600) {
            self.apps_at = Instant::now();
            let b = self.bus.clone();
            std::thread::spawn(move || b.send(Msg::Apps(apps::list_apps())));
        }
        self.ctx.request_repaint();
    }

    fn hide(&mut self) {
        self.visible = false;
        self.hidden_at = Instant::now();
        self.shared.visible.store(false, Ordering::Relaxed);
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        // hiding the whole app would also hide the floating shelf panel
        if let Some(mtm) = objc2::MainThreadMarker::new().filter(|_| !ferrocast_macos::shelf::is_visible()) {
            objc2_app_kit::NSApplication::sharedApplication(mtm).hide(None);
        }
        if let Some(pid) = self.prev_pid {
            running::activate(pid);
        }
    }

    fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Persist a data file; a failed write is surfaced instead of dropped on the floor.
    fn persist<T: serde::Serialize>(&mut self, name: &str, value: &T) {
        if let Err(e) = self.store.save(name, value) {
            instance::log(&self.store.root, &format!("saving {name} failed: {e:#}"));
            self.toast(format!("Could not save {name}: {e}"));
        }
    }

    fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
        self.ctx.request_repaint_after(Duration::from_millis(2600));
    }

    // ------------------------------------------------------------ navigation

    fn view(&self) -> &View {
        self.stack.last().unwrap()
    }

    fn push(&mut self, v: View) {
        self.sec_dirty = true;
        self.nav_at = Instant::now();
        if matches!(v, View::Windows) {
            self.win_cache = running::list_windows();
        }
        if matches!(v, View::Processes) {
            self.proc_cache = ferrocast_macos::process::list();
        }
        if matches!(v, View::Ai { .. }) {
            self.warm_claude();
        }
        self.stack.push(v);
        self.query.clear();
        self.prev_query.clear();
        self.selected = 0;
        self.scroll_to_sel = true;
        self.panel = None;
        self.focus_first = true;
    }

    fn pop(&mut self) {
        self.sec_dirty = true;
        self.nav_at = Instant::now();
        if self.stack.len() > 1 {
            self.stack.pop();
            self.query.clear();
            self.prev_query.clear();
            self.selected = 0;
            self.scroll_to_sel = true;
            self.panel = None;
            self.cool_claude();
        } else {
            self.hide();
        }
    }

    /// Segments for a wheel over these files.
    pub(crate) fn wheel_segments(&self, kind: ferrocast_core::wheel::WheelKind, files: &[PathBuf]) -> Vec<WheelSeg> {
        use ferrocast_core::convert::{family_of, targets, Family};
        let Some(first) = files.first() else { return vec![] };
        match kind {
            ferrocast_core::wheel::WheelKind::Convert => {
                let symbol = match family_of(first) {
                    Family::Image | Family::Pdf => "sf:photo",
                    Family::Audio => "sf:waveform",
                    Family::Video => "sf:film",
                    Family::Archive | Family::Folder => "sf:archivebox",
                    _ => "sf:doc.text",
                };
                targets(first, &self.tools)
                    .into_iter()
                    .filter(|t| t.needs.is_none())
                    .take(8)
                    .map(|t| WheelSeg { label: t.label.to_string(), symbol, pick: WheelPick::Convert(t) })
                    .collect()
            }
            ferrocast_core::wheel::WheelKind::Actions => {
                let f = files.to_vec();
                let paths = f.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
                let seg = |label: &str, symbol: &'static str, act: Act| WheelSeg { label: label.into(), symbol, pick: WheelPick::Act(act) };
                vec![
                    seg("Copy", "sf:doc.on.doc", Act::CopyFiles(f.clone())),
                    seg("Copy Path", "sf:link", Act::CopyText(paths)),
                    seg("Move to…", "sf:folder", Act::MoveFiles(f.clone())),
                    seg("Terminal", "sf:terminal", Act::OpenInTerminal(f[0].clone())),
                    seg("AirDrop", "sf:dot.radiowaves.left.and.right", Act::AirDrop(f.clone())),
                    seg("Quick Look", "sf:eye", Act::QuickLook(f[0].clone())),
                    seg("Float", "sf:tray.full", Act::AddToShelf(f.clone())),
                    seg("Convert…", "sf:arrow.triangle.2.circlepath", Act::Convert(f)),
                ]
            }
            ferrocast_core::wheel::WheelKind::Tools => {
                use ferrocast_core::convert::ImageOp;
                if files.iter().all(|f| family_of(f) == Family::Image) {
                    // the most used tool of each kind, so eight fit on the ring
                    let ops = [
                        ImageOp::Compress(60),
                        ImageOp::Resize(1280),
                        ImageOp::Crop(1, 1),
                        ImageOp::StripMetadata,
                        ImageOp::Rotate(90),
                        ImageOp::Redact(true),
                        ImageOp::Annotate,
                        if files.len() > 1 { ImageOp::CombinePdf } else { ImageOp::Background("FFFFFF") },
                    ];
                    ops.into_iter()
                        .map(|op| WheelSeg {
                            label: match op {
                                ImageOp::Background(_) => "Background".into(),
                                ImageOp::Annotate => "Annotate".into(),
                                _ => op.label().split(" · ").next().unwrap_or_default().to_string(),
                            },
                            symbol: image_op_symbol(&op),
                            pick: WheelPick::Image(op),
                        })
                        .collect()
                } else {
                    vec![WheelSeg {
                        label: "Open tools…".into(),
                        symbol: "sf:wrench.and.screwdriver",
                        pick: WheelPick::Act(Act::Convert(files.to_vec())),
                    }]
                }
            }
        }
    }

    /// Show the wheel centred under the pointer, without moving the drag's focus elsewhere.
    fn show_wheel(&mut self, kind: ferrocast_core::wheel::WheelKind, files: Vec<PathBuf>) {
        let segs = self.wheel_segments(kind, &files);
        if segs.is_empty() {
            return;
        }
        let size = Vec2::new(ui::WIDTH, ui::HEIGHT);
        let center = match mouse_top_left() {
            Some((m, screen)) => {
                let min = Pos2::new(
                    (m.x - size.x / 2.0).clamp(screen.min.x, (screen.max.x - size.x).max(screen.min.x)),
                    (m.y - size.y / 2.0).clamp(screen.min.y, (screen.max.y - size.y).max(screen.min.y)),
                );
                self.ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(min));
                min + size / 2.0
            }
            None => Pos2::ZERO,
        };
        self.prev_pid = running::frontmost().map(|f| f.pid).filter(|p| *p != std::process::id() as i32);
        self.stack = vec![View::Root, View::Wheel { files, center, segs, kind, sel: None }];
        self.panel = None;
        self.visible = true;
        self.had_focus = false;
        self.shown_at = Instant::now();
        self.shared.visible.store(true, Ordering::Relaxed);
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            objc2_app_kit::NSApplication::sharedApplication(mtm).unhide(None);
        }
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.ctx.request_repaint();
    }

    /// Mouse released: run the segment under the pointer, if any, and close the wheel.
    fn release_wheel(&mut self) {
        let Some(View::Wheel { center, segs, sel, .. }) = self.stack.last().cloned() else { return };
        let picked = mouse_top_left().and_then(|(m, _)| wheel_hit(m - center, segs.len())).or(sel);
        self.apply_wheel(picked);
    }

    /// Switch an open wheel to another kind, keeping its position.
    pub(crate) fn switch_wheel(&mut self, kind: ferrocast_core::wheel::WheelKind) {
        let Some(View::Wheel { files, .. }) = self.stack.last().cloned() else { return };
        let segs = self.wheel_segments(kind, &files);
        if segs.is_empty() {
            return;
        }
        if let Some(View::Wheel { segs: s, kind: k, sel, .. }) = self.stack.last_mut() {
            *s = segs;
            *k = kind;
            *sel = None;
        }
        self.ctx.request_repaint();
    }

    /// Close the wheel and run segment `picked` (None cancels).
    pub(crate) fn apply_wheel(&mut self, picked: Option<usize>) {
        let Some(View::Wheel { files, segs, .. }) = self.stack.last().cloned() else { return };
        self.hide();
        self.reset_to_root();
        let Some(i) = picked.filter(|i| *i < segs.len()) else { return };
        match segs[i].pick.clone() {
            WheelPick::Image(op) => {
                self.toast(format!("{}…", op.label()));
                services::image_op_files(files, op, self.bus.clone());
            }
            WheelPick::Convert(t) => {
                self.toast(format!("Converting to {}…", t.label));
                services::convert_files(files, t, self.tools, self.bus.clone());
            }
            WheelPick::Act(a) => self.perform(a, false),
        }
    }

    fn reset_to_root(&mut self) {
        self.sec_dirty = true;
        self.nav_at = Instant::now();
        self.stack = vec![View::Root];
        self.query.clear();
        self.selected = 0;
        self.scroll_to_sel = true;
        self.panel = None;
        self.cool_claude();
    }

    // ------------------------------------------------------------- messages

    fn drain(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            self.sec_dirty = true;
            match m {
                Msg::Wake => {}
                Msg::Toast(t) => {
                    // A sign-in or sign-out may just have finished; let Settings re-read the Keychain.
                    self.ctx.data_mut(|d| d.remove::<bool>(Id::new("chatgpt_signed")));
                    self.toast(t)
                }
                Msg::Apps(a) => {
                    self.apps = a;
                    self.rebuild();
                }
                Msg::Files(q, f) => self.files = (q, f),
                Msg::Moved(pairs) => {
                    // keep the file results pointing at the new places
                    for (old, new) in &pairs {
                        for f in self.files.1.iter_mut() {
                            if f == old {
                                *f = new.clone();
                            }
                        }
                    }
                    self.toast(format!("Moved {} item{}", pairs.len(), if pairs.len() == 1 { "" } else { "s" }));
                }
                Msg::WheelOpen(kind, files) => self.show_wheel(kind, files),
                Msg::WheelRelease => self.release_wheel(),
                Msg::WheelSwitch(k) => self.switch_wheel(k),
                Msg::ShelfChanged(v) => {
                    self.shelf = v;
                    let v = self.shelf.clone();
                    self.persist("shelf.json", &v);
                }
                Msg::ConvertFiles(files) => {
                    self.show();
                    self.reset_to_root();
                    self.push(View::Convert(files));
                }
                Msg::AiModels(k, r) => {
                    self.ai_models.insert(k, Some(r));
                }
                Msg::ChatGptDeviceCode(code) => {
                    self.show();
                    self.push(View::Output {
                        run: 0,
                        title: "Sign in with ChatGPT".into(),
                        text: format!(
                            "Your code:\n\n    {code}\n\nIt is already on your clipboard. A browser tab with {} should be open; paste the code there and approve. This screen can be closed; Ferrocast keeps waiting in the background and will tell you when it is done.",
                            ferrocast_core::chatgpt::DEVICE_VERIFICATION_URL
                        ),
                        done: true,
                    });
                }
                Msg::IconBytes(k, b) => {
                    if !self.icons.map.contains_key(&k) {
                        self.icons.prefetched.insert(k, b);
                    }
                }
                Msg::AiChunk { run, text } => {
                    for v in self.stack.iter_mut() {
                        if let View::Ai { answer, run: r, .. } = v {
                            if *r == run {
                                answer.push_str(&text);
                            }
                        }
                    }
                }
                Msg::AiDone { run, error } => {
                    for v in self.stack.iter_mut() {
                        if let View::Ai { answer, busy, run: r, .. } = v {
                            if *r == run {
                                *busy = false;
                                if let Some(e) = &error {
                                    if !answer.is_empty() {
                                        answer.push('\n');
                                    }
                                    answer.push_str(&format!("⚠ {e}"));
                                }
                            }
                        }
                    }
                }
                Msg::ImportFile(path) => {
                    let outcome = std::fs::read_to_string(&path)
                        .map_err(|e| format!("Could not read the file: {e}"))
                        .and_then(|t| ferrocast_core::import::parse_snippets(&t));
                    let text = match outcome {
                        Ok(incoming) => {
                            let (added, conflicts) = ferrocast_core::import::merge_snippets(&mut self.snippets, incoming);
                            self.save_snippets();
                            format!(
                                "Imported {added} snippet(s){}",
                                if conflicts > 0 { format!(", {conflicts} keyword(s) already in use were cleared") } else { String::new() }
                            )
                        }
                        Err(e) => {
                            instance::log(&self.store.root, &format!("snippet import failed: {e}"));
                            e
                        }
                    };
                    if self.visible {
                        self.toast(text)
                    } else {
                        services::notify("Ferrocast", &text)
                    }
                }
                Msg::Rates(r) => {
                    let _ = self.store.save("rates.json", &r);
                    self.rates = r;
                }
                Msg::Tray(id) => match id.as_str() {
                    "open" => self.show(),
                    "clipboard" => {
                        self.show();
                        self.reset_to_root();
                        self.push(View::Clipboard);
                    }
                    "settings" => {
                        self.show();
                        self.reset_to_root();
                        self.push(View::Settings);
                    }
                    "quit" => std::process::exit(0),
                    _ => {}
                },
                Msg::Hotkey(id) => {
                    let target = self.hotkeys.as_ref().and_then(|h| h.map.get(&id).cloned());
                    match target.as_deref() {
                        Some("__launcher") => self.toggle(),
                        Some(cmd) => {
                            if let Some(item) = self.base.iter().find(|i| i.id == cmd).cloned() {
                                self.prev_pid = running::frontmost().map(|f| f.pid).filter(|p| *p != std::process::id() as i32);
                                let needs_ui = matches!(item.kind, Kind::Builtin(_) | Kind::Quicklink { .. } | Kind::Script(_));
                                if needs_ui && !self.visible {
                                    self.show();
                                }
                                self.run_item(&item);
                            }
                        }
                        None => {}
                    }
                }
                Msg::Script { run, text, code } => match self.runs.remove(&run) {
                    Some(RunKind::Output) => {
                        for v in self.stack.iter_mut() {
                            if let View::Output { run: r, text: t, done, .. } = v {
                                if *r == run {
                                    *t = if code == 0 { text.clone() } else { format!("{text}\n[exit code {code}]") };
                                    *done = true;
                                }
                            }
                        }
                    }
                    Some(RunKind::Compact(title)) => {
                        let line = script::last_line(&text);
                        let line = if line.is_empty() { format!("{title} finished") } else { line };
                        if self.visible {
                            self.toast(line);
                        } else {
                            services::notify(&title, &line);
                        }
                    }
                    Some(RunKind::Silent(title)) => {
                        let line = script::last_line(&text);
                        services::notify(&title, if line.is_empty() { "Done" } else { &line });
                    }
                    Some(RunKind::Inline(p)) => {
                        self.inline.insert(p, (script::first_line(&text), Instant::now()));
                        self.rebuild();
                    }
                    None => {}
                },
            }
        }
    }

    fn tick_inline(&mut self) {
        let mut due = vec![];
        for (i, s) in self.scripts.iter().enumerate() {
            // Confirmation-marked scripts may never auto-run in the background.
            if s.mode == Mode::Inline && !s.needs_confirmation {
                if let Some(secs) = s.refresh_secs() {
                    let fresh = self.inline.get(&s.path).is_some_and(|(_, t)| t.elapsed().as_secs() < secs);
                    if !fresh && !self.runs.values().any(|r| matches!(r, RunKind::Inline(p) if *p == s.path)) {
                        due.push(i);
                    }
                }
            }
        }
        for i in due {
            let s = self.scripts[i].clone();
            let id = self.next_run;
            self.next_run += 1;
            self.runs.insert(id, RunKind::Inline(s.path.clone()));
            services::run_script(&s, vec![], id, self.bus.clone());
        }
        if self.scripts.iter().any(|s| s.mode == Mode::Inline) {
            self.ctx.request_repaint_after(Duration::from_secs(10));
        }
    }

    // ----------------------------------------------------------- list data

    fn rankable<'a>(&'a self, i: &'a Item) -> Rankable<'a> {
        Rankable { id: &i.id, title: &i.title, subtitle: &i.subtitle, keywords: &i.keywords, alias: self.settings.alias(&i.id) }
    }

    fn root_sections(&self) -> Vec<Section> {
        let q = self.query.trim();
        let now = now_secs();
        let mut out = vec![];
        if q.is_empty() {
            let favs: Vec<Item> = self.settings.favorites.iter().filter_map(|f| self.base.iter().find(|i| &i.id == f).cloned()).collect();
            if !favs.is_empty() {
                out.push(Section { title: "Favorites".into(), items: favs });
            }
            let mut recent: Vec<&Item> =
                self.base.iter().filter(|i| !self.settings.is_favorite(&i.id) && self.frecency.last_used(&i.id).is_some()).collect();
            recent.sort_by(|a, b| self.frecency.score(&b.id, now).total_cmp(&self.frecency.score(&a.id, now)));
            recent.truncate(8);
            if !recent.is_empty() {
                out.push(Section { title: "Suggestions".into(), items: recent.into_iter().cloned().collect() });
            }
            let cmds: Vec<Item> = self
                .base
                .iter()
                .filter(|i| matches!(i.kind, Kind::Builtin(_)) && !self.settings.is_favorite(&i.id))
                .take(10)
                .cloned()
                .collect();
            out.push(Section { title: "Commands".into(), items: cmds });
            let apps: Vec<Item> = self
                .base
                .iter()
                .filter(|i| matches!(i.kind, Kind::App { .. }) && !self.settings.is_favorite(&i.id))
                .take(30)
                .cloned()
                .collect();
            if !apps.is_empty() {
                out.push(Section { title: "Applications".into(), items: apps });
            }
            return out;
        }
        // calculator / color / url
        let mut top = vec![];
        if let Some((question, answer)) = ferrocast_core::timezones::evaluate(q, chrono::Utc::now()) {
            let r = calc::CalcResult { question, value: 0.0, unit: None, display: answer.clone(), raw: answer };
            top.push(Item {
                id: "time".into(),
                title: r.display.clone(),
                subtitle: r.question.clone(),
                icon: Icon::Glyph("sf:timer", [255, 99, 99]),
                accessory: "Time".into(),
                keywords: vec![],
                kind: Kind::Calc(r),
            });
        }
        if let Some(r) = calc::evaluate_with_rates(q, self.settings.rem_px, Some(&self.rates)) {
            top.push(Item {
                id: "calc".into(),
                title: r.display.clone(),
                subtitle: r.question.clone(),
                icon: Icon::Glyph("sf:equal", [255, 99, 99]),
                accessory: "Calculator".into(),
                keywords: vec![],
                kind: Kind::Calc(r),
            });
        }
        if let Some(c) = Rgba::parse(q) {
            top.push(Item {
                id: "color".into(),
                title: c.formats()[0].1.clone(),
                subtitle: q.into(),
                icon: Icon::Color(c),
                accessory: "Color".into(),
                keywords: vec![],
                kind: Kind::Color(c),
            });
        }
        if let Some(u) = looks_like_url(q) {
            top.push(Item {
                id: "url".into(),
                title: format!("Open {u}"),
                subtitle: String::new(),
                icon: Icon::Glyph("sf:globe", [86, 194, 255]),
                accessory: "URL".into(),
                keywords: vec![],
                kind: Kind::Url(u),
            });
        }
        if let Some(t) = ferrocast_core::timer::parse(q) {
            let what = if t.label.is_empty() {
                ferrocast_core::timer::describe(t.secs)
            } else {
                format!("{} ({})", t.label, ferrocast_core::timer::describe(t.secs))
            };
            top.push(Item {
                id: "timer".into(),
                title: format!("Start {what} timer"),
                subtitle: String::new(),
                icon: Icon::Glyph("sf:timer", [245, 165, 36]),
                accessory: "Timer".into(),
                keywords: vec![],
                kind: Kind::Timer(t.secs, t.label),
            });
        }
        if !top.is_empty() {
            out.push(Section { title: "Result".into(), items: top });
        }
        // alias + argument: "g rust"
        if let Some((first, rest)) = q.split_once(' ') {
            if let Some(it) = self.base.iter().find(|i| {
                self.settings.alias(&i.id).is_some_and(|a| a.eq_ignore_ascii_case(first)) && matches!(i.kind, Kind::Quicklink { .. })
            }) {
                let mut it = it.clone();
                if let Kind::Quicklink { id, .. } = &it.kind {
                    it.kind = Kind::Quicklink { id: id.clone(), arg: Some(rest.trim().to_string()) };
                }
                out.push(Section { title: "Quicklink".into(), items: vec![it] });
            }
        }
        let mut scored: Vec<(f64, &Item)> = self
            .base
            .iter()
            .filter_map(|i| ranking::score(&self.rankable(i), q, &self.frecency, now, self.settings.sensitivity).map(|s| (s, i)))
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.truncate(40);
        if !scored.is_empty() {
            out.push(Section { title: "Results".into(), items: scored.into_iter().map(|(_, i)| i.clone()).collect() });
        }
        if self.settings.file_search_in_root && self.files.0 == q && !self.files.1.is_empty() {
            out.push(Section { title: "Files".into(), items: self.files.1.iter().take(8).map(|p| file_item(p)).collect() });
        }
        let mut fb = vec![Item {
            id: "fallback:web".into(),
            title: format!("Search the web for “{q}”"),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:magnifyingglass", [120, 120, 130]),
            accessory: "Fallback".into(),
            keywords: vec![],
            kind: Kind::WebSearch(q.to_string()),
        }];
        fb.push(Item {
            id: "fallback:files".into(),
            title: format!("Search files for “{q}”"),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:magnifyingglass", [120, 120, 130]),
            accessory: "Fallback".into(),
            keywords: vec![],
            kind: Kind::Prefilled(Builtin::SearchFiles, q.to_string()),
        });
        fb.push(Item {
            id: "fallback:emoji".into(),
            title: format!("Search emoji for “{q}”"),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:face.smiling", [255, 197, 49]),
            accessory: "Fallback".into(),
            keywords: vec![],
            kind: Kind::Prefilled(Builtin::SearchEmoji, q.to_string()),
        });
        if q.split_whitespace().count() <= 3 && q.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '-' || c == '\'') {
            fb.push(Item {
                id: "fallback:dict".into(),
                title: format!("Look up “{q}” in Dictionary"),
                subtitle: String::new(),
                icon: Icon::Glyph("sf:character.book.closed", [120, 120, 130]),
                accessory: "Fallback".into(),
                keywords: vec![],
                kind: Kind::Url(format!("dict://{}", q.replace(' ', "%20"))),
            });
        }
        fb.push(Item {
            id: "fallback:ai".into(),
            title: format!("Ask AI: “{q}”"),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:sparkles", [168, 110, 255]),
            accessory: "Fallback".into(),
            keywords: vec![],
            kind: Kind::Prefilled(Builtin::AskAi, q.to_string()),
        });
        out.push(Section { title: "Use with…".into(), items: fb });
        out
    }

    fn matches(&self, q: &str, title: &str, extra: &str) -> Option<i64> {
        if q.is_empty() {
            return Some(0);
        }
        ferrocast_core::fuzzy::fuzzy_match(q, title, self.settings.sensitivity)
            .map(|m| m.score + 100)
            .or_else(|| ferrocast_core::fuzzy::fuzzy_match(q, extra, self.settings.sensitivity).map(|m| m.score))
    }

    fn sorted(&self, v: Vec<(i64, Item)>, title: &str) -> Vec<Section> {
        let mut v = v;
        if !self.query.trim().is_empty() {
            v.sort_by_key(|a| std::cmp::Reverse(a.0));
        }
        vec![Section { title: title.into(), items: v.into_iter().map(|(_, i)| i).collect() }]
    }

    /// Anything that can change the result list calls this; the next frame recomputes.
    pub(crate) fn invalidate_sections(&mut self) {
        self.sec_dirty = true;
    }

    /// `sections()` with a one-entry cache keyed on query and view. Building the list runs the
    /// fuzzy matcher, the calculator and clipboard filtering, so it must not repeat every frame.
    pub(crate) fn sections_cached(&mut self) -> Vec<Section> {
        let key = format!("{}\u{0}{}\u{0}{}", self.query, self.stack.len(), self.view().title());
        if !self.sec_dirty {
            if let Some((k, v)) = &self.sec_cache {
                if *k == key {
                    return v.clone();
                }
            }
        }
        let t0 = Instant::now();
        let v = self.sections();
        if std::env::var_os("FERROCAST_PROFILE").is_some() {
            eprintln!("sections {:?} q={:?}", t0.elapsed(), self.query);
        }
        if !self.wide_font && v.iter().flat_map(|s| s.items.iter()).any(|i| needs_wide_font(&i.title) || needs_wide_font(&i.subtitle)) {
            self.wide_font = true;
            load_fonts(&self.ctx, true);
        }
        // Fetch the icons this list will show before the painter asks for them.
        let want: Vec<String> = v
            .iter()
            .flat_map(|s| s.items.iter())
            .filter_map(|i| match &i.icon {
                Icon::App(p) => {
                    let k = format!("app:{p}");
                    (!self.icons.map.contains_key(&k) && !self.icons.prefetched.contains_key(&k) && self.icons.inflight.insert(k))
                        .then(|| p.clone())
                }
                _ => None,
            })
            .take(40)
            .collect();
        if !want.is_empty() {
            services::prefetch_app_icons(want, self.bus.clone());
        }
        self.sec_cache = Some((key, v.clone()));
        self.sec_dirty = false;
        v
    }

    fn sections(&self) -> Vec<Section> {
        let q = self.query.trim().to_string();
        match self.view() {
            View::Root => self.root_sections(),
            View::Clipboard => {
                let list = self.shared.clipboard.lock().unwrap().clone();
                let now = now_secs();
                let (mut pinned, mut today, mut yest, mut older) = (vec![], vec![], vec![], vec![]);
                for e in list {
                    let hay = format!("{} {}", e.display_title(), e.content);
                    if !q.is_empty() && !hay.to_lowercase().contains(&q.to_lowercase()) {
                        continue;
                    }
                    let icon = match e.kind {
                        ClipKind::Image => Icon::ClipImage(e.content.clone()),
                        ClipKind::Color => {
                            Rgba::parse(&e.content).map(Icon::Color).unwrap_or(Icon::Glyph("sf:doc.on.clipboard", [255, 99, 99]))
                        }
                        ClipKind::Link => Icon::Glyph("sf:link", [86, 194, 255]),
                        ClipKind::File => Icon::Glyph("sf:doc", [120, 120, 130]),
                        ClipKind::Text => Icon::Glyph("sf:text.alignleft", [90, 90, 105]),
                    };
                    let age = now - e.copied_at;
                    let pinned_flag = e.pinned;
                    let it = Item {
                        id: format!("clip:{}", e.id),
                        title: e.display_title(),
                        subtitle: String::new(),
                        icon,
                        accessory: e.source_app.clone().unwrap_or_default(),
                        keywords: vec![],
                        kind: Kind::Clip(e.id.clone()),
                    };
                    if pinned_flag {
                        pinned.push(it)
                    } else if age < 86400 {
                        today.push(it)
                    } else if age < 2 * 86400 {
                        yest.push(it)
                    } else {
                        older.push(it)
                    }
                }
                [("Pinned", pinned), ("Today", today), ("Yesterday", yest), ("Earlier", older)]
                    .into_iter()
                    .filter(|(_, v)| !v.is_empty())
                    .map(|(t, items)| Section { title: t.into(), items })
                    .collect()
            }
            View::Snippets => {
                let v = self
                    .snippets
                    .iter()
                    .filter_map(|s| {
                        self.matches(&q, &s.name, &s.text).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("snippet:{}", s.id),
                                    title: s.name.clone(),
                                    subtitle: s.text.lines().next().unwrap_or("").chars().take(60).collect(),
                                    icon: Icon::Glyph("sf:scissors", [255, 167, 38]),
                                    accessory: s.keyword.clone(),
                                    keywords: vec![],
                                    kind: Kind::Snippet(s.id.clone()),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Snippets")
            }
            View::Quicklinks => {
                let v = self
                    .quicklinks
                    .iter()
                    .filter_map(|l| {
                        self.matches(&q, &l.name, &l.link).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("quicklink:{}", l.id),
                                    title: l.name.clone(),
                                    subtitle: l.link.chars().take(70).collect(),
                                    icon: Icon::Glyph("sf:link", [86, 194, 255]),
                                    accessory: String::new(),
                                    keywords: vec![],
                                    kind: Kind::Quicklink { id: l.id.clone(), arg: None },
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Quicklinks")
            }
            View::Emoji => {
                let ql = q.to_lowercase();
                // Rank before truncating so a name that starts with the query is never cut off
                // by weaker matches that happen to come first in the catalogue.
                let mut hits: Vec<_> =
                    emojis::iter().filter(|e| ql.is_empty() || e.name().contains(&ql) || e.shortcodes().any(|s| s.contains(&ql))).collect();
                hits.sort_by_key(|e| !e.name().starts_with(&ql));
                hits.truncate(300);
                let recent: Vec<Item> = if ql.is_empty() {
                    self.recent_emoji
                        .iter()
                        .filter_map(|r| emojis::get(r))
                        .map(|e| Item {
                            id: format!("emoji:{}", e.as_str()),
                            title: e.name().to_string(),
                            subtitle: e.shortcode().map(|s| format!(":{s}:")).unwrap_or_default(),
                            icon: Icon::Emoji(e.as_str().to_string()),
                            accessory: "Recent".into(),
                            keywords: vec![],
                            kind: Kind::Emoji(e.as_str()),
                        })
                        .collect()
                } else {
                    vec![]
                };
                let v: Vec<(i64, Item)> = hits
                    .into_iter()
                    .map(|e| {
                        (
                            if e.name().starts_with(&ql) { 1 } else { 0 },
                            Item {
                                id: format!("emoji:{}", e.as_str()),
                                title: e.name().to_string(),
                                subtitle: e.shortcode().map(|s| format!(":{s}:")).unwrap_or_default(),
                                icon: Icon::Emoji(e.as_str().to_string()),
                                accessory: humanize_group(&format!("{:?}", e.group())),
                                keywords: vec![],
                                kind: Kind::Emoji(e.as_str()),
                            },
                        )
                    })
                    .collect();
                let mut out = self.sorted(v, "Emoji");
                if !recent.is_empty() {
                    out.insert(0, Section { title: "Recently used".into(), items: recent });
                }
                out
            }
            View::Files => {
                if self.files.0 == q && !self.files.1.is_empty() {
                    vec![Section { title: "Files".into(), items: self.files.1.iter().map(|p| file_item(p)).collect() }]
                } else {
                    vec![]
                }
            }
            View::Windows => {
                let v = self
                    .win_cache
                    .iter()
                    .filter_map(|w| {
                        let t = if w.title.is_empty() { w.app.clone() } else { w.title.clone() };
                        self.matches(&q, &t, &w.app).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("win:{}", w.id),
                                    title: t,
                                    subtitle: w.app.clone(),
                                    icon: Icon::Glyph("sf:macwindow.on.rectangle", [89, 212, 153]),
                                    accessory: String::new(),
                                    keywords: vec![],
                                    kind: Kind::OpenWindow(w.clone()),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Windows")
            }
            View::Processes => {
                let v = self
                    .proc_cache
                    .iter()
                    .filter_map(|pr| {
                        self.matches(&q, &pr.name, &pr.path).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("proc:{}", pr.pid),
                                    title: pr.name.clone(),
                                    subtitle: format!("PID {}", pr.pid),
                                    icon: pr.bundle().map(Icon::App).unwrap_or(Icon::Glyph("sf:chevron.right", [108, 108, 120])),
                                    accessory: format!("{:.0}% CPU · {:.0} MB", pr.cpu, pr.mem_mb),
                                    keywords: vec![],
                                    kind: Kind::Process(pr.clone()),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Processes")
            }
            View::Convert(files) => {
                let files = files.clone();
                let Some(first) = files.first() else { return vec![] };
                let names: Vec<String> = files.iter().filter_map(|f| f.file_name().map(|n| n.to_string_lossy().to_string())).collect();
                let sub = if files.len() == 1 { names[0].clone() } else { format!("{} files · {}", files.len(), names.join(", ")) };
                let v: Vec<(i64, Item)> = ferrocast_core::convert::targets(first, &self.tools)
                    .into_iter()
                    .filter_map(|t| {
                        self.matches(&q, t.label, t.ext).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("convert:{}", t.ext),
                                    title: t.label.to_string(),
                                    subtitle: sub.clone(),
                                    icon: Icon::Glyph(
                                        match ferrocast_core::convert::family_of(first) {
                                            ferrocast_core::convert::Family::Image | ferrocast_core::convert::Family::Pdf => "sf:photo",
                                            ferrocast_core::convert::Family::Audio => "sf:waveform",
                                            ferrocast_core::convert::Family::Video => "sf:film",
                                            ferrocast_core::convert::Family::Archive | ferrocast_core::convert::Family::Folder => {
                                                "sf:archivebox"
                                            }
                                            _ => "sf:doc.text",
                                        },
                                        [255, 140, 60],
                                    ),
                                    accessory: match t.needs {
                                        Some(n) => format!("Needs {}", n.split(' ').next().unwrap_or(n)),
                                        None => format!(".{} · {}", t.ext, t.engine.label()),
                                    },
                                    keywords: vec![],
                                    kind: Kind::ConvertTo(files.clone(), t),
                                },
                            )
                        })
                    })
                    .collect();
                let mut out = self.sorted(v, "Convert to");
                // image tools when every file is an image
                use ferrocast_core::convert::{family_of, Family, ImageOp};
                if files.iter().all(|f| family_of(f) == Family::Image) {
                    // a typed size or dimensions ("500kb", "40%", "800x600") becomes a tool of its own
                    let typed = ferrocast_core::convert::parse_image_query(&q);
                    let tools: Vec<Item> = typed
                        .into_iter()
                        .chain(ImageOp::offered(files.len()))
                        .filter(|op| {
                            self.matches(
                                &q,
                                &op.label(),
                                match op {
                                    ImageOp::Compress(_) => "compress smaller jpeg",
                                    ImageOp::Resize(_) => "resize scale",
                                    ImageOp::Crop(..) => "crop aspect",
                                    ImageOp::StripMetadata => "metadata exif gps strip",
                                    ImageOp::CombinePdf => "pdf combine merge",
                                    ImageOp::Rotate(_) => "rotate turn",
                                    ImageOp::Flip(_) => "flip mirror",
                                    ImageOp::Background(_) => "background pad square",
                                    ImageOp::Collage | ImageOp::CollageWith(_) => "collage grid row column featured",
                                    ImageOp::FrameWith(_) => "frame background blur shadow padding",
                                    ImageOp::RedactWith(_) => "redact hide blur privacy area",
                                    ImageOp::EditWith(_) => {
                                        "edit photo brightness contrast saturation warmth clarity sharpen grain denoise dehaze"
                                    }
                                    ImageOp::Redact(_) => "redact hide blur privacy face text",
                                    ImageOp::Adjust(_) => "edit adjust photo brightness contrast black white grayscale",
                                    ImageOp::Annotate => "annotate markup draw arrow text preview",
                                    ImageOp::CompressToKb(_) | ImageOp::CompressByPercent(_) => "compress size smaller",
                                    ImageOp::CropExact(..) => "crop exact dimensions pixels",
                                    ImageOp::SetMeta(_) => "metadata exif camera make model gps location tag",
                                },
                            )
                            .is_some()
                        })
                        .map(|op| Item {
                            id: format!("imgtool:{}", op.suffix()),
                            title: op.label(),
                            subtitle: sub.clone(),
                            icon: Icon::Glyph(
                                match op {
                                    ImageOp::Compress(_) => "sf:arrow.down.right.and.arrow.up.left",
                                    ImageOp::Resize(_) => "sf:arrow.up.left.and.down.right.magnifyingglass",
                                    ImageOp::Crop(..) => "sf:crop",
                                    ImageOp::StripMetadata => "sf:eye.slash",
                                    ImageOp::CombinePdf => "sf:doc.richtext",
                                    ImageOp::Rotate(_) => "sf:rotate.right",
                                    ImageOp::Flip(_) => "sf:arrow.left.and.right.righttriangle.left.righttriangle.right",
                                    ImageOp::Background(_) => "sf:square.dashed",
                                    ImageOp::Collage | ImageOp::CollageWith(_) => "sf:square.grid.2x2",
                                    ImageOp::FrameWith(_) => "sf:square.dashed",
                                    ImageOp::RedactWith(_) => "sf:eye.slash.fill",
                                    ImageOp::EditWith(_) => "sf:slider.horizontal.3",
                                    ImageOp::Redact(_) => "sf:eye.slash.fill",
                                    ImageOp::Adjust(_) => "sf:slider.horizontal.3",
                                    ImageOp::Annotate => "sf:pencil.tip.crop.circle",
                                    ImageOp::CompressToKb(_) | ImageOp::CompressByPercent(_) => "sf:arrow.down.right.and.arrow.up.left",
                                    ImageOp::CropExact(..) => "sf:crop",
                                    ImageOp::SetMeta(_) => "sf:tag",
                                },
                                [255, 140, 60],
                            ),
                            accessory: "Image tool".into(),
                            keywords: vec![],
                            kind: Kind::ImageTool(files.clone(), op),
                        })
                        .collect();
                    if !tools.is_empty() {
                        out.push(Section { title: "Image tools".into(), items: tools });
                    }
                }
                // trim, split and merge for audio, video and PDF
                let fam = family_of(first);
                if files.iter().all(|f| family_of(f) == fam) && matches!(fam, Family::Audio | Family::Video | Family::Pdf) {
                    use ferrocast_core::convert::FileTool;
                    let typed = ferrocast_core::convert::parse_tool_query(&q);
                    let items: Vec<Item> = typed
                        .iter()
                        .cloned()
                        .chain(FileTool::offered(fam, files.len()))
                        .filter(|t| typed.as_ref() == Some(t) || self.matches(&q, &t.label(), "trim split merge cut join").is_some())
                        .map(|t| Item {
                            id: format!("filetool:{}", t.label()),
                            title: t.label(),
                            subtitle: sub.clone(),
                            icon: Icon::Glyph(
                                match t {
                                    FileTool::Trim(..) => "sf:scissors",
                                    FileTool::SplitEvery(_) | FileTool::SplitPages => "sf:square.split.2x1",
                                    FileTool::Merge | FileTool::Join => "sf:rectangle.stack",
                                    FileTool::Mute => "sf:speaker.slash",
                                    FileTool::Speed(_) => "sf:speedometer",
                                    FileTool::Snapshot(_) => "sf:camera",
                                    FileTool::CropVideo(..) => "sf:crop",
                                    FileTool::RedactVideo(..) | FileTool::Bleep(..) => "sf:eye.slash",
                                    FileTool::Normalize(..) | FileTool::Channels(..) => "sf:speaker.wave.2",
                                    FileTool::Visualize(_) | FileTool::VisualizeImage(..) => "sf:waveform",
                                    FileTool::TrimSilence => "sf:scissors",
                                    FileTool::OrganizePdf(_) => "sf:doc.on.doc",
                                },
                                [255, 140, 60],
                            ),
                            accessory: "File tool".into(),
                            keywords: vec![],
                            kind: Kind::FileTool(files.clone(), t),
                        })
                        .collect();
                    if !items.is_empty() {
                        out.push(Section { title: "File tools".into(), items });
                    }
                }
                out.retain(|sec| !sec.items.is_empty());
                out
            }
            View::Shelf => {
                let v: Vec<(i64, Item)> = self
                    .shelf
                    .iter()
                    .filter(|p| p.exists())
                    .filter_map(|p| {
                        let it = file_item(p);
                        self.matches(&q, &it.title, &it.subtitle).map(|sc| (sc, it))
                    })
                    .collect();
                self.sorted(v, "On the shelf")
            }
            View::CalcHistory => {
                let now = now_secs();
                let v: Vec<(i64, Item)> = self
                    .calc_history
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| {
                        self.matches(&q, &e.question, &e.display).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("calc:{i}"),
                                    title: e.display.clone(),
                                    subtitle: e.question.clone(),
                                    icon: Icon::Glyph("sf:equal", [120, 120, 130]),
                                    accessory: services::ago(now - e.at),
                                    keywords: vec![],
                                    kind: Kind::Calc(ferrocast_core::calc::CalcResult {
                                        question: e.question.clone(),
                                        value: e.value,
                                        unit: e.unit.clone(),
                                        display: e.display.clone(),
                                        raw: e.raw.clone(),
                                    }),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "History")
            }
            View::Timers => {
                let now = Instant::now();
                let v: Vec<(i64, Item)> = self
                    .timers
                    .iter()
                    .enumerate()
                    .filter_map(|(i, t)| {
                        let title =
                            if t.label.is_empty() { format!("{} timer", ferrocast_core::timer::describe(t.secs)) } else { t.label.clone() };
                        self.matches(&q, &title, "").map(|sc| {
                            let left = t.ends_at.saturating_duration_since(now).as_secs();
                            (
                                sc,
                                Item {
                                    id: format!("timer:{i}"),
                                    title,
                                    subtitle: format!(
                                        "ends at {}",
                                        chrono::Local::now()
                                            .checked_add_signed(chrono::Duration::seconds(left as i64))
                                            .map(|d| d.format("%H:%M").to_string())
                                            .unwrap_or_default()
                                    ),
                                    icon: Icon::Glyph("sf:timer", [245, 165, 36]),
                                    accessory: ferrocast_core::timer::clock(left),
                                    keywords: vec![],
                                    kind: Kind::RunningTimer(i),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Running")
            }
            View::Notes => {
                let mut notes: Vec<&Note> =
                    self.notes.iter().filter(|n| q.is_empty() || n.text.to_lowercase().contains(&q.to_lowercase())).collect();
                notes.sort_by_key(|n| std::cmp::Reverse(n.modified));
                vec![Section {
                    title: "Notes".into(),
                    items: notes
                        .into_iter()
                        .map(|n| Item {
                            id: format!("note:{}", n.id),
                            title: n.title(),
                            subtitle: String::new(),
                            icon: Icon::Glyph("sf:pencil", [255, 197, 49]),
                            accessory: String::new(),
                            keywords: vec![],
                            kind: Kind::Note(n.id.clone()),
                        })
                        .collect(),
                }]
            }
            View::Colors(c) => vec![Section {
                title: "Formats".into(),
                items: c
                    .formats()
                    .into_iter()
                    .map(|(label, val)| Item {
                        id: format!("cf:{label}"),
                        title: val.clone(),
                        subtitle: label.into(),
                        icon: Icon::Color(*c),
                        accessory: String::new(),
                        keywords: vec![],
                        kind: Kind::ColorFormat(val),
                    })
                    .collect(),
            }],
            _ => vec![],
        }
    }

    // --------------------------------------------------------------- actions

    fn actions_for(&self, item: &Item) -> Vec<Action> {
        let mut a = vec![];
        match &item.kind {
            Kind::App { path, .. } => {
                a.push(Action::new("Open Application", Act::Run));
                a.push(Action::new("Show in Finder", Act::Reveal(PathBuf::from(path))).key(CMD, Key::Enter));
                a.push(Action::new("Copy Path", Act::CopyText(path.clone())).key(CMD_SHIFT, Key::C));
                if let Some(r) = running::running_apps()
                    .into_iter()
                    .find(|r| r.path.as_deref().is_some_and(|p| p.trim_end_matches('/') == path.trim_end_matches('/')))
                {
                    a.push(Action::new("Quit Application", Act::QuitApp(r.pid)).key(CMD_SHIFT, Key::Q));
                    a.push(Action::new("Hide Application", Act::HideApp(r.pid)).key(CMD_SHIFT, Key::H));
                    a.push(Action::new("Force Quit Application", Act::ForceQuitApp(r.pid)).danger());
                }
            }
            Kind::Quicklink { id, .. } => {
                a.push(Action::new("Open Quicklink", Act::Run));
                if let Some(q) = self.quicklinks.iter().find(|q| &q.id == id) {
                    a.push(Action::new("Copy Link", Act::CopyText(q.link.clone())).key(CMD_SHIFT, Key::C));
                }
                a.push(Action::new("Edit Quicklink", Act::EditQuicklink(id.clone())).key(CMD, Key::E));
                a.push(Action::new("Duplicate Quicklink", Act::DuplicateQuicklink(id.clone())).key(CMD, Key::D));
                a.push(Action::new("Create Quicklink", Act::CreateQuicklink).key(CMD, Key::N));
                a.push(Action::new("Delete Quicklink", Act::DeleteQuicklink(id.clone())).key(CTRL, Key::X).danger());
            }
            Kind::Snippet(id) => {
                a.push(Action::new("Paste Snippet to Active App", Act::Run));
                if let Some(s) = self.snippets.iter().find(|s| &s.id == id) {
                    a.push(Action::new("Copy Snippet to Clipboard", Act::CopyText(s.text.clone())).key(CMD, Key::Enter));
                }
                a.push(Action::new("Edit Snippet", Act::EditSnippet(id.clone())).key(CMD, Key::E));
                a.push(Action::new("Duplicate Snippet", Act::DuplicateSnippet(id.clone())).key(CMD, Key::D));
                a.push(Action::new("Create Snippet", Act::CreateSnippet).key(CMD, Key::N));
                a.push(Action::new("Delete Snippet", Act::DeleteSnippet(id.clone())).key(CTRL, Key::X).danger());
            }
            Kind::Clip(id) => {
                a.push(Action::new("Paste to Active App", Act::ClipPaste(id.clone())));
                a.push(Action::new("Copy to Clipboard", Act::ClipCopy(id.clone())).key(CMD, Key::Enter));
                let pinned = self.shared.clipboard.lock().unwrap().iter().any(|e| &e.id == id && e.pinned);
                a.push(Action::new(if pinned { "Unpin Entry" } else { "Pin Entry" }, Act::ClipPin(id.clone())).key(CMD, Key::Period));
                a.push(Action::new("Save as Snippet", Act::ClipSaveSnippet(id.clone())).key(CMD_SHIFT, Key::S));
                a.push(Action::new("Delete Entry", Act::ClipDelete(id.clone())).key(CTRL, Key::X).danger());
                a.push(Action::new("Delete All Entries", Act::ClipDeleteAll).key(CTRL_SHIFT, Key::X).danger());
            }
            Kind::File(p) => {
                a.push(Action::new("Open", Act::Run));
                a.push(Action::new("Show in Finder", Act::Reveal(p.clone())).key(CMD, Key::Enter));
                a.push(Action::new("Quick Look", Act::QuickLook(p.clone())).key(CMD, Key::Y));
                a.push(Action::new("Convert…", Act::Convert(vec![p.clone()])).key(CMD, Key::T));
                a.push(Action::new("Copy File", Act::CopyFiles(vec![p.clone()])).key(CMD_SHIFT, Key::Period));
                a.push(Action::new("Copy Path", Act::CopyText(p.to_string_lossy().to_string())).key(CMD_SHIFT, Key::C));
                a.push(Action::new("Move to…", Act::MoveFiles(vec![p.clone()])).key(CMD_SHIFT, Key::M));
                a.push(
                    Action::new(
                        if std::path::Path::new("/Applications/iTerm.app").exists() { "Open in iTerm" } else { "Open in Terminal" },
                        Act::OpenInTerminal(p.clone()),
                    )
                    .key(CMD_SHIFT, Key::T),
                );
                a.push(Action::new("AirDrop", Act::AirDrop(vec![p.clone()])));
                if matches!(self.view(), View::Shelf) {
                    a.push(Action::new("Remove from Shelf", Act::RemoveFromShelf(p.clone())).key(CMD, Key::Backspace));
                    a.push(Action::new("Clear Shelf", Act::ClearShelf).danger());
                } else {
                    a.push(Action::new("Float on Shelf", Act::AddToShelf(vec![p.clone()])).key(CMD_SHIFT, Key::F));
                }
                a.push(Action::new("Move to Trash", Act::Trash(p.clone())).key(CTRL, Key::X).danger());
            }
            Kind::Calc(r) => {
                a.push(Action::new("Copy Answer", Act::CopyText(r.raw.clone())));
                if matches!(self.view(), View::CalcHistory) {
                    if let Some(i) = item.id.strip_prefix("calc:").and_then(|n| n.parse().ok()) {
                        a.push(Action::new("Delete Entry", Act::DeleteCalcEntry(i)).key(CTRL, Key::X).danger());
                    }
                    a.push(Action::new("Clear History", Act::ClearCalcHistory).danger());
                }
                a.push(Action::new("Paste Answer to Active App", Act::PasteText(r.raw.clone())).key(CMD, Key::Enter));
                a.push(
                    Action::new("Copy Question and Answer", Act::CopyText(format!("{} = {}", r.question, r.display)))
                        .key(Modifiers { alt: true, ..CMD_SHIFT }, Key::C),
                );
            }
            Kind::Note(id) => {
                a.push(Action::new("Open Note", Act::Run));
                a.push(Action::new("Create Note", Act::CreateNote).key(CMD, Key::N));
                a.push(Action::new("Delete Note", Act::NoteDelete(id.clone())).key(CTRL, Key::X).danger());
            }
            Kind::Emoji(e) => {
                a.push(Action::new("Paste Emoji to Active App", Act::Run));
                a.push(Action::new("Copy Emoji to Clipboard", Act::CopyText(e.to_string())).key(CMD, Key::Enter));
            }
            Kind::ColorFormat(v) => {
                a.push(Action::new("Copy Color", Act::CopyText(v.clone())));
                a.push(Action::new("Paste Color to Active App", Act::PasteText(v.clone())).key(CMD, Key::Enter));
            }
            Kind::Script(i) => {
                a.push(Action::new("Run Script Command", Act::Run));
                if let Some(s) = self.scripts.get(*i) {
                    a.push(Action::new("Edit Script", Act::EditScript(s.path.clone())).key(CMD, Key::E));
                    a.push(Action::new("Show in Finder", Act::Reveal(s.path.clone())).key(CMD_SHIFT, Key::F));
                }
            }
            Kind::Builtin(Builtin::ClipboardHistory) | Kind::Builtin(Builtin::SearchSnippets) => {
                a.push(Action::new("Open Command", Act::Run));
            }
            Kind::System(c) => {
                let mut act = Action::new("Run Command", Act::Run);
                act.destructive = c.is_destructive();
                a.push(act);
            }
            Kind::Window(_) => a.push(Action::new("Apply to Focused Window", Act::Run)),
            Kind::Url(_) => a.push(Action::new("Open in Browser", Act::Run)),
            Kind::OpenWindow(_) => a.push(Action::new("Switch to Window", Act::Run)),
            Kind::Timer(..) => a.push(Action::new("Start Timer", Act::Run)),
            Kind::ConvertTo(files, t) => {
                a.push(Action::new(
                    match t.needs {
                        Some(_) => "Copy install command",
                        None => "Convert",
                    },
                    Act::Run,
                ));
                a.push(Action::new("Reveal original in Finder", Act::Reveal(files[0].clone())).key(CMD, Key::Enter));
            }
            Kind::RunningTimer(i) => a.push(Action::new("Cancel Timer", Act::CancelTimer(*i)).danger()),
            Kind::Process(pr) => {
                a.push(Action::new("Kill Process", Act::Run));
                a.push(Action::new("Force Kill Process", Act::KillProcess(pr.pid, true)).key(CMD, Key::Enter).danger());
                a.push(Action::new("Copy PID", Act::CopyText(pr.pid.to_string())).key(CMD_SHIFT, Key::C));
                a.push(Action::new("Copy Path", Act::CopyText(pr.path.clone())).key(CMD, Key::P));
            }
            _ => a.push(Action::new("Open", Act::Run)),
        }
        if matches!(self.view(), View::Root) && item.is_command() {
            let fav = self.settings.is_favorite(&item.id);
            a.push(Action::new(if fav { "Remove from Favorites" } else { "Add to Favorites" }, Act::ToggleFavorite).key(CMD_SHIFT, Key::F));
            if fav {
                a.push(Action::new("Move Favorite Up", Act::MoveFavorite(-1)).key(CMD, Key::ArrowUp));
                a.push(Action::new("Move Favorite Down", Act::MoveFavorite(1)).key(CMD, Key::ArrowDown));
            }
            a.push(Action::new("Set Alias…", Act::SetAlias));
            a.push(Action::new("Set Hotkey…", Act::SetHotkey).key(CMD_SHIFT, Key::Comma));
            a.push(Action::new("Reset Ranking", Act::ResetRanking));
            a.push(Action::new("Disable Command", Act::DisableCommand).key(CMD_SHIFT, Key::D).danger());
        }
        a
    }

    fn primary_label(&self, item: &Item) -> String {
        self.actions_for_cheap(item)
    }

    fn actions_for_cheap(&self, item: &Item) -> String {
        match &item.kind {
            Kind::App { .. } => "Open Application",
            Kind::Quicklink { .. } => "Open Quicklink",
            Kind::Snippet(_) => "Paste Snippet",
            Kind::Clip(_) => "Paste to Active App",
            Kind::File(_) => "Open",
            Kind::Calc(_) => "Copy Answer",
            Kind::Color(_) => "Copy Color as…",
            Kind::ColorFormat(_) => "Copy Color",
            Kind::Emoji(_) => "Paste Emoji",
            Kind::Note(_) => "Open Note",
            Kind::Script(_) => "Run Script Command",
            Kind::Url(_) => "Open in Browser",
            Kind::OpenWindow(_) => "Switch to Window",
            Kind::Process(_) => "Kill Process",
            Kind::ConvertTo(..) => "Convert",
            Kind::ImageTool(..) | Kind::FileTool(..) => "Apply",
            Kind::Timer(..) => "Start Timer",
            Kind::RunningTimer(_) => "Cancel Timer",
            Kind::WebSearch(_) => "Search",
            Kind::Window(_) => "Apply to Window",
            _ => "Run Command",
        }
        .to_string()
    }

    // ------------------------------------------------------------ execution

    fn paste_text(&mut self, text: &str) {
        clipboard::write_text(text);
        self.shared.mark_clipboard_write();
        self.paste_after_hide();
    }

    fn paste_after_hide(&mut self) {
        self.hide();
        self.reset_to_root();
        let pid = self.prev_pid;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(160));
            if let Some(p) = pid {
                running::activate(p);
            }
            std::thread::sleep(Duration::from_millis(90));
            keyboard::paste();
        });
    }

    fn copy_text(&mut self, text: &str) {
        clipboard::write_text(text);
        self.shared.mark_clipboard_write();
        self.toast("Copied to Clipboard");
        self.hide();
        self.reset_to_root();
    }

    fn open_target(&mut self, target: &str, with: Option<&str>) {
        if let Err(e) = apps::open(target, with) {
            self.toast(format!("Could not open: {e}"));
        }
        self.hide();
        self.reset_to_root();
    }

    fn run_item(&mut self, item: &Item) {
        match &item.kind {
            Kind::App { path, .. } => self.open_target(&path.clone(), None),
            Kind::System(c) => {
                if c.is_destructive() {
                    self.push(View::Confirm { msg: format!("{}?", c.title()), act: Act::RunSystem(*c) });
                } else {
                    self.run_system(*c);
                }
            }
            Kind::Window(c) => {
                let (c, gap, pid) = (*c, self.settings.window_gap, self.prev_pid);
                self.hide();
                self.reset_to_root();
                match pid {
                    Some(pid) => {
                        if let Err(e) = windows::apply(c, pid, gap) {
                            services::notify("Window Management", &e.to_string());
                        }
                    }
                    None => services::notify("Window Management", "No target window"),
                }
            }
            Kind::Builtin(b) => self.run_builtin(*b),
            Kind::Prefilled(b, q) => {
                self.run_builtin(*b);
                if let View::Ai { prompt, .. } = self.stack.last_mut().unwrap() {
                    *prompt = q.clone();
                } else {
                    self.query = q.clone();
                }
            }
            Kind::Quicklink { id, arg } => {
                let Some(ql) = self.quicklinks.iter().find(|q| &q.id == id).cloned() else { return };
                let defs = placeholders::arguments(&ql.link);
                match (defs.len(), arg) {
                    (0, _) => self.open_quicklink(&ql, HashMap::new()),
                    (1, Some(a)) if !a.is_empty() => {
                        let mut m = HashMap::new();
                        m.insert(defs[0].name.clone(), a.clone());
                        self.open_quicklink(&ql, m);
                    }
                    _ => self.push_args(ArgTarget::Quicklink(ql.id.clone()), ql.name.clone(), defs, arg.clone()),
                }
            }
            Kind::Snippet(id) => {
                let Some(s) = self.snippets.iter().find(|s| &s.id == id).cloned() else { return };
                let defs = placeholders::arguments(&s.text);
                if defs.is_empty() {
                    self.paste_snippet(&s, HashMap::new());
                } else {
                    self.push_args(ArgTarget::Snippet(s.id.clone()), s.name.clone(), defs, None);
                }
            }
            Kind::Script(i) => {
                let Some(s) = self.scripts.get(*i).cloned() else { return };
                if s.arguments.is_empty() {
                    self.run_script_cmd(*i, vec![]);
                } else {
                    let defs = s
                        .arguments
                        .iter()
                        .map(|a| ArgDef {
                            name: if a.placeholder.is_empty() { "Value".into() } else { a.placeholder.clone() },
                            options: a.options.clone(),
                            optional: a.optional,
                            password: a.kind == "password",
                        })
                        .collect();
                    self.push(View::Args {
                        target: ArgTarget::Script(*i),
                        title: s.title.clone(),
                        defs,
                        values: vec![String::new(); s.arguments.len()],
                    });
                    if let View::Args { values, defs, .. } = self.stack.last_mut().unwrap() {
                        for (v, d) in values.iter_mut().zip(defs.iter()) {
                            if let Some((_, val)) = d.options.first() {
                                *v = val.clone();
                            }
                        }
                    }
                }
            }
            Kind::Calc(r) => {
                let r = r.clone();
                self.remember_calc(&r);
                self.copy_text(&r.raw)
            }
            Kind::Color(c) => {
                self.push(View::Colors(*c));
            }
            Kind::ColorFormat(v) => {
                let v = v.clone();
                self.copy_text(&v)
            }
            Kind::Url(u) => self.open_target(&u.clone(), None),
            Kind::File(p) => self.open_target(p.to_string_lossy().as_ref(), None),
            Kind::WebSearch(q) => {
                let url = self.settings.fallback_search_url.clone();
                let mut ctx = placeholders::Context::default();
                for d in placeholders::arguments(&url) {
                    ctx.arguments.insert(d.name, q.clone());
                }
                let u = placeholders::expand(&url, Target::Quicklink, &ctx).text;
                self.open_target(&u, None);
            }
            Kind::Clip(id) => self.perform(Act::ClipPaste(id.clone()), true),
            Kind::Emoji(e) => {
                let e = e.to_string();
                self.remember_emoji(&e);
                self.paste_text(&e)
            }
            Kind::Note(id) => {
                self.push(View::NoteEdit(id.clone()));
            }
            Kind::OpenWindow(w) => {
                let w = w.clone();
                self.hide();
                self.reset_to_root();
                running::activate(w.pid);
                windows::raise_window(w.pid, &w.title);
            }
            Kind::Process(pr) => {
                let pid = pr.pid;
                self.kill_process(pid, false);
            }
            Kind::Timer(secs, label) => {
                let (secs, label) = (*secs, label.clone());
                self.start_timer(secs, label);
            }
            Kind::FileTool(files, tool) => {
                let (files, tool) = (files.clone(), tool.clone());
                self.toast(format!("{}…", tool.label()));
                services::file_tool_files(files, tool, self.tools.ffmpeg, self.bus.clone());
                self.hide();
                self.reset_to_root();
            }
            Kind::ImageTool(files, op) => {
                let (files, op) = (files.clone(), op.clone());
                self.toast(format!("{}…", op.label()));
                services::image_op_files(files, op, self.bus.clone());
                self.hide();
                self.reset_to_root();
            }
            Kind::ConvertTo(files, t) => {
                if let Some(n) = t.needs {
                    // "ffmpeg (brew install ffmpeg)" → copy the command in parentheses
                    let cmd = n.split_once('(').map(|(_, c)| c.trim_end_matches(')').to_string()).unwrap_or_else(|| n.to_string());
                    self.copy_text(&cmd);
                    self.toast(format!("Install command copied: {cmd}"));
                    return;
                }
                let (files, t) = (files.clone(), t.clone());
                self.toast(format!(
                    "Converting {} to {}…",
                    if files.len() == 1 {
                        files[0].file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                    } else {
                        format!("{} files", files.len())
                    },
                    t.label
                ));
                services::convert_files(files, t, self.tools, self.bus.clone());
                self.hide();
                self.reset_to_root();
            }
            Kind::RunningTimer(i) => {
                let i = *i;
                self.cancel_timer(i);
            }
        }
    }

    /// Keep the last 100 distinct calculations, newest first.
    fn remember_calc(&mut self, r: &ferrocast_core::calc::CalcResult) {
        if matches!(self.view(), View::CalcHistory) {
            return;
        }
        self.calc_history.retain(|e| e.question != r.question);
        self.calc_history.insert(
            0,
            ferrocast_core::models::CalcEntry {
                question: r.question.clone(),
                display: r.display.clone(),
                raw: r.raw.clone(),
                value: r.value,
                unit: r.unit.clone(),
                at: now_secs(),
            },
        );
        self.calc_history.truncate(100);
        let v = self.calc_history.clone();
        self.persist("calc-history.json", &v);
    }

    /// Up to 16 recently used emoji, newest first.
    fn remember_emoji(&mut self, e: &str) {
        self.recent_emoji.retain(|x| x != e);
        self.recent_emoji.insert(0, e.to_string());
        self.recent_emoji.truncate(16);
        let v = self.recent_emoji.clone();
        self.persist("recent-emoji.json", &v);
    }

    /// Ask Homebrew whether a newer formula is available; the answer arrives as a toast.
    fn check_updates(&mut self) {
        self.toast("Checking Homebrew…");
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let brew = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"].iter().find(|p| std::path::Path::new(p).exists());
            let Some(brew) = brew else {
                bus.send(Msg::Toast("Homebrew not found; update from the repository".into()));
                return;
            };
            let installed = std::process::Command::new(brew)
                .env("HOMEBREW_NO_AUTO_UPDATE", "1")
                .args(["list", "--versions", "ferrocast"])
                .output()
                .map(|o| o.status.success() && !o.stdout.is_empty())
                .unwrap_or(false);
            if !installed {
                bus.send(Msg::Toast("Not installed with Homebrew; update from the repository".into()));
                return;
            }
            let msg = match std::process::Command::new(brew)
                .env("HOMEBREW_NO_AUTO_UPDATE", "1")
                .args(["outdated", "--verbose", "ferrocast"])
                .output()
            {
                Ok(o) if o.status.success() => {
                    let out = String::from_utf8_lossy(&o.stdout);
                    match out.lines().find(|l| l.starts_with("ferrocast")) {
                        Some(l) => format!("Update available: {} · run brew upgrade ferrocast", l.trim_start_matches("ferrocast").trim()),
                        None => format!("Ferrocast {} is up to date", env!("CARGO_PKG_VERSION")),
                    }
                }
                Ok(o) => format!("brew: {}", String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("unknown error")),
                Err(e) => format!("Could not run brew: {e}"),
            };
            bus.send(Msg::Toast(msg));
        });
    }

    /// Start a countdown; a thread sleeps and then notifies, unless the timer was cancelled.
    fn start_timer(&mut self, secs: u64, label: String) {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let what = if label.is_empty() { format!("{} timer", ferrocast_core::timer::describe(secs)) } else { label.clone() };
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs));
            if flag.load(Ordering::Relaxed) {
                return;
            }
            services::notify("Timer done", &what);
            let _ = std::process::Command::new("/usr/bin/afplay").arg("/System/Library/Sounds/Glass.aiff").spawn();
            bus.send(Msg::Wake);
        });
        self.timers.push(RunningTimer { ends_at: Instant::now() + Duration::from_secs(secs), secs, label, cancelled });
        self.toast(format!("Timer started · {}", ferrocast_core::timer::clock(secs)));
        self.hide();
        self.reset_to_root();
    }

    fn cancel_timer(&mut self, i: usize) {
        if i < self.timers.len() {
            self.timers[i].cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            self.timers.remove(i);
            self.toast("Timer cancelled");
        }
        self.sec_dirty = true;
    }

    /// Drop timers that already fired.
    fn sweep_timers(&mut self) {
        let now = Instant::now();
        let before = self.timers.len();
        self.timers.retain(|t| t.ends_at > now);
        if self.timers.len() != before {
            self.sec_dirty = true;
        }
    }

    /// End a process from the Kill Process list and refresh it in place.
    fn kill_process(&mut self, pid: i32, force: bool) {
        let name = self.proc_cache.iter().find(|p| p.pid == pid).map(|p| p.name.clone()).unwrap_or_else(|| pid.to_string());
        if ferrocast_macos::process::kill(pid, force) {
            self.toast(format!("{} {name}", if force { "Force killed" } else { "Killed" }));
        } else {
            self.toast(format!("Could not kill {name}"));
        }
        self.proc_cache.retain(|p| p.pid != pid);
        self.sec_dirty = true;
    }

    fn run_builtin(&mut self, b: Builtin) {
        match b {
            Builtin::AskAi => self.push(View::Ai {
                prompt: String::new(),
                asked: String::new(),
                answer: String::new(),
                history: vec![],
                busy: false,
                run: 0,
            }),
            Builtin::Welcome => self.push(View::Welcome),
            Builtin::KeyboardShortcuts => self.push(View::Shortcuts),
            Builtin::About => self.push(View::About),
            Builtin::ExportData => {
                let dest = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_default();
                let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
                match services::export_backup(&self.store.root, &dest, &stamp) {
                    Ok(p) => {
                        let _ = apps::reveal(&p);
                        self.toast("Backup saved to Downloads (clipboard history excluded)");
                    }
                    Err(e) => {
                        instance::log(&self.store.root, &format!("export failed: {e}"));
                        self.toast(format!("Backup failed: {e}"));
                    }
                }
            }
            Builtin::ImportSnippets => {
                let bus = self.bus.clone();
                self.hide();
                std::thread::spawn(move || {
                    if let Some(p) = services::choose_json_file() {
                        bus.send(Msg::ImportFile(p));
                    }
                });
            }
            Builtin::RevealLog => {
                let p = instance::log_path(&self.store.root);
                if !p.exists() {
                    let _ = std::fs::write(&p, "");
                }
                let _ = apps::reveal(&p);
                self.hide();
            }
            Builtin::ClipboardHistory => self.push(View::Clipboard),
            Builtin::SearchSnippets => self.push(View::Snippets),
            Builtin::CreateSnippet => self.new_snippet(String::new()),
            Builtin::SearchQuicklinks => self.push(View::Quicklinks),
            Builtin::CreateQuicklink => {
                self.push(View::QuicklinkForm { id: None, name: String::new(), link: String::new(), open_with: String::new() })
            }
            Builtin::SearchEmoji => self.push(View::Emoji),
            Builtin::SearchFiles => self.push(View::Files),
            Builtin::SwitchWindows => self.push(View::Windows),
            Builtin::KillProcess => self.push(View::Processes),
            Builtin::Timers => self.push(View::Timers),
            Builtin::CheckUpdates => self.check_updates(),
            Builtin::CalcHistory => self.push(View::CalcHistory),
            Builtin::Shelf => {
                let v = self.shelf.clone();
                ferrocast_macos::shelf::show(&v);
                self.hide();
                self.reset_to_root();
            }
            Builtin::ConvertFiles => {
                let bus = self.bus.clone();
                self.hide();
                std::thread::spawn(move || {
                    let files = services::choose_files();
                    if !files.is_empty() {
                        bus.send(Msg::ConvertFiles(files));
                    }
                });
            }
            Builtin::SearchNotes => self.push(View::Notes),
            Builtin::CreateNote => self.new_note(),
            Builtin::CreateScript => self.push(View::CreateScript { title: String::new(), mode: 0 }),
            Builtin::OpenScriptsFolder => {
                let d = self.store.scripts_dir();
                self.open_target(&d.to_string_lossy(), None);
            }
            Builtin::ReloadScripts => {
                self.reload_scripts();
                self.toast(format!("{} script commands loaded", self.scripts.len()));
            }
            Builtin::Settings => self.push(View::Settings),
            Builtin::Quit => std::process::exit(0),
        }
    }

    fn new_snippet(&mut self, text: String) {
        self.push(View::SnippetForm { id: None, name: String::new(), text, keyword: String::new() });
    }

    fn new_note(&mut self) {
        let now = now_secs();
        let n = Note { id: uuid::Uuid::new_v4().to_string(), text: String::new(), created: now, modified: now };
        let id = n.id.clone();
        self.notes.push(n);
        self.save_notes();
        self.push(View::NoteEdit(id));
    }

    fn run_system(&mut self, c: system::SystemCommand) {
        self.hide();
        self.reset_to_root();
        let excl = self.settings.quit_all_excluded.clone();
        let bus = self.bus.clone();
        std::thread::spawn(move || match c.run(&excl) {
            Ok(msg) => {
                if matches!(
                    c,
                    system::SystemCommand::ToggleMute
                        | system::SystemCommand::VolumeUp
                        | system::SystemCommand::VolumeDown
                        | system::SystemCommand::Volume(_)
                        | system::SystemCommand::EmptyTrash
                        | system::SystemCommand::ToggleHiddenFiles
                        | system::SystemCommand::ToggleWifi
                        | system::SystemCommand::KeepAwake
                ) {
                    services::notify("Ferrocast", &msg);
                }
            }
            Err(e) => bus.send(Msg::Toast(format!("{}: {e}", c.title()))),
        });
    }

    fn push_args(&mut self, target: ArgTarget, title: String, defs: Vec<placeholders::ArgumentDef>, first: Option<String>) {
        let ad: Vec<ArgDef> = defs
            .iter()
            .map(|d| ArgDef {
                name: d.name.clone(),
                options: d.options.iter().map(|o| (o.clone(), o.clone())).collect(),
                optional: d.default.is_some(),
                password: false,
            })
            .collect();
        let mut values: Vec<String> = defs.iter().map(|d| d.default.clone().unwrap_or_default()).collect();
        for (v, d) in values.iter_mut().zip(ad.iter()) {
            if v.is_empty() {
                if let Some((_, o)) = d.options.first() {
                    *v = o.clone();
                }
            }
        }
        if let (Some(f), Some(v)) = (first, values.first_mut()) {
            *v = f;
        }
        self.push(View::Args { target, title, defs: ad, values });
    }

    fn arg_context(&self, defs: &[ArgDef], values: &[String]) -> placeholders::Context<'static> {
        let mut ctx = placeholders::Context { clipboard: services::clipboard_texts(&self.shared), ..Default::default() };
        for (i, (d, v)) in defs.iter().zip(values).enumerate() {
            ctx.arguments.insert(d.name.clone(), v.clone());
            if i == 0 {
                ctx.arguments.entry("argument".into()).or_insert(v.clone());
            }
        }
        ctx
    }

    fn open_quicklink(&mut self, q: &Quicklink, args: HashMap<String, String>) {
        let snaps = self.snippets.clone();
        let lookup = |n: &str| snaps.iter().find(|s| s.name == n).map(|s| s.text.clone());
        let ctx = placeholders::Context {
            clipboard: services::clipboard_texts(&self.shared),
            arguments: args,
            snippets: Some(&lookup),
            ..Default::default()
        };
        let is_path = q.link.starts_with('/') || q.link.starts_with('~');
        let target = placeholders::expand(&q.link, if is_path { Target::Snippet } else { Target::Quicklink }, &ctx).text;
        let with = q.open_with.clone();
        self.open_target(&target, with.as_deref());
    }

    fn paste_snippet(&mut self, s: &Snippet, args: HashMap<String, String>) {
        let snaps = self.snippets.clone();
        let lookup = |n: &str| snaps.iter().find(|s| s.name == n).map(|s| s.text.clone());
        let ctx = placeholders::Context {
            clipboard: services::clipboard_texts(&self.shared),
            arguments: args,
            snippets: Some(&lookup),
            ..Default::default()
        };
        let ex = placeholders::expand(&s.text, Target::Snippet, &ctx);
        clipboard::write_text(&ex.text);
        self.shared.mark_clipboard_write();
        let back = ex.cursor.map(|c| ex.text.chars().count().saturating_sub(c));
        self.hide();
        self.reset_to_root();
        let pid = self.prev_pid;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(160));
            if let Some(p) = pid {
                running::activate(p);
            }
            std::thread::sleep(Duration::from_millis(90));
            keyboard::paste();
            if let Some(n) = back {
                std::thread::sleep(Duration::from_millis(120));
                keyboard::tap_key(keyboard::keycode::LEFT, n);
            }
        });
    }

    fn run_script_cmd(&mut self, idx: usize, args: Vec<String>) {
        let Some(s) = self.scripts.get(idx).cloned() else { return };
        if s.needs_confirmation {
            self.push(View::Confirm {
                msg: format!("Run script “{}”? Scripts have your user account's permissions.", s.title),
                act: Act::RunScript { index: idx, args },
            });
            return;
        }
        self.execute_script_cmd(idx, args);
    }

    fn prune_clip_images(&self) {
        let list = self.shared.clipboard.lock().unwrap().clone();
        ferrocast_core::storage::prune_images(&self.store.images_dir(), &list);
    }

    fn execute_script_cmd(&mut self, idx: usize, args: Vec<String>) {
        let Some(s) = self.scripts.get(idx).cloned() else { return };
        let args =
            s.arguments.iter().zip(args).map(|(d, a)| if d.percent_encoded { placeholders::percent_encode(&a) } else { a }).collect();
        let run = self.next_run;
        self.next_run += 1;
        match s.mode {
            Mode::FullOutput | Mode::Inline => {
                self.runs.insert(run, RunKind::Output);
                // drop any Confirm/Args views so Esc on the output doesn't return to them
                while matches!(self.view(), View::Confirm { .. } | View::Args { .. }) {
                    self.stack.pop();
                }
                self.push(View::Output { run, title: s.title.clone(), text: String::new(), done: false });
            }
            Mode::Compact => {
                self.runs.insert(run, RunKind::Compact(s.title.clone()));
                self.hide();
                self.reset_to_root();
            }
            Mode::Silent => {
                self.runs.insert(run, RunKind::Silent(s.title.clone()));
                self.hide();
                self.reset_to_root();
            }
        }
        services::run_script(&s, args, run, self.bus.clone());
    }

    fn submit_args(&mut self) {
        let View::Args { target, defs, values, .. } = self.view().clone() else { return };
        if defs.iter().zip(&values).any(|(d, v)| !d.optional && v.trim().is_empty()) {
            self.toast("Fill in all required arguments");
            return;
        }
        match target {
            ArgTarget::Quicklink(id) => {
                if let Some(q) = self.quicklinks.iter().find(|q| q.id == id).cloned() {
                    let ctx = self.arg_context(&defs, &values);
                    self.open_quicklink(&q, ctx.arguments);
                }
            }
            ArgTarget::Snippet(id) => {
                if let Some(s) = self.snippets.iter().find(|s| s.id == id).cloned() {
                    let ctx = self.arg_context(&defs, &values);
                    self.paste_snippet(&s, ctx.arguments);
                }
            }
            ArgTarget::Script(i) => self.run_script_cmd(i, values),
        }
    }

    fn perform(&mut self, act: Act, confirmed: bool) {
        let destructive = matches!(
            act,
            Act::ClipDeleteAll
                | Act::Trash(_)
                | Act::DeleteSnippet(_)
                | Act::DeleteQuicklink(_)
                | Act::NoteDelete(_)
                | Act::DisableCommand
                | Act::ForceQuitApp(_)
        );
        if destructive && !confirmed {
            let msg = match &act {
                Act::ClipDeleteAll => "Delete all clipboard history?",
                Act::Trash(_) => "Move this file to the Trash?",
                Act::DeleteSnippet(_) => "Delete this snippet?",
                Act::DeleteQuicklink(_) => "Delete this quicklink?",
                Act::NoteDelete(_) => "Delete this note?",
                Act::ForceQuitApp(_) => "Force quit this application? Unsaved changes may be lost.",
                _ => "Disable this command? (re-enable in Settings)",
            };
            self.push(View::Confirm { msg: msg.into(), act });
            return;
        }
        let item = self.act_item.clone();
        self.panel = None;
        match act {
            Act::Run => {}
            Act::CopyText(t) => self.copy_text(&t),
            Act::PasteText(t) => self.paste_text(&t),
            Act::Reveal(p) => {
                let _ = apps::reveal(&p);
                self.hide();
                self.reset_to_root();
            }
            Act::QuitApp(pid) => {
                running::quit(pid);
                self.toast("Quit requested");
            }
            Act::ForceQuitApp(pid) => {
                running::force_quit(pid);
                self.toast("Force quit");
            }
            Act::KillProcess(pid, force) => self.kill_process(pid, force),
            Act::CancelTimer(i) => self.cancel_timer(i),
            Act::AddToShelf(files) => {
                let n = files.len();
                for f in files {
                    if !self.shelf.contains(&f) {
                        self.shelf.insert(0, f);
                    }
                }
                let v = self.shelf.clone();
                self.persist("shelf.json", &v);
                // Float: the always-on-top panel pinned to the screen edge
                ferrocast_macos::shelf::show(&v);
                self.toast(format!("{n} item{} on the shelf", if n == 1 { "" } else { "s" }));
            }
            Act::RemoveFromShelf(p) => {
                ferrocast_macos::shelf::remove(&p);
                self.shelf.retain(|f| *f != p);
                let v = self.shelf.clone();
                self.persist("shelf.json", &v);
                self.invalidate_sections();
            }
            Act::ClearShelf => {
                ferrocast_macos::shelf::clear();
                self.shelf.clear();
                self.persist("shelf.json", &Vec::<PathBuf>::new());
                self.invalidate_sections();
                self.toast("Shelf cleared");
            }
            Act::ClearCalcHistory => {
                self.calc_history.clear();
                let v = self.calc_history.clone();
                self.persist("calc-history.json", &v);
                self.toast("History cleared");
            }
            Act::DeleteCalcEntry(i) => {
                if i < self.calc_history.len() {
                    self.calc_history.remove(i);
                    let v = self.calc_history.clone();
                    self.persist("calc-history.json", &v);
                }
            }
            Act::CopyFiles(files) => {
                ferrocast_macos::clipboard::write_files(&files);
                self.shared.mark_clipboard_write();
                self.toast(if files.len() == 1 {
                    "File copied, paste it in Finder".to_string()
                } else {
                    format!("{} files copied", files.len())
                });
            }
            Act::MoveFiles(files) => {
                let bus = self.bus.clone();
                self.hide();
                std::thread::spawn(move || {
                    let Some(dest) = apps::choose_folder("Move to") else { return };
                    match apps::move_into(&files, &dest) {
                        Ok(new) => {
                            let pairs = files.into_iter().zip(new.iter().cloned()).collect();
                            bus.send(Msg::Moved(pairs));
                            let _ = apps::reveal(&new[0]);
                        }
                        Err(e) => bus.send(Msg::Toast(format!("Move failed: {e}"))),
                    }
                });
            }
            Act::OpenInTerminal(p) => match apps::open_in_terminal(&p) {
                Ok(_) => {
                    self.hide();
                    self.reset_to_root();
                }
                Err(e) => self.toast(format!("Could not open a terminal: {e}")),
            },
            Act::AirDrop(files) => {
                // the picker attaches to the frontmost app; hide first so it is not hidden with us
                self.hide();
                if let Err(e) = apps::airdrop(&files) {
                    self.toast(format!("AirDrop: {e}"));
                }
            }
            Act::Convert(files) => {
                self.show();
                self.push(View::Convert(files));
            }
            Act::QuickLook(p) => {
                // qlmanage shows the system Quick Look panel; it exits when the panel closes.
                let _ = std::process::Command::new("/usr/bin/qlmanage")
                    .arg("-p")
                    .arg(&p)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                self.hide();
            }
            Act::HideApp(pid) => {
                running::hide(pid);
                self.toast("Hidden");
            }
            Act::ToggleFavorite => {
                if let Some(i) = item {
                    self.settings.toggle_favorite(&i.id);
                    self.save_settings();
                }
            }
            Act::MoveFavorite(d) => {
                if let Some(i) = item {
                    self.settings.move_favorite(&i.id, d);
                    self.save_settings();
                }
            }
            Act::SetAlias => {
                if let Some(i) = item {
                    let cur = self.settings.alias(&i.id).unwrap_or("").to_string();
                    self.push(View::Alias { id: i.id, name: i.title, value: cur });
                }
            }
            Act::SetHotkey => {
                if let Some(i) = item {
                    self.push(View::Hotkey { id: i.id, name: i.title, err: String::new() });
                }
            }
            Act::ResetRanking => {
                if let Some(i) = item {
                    self.frecency.reset(&i.id);
                    self.save_frecency();
                    self.toast("Ranking reset");
                }
            }
            Act::DisableCommand => {
                if let Some(i) = item {
                    self.settings.command_mut(&i.id).disabled = true;
                    self.save_settings();
                    self.toast("Command disabled — re-enable in Settings");
                }
            }
            Act::EditSnippet(id) => {
                if let Some(s) = self.snippets.iter().find(|s| s.id == id).cloned() {
                    self.push(View::SnippetForm { id: Some(s.id), name: s.name, text: s.text, keyword: s.keyword });
                }
            }
            Act::DuplicateSnippet(id) => {
                if let Some(mut s) = self.snippets.iter().find(|s| s.id == id).cloned() {
                    s.id = uuid::Uuid::new_v4().to_string();
                    s.name = format!("{} copy", s.name);
                    s.keyword.clear();
                    self.snippets.push(s);
                    self.save_snippets();
                }
            }
            Act::DeleteSnippet(id) => {
                self.snippets.retain(|s| s.id != id);
                self.save_snippets();
                self.pop_if_confirm();
            }
            Act::CreateSnippet => self.new_snippet(String::new()),
            Act::EditQuicklink(id) => {
                if let Some(q) = self.quicklinks.iter().find(|q| q.id == id).cloned() {
                    self.push(View::QuicklinkForm {
                        id: Some(q.id),
                        name: q.name,
                        link: q.link,
                        open_with: q.open_with.unwrap_or_default(),
                    });
                }
            }
            Act::DuplicateQuicklink(id) => {
                if let Some(mut q) = self.quicklinks.iter().find(|q| q.id == id).cloned() {
                    q.id = uuid::Uuid::new_v4().to_string();
                    q.name = format!("{} copy", q.name);
                    self.quicklinks.push(q);
                    self.save_quicklinks();
                }
            }
            Act::DeleteQuicklink(id) => {
                self.quicklinks.retain(|q| q.id != id);
                self.save_quicklinks();
                self.pop_if_confirm();
            }
            Act::CreateQuicklink => {
                self.push(View::QuicklinkForm { id: None, name: String::new(), link: String::new(), open_with: String::new() })
            }
            Act::ClipCopy(ref id) | Act::ClipPaste(ref id) => {
                let paste = matches!(act, Act::ClipPaste(_));
                let entry = self.shared.clipboard.lock().unwrap().iter().find(|e| &e.id == id).cloned();
                if let Some(e) = entry {
                    match e.kind {
                        ClipKind::Image => {
                            if let Ok(b) = std::fs::read(self.store.images_dir().join(&e.content)) {
                                clipboard::write_png(&b);
                            }
                        }
                        ClipKind::File => clipboard::write_files(&e.content.lines().map(PathBuf::from).collect::<Vec<_>>()),
                        _ => clipboard::write_text(&e.content),
                    }
                    self.shared.mark_clipboard_write();
                    if paste {
                        self.paste_after_hide();
                    } else {
                        self.toast("Copied to Clipboard");
                        self.hide();
                        self.reset_to_root();
                    }
                }
            }
            Act::ClipPin(id) => {
                if let Some(e) = self.shared.clipboard.lock().unwrap().iter_mut().find(|e| e.id == id) {
                    e.pinned = !e.pinned;
                }
                self.shared.save_clipboard();
            }
            Act::ClipDelete(id) => {
                self.shared.clipboard.lock().unwrap().retain(|e| e.id != id);
                self.shared.save_clipboard();
                self.prune_clip_images();
            }
            Act::ClipDeleteAll => {
                self.shared.clipboard.lock().unwrap().retain(|e| e.pinned);
                self.shared.save_clipboard();
                self.prune_clip_images();
                self.toast("Clipboard history cleared");
            }
            Act::ClipSaveSnippet(id) => {
                let t = self.shared.clipboard.lock().unwrap().iter().find(|e| e.id == id).map(|e| e.content.clone());
                if let Some(t) = t {
                    self.new_snippet(t);
                }
            }
            Act::NoteDelete(id) => {
                self.notes.retain(|n| n.id != id);
                self.save_notes();
            }
            Act::CreateNote => self.new_note(),
            Act::Trash(p) => {
                match apps::trash(&p) {
                    Ok(()) => self.toast("Moved to Trash"),
                    Err(e) => self.toast(format!("Trash failed: {e}")),
                }
                self.files.1.retain(|f| f != &p);
            }
            Act::EditScript(p) => {
                let _ = std::process::Command::new("/usr/bin/open").arg("-t").arg(&p).status();
                self.hide();
                self.reset_to_root();
            }
            Act::RunSystem(c) => self.run_system(c),
            Act::RunScript { index, args } => self.execute_script_cmd(index, args),
        }
    }

    fn pop_if_confirm(&mut self) {
        if matches!(self.view(), View::Confirm { .. }) {
            self.pop();
        }
    }
}

fn file_item(p: &std::path::Path) -> Item {
    Item {
        id: format!("file:{}", p.display()),
        title: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        subtitle: p
            .parent()
            .map(|d| d.to_string_lossy().replace(&dirs::home_dir().unwrap_or_default().to_string_lossy().to_string(), "~"))
            .unwrap_or_default(),
        icon: Icon::App(p.to_string_lossy().to_string()),
        accessory: "File".into(),
        keywords: vec![],
        kind: Kind::File(p.to_path_buf()),
    }
}

/// `SmileysAndEmotion` -> `Smileys & Emotion`.
fn humanize_group(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out.replace(" And ", " & ")
}

fn looks_like_url(q: &str) -> Option<String> {
    let q = q.trim();
    if q.contains(char::is_whitespace) || q.len() < 4 {
        return None;
    }
    if q.starts_with("http://") || q.starts_with("https://") {
        return Some(q.to_string());
    }
    let host = q.split('/').next()?;
    let tld = host.rsplit('.').next()?;
    if host.contains('.') && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) && !host.starts_with('.') {
        return Some(format!("https://{q}"));
    }
    None
}

/// Top-left origin (points) to center the launcher on the screen under the cursor.
/// SF symbol for an image tool.
pub(crate) fn image_op_symbol(op: &ferrocast_core::convert::ImageOp) -> &'static str {
    use ferrocast_core::convert::ImageOp;
    match op {
        ImageOp::Compress(_) => "sf:arrow.down.right.and.arrow.up.left",
        ImageOp::Resize(_) => "sf:arrow.up.left.and.down.right.magnifyingglass",
        ImageOp::Crop(..) => "sf:crop",
        ImageOp::StripMetadata => "sf:eye.slash",
        ImageOp::CombinePdf => "sf:doc.richtext",
        ImageOp::Rotate(_) => "sf:rotate.right",
        ImageOp::Flip(_) => "sf:arrow.left.and.right.righttriangle.left.righttriangle.right",
        ImageOp::Background(_) => "sf:square.dashed",
        ImageOp::Collage | ImageOp::CollageWith(_) => "sf:square.grid.2x2",
        ImageOp::FrameWith(_) => "sf:square.dashed",
        ImageOp::RedactWith(_) => "sf:eye.slash.fill",
        ImageOp::EditWith(_) => "sf:slider.horizontal.3",
        ImageOp::Redact(_) => "sf:eye.slash.fill",
        ImageOp::Adjust(_) => "sf:slider.horizontal.3",
        ImageOp::Annotate => "sf:pencil.tip.crop.circle",
        ImageOp::CompressToKb(_) | ImageOp::CompressByPercent(_) => "sf:arrow.down.right.and.arrow.up.left",
        ImageOp::CropExact(..) => "sf:crop",
        ImageOp::SetMeta(_) => "sf:tag",
    }
}

/// Wheel geometry, in points from its centre.
pub(crate) const WHEEL_INNER: f32 = 58.0;
pub(crate) const WHEEL_OUTER: f32 = 178.0;

/// Segment under an offset from the wheel centre.
pub(crate) fn wheel_hit(d: Vec2, n: usize) -> Option<usize> {
    ferrocast_core::wheel::segment_at(d.x, d.y, WHEEL_INNER, WHEEL_OUTER, n)
}

/// Pointer position and its screen's visible frame, in top-left screen points.
pub(crate) fn mouse_top_left() -> Option<(Pos2, Rect)> {
    use objc2_app_kit::{NSEvent, NSScreen};
    let mtm = objc2::MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    let primary_h = screens.iter().next()?.frame().size.height;
    let m = NSEvent::mouseLocation();
    let scr = screens.iter().find(|s| {
        let f = s.frame();
        m.x >= f.origin.x && m.x < f.origin.x + f.size.width && m.y >= f.origin.y && m.y < f.origin.y + f.size.height
    })?;
    let v = scr.visibleFrame();
    let top = primary_h - (v.origin.y + v.size.height);
    let r = Rect::from_min_size(Pos2::new(v.origin.x as f32, top as f32), Vec2::new(v.size.width as f32, v.size.height as f32));
    Some((Pos2::new(m.x as f32, (primary_h - m.y) as f32), r))
}

fn launcher_origin() -> Option<Pos2> {
    use objc2_app_kit::{NSEvent, NSScreen};
    let mtm = objc2::MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    let primary_h = screens.iter().next()?.frame().size.height;
    let m = NSEvent::mouseLocation();
    let scr = screens.iter().find(|s| {
        let f = s.frame();
        m.x >= f.origin.x && m.x < f.origin.x + f.size.width && m.y >= f.origin.y && m.y < f.origin.y + f.size.height
    })?;
    let v = scr.visibleFrame();
    let x = v.origin.x + (v.size.width - ui::WIDTH as f64) / 2.0;
    let top = primary_h - (v.origin.y + v.size.height);
    let y = top + v.size.height * 0.22;
    Some(Pos2::new(x as f32, y as f32))
}

#[path = "draw.rs"]
mod draw;
