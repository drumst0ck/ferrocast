//! Background services: clipboard monitor, snippet expansion, hotkeys, tray, scripts.

use ferrocast_core::models::*;
use ferrocast_core::placeholders::{self, Target};
use ferrocast_core::script::{self, ScriptCommand};
use ferrocast_core::storage::{self, Store};
use ferrocast_macos::{apps::AppInfo, clipboard, expansion, running};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

pub enum Msg {
    Hotkey(u32),
    Tray(String),
    Apps(Vec<AppInfo>),
    Files(String, Vec<PathBuf>),
    /// PNG thumbnail of page `index` of the PDF being organized (None: could not render).
    PdfThumb(PathBuf, usize, Option<Vec<u8>>),
    /// PNG of the image being annotated, scaled for display (None: unreadable).
    AnnotateLoaded(PathBuf, Option<Vec<u8>>),
    /// PNG of the image being edited, scaled for the live preview (None: unreadable).
    EditLoaded(PathBuf, Option<Vec<u8>>),
    /// PNG of the image being cropped, scaled for display (None: unreadable).
    CropLoaded(PathBuf, Option<Vec<u8>>),
    /// Tags of the file in the Edit Metadata window (Err: message to show).
    MetadataLoaded(PathBuf, Result<Vec<ferrocast_core::metaform::Field>, String>),
    /// Pixel size of the first image in the Compress window (None: unreadable).
    CompressProbed(PathBuf, Option<(u32, u32)>),
    /// Duration, audio and size of the first file in the media Compress window (None: unreadable).
    MediaProbed(PathBuf, Option<ferrocast_core::compress::MediaInfo>),
    /// PNG of the image being redacted, scaled for display (None: unreadable).
    RedactLoaded(PathBuf, Option<Vec<u8>>),
    /// PNG of the image being framed, scaled for the live preview (None: unreadable).
    FrameLoaded(PathBuf, Option<Vec<u8>>),
    /// Small decoded copies (w, h, RGBA) of the pictures of a collage being edited.
    CollageLoaded(Vec<PathBuf>, Vec<(usize, usize, Vec<u8>)>),
    /// Duration and frame rate of the video being trimmed (None: unreadable).
    VideoInfo(PathBuf, Option<(f64, f64)>),
    /// Mono samples of the audio being trimmed (Err: message to show).
    AudioLoaded(PathBuf, Result<crate::media_tools::AudioSamples, String>),
    /// Integrated loudness (LUFS) of the file in the Normalize window (None: could not be measured).
    LoudnessMeasured(PathBuf, Option<f64>),
    /// Channel count of the file in the Channels window (None: unreadable).
    ChannelsProbed(PathBuf, Option<u32>),
    /// Finder selection read for the format wheel (Err: message to show).
    FinderSelection(Result<Vec<PathBuf>, String>),
    /// Duration of the video whose speed is being set (None: unreadable).
    VideoDuration(PathBuf, Option<f64>),
    /// Pixel size and a preview frame of the video being cropped (None: unreadable).
    VideoCropReady(PathBuf, Option<((u32, u32), Vec<u8>)>),
    VideoRedactReady(PathBuf, Option<((u32, u32), f64, Vec<u8>)>),
    /// Preview frame of the video being trimmed at this time (None: could not extract).
    VideoFrame(PathBuf, f64, Option<Vec<u8>>),
    /// A picture chosen to stamp onto the image being annotated: where, and its RGBA pixels.
    StampChosen(PathBuf, (f32, f32), Option<(usize, usize, Vec<u8>)>),
    Script {
        run: u64,
        text: String,
        code: i32,
    },
    Toast(String),
    Rates(ferrocast_core::currency::Rates),
    ImportFile(PathBuf),
    AiChunk {
        run: u64,
        text: String,
    },
    AiDone {
        run: u64,
        error: Option<String>,
    },
    /// Icon PNG bytes fetched in the background for the first screen.
    IconBytes(String, Vec<u8>),
    /// Device-code sign-in: show this code to the user while we poll.
    ChatGptDeviceCode(String),
    /// Files picked for conversion.
    ConvertFiles(Vec<PathBuf>),
    /// Files moved by Move to…: (old, new) paths.
    Moved(Vec<(PathBuf, PathBuf)>),
    /// A key was tapped while dragging these files: show the wheel.
    WheelOpen(ferrocast_core::wheel::WheelKind, Vec<PathBuf>),
    /// The floating shelf panel's items changed (drop in, remove, clear).
    ShelfChanged(Vec<PathBuf>),
    /// Another wheel key was tapped while the wheel was open.
    WheelSwitch(ferrocast_core::wheel::WheelKind),
    /// The mouse was released while the wheel was open.
    WheelRelease,
    /// Model list for the Settings picker, keyed by `models_key`.
    AiModels(String, Result<Vec<String>, String>),
    Wake,
}

#[derive(Clone)]
pub struct Bus {
    pub tx: Sender<Msg>,
    pub ctx: egui::Context,
}

impl Bus {
    pub fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
        self.ctx.request_repaint();
    }
}

/// State shared with background threads.
pub struct Shared {
    pub store: Store,
    pub clipboard: Mutex<Vec<ClipEntry>>,
    pub snippets: RwLock<Vec<Snippet>>,
    pub settings: RwLock<Settings>,
    pub ignored_clipboard_count: AtomicI64,
    pub visible: AtomicBool,
    pub keywords: expansion::Keywords,
}

/// `just now`, `5 min ago`, `3 h ago`, `2 d ago`.
pub fn ago(secs: i64) -> String {
    match secs {
        s if s < 60 => "just now".into(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86400 => format!("{} h ago", s / 3600),
        s => format!("{} d ago", s / 86400),
    }
}

/// Fetch icon PNGs for app paths off the main thread so the first screen paints complete.
pub fn prefetch_app_icons(paths: Vec<String>, bus: Bus) {
    if std::env::var_os("FERROCAST_NO_PREFETCH").is_some() {
        return;
    }
    std::thread::spawn(move || {
        for p in paths {
            if let Some(b) = ferrocast_macos::files::icon_png(&p, 64.0) {
                bus.tx.send(Msg::IconBytes(format!("app:{p}"), b)).ok();
            }
        }
        bus.ctx.request_repaint();
    });
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Shared {
    /// Ignore exactly this self-authored pasteboard version, not a time window.
    pub fn ignore_clipboard_count(&self, count: isize) {
        self.ignored_clipboard_count.store(count as i64, Ordering::SeqCst);
    }

    pub fn mark_clipboard_write(&self) {
        self.ignore_clipboard_count(clipboard::change_count());
    }

    pub fn refresh_keywords(&self) {
        let kws: Vec<String> = self.snippets.read().unwrap().iter().map(|s| s.keyword.clone()).filter(|k| !k.is_empty()).collect();
        *self.keywords.write().unwrap() = kws;
    }

    pub fn save_clipboard(&self) {
        let c = self.clipboard.lock().unwrap();
        let _ = self.store.save("clipboard.json", &*c);
    }
}

// ---------------------------------------------------------------------------
// Clipboard history

fn record(shared: &Shared, content: clipboard::ClipContent, source: Option<String>) {
    let now = now_secs();
    let (settings_retention,) = (shared.settings.read().unwrap().clipboard_retention,);
    let mut list = shared.clipboard.lock().unwrap();
    let mut entry = match content {
        clipboard::ClipContent::Text(t) => {
            if t.len() > 200_000 {
                return;
            }
            ClipEntry {
                id: uuid::Uuid::new_v4().to_string(),
                kind: ClipEntry::classify_text(&t),
                content: t,
                title: None,
                copied_at: now,
                pinned: false,
                source_app: source,
                times_copied: 1,
                width: 0,
                height: 0,
            }
        }
        clipboard::ClipContent::Files(f) => ClipEntry {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ClipKind::File,
            content: f.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n"),
            title: None,
            copied_at: now,
            pinned: false,
            source_app: source,
            times_copied: 1,
            width: 0,
            height: 0,
        },
        clipboard::ClipContent::Image { png, width, height } => {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            png.hash(&mut h);
            let name = format!("{:016x}.png", h.finish());
            let dir = shared.store.images_dir();
            let _ = std::fs::create_dir_all(&dir);
            let p = dir.join(&name);
            if !p.exists() {
                let _ = std::fs::write(&p, &png);
            }
            ClipEntry {
                id: uuid::Uuid::new_v4().to_string(),
                kind: ClipKind::Image,
                content: name,
                title: None,
                copied_at: now,
                pinned: false,
                source_app: source,
                times_copied: 1,
                width,
                height,
            }
        }
    };
    if let Some(i) = list.iter().position(|e| e.kind == entry.kind && e.content == entry.content) {
        let old = list.remove(i);
        entry.id = old.id;
        entry.pinned = old.pinned;
        entry.title = old.title;
        entry.times_copied = old.times_copied + 1;
    }
    list.insert(0, entry);
    // retention + cap
    if let Some(secs) = settings_retention.secs() {
        list.retain(|e| e.pinned || now - e.copied_at < secs);
    }
    let mut unpinned = 0;
    list.retain(|e| {
        if e.pinned {
            return true;
        }
        unpinned += 1;
        unpinned <= 1000
    });
    storage::prune_images(&shared.store.images_dir(), &list);
    let _ = shared.store.save("clipboard.json", &*list);
}

/// Samples mouse, modifiers and the drag pasteboard every 25 ms (no permission needed)
/// and reports wheel open/release events.
pub fn start_wheel(shared: Arc<Shared>, bus: Bus) {
    use ferrocast_core::wheel::{key_mask, Sample, Tracker, WheelEvent};
    use ferrocast_macos::drag;
    std::thread::Builder::new()
        .name("drop-wheel".into())
        .spawn(move || {
            let mut tracker = Tracker::default();
            loop {
                let (on, ck, ak, tk) = {
                    let s = shared.settings.read().unwrap();
                    (s.drop_wheel, key_mask(&s.wheel_convert_key), key_mask(&s.wheel_actions_key), key_mask(&s.wheel_tools_key))
                };
                if !on {
                    tracker = Tracker::default();
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                std::thread::sleep(Duration::from_millis(25));
                objc2::rc::autoreleasepool(|_| {
                    let left = drag::left_down();
                    let bits = drag::modifier_bits();
                    let (convert_key, actions_key, tools_key) =
                        (ck != 0 && bits & ck != 0, ak != 0 && bits & ak != 0, tk != 0 && bits & tk != 0);
                    // reading the pasteboard items only matters at the moment a key is tapped
                    let files = if left && (convert_key || actions_key || tools_key) { drag::drag_files() } else { vec![] };
                    let sample = Sample {
                        left_down: left,
                        drag_count: if left { drag::drag_count() } else { 0 },
                        has_files: !files.is_empty(),
                        convert_key,
                        actions_key,
                        tools_key,
                    };
                    match tracker.step(sample) {
                        Some(WheelEvent::Open(k)) => bus.send(Msg::WheelOpen(k, files)),
                        Some(WheelEvent::Switch(k)) => bus.send(Msg::WheelSwitch(k)),
                        Some(WheelEvent::Release) => bus.send(Msg::WheelRelease),
                        None => {}
                    }
                });
            }
        })
        .expect("spawn wheel thread");
}

pub fn start_clipboard(shared: Arc<Shared>, bus: Bus) {
    std::thread::Builder::new()
        .name("clipboard-monitor".into())
        .spawn(move || {
            let mut last = clipboard::change_count();
            loop {
                std::thread::sleep(Duration::from_millis(400));
                objc2::rc::autoreleasepool(|_| {
                    let c = clipboard::change_count();
                    let record_change =
                        crate::clipboard_policy::should_record(c, last, shared.ignored_clipboard_count.load(Ordering::SeqCst) as isize);
                    last = c;
                    if !record_change {
                        return;
                    }
                    let (enabled, ignored) = {
                        let s = shared.settings.read().unwrap();
                        (s.clipboard_enabled, s.clipboard_ignored_apps.clone())
                    };
                    if !enabled || clipboard::is_concealed() {
                        return;
                    }
                    let front = running::frontmost();
                    if let Some(f) = &front {
                        if f.bundle_id.as_ref().is_some_and(|b| ignored.contains(b)) {
                            return;
                        }
                    }
                    if let Some(content) = clipboard::read() {
                        record(&shared, content, front.map(|f| f.name));
                        bus.send(Msg::Wake);
                    }
                });
            }
        })
        .ok();
}

// ---------------------------------------------------------------------------
// Snippet expansion

pub fn clipboard_texts(shared: &Shared) -> Vec<String> {
    let mut v = vec![];
    if !clipboard::is_concealed() {
        if let Some(t) = clipboard::read_text() {
            v.push(t);
        }
    }
    for e in
        shared.clipboard.lock().unwrap().iter().filter(|e| matches!(e.kind, ClipKind::Text | ClipKind::Link | ClipKind::Color)).take(10)
    {
        if v.first() != Some(&e.content) {
            v.push(e.content.clone());
        }
    }
    v
}

pub fn start_expansion(shared: Arc<Shared>) {
    shared.refresh_keywords();
    let s1 = shared.clone();
    let enabled: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(move || {
        let st = s1.settings.read().unwrap();
        if !st.snippet_expansion || s1.visible.load(Ordering::Relaxed) {
            return false;
        }
        match running::frontmost().and_then(|f| f.bundle_id) {
            Some(b) => !st.snippet_disabled_apps.contains(&b),
            None => true,
        }
    });
    let s2 = shared.clone();
    let on_match: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |kw: String| {
        // Never replace or read a password-manager/transient clipboard.
        if clipboard::is_concealed() {
            return;
        }
        let Some(sn) = s2.snippets.read().unwrap().iter().find(|s| s.keyword == kw).cloned() else { return };
        let snaps = s2.snippets.read().unwrap().clone();
        let lookup = |n: &str| snaps.iter().find(|s| s.name == n).map(|s| s.text.clone());
        let ctx = placeholders::Context { clipboard: clipboard_texts(&s2), snippets: Some(&lookup), ..Default::default() };
        let ex = placeholders::expand(&sn.text, Target::Snippet, &ctx);
        let back = ex.cursor.map(|c| ex.text.chars().count().saturating_sub(c));
        expansion::inject(kw.chars().count(), &ex.text, back, |count| s2.ignore_clipboard_count(count));
    });
    if let Err(e) = expansion::start(shared.keywords.clone(), enabled, on_match) {
        eprintln!("snippet expansion unavailable: {e}");
    }
}

// ---------------------------------------------------------------------------
// Global hotkeys

pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    pub map: HashMap<u32, String>,
}

impl Hotkeys {
    pub fn new(bus: Bus) -> Option<Hotkeys> {
        let manager = GlobalHotKeyManager::new().ok()?;
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state == HotKeyState::Pressed {
                bus.send(Msg::Hotkey(e.id));
            }
        }));
        Some(Hotkeys { manager, registered: vec![], map: HashMap::new() })
    }

    /// Re-register every hotkey from settings; returns error strings.
    pub fn apply(&mut self, s: &Settings) -> Vec<String> {
        for h in self.registered.drain(..) {
            let _ = self.manager.unregister(h);
        }
        self.map.clear();
        let mut errors = vec![];
        let mut add = |this: &mut Hotkeys, spec: &str, target: &str| match HotKey::from_str(spec) {
            Ok(h) => match this.manager.register(h) {
                Ok(()) => {
                    this.map.insert(h.id(), target.to_string());
                    this.registered.push(h);
                }
                Err(e) => errors.push(format!("{spec}: {e}")),
            },
            Err(e) => errors.push(format!("{spec}: {e}")),
        };
        add(self, &s.launcher_hotkey, "__launcher");
        for (id, c) in &s.commands {
            if let Some(h) = c.hotkey.as_deref().filter(|h| !h.is_empty()) {
                add(self, h, id);
            }
        }
        errors
    }
}

/// Human readable hotkey: `Alt+Shift+KeyL` → `⌥⇧L`.
pub fn hotkey_label(spec: &str) -> String {
    let parts: Vec<String> = spec
        .split('+')
        .map(|t| match t.to_uppercase().as_str() {
            "ALT" | "OPTION" => "⌥".to_string(),
            "SHIFT" => "⇧".to_string(),
            "CONTROL" | "CTRL" => "⌃".to_string(),
            "SUPER" | "CMD" | "COMMAND" => "⌘".to_string(),
            "SPACE" => "Space".to_string(),
            _ => t.trim_start_matches("Key").trim_start_matches("Digit").to_string(),
        })
        .collect();
    // Modifiers run together (⇧⌘K); a named key such as Space or F3 gets a space before it.
    let mut out = String::new();
    for p in &parts {
        if !out.is_empty() && p.chars().count() > 1 {
            out.push(' ');
        }
        out.push_str(p);
    }
    out
}

/// egui key → global-hotkey key name.
pub fn key_code_name(k: egui::Key) -> Option<String> {
    use egui::Key::*;
    let n = k.name();
    Some(match k {
        Space => "Space".into(),
        Enter => "Enter".into(),
        Tab => "Tab".into(),
        Backspace => "Backspace".into(),
        ArrowUp => "ArrowUp".into(),
        ArrowDown => "ArrowDown".into(),
        ArrowLeft => "ArrowLeft".into(),
        ArrowRight => "ArrowRight".into(),
        Comma => "Comma".into(),
        Period => "Period".into(),
        Slash => "Slash".into(),
        Semicolon => "Semicolon".into(),
        Minus => "Minus".into(),
        Equals => "Equal".into(),
        Backtick => "Backquote".into(),
        OpenBracket => "BracketLeft".into(),
        CloseBracket => "BracketRight".into(),
        Backslash => "Backslash".into(),
        Quote => "Quote".into(),
        Num0 | Num1 | Num2 | Num3 | Num4 | Num5 | Num6 | Num7 | Num8 | Num9 => format!("Digit{n}"),
        F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 => n.to_string(),
        _ if n.len() == 1 && n.chars().all(|c| c.is_ascii_alphabetic()) => format!("Key{}", n.to_uppercase()),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Tray

pub fn make_tray(bus: Bus) -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    let menu = Menu::new();
    let items = [("open", "Open Ferrocast"), ("clipboard", "Clipboard History"), ("settings", "Settings…")];
    for (id, label) in items {
        menu.append(&MenuItem::with_id(id, label, true, None)).ok()?;
    }
    menu.append(&PredefinedMenuItem::separator()).ok()?;
    menu.append(&MenuItem::with_id("quit", "Quit Ferrocast", true, None)).ok()?;
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| bus.send(Msg::Tray(e.id.0.clone()))));

    // 32x32 ring template icon
    let n = 32usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            let d = (dx * dx + dy * dy).sqrt();
            let in_ring = (9.0..13.5).contains(&d);
            // bolt-ish notch
            let notch = dx > -1.0 && dx < 6.0 && dy.abs() < 2.0 && d < 13.5;
            if in_ring || notch {
                let i = (y * n + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    let icon = tray_icon::Icon::from_rgba(rgba, n as u32, n as u32).ok()?;
    tray_icon::TrayIconBuilder::new().with_menu(Box::new(menu)).with_icon_templated(icon).with_tooltip("Ferrocast").build().ok()
}

// ---------------------------------------------------------------------------
// Scripts

pub fn load_scripts(dirs: &[PathBuf]) -> Vec<ScriptCommand> {
    fn walk(dir: &std::path::Path, depth: usize, out: &mut Vec<ScriptCommand>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth > 0 && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
                    walk(&p, depth - 1, out);
                }
            } else if e.metadata().map(|m| m.len() < 256 * 1024).unwrap_or(false) {
                if let Ok(bytes) = std::fs::read(&p) {
                    let src = String::from_utf8_lossy(&bytes);
                    if let Some(c) = script::parse(&p, &src) {
                        out.push(c);
                    }
                }
            }
        }
    }
    let mut out = vec![];
    for d in dirs {
        walk(d, 2, &mut out);
    }
    out.sort_by_key(|a| a.title.to_lowercase());
    out
}

/// Ensure the default scripts directory exists with an example.
pub fn ensure_scripts_dir(store: &Store) {
    let dir = store.scripts_dir();
    if dir.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("hello-world.sh");
    let _ = std::fs::write(&p, script::template("Hello World", "fullOutput", "Examples"));
    let p2 = dir.join("uptime.sh");
    let _ = std::fs::write(
        &p2,
        "#!/bin/bash\n\n# @raycast.schemaVersion 1\n# @raycast.title Uptime\n# @raycast.mode compact\n# @raycast.packageName Examples\n\nuptime | sed 's/.*up/up/'\n",
    );
    for f in [p, p2] {
        set_exec(&f);
    }
}

pub fn set_exec(p: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(m) = std::fs::metadata(p) {
        let mut perm = m.permissions();
        perm.set_mode(0o755);
        let _ = std::fs::set_permissions(p, perm);
    }
}

pub fn run_script(cmd: &ScriptCommand, args: Vec<String>, run: u64, bus: Bus) {
    let path = cmd.path.clone();
    let dir = cmd.working_dir();
    std::thread::spawn(move || {
        set_exec(&path);
        let mut path_env = std::env::var("PATH").unwrap_or_default();
        for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
            if !path_env.split(':').any(|p| p == extra) {
                path_env = format!("{path_env}:{extra}");
            }
        }
        let out = std::process::Command::new(&path).args(&args).current_dir(&dir).env("PATH", path_env).output();
        match out {
            Ok(o) => {
                let mut text = String::from_utf8_lossy(&o.stdout).to_string();
                let err = String::from_utf8_lossy(&o.stderr);
                if !err.trim().is_empty() {
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push_str(&err);
                }
                bus.send(Msg::Script { run, text, code: o.status.code().unwrap_or(-1) });
            }
            Err(e) => bus.send(Msg::Script { run, text: format!("Failed to run: {e}"), code: -1 }),
        }
    });
}

pub fn notify(title: &str, msg: &str) {
    // tests dispatch real jobs that fail on purpose; they must not raise system notifications
    if cfg!(test) {
        return;
    }
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        msg.replace('\\', "\\\\").replace('"', "\\\""),
        title.replace('\\', "\\\\").replace('"', "\\\"")
    );
    std::thread::spawn(move || {
        let _ = ferrocast_macos::system::osascript(&script);
    });
}

pub fn decode_png(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

/// Refresh exchange rates in the background (ECB data via frankfurter.dev, no API key).
pub fn refresh_rates(bus: Bus) {
    std::thread::spawn(move || {
        let out = std::process::Command::new("/usr/bin/curl").args(["-sf", "-m", "15", ferrocast_core::currency::RATES_URL]).output();
        if let Ok(o) = out {
            if let Some(r) = ferrocast_core::currency::Rates::parse(&String::from_utf8_lossy(&o.stdout), now_secs()) {
                bus.send(Msg::Rates(r));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// AI (OpenAI-compatible endpoint; API key lives in the macOS Keychain)

const KEYCHAIN_SERVICE: &str = "ferrocast-ai";
const KEYCHAIN_SERVICE_ANTHROPIC: &str = "ferrocast-anthropic";
const KEYCHAIN_SERVICE_CHATGPT: &str = "ferrocast-chatgpt";

fn keychain_service(p: AiProvider) -> &'static str {
    match p {
        AiProvider::Anthropic => KEYCHAIN_SERVICE_ANTHROPIC,
        _ => KEYCHAIN_SERVICE,
    }
}

pub fn ai_key() -> Option<String> {
    ai_key_for(AiProvider::OpenAiCompatible)
}

pub fn ai_key_for(p: AiProvider) -> Option<String> {
    let out =
        std::process::Command::new("/usr/bin/security").args(["find-generic-password", "-s", keychain_service(p), "-w"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|k| !k.is_empty())
}

pub fn set_ai_key_for(p: AiProvider, key: &str) -> bool {
    let key = key.trim();
    let mut cmd = std::process::Command::new("/usr/bin/security");
    if key.is_empty() {
        cmd.args(["delete-generic-password", "-s", keychain_service(p)]);
    } else {
        cmd.args(["add-generic-password", "-U", "-s", keychain_service(p), "-a", "ferrocast", "-w", key]);
    }
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

// ---- Sign in with ChatGPT

pub fn chatgpt_credentials() -> Option<ferrocast_core::chatgpt::Credentials> {
    let out = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE_CHATGPT, "-w"])
        .output()
        .ok()?;
    out.status.success().then(|| serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).ok()).flatten()
}

fn save_chatgpt_credentials(c: Option<&ferrocast_core::chatgpt::Credentials>) -> bool {
    let mut cmd = std::process::Command::new("/usr/bin/security");
    match c {
        None => cmd.args(["delete-generic-password", "-s", KEYCHAIN_SERVICE_CHATGPT]),
        Some(c) => cmd.args([
            "add-generic-password",
            "-U",
            "-s",
            KEYCHAIN_SERVICE_CHATGPT,
            "-a",
            "ferrocast",
            "-w",
            &serde_json::to_string(c).unwrap_or_default(),
        ]),
    };
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

pub fn chatgpt_sign_out() -> bool {
    save_chatgpt_credentials(None)
}

/// POST a form body to the token endpoint through curl (body via stdin, never argv).
fn token_post(body: &str) -> Result<String, String> {
    use ferrocast_core::ai::curl_config_quote as q;
    use std::io::Write;
    use std::process::{Command, Stdio};
    let cfg = format!(
        "url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/x-www-form-urlencoded\"\ndata = {}\n",
        q(ferrocast_core::chatgpt::TOKEN_URL),
        q(body)
    );
    let mut child = Command::new("/usr/bin/curl")
        .args(["-sS", "-m", "30", "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child.stdin.take().ok_or("no stdin")?.write_all(cfg.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("curl failed").to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// POST a JSON body through curl; returns (HTTP status, body).
fn json_post(url: &str, body: &str) -> Result<(u16, String), String> {
    use ferrocast_core::ai::curl_config_quote as q;
    use std::io::Write;
    use std::process::{Command, Stdio};
    let cfg = format!("url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/json\"\ndata = {}\n", q(url), q(body));
    let mut child = Command::new("/usr/bin/curl")
        .args(["-sS", "-m", "30", "-w", "\n%{http_code}", "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child.stdin.take().ok_or("no stdin")?.write_all(cfg.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("curl failed").to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", "0"));
    Ok((code.trim().parse().unwrap_or(0), body.to_string()))
}

/// Device-code sign-in: no local port. Shows a code, opens the verification page, polls until
/// the user approves, then exchanges and stores the credentials.
pub fn chatgpt_sign_in_device(bus: Bus) {
    use ferrocast_core::chatgpt::{self, DevicePoll};
    std::thread::spawn(move || {
        let start = match json_post(chatgpt::DEVICE_USER_CODE_URL, &chatgpt::device_start_body()) {
            Ok((404, _)) => {
                Err("OpenAI's device-code sign-in is not enabled right now; free port 1455 and use the browser flow.".to_string())
            }
            Ok((s, b)) if (200..300).contains(&s) => chatgpt::parse_device_start(&b),
            Ok((s, b)) => Err(format!("device-code request failed ({s}): {}", b.chars().take(160).collect::<String>())),
            Err(e) => Err(e),
        };
        let d = match start {
            Ok(d) => d,
            Err(e) => {
                bus.send(Msg::Toast(format!("ChatGPT sign-in: {e}")));
                return;
            }
        };
        clipboard::write_text(&d.user_code);
        bus.send(Msg::ChatGptDeviceCode(d.user_code.clone()));
        let _ = std::process::Command::new("/usr/bin/open").arg(chatgpt::DEVICE_VERIFICATION_URL).status();
        let deadline = std::time::Instant::now() + Duration::from_secs(15 * 60);
        let mut wait = Duration::from_secs(d.interval_secs);
        let ready = loop {
            std::thread::sleep(wait);
            if std::time::Instant::now() > deadline {
                break Err("the code expired before it was approved".to_string());
            }
            match json_post(chatgpt::DEVICE_TOKEN_URL, &chatgpt::device_poll_body(&d)).map(|(s, b)| chatgpt::parse_device_poll(s, &b)) {
                Ok(DevicePoll::Pending) => {}
                Ok(DevicePoll::SlowDown) => wait += Duration::from_secs(5),
                Ok(DevicePoll::Ready { code, verifier }) => break Ok((code, verifier)),
                Ok(DevicePoll::Failed(e)) => break Err(e),
                Err(e) => break Err(e),
            }
        };
        let msg = match ready
            .and_then(|(code, ver)| token_post(&chatgpt::device_exchange_body(&code, &ver)))
            .and_then(|b| chatgpt::parse_token_response(&b, now_secs()))
        {
            Ok(c) => {
                if save_chatgpt_credentials(Some(&c)) {
                    "Signed in with ChatGPT".to_string()
                } else {
                    "Signed in, but the Keychain refused to store the session".to_string()
                }
            }
            Err(e) => format!("ChatGPT sign-in failed: {e}"),
        };
        bus.send(Msg::Toast(msg));
    });
}

/// Credentials with a valid access token, refreshing and re-saving when expired.
pub fn chatgpt_fresh_credentials() -> Result<ferrocast_core::chatgpt::Credentials, String> {
    use ferrocast_core::chatgpt;
    let c = chatgpt_credentials().ok_or("Not signed in with ChatGPT. Open Settings → AI → ChatGPT and sign in.")?;
    if !c.is_expired(now_secs()) {
        return Ok(c);
    }
    let body = token_post(&chatgpt::refresh_body(&c.refresh))?;
    let fresh = chatgpt::parse_token_response(&body, now_secs())
        .map_err(|e| format!("ChatGPT session expired and could not be refreshed ({e}). Sign in again in Settings → AI."))?;
    save_chatgpt_credentials(Some(&fresh));
    Ok(fresh)
}

/// Browser sign-in: open the authorization page, catch the redirect on 127.0.0.1:1455, exchange
/// the code, store the credentials. Reports progress and the outcome as toasts.
pub fn chatgpt_sign_in(bus: Bus) {
    use ferrocast_core::chatgpt;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    std::thread::spawn(move || {
        let (verifier, challenge) = chatgpt::pkce();
        let state = chatgpt::random_state();
        let listener = match TcpListener::bind(("127.0.0.1", chatgpt::CALLBACK_PORT)) {
            Ok(l) => l,
            Err(e) => {
                bus.send(Msg::Toast(format!(
                    "Port {} is busy ({e}). Quit Codex CLI or Pi if they are signing in, then retry.",
                    chatgpt::CALLBACK_PORT
                )));
                return;
            }
        };
        let _ = listener.set_nonblocking(true);
        let url = chatgpt::authorize_url(&challenge, &state);
        let _ = std::process::Command::new("/usr/bin/open").arg(&url).status();
        bus.send(Msg::Toast("Finish signing in to ChatGPT in your browser…".into()));
        let deadline = std::time::Instant::now() + Duration::from_secs(5 * 60);
        let code = loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let first = req.lines().next().unwrap_or("").to_string();
                    match chatgpt::parse_callback(&first) {
                        Some((code, st)) if st.as_deref() == Some(state.as_str()) => {
                            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><meta charset=utf-8><title>Ferrocast</title><body style=\"font-family:-apple-system,system-ui;background:#0f0f11;color:#f4f4f6;display:grid;place-items:center;height:100vh;margin:0\"><div style=\"text-align:center\"><h1 style=\"font-weight:300\">Signed in to ChatGPT</h1><p>You can close this tab and go back to Ferrocast.</p></div></body>");
                            break Some(code);
                        }
                        Some(_) => {
                            let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\nState mismatch");
                            break None;
                        }
                        None => {
                            // favicon and friends
                            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() > deadline {
                        break None;
                    }
                    std::thread::sleep(Duration::from_millis(150));
                }
                Err(_) => break None,
            }
        };
        let Some(code) = code else {
            bus.send(Msg::Toast("ChatGPT sign-in was not completed.".into()));
            return;
        };
        let msg = match token_post(&chatgpt::exchange_body(&code, &verifier)).and_then(|b| chatgpt::parse_token_response(&b, now_secs())) {
            Ok(c) => {
                if save_chatgpt_credentials(Some(&c)) {
                    "Signed in with ChatGPT".to_string()
                } else {
                    "Signed in, but the Keychain refused to store the session".to_string()
                }
            }
            Err(e) => format!("ChatGPT sign-in failed: {e}"),
        };
        bus.send(Msg::Toast(msg));
    });
}

/// A long-lived `claude -p --input-format stream-json` process. Starting the CLI costs several
/// seconds; keeping it alive turns every later question into model latency only, and the CLI
/// keeps the conversation context itself.
pub struct ClaudeSession {
    stdin: Mutex<Option<std::process::ChildStdin>>,
    child: Mutex<std::process::Child>,
    /// Run id of the question being answered, read by the output thread.
    current: Arc<Mutex<Option<u64>>>,
}

impl ClaudeSession {
    /// Spawn the CLI and the thread that forwards its stream to the bus. Returns quickly; the
    /// process boots in the background.
    pub fn start(settings: &Settings, bus: Bus) -> Result<Arc<ClaudeSession>, String> {
        use ferrocast_core::ai::{self, Sse};
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        let exe =
            claude_code_path().ok_or("Claude Code is not installed. Install it from claude.ai/code and run `claude` once to sign in.")?;
        let mut cmd = Command::new(exe);
        cmd.args([
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--include-partial-messages",
            "--verbose",
            "--no-session-persistence",
            "--strict-mcp-config",
            "--system-prompt",
            ai::SYSTEM_PROMPT,
            "--tools",
            "",
        ]);
        if !settings.anthropic_model.trim().is_empty() {
            cmd.args(["--model", settings.anthropic_model.trim()]);
        }
        let mut child = cmd
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Could not start Claude Code: {e}"))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let current: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));
        let cur = current.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let run = *cur.lock().unwrap();
                match ai::parse_claude_code_line(&line) {
                    Sse::Delta(t) => {
                        if let Some(run) = run {
                            bus.send(Msg::AiChunk { run, text: t });
                        }
                    }
                    Sse::Done => {
                        if let Some(run) = cur.lock().unwrap().take() {
                            bus.send(Msg::AiDone { run, error: None });
                        }
                    }
                    Sse::Error(e) => {
                        if let Some(run) = cur.lock().unwrap().take() {
                            bus.send(Msg::AiDone { run, error: Some(e) });
                        }
                    }
                    Sse::Ignore => {}
                }
            }
            // The process ended: fail whatever was in flight so the UI does not spin forever.
            if let Some(run) = cur.lock().unwrap().take() {
                bus.send(Msg::AiDone { run, error: Some("Claude Code exited. Ask again to start a new session.".into()) });
            }
        });
        Ok(Arc::new(ClaudeSession { stdin: Mutex::new(stdin), child: Mutex::new(child), current }))
    }

    /// Send one user turn; the answer streams back through the bus under `run`.
    pub fn ask(&self, run: u64, question: &str) -> Result<(), String> {
        use std::io::Write;
        *self.current.lock().unwrap() = Some(run);
        let msg = serde_json::json!({"type": "user", "message": {"role": "user", "content": question}}).to_string() + "\n";
        let mut g = self.stdin.lock().unwrap();
        let w = g.as_mut().ok_or("Claude Code session is closed")?;
        w.write_all(msg.as_bytes()).and_then(|_| w.flush()).map_err(|e| format!("Could not talk to Claude Code: {e}"))
    }

    pub fn is_alive(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(None))
    }

    pub fn kill(&self) {
        *self.stdin.lock().unwrap() = None; // closing stdin lets the CLI exit cleanly
        let _ = self.child.lock().unwrap().kill();
    }
}

/// Path of the Claude Code CLI, if installed.
pub fn claude_code_path() -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    [
        home.join(".local/bin/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
        home.join(".claude/local/claude"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

/// Cache key for a provider's model list (the server address matters for OpenAI-compatible).
pub fn models_key(settings: &Settings) -> String {
    match settings.ai_provider {
        AiProvider::OpenAiCompatible => format!("openai|{}", settings.ai_base_url.trim().trim_end_matches('/')),
        p => format!("{p:?}"),
    }
}

/// Models the ChatGPT (Codex) backend serves; the same catalogue Pi ships for this provider.
pub const CHATGPT_MODELS: &[&str] = &[
    "gpt-5.3-codex-spark",
    "gpt-5.5",
    "gpt-5.6-luna",
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-6-astra",
    "gpt-6-luna",
    "gpt-6-sol",
    "gpt-6.1-sol",
];
/// Claude Code model aliases.
pub const CLAUDE_CODE_MODELS: &[&str] = &["sonnet", "opus", "haiku"];

/// Fetch the model list for the configured provider and post it to the bus.
pub fn fetch_models(settings: &Settings, bus: Bus) {
    use ferrocast_core::ai;
    use std::process::{Command, Stdio};
    let key = models_key(settings);
    let provider = settings.ai_provider;
    let base = settings.ai_base_url.trim().trim_end_matches('/').to_string();
    std::thread::spawn(move || {
        let result: Result<Vec<String>, String> = match provider {
            AiProvider::ChatGpt => Ok(CHATGPT_MODELS.iter().map(|s| s.to_string()).collect()),
            AiProvider::ClaudeCode => Ok(CLAUDE_CODE_MODELS.iter().map(|s| s.to_string()).collect()),
            _ => {
                let (url, key_header) = match provider {
                    AiProvider::Anthropic => {
                        (ai::ANTHROPIC_MODELS_URL.to_string(), ai_key_for(AiProvider::Anthropic).map(|k| format!("x-api-key: {k}")))
                    }
                    _ => (format!("{base}/models"), ai_key().map(|k| format!("Authorization: Bearer {k}"))),
                };
                let mut cfg = format!("url = {}\n", ai::curl_config_quote(&url));
                if provider == AiProvider::Anthropic {
                    cfg.push_str("header = \"anthropic-version: 2023-06-01\"\n");
                }
                if let Some(h) = key_header {
                    cfg.push_str(&format!("header = {}\n", ai::curl_config_quote(&h)));
                }
                let out = Command::new("/usr/bin/curl")
                    .args(["-sS", "-m", "8", "-K", "-"])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .and_then(|mut c| {
                        use std::io::Write;
                        if let Some(mut i) = c.stdin.take() {
                            let _ = i.write_all(cfg.as_bytes());
                        }
                        c.wait_with_output()
                    });
                match out {
                    Ok(o) if o.status.success() => match serde_json::from_slice::<serde_json::Value>(&o.stdout) {
                        Ok(v) if v.get("error").is_some() => {
                            Err(v.pointer("/error/message").and_then(|m| m.as_str()).unwrap_or("server error").to_string())
                        }
                        Ok(v) => {
                            let mut m: Vec<String> = v["data"]
                                .as_array()
                                .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
                                .unwrap_or_default();
                            m.sort();
                            Ok(m)
                        }
                        Err(_) => Err("not a model list".into()),
                    },
                    Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("could not connect").trim().to_string()),
                    Err(e) => Err(e.to_string()),
                }
            }
        };
        bus.send(Msg::AiModels(key, result));
    });
}

/// Check the configured provider: list models (OpenAI-style or Anthropic) or run `claude --version`.
pub fn test_ai_connection(settings: &Settings, bus: Bus) {
    use ferrocast_core::ai;
    use std::process::{Command, Stdio};
    let provider = settings.ai_provider;
    let base = settings.ai_base_url.trim().trim_end_matches('/').to_string();
    std::thread::spawn(move || {
        let msg = match provider {
            AiProvider::ChatGpt => match chatgpt_fresh_credentials() {
                Ok(c) => format!("Signed in with ChatGPT · token valid for {} min", ((c.expires - now_secs()) / 60).max(0)),
                Err(e) => e,
            },
            AiProvider::ClaudeCode => match claude_code_path() {
                None => "Claude Code not found. Install it from claude.ai/code and run `claude` once to sign in.".to_string(),
                Some(p) => match Command::new(&p).arg("--version").output() {
                    Ok(o) if o.status.success() => format!("Claude Code ready · {}", String::from_utf8_lossy(&o.stdout).trim()),
                    Ok(o) => format!("Claude Code failed: {}", String::from_utf8_lossy(&o.stderr).trim()),
                    Err(e) => format!("Could not run Claude Code: {e}"),
                },
            },
            _ => {
                let (url, key_header) = match provider {
                    AiProvider::Anthropic => {
                        (ai::ANTHROPIC_MODELS_URL.to_string(), ai_key_for(AiProvider::Anthropic).map(|k| format!("x-api-key: {k}")))
                    }
                    _ => (format!("{base}/models"), ai_key().map(|k| format!("Authorization: Bearer {k}"))),
                };
                let mut cfg = format!("url = {}\n", ai::curl_config_quote(&url));
                if provider == AiProvider::Anthropic {
                    cfg.push_str("header = \"anthropic-version: 2023-06-01\"\n");
                }
                if let Some(h) = key_header {
                    cfg.push_str(&format!("header = {}\n", ai::curl_config_quote(&h)));
                }
                let out = Command::new("/usr/bin/curl")
                    .args(["-sS", "-m", "8", "-K", "-"])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .and_then(|mut c| {
                        use std::io::Write;
                        if let Some(mut i) = c.stdin.take() {
                            let _ = i.write_all(cfg.as_bytes());
                        }
                        c.wait_with_output()
                    });
                match out {
                    Ok(o) if o.status.success() => match serde_json::from_slice::<serde_json::Value>(&o.stdout) {
                        Ok(v) if v.get("error").is_some() => {
                            format!("Server error: {}", v.pointer("/error/message").and_then(|m| m.as_str()).unwrap_or("unknown"))
                        }
                        Ok(v) => {
                            let models: Vec<String> = v["data"]
                                .as_array()
                                .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
                                .unwrap_or_default();
                            match models.len() {
                                0 => "Connected, but the server lists no models".to_string(),
                                1 => format!("Connected · 1 model: {}", models[0]),
                                n => format!("Connected · {n} models, e.g. {}", models[0]),
                            }
                        }
                        Err(_) => format!("Server answered, but not with a model list ({} bytes)", o.stdout.len()),
                    },
                    Ok(o) => format!(
                        "Could not connect: {}",
                        String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("unknown error").trim()
                    ),
                    Err(e) => format!("Could not start curl: {e}"),
                }
            }
        };
        bus.send(Msg::Toast(msg));
    });
}

/// Streaming answer from the configured provider; `on_delta` receives text as it arrives.
/// Requests (including keys) go to curl or Claude Code via stdin, never argv.
pub fn ai_stream(settings: &Settings, history: &[(String, String)], question: &str, on_delta: impl FnMut(&str)) -> Result<(), String> {
    let key = match settings.ai_provider {
        AiProvider::ClaudeCode | AiProvider::ChatGpt => None,
        p => ai_key_for(p),
    };
    ai_stream_with_key(settings, key, history, question, on_delta)
}

/// `ai_stream` with the API key supplied by the caller (tests use a mock server and a fixed key).
pub fn ai_stream_with_key(
    settings: &Settings,
    key: Option<String>,
    history: &[(String, String)],
    question: &str,
    mut on_delta: impl FnMut(&str),
) -> Result<(), String> {
    use ferrocast_core::ai::{self, Sse};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::process::{Command, Stdio};
    let provider = settings.ai_provider;
    let (mut child, stdin_payload, parse): (std::process::Child, String, fn(&str) -> Sse) = match provider {
        AiProvider::ClaudeCode => {
            let exe = claude_code_path()
                .ok_or("Claude Code is not installed. Install it from claude.ai/code and run `claude` once to sign in.")?;
            let mut cmd = Command::new(exe);
            cmd.args([
                "-p",
                "--output-format",
                "stream-json",
                "--include-partial-messages",
                "--verbose",
                "--no-session-persistence",
                "--strict-mcp-config",
                "--system-prompt",
                ai::SYSTEM_PROMPT,
                "--tools",
                "",
            ]);
            if !settings.anthropic_model.trim().is_empty() {
                cmd.args(["--model", settings.anthropic_model.trim()]);
            }
            let child = cmd
                .current_dir(std::env::temp_dir())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Could not start Claude Code: {e}"))?;
            (child, ai::transcript_prompt(history, question), ai::parse_claude_code_line)
        }
        AiProvider::ChatGpt => {
            use ferrocast_core::chatgpt;
            let creds = chatgpt_fresh_credentials()?;
            let model = if settings.chatgpt_model.trim().is_empty() { chatgpt::DEFAULT_MODEL } else { settings.chatgpt_model.trim() };
            let cfg = chatgpt::curl_config(&creds, &chatgpt::build_request(model, history, question));
            let child = Command::new("/usr/bin/curl")
                .args(["-sS", "-N", "-m", "300", "-K", "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Could not start curl: {e}"))?;
            (child, cfg, chatgpt::parse_sse_line)
        }
        AiProvider::Anthropic => {
            let key = key.clone().ok_or("No Anthropic API key yet. Add one in Settings → AI.")?;
            let model =
                if settings.anthropic_model.trim().is_empty() { ai::DEFAULT_ANTHROPIC_MODEL } else { settings.anthropic_model.trim() };
            let cfg = ai::anthropic_curl_config(&key, &ai::build_anthropic_request(model, history, question));
            let child = Command::new("/usr/bin/curl")
                .args(["-sS", "-N", "-m", "300", "-K", "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Could not start curl: {e}"))?;
            (child, cfg, ai::parse_anthropic_sse_line)
        }
        AiProvider::OpenAiCompatible => {
            let url = ai::endpoint(&settings.ai_base_url);
            let cfg = ai::curl_config(&url, key.as_deref(), &ai::build_chat_request(&settings.ai_model, history, question));
            let child = Command::new("/usr/bin/curl")
                .args(["-sS", "-N", "-m", "300", "-K", "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Could not start curl: {e}"))?;
            (child, cfg, ai::parse_sse_line)
        }
    };
    child.stdin.take().ok_or("no stdin")?.write_all(stdin_payload.as_bytes()).map_err(|e| e.to_string())?;
    let mut got = false;
    let mut raw = String::new();
    for line in BufReader::new(child.stdout.take().ok_or("no stdout")?).lines().map_while(Result::ok) {
        match parse(&line) {
            Sse::Delta(t) => {
                got = true;
                on_delta(&t);
            }
            Sse::Done => break,
            Sse::Error(e) => {
                let _ = child.kill();
                return Err(e);
            }
            Sse::Ignore => {
                if provider != AiProvider::ClaudeCode && !line.starts_with("data:") && !line.starts_with(':') && !line.starts_with("event:")
                {
                    raw.push_str(&line);
                    raw.push('\n');
                }
            }
        }
    }
    let _ = child.wait();
    if got {
        return Ok(());
    }
    // No stream deltas: a plain JSON answer or error body, or a connection failure.
    if provider == AiProvider::OpenAiCompatible && !raw.trim().is_empty() {
        return ai::parse_response(&raw).map(|t| on_delta(&t));
    }
    if matches!(provider, AiProvider::Anthropic | AiProvider::ChatGpt) && !raw.trim().is_empty() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(m) = v.pointer("/error/message").or_else(|| v.get("detail")).and_then(|m| m.as_str()) {
                return Err(m.to_string());
            }
        }
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    Err(if err.contains("Failed to connect") || err.contains("Connection refused") {
        format!("Cannot reach {}. Is your AI server (e.g. Ollama) running? Change it in Settings.", settings.ai_base_url)
    } else if err.trim().is_empty() {
        "The AI server returned an empty answer".to_string()
    } else {
        err.trim().lines().last().unwrap_or("").to_string()
    })
}

pub fn ask_ai(settings: &Settings, history: Vec<(String, String)>, question: String, run: u64, bus: Bus) {
    let settings = settings.clone();
    std::thread::spawn(move || {
        let b = bus.clone();
        let result = ai_stream(&settings, &history, &question, |t| b.send(Msg::AiChunk { run, text: t.to_string() }));
        bus.send(Msg::AiDone { run, error: result.err() });
    });
}

/// Enable/disable launching at login (per-user LaunchAgent; takes effect at next login).
pub fn set_launch_at_login(enabled: bool) -> Result<(), String> {
    let dir = dirs::home_dir().ok_or("no home directory")?.join("Library/LaunchAgents");
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    ferrocast_core::launch::set_enabled(&dir, &ferrocast_core::launch::stable_exe_path(&exe.to_string_lossy()), enabled)
        .map_err(|e| e.to_string())
}

/// Archive user data into `dest_dir` as a timestamped `.tar.gz`. Clipboard history, the lock and the
/// log are excluded on purpose: backups are meant to be shared and must not leak copied secrets.
pub fn export_backup(data_root: &std::path::Path, dest_dir: &std::path::Path, stamp: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let out = dest_dir.join(format!("Ferrocast-backup-{stamp}.tar.gz"));
    let parent = data_root.parent().ok_or("invalid data directory")?;
    let name = data_root.file_name().ok_or("invalid data directory")?;
    let st = std::process::Command::new("/usr/bin/tar")
        .arg("-czf")
        .arg(&out)
        .args(["--exclude", "clipboard.json", "--exclude", "clipboard-images", "--exclude", "ferrocast.lock", "--exclude", "ferrocast.log"])
        .arg("-C")
        .arg(parent)
        .arg(name)
        .status()
        .map_err(|e| e.to_string())?;
    st.success().then_some(out).ok_or_else(|| "tar failed".to_string())
}

/// Convert every file to `target` in a background thread; one toast at the end, first output
/// revealed in Finder. Failures are reported with the tool's last stderr line.
pub fn convert_files(files: Vec<PathBuf>, target: ferrocast_core::convert::Target, tools: ferrocast_core::convert::Tools, bus: Bus) {
    std::thread::spawn(move || {
        let (outputs, errors) = run_conversion(&files, &target, &tools);
        let msg = match (outputs.len(), errors.len()) {
            (0, _) => format!("Conversion failed: {}", errors.first().cloned().unwrap_or_default()),
            (n, 0) => format!("Converted {n} file{} to {}", if n == 1 { "" } else { "s" }, target.label),
            (n, e) => format!("Converted {n}, {e} failed: {}", errors[0]),
        };
        if let Some(first) = outputs.first() {
            let _ = ferrocast_macos::apps::reveal(first);
        }
        notify("Ferrocast", &msg);
        bus.send(Msg::Toast(msg));
    });
}

/// Run the planned conversion for each file, synchronously. Returns the files written and
/// one message per failure. No UI side effects, so it can be tested.
pub fn run_conversion(
    files: &[PathBuf],
    target: &ferrocast_core::convert::Target,
    tools: &ferrocast_core::convert::Tools,
) -> (Vec<PathBuf>, Vec<String>) {
    use ferrocast_core::convert::{output_path, plan, soffice_written_path, Engine};
    {
        let mut outputs: Vec<PathBuf> = vec![];
        let mut errors: Vec<String> = vec![];
        for f in files {
            let out = output_path(f, target.ext, |p| p.exists());
            // cwebp reads png/jpeg/tiff/webp only: other images go through a sips PNG
            let mut pivot_png: Option<PathBuf> = None;
            let f: &std::path::PathBuf = if target.engine == Engine::Cwebp
                && !matches!(ferrocast_core::convert::ext(f).as_str(), "png" | "jpg" | "jpeg" | "tif" | "tiff" | "webp")
            {
                static PIVOT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let tmp = std::env::temp_dir().join(format!(
                    "ferrocast-cw-{}-{}.png",
                    std::process::id(),
                    PIVOT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                match sips(&["-s", "format", "png", &f.to_string_lossy(), "--out", &tmp.to_string_lossy()]) {
                    Ok(()) => {
                        pivot_png = Some(tmp);
                        pivot_png.as_ref().unwrap_or(f)
                    }
                    Err(e) => {
                        errors.push(format!("{}: {e}", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                        continue;
                    }
                }
            } else {
                f
            };
            if target.engine == Engine::Route {
                match crate::route::route_convert(f, target, tools) {
                    Ok(mut o) => outputs.append(&mut o),
                    Err(e) => errors.push(format!("{}: {e}", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
                }
                continue;
            }
            if matches!(target.engine, Engine::SubtitleText | Engine::SlideImages | Engine::Unrar) {
                match special_convert(f, target, tools) {
                    Ok(mut o) => outputs.append(&mut o),
                    Err(e) => errors.push(format!("{}: {e}", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
                }
                continue;
            }
            if target.engine == Engine::Pdfkit {
                match pdfkit_convert(f, target.ext) {
                    Ok(mut o) => outputs.append(&mut o),
                    Err(e) => errors.push(format!("{}: {e}", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
                }
                continue;
            }
            let mut cmd = match plan(f, target, &out, tools) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(e);
                    if let Some(p) = &pivot_png {
                        let _ = std::fs::remove_file(p);
                    }
                    continue;
                }
            };
            // soffice names its own output <stem>.<ext> in --outdir; next to the original that
            // could overwrite a different file. Work in a scratch folder, then move to `out`.
            let scratch = (target.engine == Engine::Soffice).then(|| {
                let d = std::env::temp_dir().join(format!(
                    "ferrocast-so-{}-{}",
                    std::process::id(),
                    out.file_name().map(|n| n.to_string_lossy().len()).unwrap_or(0)
                ));
                let _ = std::fs::remove_dir_all(&d);
                let _ = std::fs::create_dir_all(&d);
                if let Some(i) = cmd.iter().position(|a| a == "--outdir") {
                    cmd[i + 1] = d.to_string_lossy().to_string();
                }
                d
            });
            let res = std::process::Command::new(&cmd[0]).args(&cmd[1..]).output();
            match res {
                Ok(o) if o.status.success() => {
                    if let Some(d) = &scratch {
                        let written = soffice_written_path(f, &d.join("x"), target.ext);
                        if written.exists() && std::fs::rename(&written, &out).is_err() {
                            // another volume: copy, then drop the scratch copy
                            let _ = std::fs::copy(&written, &out);
                        }
                    }
                    if out.exists() {
                        outputs.push(out);
                    } else {
                        errors.push(format!(
                            "{}: no output produced",
                            f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                        ));
                    }
                }
                Ok(o) => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    let last = err.lines().last().unwrap_or("conversion failed").trim();
                    // ffmpeg's "-vn" on a silent video leaves no stream to write
                    let msg = if target.engine == Engine::Ffmpeg
                        && matches!(target.ext, "mp3" | "m4a" | "wav" | "aiff" | "ogg" | "opus" | "wma" | "flac")
                        && last.contains("Invalid argument")
                    {
                        "this video has no audio track".to_string()
                    } else {
                        last.to_string()
                    };
                    errors.push(format!("{}: {msg}", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                }
                Err(e) => errors.push(format!("could not run {}: {e}", cmd[0])),
            }
            if let Some(d) = scratch {
                let _ = std::fs::remove_dir_all(d);
            }
            if let Some(p) = pivot_png {
                let _ = std::fs::remove_file(p);
            }
        }
        (outputs, errors)
    }
}

/// Conversions that are not one command: subtitle text, slides → page images, RAR → ZIP.
fn special_convert(
    f: &std::path::Path,
    target: &ferrocast_core::convert::Target,
    tools: &ferrocast_core::convert::Tools,
) -> Result<Vec<PathBuf>, String> {
    use ferrocast_core::convert::{output_path, subtitle_text, Engine};
    let scratch = std::env::temp_dir().join(format!(
        "ferrocast-sp-{}-{}",
        std::process::id(),
        f.file_name().map(|n| n.to_string_lossy().len()).unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let res = (|| -> Result<Vec<PathBuf>, String> {
        match target.engine {
            Engine::SubtitleText => {
                let text = subtitle_text(&std::fs::read_to_string(f).map_err(|e| e.to_string())?);
                let out = output_path(f, "txt", |p| p.exists());
                std::fs::write(&out, text + "\n").map_err(|e| e.to_string())?;
                Ok(vec![out])
            }
            Engine::SlideImages => {
                let so = tools.soffice.ok_or("LibreOffice is not installed")?;
                let o = std::process::Command::new(so)
                    .args(["--headless", "--convert-to", "pdf", "--outdir"])
                    .arg(&scratch)
                    .arg(f)
                    .output()
                    .map_err(|e| e.to_string())?;
                let pdf = scratch.join(format!("{}.pdf", f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()));
                if !o.status.success() || !pdf.exists() {
                    return Err("LibreOffice could not render the slides".into());
                }
                // name the pages after the original deck, not the scratch PDF
                let named = scratch.join(f.file_name().ok_or("bad path")?).with_extension("pdf");
                if named != pdf {
                    std::fs::rename(&pdf, &named).map_err(|e| e.to_string())?;
                }
                let pages = pdfkit_convert(&named, target.ext)?;
                // pdfkit wrote next to the scratch PDF: move beside the original
                let mut outs = vec![];
                for p in pages {
                    let dest = f.parent().unwrap_or(std::path::Path::new(".")).join(p.file_name().ok_or("bad path")?);
                    let dest = if dest.exists() {
                        ferrocast_core::convert::output_with_suffix(&dest, "", target.ext, |q| q.exists())
                    } else {
                        dest
                    };
                    std::fs::rename(&p, &dest).or_else(|_| std::fs::copy(&p, &dest).map(|_| ())).map_err(|e| e.to_string())?;
                    outs.push(dest);
                }
                Ok(outs)
            }
            Engine::Unrar => {
                let un = tools.unrar.ok_or("unrar is not installed")?;
                let ex = scratch.join("x");
                std::fs::create_dir_all(&ex).map_err(|e| e.to_string())?;
                let o = std::process::Command::new(un)
                    .args(["x", "-y", "-idq"])
                    .arg(f)
                    .arg(format!("{}/", ex.display()))
                    .output()
                    .map_err(|e| e.to_string())?;
                if !o.status.success() {
                    return Err(String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .last()
                        .unwrap_or("could not extract the RAR")
                        .trim()
                        .to_string());
                }
                let out = output_path(f, "zip", |p| p.exists());
                let st = std::process::Command::new("/usr/bin/ditto")
                    .args(["-c", "-k", "--sequesterRsrc"])
                    .arg(&ex)
                    .arg(&out)
                    .status()
                    .map_err(|e| e.to_string())?;
                if st.success() && out.exists() {
                    Ok(vec![out])
                } else {
                    Err("could not write the ZIP".into())
                }
            }
            _ => Err("not a special conversion".into()),
        }
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    res
}

/// PDF → page images (300 DPI, every page) or its text layer, through PDFKit/AppKit.
pub(crate) fn pdfkit_convert(f: &std::path::Path, ext: &str) -> Result<Vec<PathBuf>, String> {
    use ferrocast_core::convert::{output_path, output_with_suffix};
    let tmp = std::env::temp_dir().join(format!("ferrocast-pdfkit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let run = |script: &str, args: &[&str]| {
        std::process::Command::new("/usr/bin/osascript")
            .args(["-l", "JavaScript", "-e", script])
            .args(args)
            .output()
            .map_err(|e| e.to_string())
    };
    let result = (|| -> Result<Vec<PathBuf>, String> {
        if ext == "txt" || ext == "md" {
            let o = run(
                "function run(a){ObjC.import('PDFKit');var d=$.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(a[0]));return d?ObjC.unwrap(d.string)||'':''}",
                &[&f.to_string_lossy()],
            )?;
            let text = String::from_utf8_lossy(&o.stdout).trim_end().to_string();
            if text.trim().is_empty() {
                return Err("no text layer in this PDF (it may be a scan)".into());
            }
            let out = output_path(f, ext, |p| p.exists());
            std::fs::write(&out, text + "\n").map_err(|e| e.to_string())?;
            return Ok(vec![out]);
        }
        const PAGES: &str = "function run(a){ObjC.import('AppKit');ObjC.import('Foundation');var rep=$.NSPDFImageRep.imageRepWithData($.NSData.dataWithContentsOfFile(a[0]));if(!rep){return '0'}var n=rep.pageCount;var s=300/72;for(var i=0;i<n;i++){rep.currentPage=i;var b=rep.bounds;var w=Math.round(b.size.width*s),h=Math.round(b.size.height*s);var bmp=$.NSBitmapImageRep.alloc.initWithBitmapDataPlanesPixelsWidePixelsHighBitsPerSampleSamplesPerPixelHasAlphaIsPlanarColorSpaceNameBytesPerRowBitsPerPixel($(),w,h,8,4,true,false,$('NSCalibratedRGBColorSpace'),0,0);var ctx=$.NSGraphicsContext.graphicsContextWithBitmapImageRep(bmp);$.NSGraphicsContext.saveGraphicsState;$.NSGraphicsContext.currentContext=ctx;$.NSColor.whiteColor.set;$.NSBezierPath.fillRect($.NSMakeRect(0,0,w,h));rep.drawInRect($.NSMakeRect(0,0,w,h));$.NSGraphicsContext.restoreGraphicsState;var t=a[2]=='jpg'?$.NSBitmapImageFileTypeJPEG:$.NSBitmapImageFileTypePNG;bmp.representationUsingTypeProperties(t,$()).writeToFileAtomically(a[1]+'/'+(i+1)+'.'+a[2],true)}return ''+n}";
        let o = run(PAGES, &[&f.to_string_lossy(), &tmp.to_string_lossy(), ext])?;
        let n: usize = String::from_utf8_lossy(&o.stdout).trim().parse().unwrap_or(0);
        if n == 0 {
            return Err("could not read the PDF pages".into());
        }
        let mut outs = vec![];
        for i in 1..=n {
            let out =
                if n == 1 { output_path(f, ext, |p| p.exists()) } else { output_with_suffix(f, &format!("page {i}"), ext, |p| p.exists()) };
            std::fs::copy(tmp.join(format!("{i}.{ext}")), &out).map_err(|e| e.to_string())?;
            outs.push(out);
        }
        Ok(outs)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Pixel size of an image via sips.
pub(crate) fn image_size(p: &std::path::Path) -> Option<(u32, u32)> {
    let o = std::process::Command::new("/usr/bin/sips").args(["-g", "pixelWidth", "-g", "pixelHeight"]).arg(p).output().ok()?;
    let t = String::from_utf8_lossy(&o.stdout);
    let get = |k: &str| t.lines().find_map(|l| l.trim().strip_prefix(k).map(|v| v.trim().parse::<u32>().ok())).flatten();
    Some((get("pixelWidth:")?, get("pixelHeight:")?))
}

/// sips under a hard deadline, with stderr drained so a chatty run cannot stall.
fn sips(args: &[&str]) -> Result<(), String> {
    crate::route::run_limited(std::process::Command::new("/usr/bin/sips").args(args), 120).map_err(|e| {
        if e.is_empty() {
            "sips failed".into()
        } else {
            e
        }
    })
}

/// PNG bytes of `f` scaled so the longest side is at most `max_px`, for showing on screen.
pub(crate) fn display_png(f: &std::path::Path, max_px: u32) -> Option<Vec<u8>> {
    let tmp = std::env::temp_dir().join(format!(
        "ferrocast-disp-{}-{}.png",
        std::process::id(),
        COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let ok = sips(&["-Z", &max_px.to_string(), "-s", "format", "png", &f.to_string_lossy(), "--out", &tmp.to_string_lossy()]).is_ok();
    let bytes = ok.then(|| std::fs::read(&tmp).ok()).flatten();
    let _ = std::fs::remove_file(&tmp);
    bytes
}

/// Draw `doc` on a full-size copy of the picture and save it as a PNG next to the original.
pub(crate) fn annotate_save(f: &std::path::Path, doc: &ferrocast_core::annotate::Doc) -> Result<PathBuf, String> {
    use ferrocast_core::convert::output_with_suffix;
    check_source_size(f)?;
    let tmp = std::env::temp_dir().join(format!(
        "ferrocast-annot-{}-{}.png",
        std::process::id(),
        COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let loaded = sips(&["-s", "format", "png", &f.to_string_lossy(), "--out", &tmp.to_string_lossy()])
        .and_then(|_| image::open(&tmp).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&tmp);
    let img = loaded?.to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut buf = img.into_raw();
    doc.render(&mut buf, w, h);
    let out_img = image::RgbaImage::from_raw(w as u32, h as u32, buf).ok_or("could not build the image")?;
    let out = output_with_suffix(f, "annotated", "png", |p| p.exists());
    out_img.save(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Refuse sources that would need more than ~120 million pixels (480 MB as RGBA) to edit.
fn check_source_size(f: &std::path::Path) -> Result<(), String> {
    match image_size(f) {
        Some((w, h)) if u64::from(w) * u64::from(h) > 120_000_000 => Err(format!("the image is too large to edit here ({w}×{h} px)")),
        _ => Ok(()),
    }
}

/// Run one image tool on each file (or once for Combine). Returns outputs and failures.
/// Detect faces or text with Vision (on-device) and cover them with black boxes.
/// Writes a PNG next to the original; errors when nothing was found.
fn redact(f: &std::path::Path, faces: bool) -> Result<PathBuf, String> {
    use ferrocast_core::redact::{RedactSpec, Source, Style};
    let source = if faces { Source::Faces } else { Source::Text };
    redact_with(f, &RedactSpec { source, style: Style::Solid([0, 0, 0]) })
}

fn redact_with(f: &std::path::Path, spec: &ferrocast_core::redact::RedactSpec) -> Result<PathBuf, String> {
    check_source_size(f)?;
    use ferrocast_core::convert::{output_with_suffix, redact_rect};
    use ferrocast_core::redact::{clamp_area, Area, Source, Style};
    let tmp = std::env::temp_dir().join(format!(
        "ferrocast-redact-{}-{}.png",
        std::process::id(),
        COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let loaded = sips(&["-s", "format", "png", &f.to_string_lossy(), "--out", &tmp.to_string_lossy()])
        .and_then(|_| image::open(&tmp).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&tmp);
    let mut img = loaded?.to_rgba8();
    let (iw, ih) = img.dimensions();
    let areas: Vec<Area> = match &spec.source {
        Source::Areas(a) => {
            let inside: Vec<Area> = a.iter().filter_map(|a| clamp_area(*a, iw, ih)).collect();
            if inside.is_empty() {
                return Err(format!("the areas lie outside the image ({iw}×{ih} px)"));
            }
            inside
        }
        src => {
            let faces = *src == Source::Faces;
            let regions = ferrocast_macos::vision::detect(f, faces, !faces).map_err(|e| e.to_string())?;
            if regions.is_empty() {
                return Err(if faces { "no faces found".into() } else { "no text found".into() });
            }
            regions
                .iter()
                .map(|r| {
                    let (x, y, w, h) = redact_rect(r.x, r.y, r.w, r.h, iw, ih, if faces { 0.15 } else { 0.05 });
                    Area { x, y, w, h }
                })
                .collect()
        }
    };
    let original = img.clone();
    for a in areas {
        let (x1, y1) = ((a.x + a.w).min(iw), (a.y + a.h).min(ih));
        match spec.style {
            Style::Solid(c) => {
                for py in a.y..y1 {
                    for px in a.x..x1 {
                        img.put_pixel(px, py, image::Rgba([c[0], c[1], c[2], 255]));
                    }
                }
            }
            Style::Blur(r) => {
                // blur the area itself plus a margin, so the edge does not fade to black
                let m = (r * 2.0) as u32;
                let (bx, by) = (a.x.saturating_sub(m), a.y.saturating_sub(m));
                let (bx1, by1) = ((x1 + m).min(iw), (y1 + m).min(ih));
                let crop = image::imageops::crop_imm(&original, bx, by, bx1 - bx, by1 - by).to_image();
                let blurred = image::imageops::blur(&crop, r);
                for py in a.y..y1 {
                    for px in a.x..x1 {
                        img.put_pixel(px, py, *blurred.get_pixel(px - bx, py - by));
                    }
                }
            }
        }
    }
    let out = output_with_suffix(f, "redacted", "png", |p| p.exists());
    img.save(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Trim, split or merge audio, video or PDF files. Outputs go next to the first input.
pub fn run_file_tool(files: &[PathBuf], tool: &ferrocast_core::convert::FileTool, ffmpeg: Option<&str>) -> (Vec<PathBuf>, Vec<String>) {
    use ferrocast_core::convert::{ext, output_with_suffix, FileTool};
    let mut outs = vec![];
    let mut errs = vec![];
    let Some(first) = files.first() else { return (outs, errs) };
    let e0 = ext(first);
    let is_pdf = e0 == "pdf";
    let tmp = std::env::temp_dir().join(format!("ferrocast-ft-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let need_ff = |errs: &mut Vec<String>| {
        if ffmpeg.is_none() {
            errs.push("ffmpeg is not installed (brew install ffmpeg)".into());
        }
        ffmpeg
    };
    match tool {
        FileTool::Mute
        | FileTool::Speed(_)
        | FileTool::Snapshot(_)
        | FileTool::CropVideo(..)
        | FileTool::RedactVideo(..)
        | FileTool::RedactVideoMany(..)
        | FileTool::Normalize(..)
        | FileTool::Visualize(_)
        | FileTool::VisualizeImage(..)
        | FileTool::TrimSilence
        | FileTool::TrimExact(..)
        | FileTool::SplitPoints(..)
        | FileTool::Channels(..)
        | FileTool::Bleep(..)
        | FileTool::Compress(..)
        | FileTool::Join => {
            return crate::media_tools::run(files, tool, ffmpeg);
        }
        FileTool::EditMetadata(edits, remove_all) => {
            for f in files {
                if ext(f) == "pdf" {
                    match crate::media_tools::edit_pdf_metadata(f, edits, *remove_all) {
                        Ok(p) => outs.push(p),
                        Err(e) => errs.push(format!("{}: {e}", f.display())),
                    }
                } else {
                    // audio, video and GIF need ffmpeg; the executor names the install hint
                    let (o, e) = crate::media_tools::run(std::slice::from_ref(f), tool, ffmpeg);
                    outs.extend(o);
                    errs.extend(e);
                }
            }
        }
        FileTool::CompressPdf(preset) => {
            let (o, e) = crate::route::compress_pdf(ferrocast_core::convert::Tools::detect().gs, files, *preset);
            outs.extend(o);
            errs.extend(e);
        }
        FileTool::Extract => {
            let tools = ferrocast_core::convert::Tools::detect();
            for f in files {
                match crate::route::extract_archive(f, &tools) {
                    Ok(dir) => outs.push(dir),
                    Err(e) => errs.push(e),
                }
            }
        }
        FileTool::OrganizePdf(sel) => {
            let pages: Vec<_> = sel.iter().map(|&(index, quarter_turns)| ferrocast_macos::pdf::PageSel { index, quarter_turns }).collect();
            for f in files.iter().filter(|f| ext(f) == "pdf") {
                let out = output_with_suffix(f, &tool.suffix(), "pdf", |p| p.exists());
                match ferrocast_macos::pdf::organize(f, &pages, &out) {
                    Ok(()) => outs.push(out),
                    Err(e) => errs.push(format!("{}: {e}", f.display())),
                }
            }
            if files.iter().all(|f| ext(f) != "pdf") {
                errs.push("this tool requires PDF files".into());
            }
        }
        FileTool::Merge if is_pdf => {
            let out = output_with_suffix(first, &tool.suffix(), "pdf", |p| p.exists());
            let join = "/System/Library/Automator/Combine PDF Pages.action/Contents/MacOS/join";
            let ok = std::process::Command::new(join).arg("-o").arg(&out).args(files).status().map(|s| s.success()).unwrap_or(false);
            if ok && out.exists() {
                outs.push(out);
            } else {
                errs.push("could not merge the PDFs".into());
            }
        }
        FileTool::SplitPages => {
            let js = "function run(a){ObjC.import('PDFKit');var d=$.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(a[0]));if(!d.js&&!d){return '0'}var n=d.pageCount;for(var i=0;i<n;i++){var o=$.PDFDocument.alloc.init;o.insertPageAtIndex(d.pageAtIndex(i),0);o.writeToFile(a[1]+'/'+(i+1)+'.pdf')}return ''+n}";
            let res = std::process::Command::new("/usr/bin/osascript").args(["-l", "JavaScript", "-e", js]).arg(first).arg(&tmp).output();
            let n: usize = res.ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok()).unwrap_or(0);
            if n == 0 {
                errs.push("could not read the PDF pages".into());
            }
            for i in 1..=n {
                let out = output_with_suffix(first, &format!("page {i}"), "pdf", |p| p.exists());
                if std::fs::copy(tmp.join(format!("{i}.pdf")), &out).is_ok() {
                    outs.push(out);
                }
            }
        }
        FileTool::Merge => {
            if let Some(ff) = need_ff(&mut errs) {
                let list: String = files.iter().map(|f| format!("file '{}'\n", f.to_string_lossy().replace('\'', "'\\''"))).collect();
                let lf = tmp.join("list.txt");
                let _ = std::fs::write(&lf, list);
                let out = output_with_suffix(first, &tool.suffix(), &e0, |p| p.exists());
                let r = std::process::Command::new(ff)
                    .args(["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i"])
                    .arg(&lf)
                    .args(["-c", "copy"])
                    .arg(&out)
                    .output();
                match r {
                    Ok(o) if o.status.success() && out.exists() => outs.push(out),
                    Ok(o) => errs.push(format!(
                        "merge failed (files need the same format): {}",
                        String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("")
                    )),
                    Err(e) => errs.push(e.to_string()),
                }
            }
        }
        FileTool::Trim(a, b) => {
            if let Some(ff) = need_ff(&mut errs) {
                for f in files {
                    let out = output_with_suffix(f, &tool.suffix(), &ext(f), |p| p.exists());
                    let r = std::process::Command::new(ff)
                        .args(["-y", "-loglevel", "error", "-ss", &a.to_string(), "-to", &b.to_string(), "-i"])
                        .arg(f)
                        .args(["-c", "copy"])
                        .arg(&out)
                        .output();
                    match r {
                        Ok(o) if o.status.success() && out.exists() => outs.push(out),
                        Ok(o) => errs.push(format!("{}: {}", f.display(), String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""))),
                        Err(e) => errs.push(e.to_string()),
                    }
                }
            }
        }
        FileTool::SplitEvery(n) => {
            if let Some(ff) = need_ff(&mut errs) {
                for f in files {
                    let pattern = tmp.join(format!("seg%03d.{}", ext(f)));
                    let r = std::process::Command::new(ff)
                        .args(["-y", "-loglevel", "error", "-i"])
                        .arg(f)
                        .args(["-f", "segment", "-segment_time", &n.to_string(), "-c", "copy", "-reset_timestamps", "1"])
                        .arg(&pattern)
                        .output();
                    if !matches!(&r, Ok(o) if o.status.success()) {
                        errs.push(format!("{}: could not split", f.display()));
                        continue;
                    }
                    let mut parts: Vec<_> = std::fs::read_dir(&tmp)
                        .map(|d| {
                            d.flatten()
                                .map(|e| e.path())
                                .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("seg")))
                                .collect()
                        })
                        .unwrap_or_default();
                    parts.sort();
                    for (i, p) in parts.iter().enumerate() {
                        let out = output_with_suffix(f, &format!("part {}", i + 1), &ext(f), |p| p.exists());
                        if std::fs::copy(p, &out).is_ok() {
                            outs.push(out);
                        }
                        let _ = std::fs::remove_file(p);
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    (outs, errs)
}

/// Result line for a PDF compression: both sizes, or the honest "no gain" note (the output is kept).
pub(crate) fn pdf_size_report(before: u64, after: u64) -> String {
    if after < before {
        format!("{before} B → {after} B")
    } else {
        "no smaller: the PDF was already compact".into()
    }
}

pub fn file_tool_files(files: Vec<PathBuf>, tool: ferrocast_core::convert::FileTool, ffmpeg: Option<&'static str>, bus: Bus) {
    std::thread::spawn(move || {
        let (outs, errs) = run_file_tool(&files, &tool, ffmpeg);
        // only when one file produced one result, so the pair is certain
        let report = match (&tool, ffmpeg, files.as_slice(), outs.as_slice()) {
            (ferrocast_core::convert::FileTool::Normalize(..), Some(ff), [b], [a]) => crate::media_tools::normalize_report(ff, b, a),
            (ferrocast_core::convert::FileTool::CompressPdf(_), _, [b], [a]) => {
                let len = |p: &PathBuf| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                Some(pdf_size_report(len(b), len(a)))
            }
            _ => None,
        };
        let msg = match (outs.len(), errs.len()) {
            (0, _) => format!("{} failed: {}", tool.label(), errs.first().cloned().unwrap_or_default()),
            (n, 0) => format!(
                "{} · {n} file{} written{}",
                tool.label(),
                if n == 1 { "" } else { "s" },
                report.as_ref().map(|r| format!(" · {r}")).unwrap_or_default()
            ),
            (n, e) => format!("{} · {n} written, {e} failed: {}", tool.label(), errs[0]),
        };
        if let Some(f) = outs.first() {
            let _ = ferrocast_macos::apps::reveal(f);
        }
        notify("Ferrocast", &msg);
        bus.send(Msg::Toast(msg));
    });
}

/// JPEG at the highest quality that meets a size target (KB, or percent smaller than the source).
fn compress_to_size(f: &std::path::Path, op: &ferrocast_core::convert::ImageOp) -> Result<PathBuf, String> {
    use ferrocast_core::convert::ImageOp;
    let original = std::fs::metadata(f).map_err(|e| e.to_string())?.len();
    let target = match op {
        ImageOp::CompressToKb(kb) => *kb as u64 * 1024,
        ImageOp::CompressByPercent(p) => original * (100 - *p as u64) / 100,
        _ => return Err("not a size target".into()),
    };
    fit_jpeg(f, f, target, &op.suffix())
}

/// Binary search over JPEG quality of `input` for the best one that fits `target` bytes; the copy is
/// written next to `named_after` (never overwriting) as `<name> <suffix>.jpg`.
fn fit_jpeg(input: &std::path::Path, named_after: &std::path::Path, target: u64, suffix: &str) -> Result<PathBuf, String> {
    use ferrocast_core::convert::{fit_quality, output_with_suffix};
    let tmp = std::env::temp_dir().join(format!(
        "ferrocast-fit-{}-{}.jpg",
        std::process::id(),
        COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let encode = |q: u8| -> Option<u64> {
        sips(&["-s", "format", "jpeg", "-s", "formatOptions", &q.to_string(), &input.to_string_lossy(), "--out", &tmp.to_string_lossy()])
            .ok()?;
        std::fs::metadata(&tmp).ok().map(|m| m.len())
    };
    let q = fit_quality(target, encode);
    let out = output_with_suffix(named_after, suffix, "jpg", |p| p.exists());
    let res =
        sips(&["-s", "format", "jpeg", "-s", "formatOptions", &q.to_string(), &input.to_string_lossy(), "--out", &out.to_string_lossy()]);
    let _ = std::fs::remove_file(&tmp);
    res?;
    let size = std::fs::metadata(&out).map_err(|e| e.to_string())?.len();
    if size > target {
        // lowest quality still too big: keep the file but say so
        return Err(format!("smallest result is {} KB, above the target; try resizing too", size / 1024));
    }
    Ok(out)
}

/// Compress window executor: shrink first when asked (aspect kept, never upscaled), then encode a JPEG
/// for the goal. The original is only read; the copy never overwrites anything.
fn compress_with(
    f: &std::path::Path,
    goal: ferrocast_core::compress::Goal,
    resize: &ferrocast_core::compress::Resize,
    suffix: &str,
) -> Result<PathBuf, String> {
    use ferrocast_core::compress::{estimate_bytes, resized_dims, Goal, Preset};
    use ferrocast_core::convert::output_with_suffix;
    check_source_size(f)?;
    let original = std::fs::metadata(f).map_err(|e| e.to_string())?.len();
    let wants_resize = resize.max_edge.is_some() || resize.percent.is_some_and(|p| p < 100);
    let mut shrunk: Option<PathBuf> = None;
    if wants_resize {
        let (w, h) = image_size(f).ok_or("could not read the image size")?;
        let smaller = resize.max_edge.is_some_and(|e| e < w.max(h)) || resize.percent.is_some_and(|p| p < 100);
        if smaller {
            let (nw, nh) = resized_dims(w, h, resize).ok_or("invalid resize")?;
            if (nw, nh) != (w, h) {
                let tmp = std::env::temp_dir().join(format!(
                    "ferrocast-cwr-{}-{}.png",
                    std::process::id(),
                    COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                let r = sips(&[
                    "-z",
                    &nh.to_string(),
                    &nw.to_string(),
                    "-s",
                    "format",
                    "png",
                    &f.to_string_lossy(),
                    "--out",
                    &tmp.to_string_lossy(),
                ]);
                if let Err(e) = r {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e);
                }
                shrunk = Some(tmp);
            }
        }
    }
    let input = shrunk.as_deref().unwrap_or(f);
    let res = match goal {
        Goal::TargetBytes(t) => fit_jpeg(input, f, t, suffix),
        // a percentage is relative to the original file, not to the resized copy
        Goal::Percent(p) => fit_jpeg(input, f, estimate_bytes(original, Goal::Percent(p)), suffix),
        Goal::Preset(p) => {
            let q = match p {
                Preset::Light => 85,
                Preset::Balanced => 65,
                Preset::Strong => 45,
            };
            let out = output_with_suffix(f, suffix, "jpg", |p| p.exists());
            sips(&[
                "-s",
                "format",
                "jpeg",
                "-s",
                "formatOptions",
                &q.to_string(),
                &input.to_string_lossy(),
                "--out",
                &out.to_string_lossy(),
            ])
            .map(|_| out)
        }
    };
    if let Some(t) = shrunk {
        let _ = std::fs::remove_file(t);
    }
    res
}

/// Apply a one-step adjustment; writes a PNG next to the original.
fn adjust(f: &std::path::Path, a: ferrocast_core::convert::Adjust) -> Result<PathBuf, String> {
    use ferrocast_core::convert::{output_with_suffix, Adjust};
    let tmp = std::env::temp_dir().join(format!("ferrocast-adjust-{}.png", std::process::id()));
    sips(&["-s", "format", "png", &f.to_string_lossy(), "--out", &tmp.to_string_lossy()])?;
    let img = image::open(&tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    let img = match a {
        Adjust::BlackAndWhite => image::DynamicImage::ImageLumaA8(img.to_luma_alpha8()),
        Adjust::Brighter => img.brighten(30),
        Adjust::Darker => img.brighten(-30),
        Adjust::MoreContrast => img.adjust_contrast(20.0),
    };
    let out = output_with_suffix(f, "edited", "png", |p| p.exists());
    img.save(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn edit_photo(f: &std::path::Path, o: &ferrocast_core::photo::PhotoOpts) -> Result<PathBuf, String> {
    check_source_size(f)?;
    use ferrocast_core::convert::output_with_suffix;
    let tmp = std::env::temp_dir().join(format!(
        "ferrocast-edit-{}-{}.png",
        std::process::id(),
        COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let loaded = sips(&["-s", "format", "png", &f.to_string_lossy(), "--out", &tmp.to_string_lossy()])
        .and_then(|_| image::open(&tmp).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&tmp);
    let mut img = loaded?.to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    ferrocast_core::photo::apply(&mut img, w, h, o);
    let out = output_with_suffix(f, "edited", "png", |p| p.exists());
    img.save(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Lay the images out in a grid of 600 px cells on white, as one PNG next to the first.
static COLLAGE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Lay decoded pictures out per `o` (each fills its cell, centred, with rounded corners). Pure
/// pixel work so the editor can preview it on small copies.
pub(crate) fn collage_compose(images: &[image::RgbaImage], o: &ferrocast_core::collage::CollageOpts) -> image::RgbaImage {
    use ferrocast_core::collage::{corner_alpha, layout};
    let ((cw, ch), cells) = layout(images.len(), o);
    let bg = o.bg.map(|c| image::Rgba([c[0], c[1], c[2], 255])).unwrap_or(image::Rgba([0, 0, 0, 0]));
    let mut canvas = image::RgbaImage::from_pixel(cw.max(1), ch.max(1), bg);
    for (img, cell) in images.iter().zip(&cells) {
        // fill the cell (cover), centred
        let scale = (cell.w as f32 / img.width() as f32).max(cell.h as f32 / img.height() as f32);
        let (nw, nh) =
            (((img.width() as f32 * scale).ceil() as u32).max(cell.w), ((img.height() as f32 * scale).ceil() as u32).max(cell.h));
        let scaled = image::imageops::resize(img, nw, nh, image::imageops::FilterType::Triangle);
        let (ox, oy) = ((nw - cell.w) / 2, (nh - cell.h) / 2);
        for y in 0..cell.h {
            for x in 0..cell.w {
                let a = corner_alpha(x, y, cell.w, cell.h, o.radius);
                if a <= 0.0 {
                    continue;
                }
                let mut px = *scaled.get_pixel(x + ox, y + oy);
                px.0[3] = (px.0[3] as f32 * a) as u8;
                let dst = canvas.get_pixel_mut(cell.x + x, cell.y + y);
                let sa = px.0[3] as f32 / 255.0;
                for c in 0..3 {
                    dst.0[c] = (px.0[c] as f32 * sa + dst.0[c] as f32 * (1.0 - sa)) as u8;
                }
                dst.0[3] = (px.0[3] as f32 + dst.0[3] as f32 * (1.0 - sa)).min(255.0) as u8;
            }
        }
    }
    canvas
}

fn collage(files: &[PathBuf], o: &ferrocast_core::collage::CollageOpts) -> Result<PathBuf, String> {
    use ferrocast_core::collage::layout;
    use ferrocast_core::convert::output_with_suffix;
    let first = files.first().ok_or("no images")?;
    let ((cw, ch), cells) = layout(files.len(), o);
    if u64::from(cw) * u64::from(ch) > 120_000_000 {
        return Err("the collage would be too large; use a smaller width".into());
    }
    let tmp = std::env::temp_dir().join(format!(
        "ferrocast-collage-{}-{}",
        std::process::id(),
        COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let loaded: Result<Vec<image::RgbaImage>, String> = (|| {
        let mut v = vec![];
        for (i, (f, cell)) in files.iter().zip(&cells).enumerate() {
            // sips reads every format and scales; the image crate only needs PNG
            let png = tmp.join(format!("{i}.png"));
            sips(&["-s", "format", "png", "-Z", &cell.w.max(cell.h).to_string(), &f.to_string_lossy(), "--out", &png.to_string_lossy()])?;
            v.push(image::open(&png).map_err(|e| e.to_string())?.to_rgba8());
        }
        Ok(v)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    let canvas = collage_compose(&loaded?, o);
    let out = output_with_suffix(first, "collage", "png", |p| p.exists());
    canvas.save(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Put `img` on its backdrop with padding, rounded corners and a shadow. Pure pixel work, so the
/// editor can run it on a downscaled copy for the live preview. `bg` is the decoded background
/// picture when the backdrop is an image.
pub(crate) fn frame_compose(
    img: &image::RgbaImage,
    bg: Option<&image::RgbaImage>,
    o: &ferrocast_core::frame::FrameOpts,
) -> image::RgbaImage {
    use ferrocast_core::collage::corner_alpha;
    use ferrocast_core::frame::{canvas_size, shadow_offset, Backdrop};
    let (w, h) = (img.width(), img.height());
    let (cw, ch) = canvas_size(w, h, o);
    let mut canvas = match &o.backdrop {
        Backdrop::Image(_) => {
            let Some(bg) = bg else {
                return image::RgbaImage::from_pixel(cw, ch, image::Rgba([255, 255, 255, 255]));
            };
            let scale = (cw as f32 / bg.width() as f32).max(ch as f32 / bg.height() as f32);
            let (nw, nh) = (((bg.width() as f32 * scale).ceil() as u32).max(cw), ((bg.height() as f32 * scale).ceil() as u32).max(ch));
            let big = image::imageops::resize(bg, nw, nh, image::imageops::FilterType::Triangle);
            image::imageops::crop_imm(&big, (nw - cw) / 2, (nh - ch) / 2, cw, ch).to_image()
        }
        Backdrop::Color(c) => image::RgbaImage::from_pixel(cw, ch, image::Rgba([c[0], c[1], c[2], 255])),
        Backdrop::Blur(r) => {
            let r = *r;
            // cover the canvas with the picture, then blur it
            let scale = (cw as f32 / w as f32).max(ch as f32 / h as f32);
            let (nw, nh) = (((w as f32 * scale).ceil() as u32).max(cw), ((h as f32 * scale).ceil() as u32).max(ch));
            // blur at a quarter of the size: same look, far less work
            let small = image::imageops::resize(img, (nw / 4).max(1), (nh / 4).max(1), image::imageops::FilterType::Triangle);
            let blurred = image::imageops::blur(&small, (r / 4.0).max(0.5));
            let big = image::imageops::resize(&blurred, nw, nh, image::imageops::FilterType::Triangle);
            image::imageops::crop_imm(&big, (nw - cw) / 2, (nh - ch) / 2, cw, ch).to_image()
        }
    };
    let rect_alpha = |x: u32, y: u32| corner_alpha(x, y, w, h, o.radius);
    if o.shadow > 0 {
        let (dx, dy) = shadow_offset(o);
        let mut mask = image::GrayImage::new(cw, ch);
        for y in 0..h {
            for x in 0..w {
                let (mx, my) = (o.pad as i32 + x as i32 + dx, o.pad as i32 + y as i32 + dy);
                if mx >= 0 && my >= 0 && (mx as u32) < cw && (my as u32) < ch {
                    mask.put_pixel(mx as u32, my as u32, image::Luma([(rect_alpha(x, y) * 255.0) as u8]));
                }
            }
        }
        let soft = image::imageops::blur(&mask, o.shadow as f32 / 2.0);
        for (x, y, m) in soft.enumerate_pixels() {
            let a = m.0[0] as f32 / 255.0 * 0.55;
            let dst = canvas.get_pixel_mut(x, y);
            for c in 0..3 {
                dst.0[c] = (dst.0[c] as f32 * (1.0 - a)) as u8;
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let src = img.get_pixel(x, y).0;
            let a = rect_alpha(x, y) * (src[3] as f32 / 255.0);
            if a <= 0.0 {
                continue;
            }
            let dst = canvas.get_pixel_mut(o.pad + x, o.pad + y);
            for (d, s) in dst.0.iter_mut().zip(src).take(3) {
                *d = (s as f32 * a + *d as f32 * (1.0 - a)) as u8;
            }
        }
    }
    canvas
}

fn frame_image(f: &std::path::Path, o: &ferrocast_core::frame::FrameOpts) -> Result<PathBuf, String> {
    check_source_size(f)?;
    use ferrocast_core::convert::output_with_suffix;
    use ferrocast_core::frame::canvas_size;
    let tmp = std::env::temp_dir().join(format!(
        "ferrocast-frame-{}-{}.png",
        std::process::id(),
        COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let loaded = sips(&["-s", "format", "png", &f.to_string_lossy(), "--out", &tmp.to_string_lossy()])
        .and_then(|_| image::open(&tmp).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&tmp);
    let img = loaded?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    let (cw, ch) = canvas_size(w, h, o);
    if u64::from(cw) * u64::from(ch) > 120_000_000 {
        return Err("the result would be too large; use less padding".into());
    }
    let bg = match &o.backdrop {
        ferrocast_core::frame::Backdrop::Image(path) => {
            // canonical absolute path: a name starting with `-` can never be read as a sips option
            let canon = std::path::Path::new(path).canonicalize().ok().filter(|p| p.is_file());
            let Some(canon) = canon else {
                return Err(format!("{path} is not a readable image"));
            };
            let path = canon.to_string_lossy().to_string();
            let bg_tmp = std::env::temp_dir().join(format!(
                "ferrocast-framebg-{}-{}.png",
                std::process::id(),
                COLLAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let loaded = sips(&["-s", "format", "png", &path, "--out", &bg_tmp.to_string_lossy()])
                .and_then(|_| image::open(&bg_tmp).map_err(|e| e.to_string()));
            let _ = std::fs::remove_file(&bg_tmp);
            Some(loaded.map_err(|e| format!("background image: {e}"))?.to_rgba8())
        }
        _ => None,
    };
    let canvas = frame_compose(&img, bg.as_ref(), o);
    let out = output_with_suffix(f, "framed", "png", |p| p.exists());
    canvas.save(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn run_image_op(files: &[PathBuf], op: &ferrocast_core::convert::ImageOp) -> (Vec<PathBuf>, Vec<String>) {
    use ferrocast_core::convert::{crop_size, ext, output_with_suffix, strip_jpeg, strip_png, ImageOp};
    let name = |p: &std::path::Path| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut outs = vec![];
    let mut errs = vec![];
    if *op == ImageOp::CombinePdf {
        let Some(first) = files.first() else { return (outs, errs) };
        let out = output_with_suffix(first, &op.suffix(), "pdf", |p| p.exists());
        let tmp = std::env::temp_dir().join(format!("ferrocast-pdf-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let mut pages = vec![];
        for (i, f) in files.iter().enumerate() {
            let pg = tmp.join(format!("{i:04}.pdf"));
            match sips(&["-s", "format", "pdf", &f.to_string_lossy(), "--out", &pg.to_string_lossy()]) {
                Ok(()) => pages.push(pg),
                Err(e) => errs.push(format!("{}: {e}", name(f))),
            }
        }
        let join = "/System/Library/Automator/Combine PDF Pages.action/Contents/MacOS/join";
        let ok = !pages.is_empty()
            && std::process::Command::new(join).arg("-o").arg(&out).args(&pages).status().map(|s| s.success()).unwrap_or(false);
        let _ = std::fs::remove_dir_all(&tmp);
        if ok && out.exists() {
            outs.push(out);
        } else if errs.is_empty() {
            errs.push("could not combine the pages into a PDF".into());
        }
        return (outs, errs);
    }
    if let ImageOp::Redact(faces) = op {
        for f in files {
            match redact(f, *faces) {
                Ok(o) => outs.push(o),
                Err(e) => errs.push(format!("{}: {e}", name(f))),
            }
        }
        return (outs, errs);
    }
    if matches!(op, ImageOp::CompressToKb(_) | ImageOp::CompressByPercent(_)) {
        for f in files {
            match compress_to_size(f, op) {
                Ok(o) => outs.push(o),
                Err(e) => errs.push(format!("{}: {e}", name(f))),
            }
        }
        return (outs, errs);
    }
    if let ImageOp::CompressWith(goal, resize) = op {
        for f in files {
            match compress_with(f, *goal, resize, &op.suffix()) {
                Ok(o) => outs.push(o),
                Err(e) => errs.push(format!("{}: {e}", name(f))),
            }
        }
        return (outs, errs);
    }
    if let ImageOp::EditWith(o) = op {
        for f in files {
            match edit_photo(f, o) {
                Ok(p) => outs.push(p),
                Err(e) => errs.push(format!("{}: {e}", name(f))),
            }
        }
        return (outs, errs);
    }
    if let ImageOp::RedactWith(spec) = op {
        for f in files {
            match redact_with(f, spec) {
                Ok(p) => outs.push(p),
                Err(e) => errs.push(format!("{}: {e}", name(f))),
            }
        }
        return (outs, errs);
    }
    if let ImageOp::FrameWith(o) = op {
        for f in files {
            match frame_image(f, o) {
                Ok(p) => outs.push(p),
                Err(e) => errs.push(format!("{}: {e}", name(f))),
            }
        }
        return (outs, errs);
    }
    if let ImageOp::Adjust(a) = op {
        for f in files {
            match adjust(f, *a) {
                Ok(o) => outs.push(o),
                Err(e) => errs.push(format!("{}: {e}", name(f))),
            }
        }
        return (outs, errs);
    }
    if let ImageOp::Collage | ImageOp::CollageWith(_) = op {
        let opts = if let ImageOp::CollageWith(o) = op { *o } else { Default::default() };
        match collage(files, &opts) {
            Ok(o) => outs.push(o),
            Err(e) => errs.push(e),
        }
        return (outs, errs);
    }
    for f in files {
        let e = ext(f);
        let res: Result<PathBuf, String> = (|| match op {
            ImageOp::Compress(q) => {
                let out = output_with_suffix(f, &op.suffix(), "jpg", |p| p.exists());
                sips(&[
                    "-s",
                    "format",
                    "jpeg",
                    "-s",
                    "formatOptions",
                    &q.to_string(),
                    &f.to_string_lossy(),
                    "--out",
                    &out.to_string_lossy(),
                ])?;
                Ok(out)
            }
            ImageOp::Resize(px) => {
                let out = output_with_suffix(f, &op.suffix(), if e.is_empty() { "png" } else { &e }, |p| p.exists());
                sips(&["-Z", &px.to_string(), &f.to_string_lossy(), "--out", &out.to_string_lossy()])?;
                Ok(out)
            }
            ImageOp::Crop(rw, rh) => {
                let (w, h) = image_size(f).ok_or("could not read the image size")?;
                let (cw, ch) = crop_size(w, h, *rw, *rh);
                let out = output_with_suffix(f, &op.suffix(), if e.is_empty() { "png" } else { &e }, |p| p.exists());
                // sips crops around the centre
                sips(&["-c", &ch.to_string(), &cw.to_string(), &f.to_string_lossy(), "--out", &out.to_string_lossy()])?;
                Ok(out)
            }
            ImageOp::StripMetadata => {
                let bytes = std::fs::read(f).map_err(|e| e.to_string())?;
                let clean = match e.as_str() {
                    "jpg" | "jpeg" => strip_jpeg(&bytes),
                    "png" => strip_png(&bytes),
                    _ => None,
                }
                .ok_or("metadata removal supports JPEG and PNG; convert to one of those first")?;
                let out = output_with_suffix(f, &op.suffix(), &e, |p| p.exists());
                std::fs::write(&out, clean).map_err(|e| e.to_string())?;
                Ok(out)
            }
            ImageOp::Rotate(d) => {
                let out = output_with_suffix(f, &op.suffix(), if e.is_empty() { "png" } else { &e }, |p| p.exists());
                sips(&["-r", &d.to_string(), &f.to_string_lossy(), "--out", &out.to_string_lossy()])?;
                Ok(out)
            }
            ImageOp::Flip(h) => {
                let out = output_with_suffix(f, &op.suffix(), if e.is_empty() { "png" } else { &e }, |p| p.exists());
                sips(&["-f", if *h { "horizontal" } else { "vertical" }, &f.to_string_lossy(), "--out", &out.to_string_lossy()])?;
                Ok(out)
            }
            ImageOp::Background(c) => {
                let (w, h) = image_size(f).ok_or("could not read the image size")?;
                let side = w.max(h).to_string();
                // JPEG has no alpha, so a padded PNG keeps the background exact
                let out = output_with_suffix(f, &op.suffix(), "png", |p| p.exists());
                sips(&["-s", "format", "png", "-p", &side, &side, "--padColor", c, &f.to_string_lossy(), "--out", &out.to_string_lossy()])?;
                Ok(out)
            }
            ImageOp::CropExact(w, h) => {
                let (iw, ih) = image_size(f).ok_or("could not read the image size")?;
                let out = output_with_suffix(f, &op.suffix(), if e.is_empty() { "png" } else { &e }, |p| p.exists());
                // never ask sips for more than the image has
                sips(&["-c", &(*h).min(ih).to_string(), &(*w).min(iw).to_string(), &f.to_string_lossy(), "--out", &out.to_string_lossy()])?;
                Ok(out)
            }
            ImageOp::CropRect(x, y, w, h) => {
                let (iw, ih) = image_size(f).ok_or("could not read the image size")?;
                if *w == 0 || *h == 0 || u64::from(*x) + u64::from(*w) > u64::from(iw) || u64::from(*y) + u64::from(*h) > u64::from(ih) {
                    return Err(format!("the crop ({x},{y} {w}×{h}) is outside the image ({iw}×{ih} px)"));
                }
                let out = output_with_suffix(f, &op.suffix(), if e.is_empty() { "png" } else { &e }, |p| p.exists());
                // sips crops height × width at an offset measured from the top-left corner
                sips(&[
                    "-c",
                    &h.to_string(),
                    &w.to_string(),
                    "--cropOffset",
                    &y.to_string(),
                    &x.to_string(),
                    &f.to_string_lossy(),
                    "--out",
                    &out.to_string_lossy(),
                ])?;
                Ok(out)
            }
            ImageOp::SetMeta(m) => {
                let bytes = std::fs::read(f).map_err(|e| e.to_string())?;
                let tagged =
                    ferrocast_core::convert::set_jpeg_meta(&bytes, m).ok_or("writing tags supports JPEG; convert to JPEG first")?;
                let out = output_with_suffix(f, &op.suffix(), &e, |p| p.exists());
                std::fs::write(&out, tagged).map_err(|e| e.to_string())?;
                Ok(out)
            }
            ImageOp::Annotate => {
                // never mark up the original: copy it, then open the copy in Preview
                let out = output_with_suffix(f, &op.suffix(), if e.is_empty() { "png" } else { &e }, |p| p.exists());
                std::fs::copy(f, &out).map_err(|e| e.to_string())?;
                if std::env::var_os("FERROCAST_NO_OPEN").is_none() {
                    let _ = std::process::Command::new("/usr/bin/open").args(["-a", "Preview"]).arg(&out).status();
                }
                Ok(out)
            }
            ImageOp::CombinePdf
            | ImageOp::Collage
            | ImageOp::CollageWith(_)
            | ImageOp::FrameWith(_)
            | ImageOp::RedactWith(_)
            | ImageOp::EditWith(_)
            | ImageOp::Redact(_)
            | ImageOp::Adjust(_)
            | ImageOp::CompressToKb(_)
            | ImageOp::CompressByPercent(_)
            | ImageOp::CompressWith(..) => unreachable!(),
        })();
        match res {
            Ok(o) if o.exists() => outs.push(o),
            Ok(_) => errs.push(format!("{}: no output produced", name(f))),
            Err(e) => errs.push(format!("{}: {e}", name(f))),
        }
    }
    (outs, errs)
}

pub fn image_op_files(files: Vec<PathBuf>, op: ferrocast_core::convert::ImageOp, bus: Bus) {
    std::thread::spawn(move || {
        let (outs, errs) = run_image_op(&files, &op);
        let msg = match (outs.len(), errs.len()) {
            (0, _) => format!("{} failed: {}", op.label(), errs.first().cloned().unwrap_or_default()),
            (n, 0) => format!("{} · {n} file{} written", op.label(), if n == 1 { "" } else { "s" }),
            (n, e) => format!("{} · {n} written, {e} failed: {}", op.label(), errs[0]),
        };
        // Annotate already opened Preview; revealing would pull Finder in front of it
        if let Some(f) = outs.first().filter(|_| op != ferrocast_core::convert::ImageOp::Annotate) {
            let _ = ferrocast_macos::apps::reveal(f);
        }
        notify("Ferrocast", &msg);
        bus.send(Msg::Toast(msg));
    });
}

/// Native multi-file chooser (blocks; call from a thread).
pub fn choose_files() -> Vec<PathBuf> {
    let out = ferrocast_macos::system::osascript(
        "set fs to choose file with prompt \"Choose files to convert\" with multiple selections allowed\nset outp to \"\"\nrepeat with f in fs\nset outp to outp & POSIX path of f & linefeed\nend repeat\nreturn outp",
    )
    .unwrap_or_default();
    out.lines().filter(|l| !l.trim().is_empty()).map(|l| PathBuf::from(l.trim())).collect()
}

/// Ask for one image with the native dialog and return it decoded (max 1200 px), or None when
/// cancelled or unreadable (blocks; call from a thread).
pub fn choose_stamp_image() -> Option<(usize, usize, Vec<u8>)> {
    let out = ferrocast_macos::system::osascript(
        "POSIX path of (choose file of type {\"public.image\"} with prompt \"Choose an image to place\")",
    )
    .ok()?;
    let path = PathBuf::from(out.trim());
    let png = display_png(&path, 1200)?;
    let img = image::load_from_memory(&png).ok()?.to_rgba8();
    Some((img.width() as usize, img.height() as usize, img.into_raw()))
}

/// Ask the user for a JSON file with a native dialog (blocks; call from a thread).
pub fn choose_json_file() -> Option<PathBuf> {
    let out = ferrocast_macos::system::osascript(
        "POSIX path of (choose file of type {\"public.json\"} with prompt \"Choose a snippets JSON file\")",
    )
    .ok()?;
    (!out.is_empty()).then(|| PathBuf::from(out))
}

#[cfg(test)]
mod convert_tests {
    use super::*;
    use ferrocast_core::convert::{targets, Tools};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fc-conv-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn png_to_jpeg_with_sips_and_no_overwrite() {
        let d = tmp("img");
        let src = d.join("photo.png");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/icon-1024.png"), &src).unwrap();
        let t = targets(&src, &Tools::default()).into_iter().find(|t| t.ext == "jpg").unwrap();
        let (out, err) = run_conversion(std::slice::from_ref(&src), &t, &Tools::default());
        assert!(err.is_empty(), "{err:?}");
        assert_eq!(out, vec![d.join("photo.jpg")]);
        assert_eq!(&std::fs::read(&out[0]).unwrap()[..3], &[0xFF, 0xD8, 0xFF], "JPEG magic");
        // second run must not overwrite
        let (out2, _) = run_conversion(&[src], &t, &Tools::default());
        assert_eq!(out2, vec![d.join("photo 2.jpg")]);
    }

    #[test]
    fn text_to_docx_and_folder_to_zip() {
        let d = tmp("doc");
        let txt = d.join("notes.txt");
        std::fs::write(&txt, "hello").unwrap();
        let t = targets(&txt, &Tools::default()).into_iter().find(|t| t.ext == "docx").unwrap();
        let (out, err) = run_conversion(&[txt], &t, &Tools::default());
        assert!(err.is_empty(), "{err:?}");
        assert_eq!(&std::fs::read(&out[0]).unwrap()[..2], b"PK", "docx is a zip");
        let folder = d.join("folder");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("a.txt"), "a").unwrap();
        let z = targets(&folder, &Tools::default()).into_iter().find(|t| t.ext == "zip").unwrap();
        let (out, err) = run_conversion(&[folder], &z, &Tools::default());
        assert!(err.is_empty(), "{err:?}");
        assert_eq!(out, vec![d.join("folder.zip")]);
    }

    /// A photo-like JPEG (smooth gradients plus noise, a few hundred KB) made with sips.
    fn photo_jpeg(dir: &std::path::Path) -> PathBuf {
        let (w, h) = (1200u32, 800u32);
        let mut img = image::RgbImage::new(w, h);
        let mut seed = 12345u32;
        for (x, y, px) in img.enumerate_pixels_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let n = (seed >> 24) as i32 % 40 - 20;
            let c = |v: u32| (v as i32 + n).clamp(0, 255) as u8;
            *px = image::Rgb([c(x * 255 / w), c(y * 255 / h), c((x + y) * 255 / (w + h))]);
        }
        let png = dir.join("photo-src.png");
        img.save(&png).unwrap();
        let jpg = dir.join("photo.jpg");
        assert!(std::process::Command::new("/usr/bin/sips")
            .args(["-s", "format", "jpeg", "-s", "formatOptions", "92"])
            .arg(&png)
            .arg("--out")
            .arg(&jpg)
            .status()
            .unwrap()
            .success());
        assert!(std::fs::metadata(&jpg).unwrap().len() > 150_000, "the fixture should be photo-like");
        jpg
    }

    #[test]
    fn compress_with_goals_resize_naming_and_missing_file() {
        use ferrocast_core::compress::{Goal, Preset, Resize};
        use ferrocast_core::convert::ImageOp;
        let d = tmp("compress-with");
        let src = photo_jpeg(&d);
        let before = std::fs::read(&src).unwrap();
        let orig = before.len() as u64;
        let one = std::slice::from_ref(&src);
        let none = Resize::default();
        // percent: smaller than the original, and at least that much smaller
        let (o, e) = run_image_op(one, &ImageOp::CompressWith(Goal::Percent(50), none));
        assert!(e.is_empty(), "{e:?}");
        let sz = std::fs::metadata(&o[0]).unwrap().len();
        assert!(sz < orig && sz <= orig / 2, "50 % smaller: {sz} of {orig}");
        // a size target is met
        let target = orig / 3;
        let (o, e) = run_image_op(one, &ImageOp::CompressWith(Goal::TargetBytes(target), none));
        assert!(e.is_empty(), "{e:?}");
        assert!(std::fs::metadata(&o[0]).unwrap().len() <= target, "within the target");
        // presets get stronger
        let size_of = |p| {
            let (o, e) = run_image_op(one, &ImageOp::CompressWith(Goal::Preset(p), none));
            assert!(e.is_empty(), "{e:?}");
            std::fs::metadata(&o[0]).unwrap().len()
        };
        let (l, s) = (size_of(Preset::Light), size_of(Preset::Strong));
        assert!(s < l, "strong {s} < light {l}");
        // resize while compressing: the copy is 400 px wide, aspect kept
        let (o, e) =
            run_image_op(one, &ImageOp::CompressWith(Goal::Preset(Preset::Balanced), Resize { max_edge: Some(400), percent: None }));
        assert!(e.is_empty(), "{e:?}");
        let (w, h) = image_size(&o[0]).unwrap();
        assert_eq!(w, 400);
        assert!((265..=268).contains(&h), "aspect kept: {h}");
        // never upscales
        let (o, e) = run_image_op(one, &ImageOp::CompressWith(Goal::Preset(Preset::Light), Resize { max_edge: Some(5000), percent: None }));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((1200, 800)));
        // resize and a size target together
        let (o, e) = run_image_op(one, &ImageOp::CompressWith(Goal::TargetBytes(40_000), Resize { max_edge: Some(600), percent: None }));
        assert!(e.is_empty(), "{e:?}");
        assert!(std::fs::metadata(&o[0]).unwrap().len() <= 40_000);
        assert_eq!(image_size(&o[0]).unwrap().0, 600);
        // the original is untouched
        assert_eq!(std::fs::read(&src).unwrap(), before);
        // naming: a second run never overwrites
        let d2 = tmp("compress-names");
        let src2 = d2.join("pic.jpg");
        std::fs::copy(&src, &src2).unwrap();
        let op = ImageOp::CompressWith(Goal::Preset(Preset::Balanced), none);
        let (a, _) = run_image_op(std::slice::from_ref(&src2), &op);
        let (b, _) = run_image_op(std::slice::from_ref(&src2), &op);
        assert_eq!(a[0], d2.join("pic compressed.jpg"));
        assert_eq!(b[0], d2.join("pic compressed 2.jpg"));
        // a missing file is reported, nothing panics, nothing is written
        let gone = d2.join("gone.jpg");
        let (o, e) = run_image_op(std::slice::from_ref(&gone), &op);
        assert!(o.is_empty() && e.len() == 1 && e[0].contains("gone.jpg"), "{o:?} {e:?}");
        assert!(!d2.join("gone compressed.jpg").exists());
    }

    /// Vision needs an inference context that virtual CI machines do not have; there the
    /// real-file tool test is skipped (it still runs in full on a real Mac).
    fn vision_cannot_run() -> bool {
        let fixture = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../macos/fixtures/text.png"));
        matches!(ferrocast_macos::vision::detect(fixture, true, false), Err(e) if e.to_string().contains("inference context"))
    }

    #[test]
    fn image_tools_on_real_files() {
        use ferrocast_core::convert::ImageOp;
        if vision_cannot_run() {
            eprintln!("skipped: Vision cannot create an inference context on this machine");
            return;
        }
        let d = tmp("tools");
        let src = d.join("pic.jpg");
        let icon = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/icon-1024.png");
        assert!(std::process::Command::new("/usr/bin/sips")
            .args(["-s", "format", "jpeg", icon, "--out"])
            .arg(&src)
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new("/usr/bin/sips")
            .args(["-s", "make", "TestCam", "-s", "artist", "Me"])
            .arg(&src)
            .status()
            .unwrap()
            .success());
        let one = std::slice::from_ref(&src);

        let (o, e) = run_image_op(one, &ImageOp::Resize(640));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((640, 640)));

        let (o, e) = run_image_op(one, &ImageOp::Crop(16, 9));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((1024, 576)));

        let (o, e) = run_image_op(one, &ImageOp::Compress(40));
        assert!(e.is_empty(), "{e:?}");
        assert!(std::fs::metadata(&o[0]).unwrap().len() < std::fs::metadata(&src).unwrap().len());

        let tags = |p: &std::path::Path| {
            String::from_utf8_lossy(
                &std::process::Command::new("/usr/bin/sips").args(["-g", "make", "-g", "artist"]).arg(p).output().unwrap().stdout,
            )
            .to_string()
        };
        assert!(tags(&src).contains("TestCam"));
        let (o, e) = run_image_op(one, &ImageOp::StripMetadata);
        assert!(e.is_empty(), "{e:?}");
        assert!(!tags(&o[0]).contains("TestCam") && !tags(&o[0]).contains("Me"), "{}", tags(&o[0]));
        assert_eq!(image_size(&o[0]), Some((1024, 1024)), "image data intact");

        let second = d.join("pic2.png");
        std::fs::copy(icon, &second).unwrap();
        let (o, e) = run_image_op(&[src.clone(), second], &ImageOp::CombinePdf);
        assert!(e.is_empty(), "{e:?}");
        let pdf = std::fs::read(&o[0]).unwrap();
        // count "/Type" [spaces] "/Page" not followed by "s" (that would be /Pages)
        let mut pages = 0;
        for i in 0..pdf.len().saturating_sub(12) {
            if &pdf[i..i + 5] == b"/Type" {
                let mut j = i + 5;
                while pdf[j] == b' ' {
                    j += 1;
                }
                if pdf[j..].starts_with(b"/Page") && pdf.get(j + 5) != Some(&b's') {
                    pages += 1;
                }
            }
        }
        assert_eq!(pages, 2, "one page per image");

        // a 16:9 picture makes rotation and padding observable
        let (wide, e) = run_image_op(one, &ImageOp::Crop(16, 9));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&wide[0]), Some((1024, 576)));
        let (o, e) = run_image_op(&wide, &ImageOp::Rotate(90));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((576, 1024)));
        let (o, e) = run_image_op(&wide, &ImageOp::Flip(true));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((1024, 576)));
        let (o, e) = run_image_op(&wide, &ImageOp::Background("FFFFFF"));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((1024, 1024)));
        let img = image::open(&o[0]).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0[..3], [255, 255, 255], "padding is white");
        let (o, e) = run_image_op(&[src.clone(), wide[0].clone(), src.clone()], &ImageOp::Collage);
        assert!(e.is_empty(), "{e:?}");
        // 3 images → 2×2 grid of 600 px cells with 12 px gaps
        assert_eq!(image_size(&o[0]), Some((1236, 1236)));
        assert!(src.exists() && wide[0].exists(), "originals untouched");

        // collage options: a row at an exact width, square corners cut to the background, no overwrite
        let opts = ferrocast_core::collage::parse("collage row 1200 gap 20 radius 40 bg ff0000").unwrap();
        let (o, e) = run_image_op(&[src.clone(), wide[0].clone(), src.clone()], &ImageOp::CollageWith(opts));
        assert!(e.is_empty(), "{e:?}");
        let (w, h) = image_size(&o[0]).unwrap();
        assert!((1192..=1200).contains(&w), "width {w}");
        assert!(h < w / 2, "a row of three is wide and short: {w}x{h}");
        let img = image::open(&o[0]).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0, 255], "edge is the red background");
        assert_eq!(img.get_pixel(20, 20).0, [255, 0, 0, 255], "rounded corner shows the background");
        assert_ne!(img.get_pixel(w / 6, h / 2).0, [255, 0, 0, 255], "picture centre is not background");
        // offset crop: the rectangle's own pixels, not the centre; a rectangle outside the image is refused
        let crop_base = d.join("crop-base.png");
        sips(&["-s", "format", "png", &wide[0].to_string_lossy(), "--out", &crop_base.to_string_lossy()]).unwrap();
        // the source is a JPEG, so the copy is re-encoded: compare with a tolerance, and prove the
        // offset by being closer to the rectangle's own pixels than to the top-left corner's
        let (cr, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::CropRect(400, 200, 300, 200));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&cr[0]), Some((300, 200)), "exact size");
        let (crp, crb) = (d.join("crop-out.png"), image::open(&crop_base).unwrap().to_rgba8());
        sips(&["-s", "format", "png", &cr[0].to_string_lossy(), "--out", &crp.to_string_lossy()]).unwrap();
        let cri = image::open(&crp).unwrap().to_rgba8();
        let dist = |a: &image::Rgba<u8>, b: &image::Rgba<u8>| (0..3).map(|c| (a.0[c] as i32 - b.0[c] as i32).abs()).sum::<i32>();
        let (mut from_offset, mut from_origin) = (0, 0);
        for y in (0..200).step_by(10) {
            for x in (0..300).step_by(10) {
                from_offset += dist(cri.get_pixel(x, y), crb.get_pixel(400 + x, 200 + y));
                from_origin += dist(cri.get_pixel(x, y), crb.get_pixel(x, y));
            }
        }
        assert!(
            from_offset * 4 < from_origin,
            "the copy matches the offset rectangle ({from_offset}) far better than the corner ({from_origin})"
        );
        let (none, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::CropRect(900, 500, 300, 200));
        assert!(none.is_empty() && e[0].contains("outside the image"), "{e:?}");
        // annotated copy: drawing lands at full size, the source is untouched, nothing is overwritten
        let mut ad = ferrocast_core::annotate::Doc::default();
        ad.add(
            ferrocast_core::annotate::Shape::Rect((100.0, 100.0), (400.0, 300.0)),
            ferrocast_core::annotate::Style { color: [0, 255, 0], width: 6.0, fill: true },
        );
        let src_bytes = std::fs::read(&wide[0]).unwrap();
        let a1 = annotate_save(&wide[0], &ad).unwrap();
        let a2 = annotate_save(&wide[0], &ad).unwrap();
        assert!(a1 != a2 && a1.exists() && a2.exists(), "never overwrites");
        assert_eq!(std::fs::read(&wide[0]).unwrap(), src_bytes, "source untouched");
        assert_eq!(image_size(&a1), Some((1024, 576)), "full size");
        let ai = image::open(&a1).unwrap().to_rgba8();
        assert_eq!(ai.get_pixel(250, 200).0, [0, 255, 0, 255], "drawn area");
        // text lands at full size: dark ink appears inside its box on a white-ish area (skipped without the font)
        if ferrocast_core::annotate::text_available() {
            let mut td = ferrocast_core::annotate::Doc::default();
            td.add(
                ferrocast_core::annotate::Shape::Text((600.0, 400.0), "Hello".into()),
                ferrocast_core::annotate::Style { color: [0, 0, 255], width: 8.0, fill: false },
            );
            let t1 = annotate_save(&wide[0], &td).unwrap();
            let (tw, th) = ferrocast_core::annotate::text_extent("Hello", ferrocast_core::annotate::text_size(8.0));
            let ti = image::open(&t1).unwrap().to_rgba8();
            let blue = (600..600 + tw as u32)
                .flat_map(|x| (400..400 + th as u32).map(move |y| (x, y)))
                .filter(|&(x, y)| ti.get_pixel(x, y).0[2] > ti.get_pixel(x, y).0[0].saturating_add(60))
                .count();
            assert!(blue > 30, "text ink at full size: {blue}");
            let mut cd = ferrocast_core::annotate::Doc::default();
            cd.add(
                ferrocast_core::annotate::Shape::Callout { at: (600.0, 100.0), tip: (300.0, 400.0), text: "Note".into() },
                ferrocast_core::annotate::Style { color: [0, 0, 0], width: 8.0, fill: false },
            );
            let c1 = annotate_save(&wide[0], &cd).unwrap();
            let ci = image::open(&c1).unwrap().to_rgba8();
            assert_eq!(ci.get_pixel(604, 104).0[..3], [0, 0, 0], "callout box at full size");
            assert_eq!(ci.get_pixel(300, 400).0[..3], [0, 0, 0], "pointer reaches the tip");
            // an image stamp lands at full size with its own pixels
            let red = ferrocast_core::annotate::Img::new(2, 2, [255u8, 0, 0, 255].repeat(4)).unwrap();
            let mut sd = ferrocast_core::annotate::Doc::default();
            sd.add(
                ferrocast_core::annotate::Shape::Stamp { at: (700.0, 300.0), size: (100.0, 80.0), img: red },
                ferrocast_core::annotate::Style::default(),
            );
            let s1 = annotate_save(&wide[0], &sd).unwrap();
            let si = image::open(&s1).unwrap().to_rgba8();
            assert_eq!(si.get_pixel(750, 340).0, [255, 0, 0, 255], "stamp at full size");
            assert_eq!(si.get_pixel(699, 340), ai.get_pixel(699, 340), "outside the stamp untouched");
        }
        let abase = d.join("annot-base.png");
        sips(&["-s", "format", "png", &wide[0].to_string_lossy(), "--out", &abase.to_string_lossy()]).unwrap();
        assert_eq!(ai.get_pixel(900, 500), image::open(&abase).unwrap().to_rgba8().get_pixel(900, 500), "outside the drawing untouched");
        // edit photo: brightness lifts pixels, grayscale-saturation equalises channels, 0 is a copy
        let ed = ferrocast_core::photo::parse("edit brightness 20").unwrap();
        let (ep, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::EditWith(ed));
        assert!(e.is_empty(), "{e:?}");
        let (pw, ph) = image_size(&ep[0]).unwrap();
        assert_eq!((pw, ph), (1024, 576), "size is kept");
        let edit_base = d.join("edit-base.png");
        sips(&["-s", "format", "png", &wide[0].to_string_lossy(), "--out", &edit_base.to_string_lossy()]).unwrap();
        let (before, after) = (image::open(&edit_base).unwrap().to_rgba8(), image::open(&ep[0]).unwrap().to_rgba8());
        let mean = |i: &image::RgbaImage| {
            i.pixels().map(|p| p.0[0] as u64 + p.0[1] as u64 + p.0[2] as u64).sum::<u64>() / (i.width() as u64 * i.height() as u64)
        };
        assert!(mean(&after) > mean(&before) + 20, "brighter: {} → {}", mean(&before), mean(&after));
        let gs = ferrocast_core::photo::parse("edit saturation -100").unwrap();
        let (gp, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::EditWith(gs));
        assert!(e.is_empty(), "{e:?}");
        let gi = image::open(&gp[0]).unwrap().to_rgba8();
        assert!(gi.pixels().all(|p| p.0[0] == p.0[1] && p.0[1] == p.0[2]), "fully desaturated");
        assert!(ep[0] != gp[0], "never overwrites");
        // redact typed areas: solid colour, then blur; outside the area the pixels are untouched
        // the image crate only reads PNG: compare against a PNG copy of the same pixels
        let base_png = d.join("redact-base.png");
        sips(&["-s", "format", "png", &wide[0].to_string_lossy(), "--out", &base_png.to_string_lossy()]).unwrap();
        let base = image::open(&base_png).unwrap().to_rgba8();
        let spec = ferrocast_core::redact::parse("redact 100,100 200x100 500,300 50x50 color ff0000").unwrap();
        let (rd, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::RedactWith(spec));
        assert!(e.is_empty(), "{e:?}");
        let ri = image::open(&rd[0]).unwrap().to_rgba8();
        assert_eq!(ri.get_pixel(150, 150).0, [255, 0, 0, 255], "first area is solid");
        assert_eq!(ri.get_pixel(520, 320).0, [255, 0, 0, 255], "second area is solid");
        assert_eq!(ri.get_pixel(299, 199).0, [255, 0, 0, 255], "last pixel of the area");
        assert_eq!(ri.get_pixel(300, 200), base.get_pixel(300, 200), "just outside is untouched");
        assert_eq!(ri.get_pixel(99, 99), base.get_pixel(99, 99), "just outside is untouched");
        let spec = ferrocast_core::redact::parse("redact 100,100 200x100 blur 15").unwrap();
        let (rb, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::RedactWith(spec));
        assert!(e.is_empty(), "{e:?}");
        let bi = image::open(&rb[0]).unwrap().to_rgba8();
        let diff = (0..100).filter(|i| bi.get_pixel(110 + i, 150) != base.get_pixel(110 + i, 150)).count();
        assert!(diff > 20, "blur changes the area ({diff} of 100 pixels)");
        assert_eq!(bi.get_pixel(300, 200), base.get_pixel(300, 200), "outside the blur is untouched");
        assert!(rd[0] != rb[0], "never overwrites");
        let spec = ferrocast_core::redact::parse("redact 5000,5000 10x10").unwrap();
        let (none, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::RedactWith(spec));
        assert!(none.is_empty() && e.len() == 1 && e[0].contains("outside the image"), "{e:?}");
        // add a background: padded canvas, colour at the edge, rounded corner shows it, shadow darkens
        let fo = ferrocast_core::frame::parse("frame pad 50 radius 60 bg 0000ff").unwrap();
        let (fr, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::FrameWith(fo));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&fr[0]), Some((1024 + 100, 576 + 100)));
        let fi = image::open(&fr[0]).unwrap().to_rgba8();
        assert_eq!(fi.get_pixel(0, 0).0, [0, 0, 255, 255], "padding is the colour");
        assert_eq!(fi.get_pixel(50, 50).0, [0, 0, 255, 255], "rounded corner shows the colour");
        assert_ne!(fi.get_pixel(562, 338).0, [0, 0, 255, 255], "picture centre is the picture");
        let sh = ferrocast_core::frame::parse("frame pad 80 shadow 40 bg ffffff").unwrap();
        let (fs, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::FrameWith(sh));
        assert!(e.is_empty(), "{e:?}");
        let si = image::open(&fs[0]).unwrap().to_rgba8();
        let below = si.get_pixel(80 + 512, 80 + 576 + 10).0;
        assert!(below[0] < 250, "shadow darkens the area under the picture: {below:?}");
        assert_eq!(si.get_pixel(2, 2).0, [255, 255, 255, 255], "far corner stays clean");
        // another image as the background: its colour shows in the padding
        let red_bg = d.join("red-bg.png");
        image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 0, 0, 255])).save(&red_bg).unwrap();
        let im = ferrocast_core::frame::parse(&format!("frame image {} pad 30", red_bg.display())).unwrap();
        let (fm, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::FrameWith(im));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image::open(&fm[0]).unwrap().to_rgba8().get_pixel(2, 2).0, [255, 0, 0, 255], "padding shows the background image");
        let bad = ferrocast_core::frame::parse("frame image /nonexistent/bg.png").unwrap();
        let (none, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::FrameWith(bad));
        assert!(none.is_empty() && e[0].contains("not a readable image"), "{e:?}");
        let bl = ferrocast_core::frame::parse("frame blur 40").unwrap();
        let (fb, e) = run_image_op(std::slice::from_ref(&wide[0]), &ImageOp::FrameWith(bl));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&fb[0]), Some((1024 + 128, 576 + 128)));
        assert!(fr[0] != fs[0] && fs[0] != fb[0], "never overwrites");
        let (o2, e) =
            run_image_op(&[src.clone(), src.clone()], &ferrocast_core::convert::parse_image_query("collage featured bg none").unwrap());
        assert!(e.is_empty(), "{e:?}");
        assert_ne!(o[0], o2[0], "never overwrites");
        assert_eq!(image::open(&o2[0]).unwrap().to_rgba8().get_pixel(0, 0).0[3], 0, "bg none is transparent");

        // Redact text: the rendered line is covered with black, the margins stay white
        let txt = d.join("text.png");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../macos/fixtures/text.png"), &txt).unwrap();
        let dark = |p: &std::path::Path| image::open(p).unwrap().to_rgba8().pixels().filter(|px| px.0[0] < 20).count();
        let before = dark(&txt);
        let (o, e) = run_image_op(std::slice::from_ref(&txt), &ImageOp::Redact(false));
        assert!(e.is_empty(), "{e:?}");
        assert!(dark(&o[0]) > before * 3, "text boxes filled: {} vs {before}", dark(&o[0]));
        let red = image::open(&o[0]).unwrap().to_rgba8();
        assert_eq!(red.get_pixel(red.width() - 1, red.height() - 1).0[..3], [255, 255, 255], "untouched corner");
        // adjustments: brighter raises the mean, black & white makes R == G == B
        use ferrocast_core::convert::Adjust;
        let mean = |p: &std::path::Path| {
            let i = image::open(p).unwrap().to_rgba8();
            i.pixels().map(|px| px.0[0] as u64 + px.0[1] as u64 + px.0[2] as u64).sum::<u64>() / (i.width() * i.height()) as u64
        };
        // the image crate here decodes PNG only: measure the JPEG source through a PNG copy
        let base_png = d.join("base.png");
        sips(&["-s", "format", "png", &src.to_string_lossy(), "--out", &base_png.to_string_lossy()]).unwrap();
        let base = mean(&base_png);
        let (o, e) = run_image_op(one, &ImageOp::Adjust(Adjust::Brighter));
        assert!(e.is_empty(), "{e:?}");
        assert!(mean(&o[0]) > base);
        let (o, e) = run_image_op(one, &ImageOp::Adjust(Adjust::Darker));
        assert!(e.is_empty(), "{e:?}");
        assert!(mean(&o[0]) < base);
        let (o, e) = run_image_op(one, &ImageOp::Adjust(Adjust::BlackAndWhite));
        assert!(e.is_empty(), "{e:?}");
        assert!(image::open(&o[0]).unwrap().to_rgba8().pixels().all(|px| px.0[0] == px.0[1] && px.0[1] == px.0[2]));
        // compress to a size target: the JPEG source is ~hundreds of KB, ask for far less
        let orig = std::fs::metadata(&src).unwrap().len();
        let (o, e) = run_image_op(one, &ImageOp::CompressByPercent(50));
        assert!(e.is_empty(), "{e:?}");
        assert!(std::fs::metadata(&o[0]).unwrap().len() <= orig / 2, "50% smaller");
        let kb = (orig / 1024 / 2).max(1) as u32;
        let (o, e) = run_image_op(one, &ImageOp::CompressToKb(kb));
        assert!(e.is_empty(), "{e:?}");
        assert!(std::fs::metadata(&o[0]).unwrap().len() <= kb as u64 * 1024, "within {kb} KB");
        // an impossible target reports instead of pretending
        let (o, e) = run_image_op(one, &ImageOp::CompressToKb(1));
        assert!(o.is_empty() && e[0].contains("above the target"), "{e:?}");
        // exact crop: dimensions are exact, and clamped to the image
        let (o, e) = run_image_op(one, &ImageOp::CropExact(300, 200));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((300, 200)));
        let (o, e) = run_image_op(one, &ImageOp::CropExact(5000, 5000));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((1024, 1024)), "never larger than the image");
        // camera and GPS tags written into EXIF, read back by sips; pixels untouched
        let meta = ferrocast_core::convert::MetaEdit { make: "Ferro".into(), model: "Cast 1".into(), gps: Some((40.4168, -3.7038)) };
        let (o, e) = run_image_op(one, &ImageOp::SetMeta(meta));
        assert!(e.is_empty(), "{e:?}");
        let tagged = tags(&o[0]);
        assert!(tagged.contains("Ferro"), "{tagged}");
        let model =
            String::from_utf8_lossy(&std::process::Command::new("/usr/bin/sips").args(["-g", "model"]).arg(&o[0]).output().unwrap().stdout)
                .to_string();
        assert!(model.contains("Cast 1"), "{model}");
        // GPS read back by ImageIO (independent of our writer); it reports magnitudes plus a ref
        let js = format!(
            "ObjC.import('ImageIO');ObjC.import('Foundation');var s=$.CGImageSourceCreateWithURL($.NSURL.fileURLWithPath('{}'),$());JSON.stringify(ObjC.deepUnwrap(ObjC.castRefToObject($.CGImageSourceCopyPropertiesAtIndex(s,0,$())))['{{GPS}}'])",
            o[0].display()
        );
        let gps = String::from_utf8_lossy(
            &std::process::Command::new("/usr/bin/osascript").args(["-l", "JavaScript", "-e", &js]).output().unwrap().stdout,
        )
        .to_string();
        assert!(gps.contains("\"LatitudeRef\":\"N\"") && gps.contains("\"LongitudeRef\":\"W\""), "{gps}");
        assert!(gps.contains("40.416") && gps.contains("3.703"), "{gps}");
        assert_eq!(image_size(&o[0]), Some((1024, 1024)), "image data intact");
        assert!(!tags(&o[0]).contains("TestCam"), "old camera replaced");
        // PNG cannot carry these tags: a clear error, no file
        let (o, e) = run_image_op(
            std::slice::from_ref(&wide[0]),
            &ImageOp::SetMeta(ferrocast_core::convert::MetaEdit { make: "X".into(), model: String::new(), gps: None }),
        );
        assert!(e.is_empty() || o.is_empty());
        // annotate makes a byte-identical copy (Preview is not opened under test)
        std::env::set_var("FERROCAST_NO_OPEN", "1");
        let (o, e) = run_image_op(one, &ImageOp::Annotate);
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(std::fs::read(&o[0]).unwrap(), std::fs::read(&src).unwrap());
        assert!(o[0].to_string_lossy().contains("annotated"));
        // no faces in a text picture → a clear error, no file
        let (o, e) = run_image_op(std::slice::from_ref(&txt), &ImageOp::Redact(true));
        assert!(o.is_empty() && e[0].contains("no faces found"), "{e:?}");
    }

    /// Uses ffmpeg and LibreOffice when installed; skips each case otherwise.
    #[test]
    fn optional_tools_on_real_files() {
        let tools = Tools::detect();
        let d = tmp("optional");
        let icon = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/icon-1024.png");
        let pick = |p: &std::path::Path, ext: &str| targets(p, &tools).into_iter().find(|t| t.ext == ext).unwrap();
        if tools.cwebp.is_some() {
            let png = d.join("i.png");
            std::fs::copy(icon, &png).unwrap();
            let (o, e) = run_conversion(std::slice::from_ref(&png), &pick(&png, "webp"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert_eq!(&std::fs::read(&o[0]).unwrap()[8..12], b"WEBP");
        }
        if let Some(ff) = tools.ffmpeg {
            // one second of silence → WAV (afconvert) → MP3 (ffmpeg)
            let wav = d.join("s.wav");
            assert!(std::process::Command::new(ff)
                .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=mono", "-t", "1"])
                .arg(&wav)
                .status()
                .unwrap()
                .success());
            let (o, e) = run_conversion(std::slice::from_ref(&wav), &pick(&wav, "mp3"), &tools);
            assert!(e.is_empty(), "{e:?}");
            let b = std::fs::read(&o[0]).unwrap();
            assert!(b.starts_with(b"ID3") || (b[0] == 0xFF && b[1] & 0xE0 == 0xE0), "mp3 header");
            let (o, e) = run_conversion(std::slice::from_ref(&wav), &pick(&wav, "m4a"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert_eq!(&std::fs::read(&o[0]).unwrap()[4..8], b"ftyp");
            let (o, e) = run_conversion(std::slice::from_ref(&wav), &pick(&wav, "ogg"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(b"OggS"));
            // a 1 s test clip → MOV is the source for video targets
            let mov = d.join("clip.mov");
            assert!(std::process::Command::new(ff)
                .args([
                    "-y",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=160x120:rate=10",
                    "-t",
                    "1",
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p"
                ])
                .arg(&mov)
                .status()
                .unwrap()
                .success());
            let srt = d.join("s.srt");
            std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHello\n").unwrap();
            let (o, e) = run_conversion(std::slice::from_ref(&srt), &pick(&srt, "vtt"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read_to_string(&o[0]).unwrap().starts_with("WEBVTT"));
            for ext in ["webm", "mkv", "gif", "mp4"] {
                let (o, e) = run_conversion(std::slice::from_ref(&mov), &pick(&mov, ext), &tools);
                assert!(e.is_empty(), "{ext}: {e:?}");
                assert!(std::fs::metadata(&o[0]).unwrap().len() > 100, "{ext} has content");
            }
        }
        if tools.soffice.is_some() {
            let txt = d.join("doc.txt");
            std::fs::write(&txt, "Hello from Ferrocast").unwrap();
            let docx = run_conversion(std::slice::from_ref(&txt), &pick(&txt, "docx"), &tools).0.remove(0);
            let (o, e) = run_conversion(std::slice::from_ref(&docx), &pick(&docx, "pdf"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(b"%PDF"));
            let csv = d.join("t.csv");
            std::fs::write(&csv, "a,b\n1,2\n").unwrap();
            let (o, e) = run_conversion(std::slice::from_ref(&csv), &pick(&csv, "xlsx"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(b"PK"));
            let xlsx = o[0].clone();
            let (o, e) = run_conversion(std::slice::from_ref(&xlsx), &pick(&xlsx, "csv"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read_to_string(&o[0]).unwrap().contains("1,2"));
            // PDF → DOCX needs the Writer PDF import filter
            let pdf = d.join("doc.pdf");
            assert!(pdf.exists());
            // PDF text layer → TXT and Markdown
            let (o, e) = run_conversion(std::slice::from_ref(&pdf), &pick(&pdf, "txt"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read_to_string(&o[0]).unwrap().contains("Hello from Ferrocast"));
            let (o, e) = run_conversion(std::slice::from_ref(&pdf), &pick(&pdf, "md"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read_to_string(&o[0]).unwrap().contains("Hello from Ferrocast"));
            let (o, e) = run_conversion(std::slice::from_ref(&pdf), &pick(&pdf, "docx"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(b"PK"));
            // new targets: EPUB, PPTX from a PDF, ODS, TSV (tabs, not commas)
            let (o, e) = run_conversion(std::slice::from_ref(&txt), &pick(&txt, "epub"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(b"PK"), "epub is a zip");
            let (o, e) = run_conversion(std::slice::from_ref(&pdf), &pick(&pdf, "pptx"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(b"PK"), "pptx is a zip");
            let (o, e) = run_conversion(std::slice::from_ref(&csv), &pick(&csv, "ods"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(b"PK"));
            let (o, e) = run_conversion(std::slice::from_ref(&csv), &pick(&csv, "tsv"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert_eq!(o[0].extension().unwrap(), "tsv");
            assert!(std::fs::read_to_string(&o[0]).unwrap().contains("1\t2"), "tab separated");
            // soffice writes into a scratch folder: an existing <stem>.<ext> is never overwritten
            let keep = d.join("t.pdf");
            std::fs::write(&keep, "precious").unwrap();
            let (o, e) = run_conversion(std::slice::from_ref(&csv), &pick(&csv, "pdf"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert_eq!(std::fs::read_to_string(&keep).unwrap(), "precious");
            assert!(o[0].to_string_lossy().ends_with("t 2.pdf"), "{o:?}");
        }
        // PDF pages: every page, 300 DPI (a 1024 pt page is 4267 px), JPEG too
        let (p1, p2) = (d.join("one.pdf"), d.join("two.pdf"));
        for p in [&p1, &p2] {
            sips(&["-s", "format", "pdf", icon, "--out", &p.to_string_lossy()]).unwrap();
        }
        let two = d.join("both.pdf");
        let (mm, e) = run_file_tool(&[p1.clone(), p2.clone()], &ferrocast_core::convert::FileTool::Merge, None);
        assert!(e.is_empty(), "{e:?}");
        std::fs::rename(&mm[0], &two).unwrap();
        let (o, e) = run_conversion(std::slice::from_ref(&two), &pick(&two, "png"), &tools);
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(o.len(), 2, "one image per page: {o:?}");
        assert_eq!(image_size(&o[0]), Some((4267, 4267)));
        assert!(o[1].to_string_lossy().contains("page 2"));
        let (o, e) = run_conversion(std::slice::from_ref(&p1), &pick(&p1, "jpg"), &tools);
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].extension().unwrap(), "jpg", "single page keeps the plain name");
        // an image-only PDF has no text layer: say so instead of writing an empty file
        let (o, e) = run_conversion(std::slice::from_ref(&p1), &pick(&p1, "txt"), &tools);
        assert!(o.is_empty() && e[0].contains("no text layer"), "{e:?}");
        // subtitles → plain text (no timings, numbers or markup)
        let srt = d.join("cues.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHello <i>there</i>\n\n2\n00:00:03,000 --> 00:00:04,500\nSecond line\n")
            .unwrap();
        let (o, e) = run_conversion(std::slice::from_ref(&srt), &pick(&srt, "txt"), &tools);
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(std::fs::read_to_string(&o[0]).unwrap(), "Hello there\nSecond line\n");
        // RAR → ZIP (needs unrar; the rar binary builds the fixture)
        if let (Some(_), true) = (tools.unrar, std::path::Path::new("/Applications/rar/rar").exists()) {
            let src = d.join("rarsrc");
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(src.join("in.txt"), "inside").unwrap();
            let rar = d.join("pack.rar");
            // the bundled rar/unrar can be x86-only ("Bad CPU type" without Rosetta): then skip
            let built =
                std::process::Command::new("/Applications/rar/rar").args(["a", "-idq", "-ep"]).arg(&rar).arg(src.join("in.txt")).status();
            if matches!(built, Ok(st) if st.success()) {
                let (o, e) = run_conversion(std::slice::from_ref(&rar), &pick(&rar, "zip"), &tools);
                assert!(e.is_empty(), "{e:?}");
                let listing = std::process::Command::new("/usr/bin/unzip").arg("-l").arg(&o[0]).output().unwrap();
                assert!(String::from_utf8_lossy(&listing.stdout).contains("in.txt"), "{}", String::from_utf8_lossy(&listing.stdout));
            }
        }
        // slides → one image per slide (LibreOffice renders the deck to PDF first)
        if tools.soffice.is_some() {
            let odp_src = d.join("deck.txt");
            std::fs::write(&odp_src, "Slide text").unwrap();
            let deck = run_conversion(std::slice::from_ref(&odp_src), &pick(&odp_src, "pdf"), &tools).0.remove(0);
            let pptx = run_conversion(std::slice::from_ref(&deck), &pick(&deck, "pptx"), &tools).0.remove(0);
            let (o, e) = run_conversion(std::slice::from_ref(&pptx), &pick(&pptx, "png"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(!o.is_empty() && o.iter().all(|p| p.parent() == pptx.parent() && std::fs::metadata(p).unwrap().len() > 1000), "{o:?}");
        }
        // TAR, and SVG input through sips
        let tarred = run_conversion(std::slice::from_ref(&d), &pick(&d, "tar"), &tools);
        assert!(tarred.1.is_empty(), "{:?}", tarred.1);
        assert!(std::fs::metadata(&tarred.0[0]).unwrap().len() > 0);
        let svg = d.join("a.svg");
        std::fs::write(
            &svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="60"><rect width="100" height="60" fill="red"/></svg>"#,
        )
        .unwrap();
        let (o, e) = run_conversion(std::slice::from_ref(&svg), &pick(&svg, "png"), &tools);
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(image_size(&o[0]), Some((100, 60)));
        if let Some(ff) = tools.ffmpeg {
            let wav = d.join("w.wav");
            assert!(std::process::Command::new(ff)
                .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=duration=1"])
                .arg(&wav)
                .status()
                .unwrap()
                .success());
            let (o, e) = run_conversion(std::slice::from_ref(&wav), &pick(&wav, "wma"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(&[0x30, 0x26, 0xB2, 0x75]), "ASF header");
            let mov = d.join("v.mov");
            assert!(std::process::Command::new(ff)
                .args([
                    "-y",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=160x120:rate=10",
                    "-t",
                    "1",
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p"
                ])
                .arg(&mov)
                .status()
                .unwrap()
                .success());
            let (o, e) = run_conversion(std::slice::from_ref(&mov), &pick(&mov, "wmv"), &tools);
            assert!(e.is_empty(), "{e:?}");
            assert!(std::fs::read(&o[0]).unwrap().starts_with(&[0x30, 0x26, 0xB2, 0x75]), "ASF header");
        }
    }

    /// Trim, split and merge on real audio and PDF files (ffmpeg part skipped when not installed).
    #[test]
    fn file_tools_on_real_files() {
        use ferrocast_core::convert::FileTool;
        let tools = Tools::detect();
        let d = tmp("filetools");
        let dur = |p: &std::path::Path, ff: &str| -> f64 {
            let probe = std::path::Path::new(ff).with_file_name("ffprobe");
            let o = std::process::Command::new(probe)
                .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
                .arg(p)
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).trim().parse().unwrap()
        };
        if let Some(ff) = tools.ffmpeg {
            let mp3 = d.join("a.mp3");
            assert!(std::process::Command::new(ff)
                .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=10"])
                .arg(&mp3)
                .status()
                .unwrap()
                .success());
            let (o, e) = run_file_tool(std::slice::from_ref(&mp3), &FileTool::Trim(2.0, 5.0), Some(ff));
            assert!(e.is_empty(), "{e:?}");
            assert!((dur(&o[0], ff) - 3.0).abs() < 0.2, "trimmed to ~3 s");
            let (o, e) = run_file_tool(std::slice::from_ref(&mp3), &FileTool::SplitEvery(4), Some(ff));
            assert!(e.is_empty(), "{e:?}");
            assert_eq!(o.len(), 3, "10 s in 4 s pieces: {o:?}");
            assert!(o[0].to_string_lossy().contains("part 1"));
            let (m, e) = run_file_tool(&[mp3.clone(), mp3.clone()], &FileTool::Merge, Some(ff));
            assert!(e.is_empty(), "{e:?}");
            assert!((dur(&m[0], ff) - 20.0).abs() < 0.5, "merged ~20 s");
            assert!(mp3.exists(), "original untouched");
        }
        // PDF: two one-page PDFs → merge → split back into two pages
        let icon = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/icon-1024.png");
        let (p1, p2) = (d.join("one.pdf"), d.join("two.pdf"));
        for p in [&p1, &p2] {
            sips(&["-s", "format", "pdf", icon, "--out", &p.to_string_lossy()]).unwrap();
        }
        let (m, e) = run_file_tool(&[p1.clone(), p2.clone()], &FileTool::Merge, tools.ffmpeg);
        assert!(e.is_empty(), "{e:?}");
        let (pages, e) = run_file_tool(std::slice::from_ref(&m[0]), &FileTool::SplitPages, tools.ffmpeg);
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(pages.len(), 2, "{pages:?}");
        assert!(pages.iter().all(|p| std::fs::read(p).unwrap().starts_with(b"%PDF")));
        // without ffmpeg an audio tool says how to get it instead of failing silently
        let (o, e) = run_file_tool(&[d.join("a.mp3")], &FileTool::Trim(0.0, 1.0), None);
        assert!(o.is_empty() && e[0].contains("brew install ffmpeg"), "{e:?}");
    }

    #[test]
    fn missing_tool_reports_instead_of_running() {
        let d = tmp("missing");
        let src = d.join("p.png");
        std::fs::write(&src, b"x").unwrap();
        let t = targets(&src, &Tools::default()).into_iter().find(|t| t.ext == "webp").unwrap();
        let (out, err) = run_conversion(&[src], &t, &Tools::default());
        assert!(out.is_empty());
        assert!(err[0].contains("cwebp"));
    }
}

#[cfg(test)]
mod hotkey_tests {
    use super::*;

    #[test]
    fn hotkey_label_spacing() {
        assert_eq!(hotkey_label("Alt+Space"), "⌥ Space");
        assert_eq!(hotkey_label("Shift+Super+KeyK"), "⇧⌘K");
        assert_eq!(hotkey_label("Control+F3"), "⌃ F3");
    }
}

#[cfg(test)]
mod backup_tests {
    use super::*;

    #[test]
    fn backup_excludes_clipboard_history_and_includes_user_data() {
        let base = std::env::temp_dir().join(format!("ferrocast-bk-{}", uuid::Uuid::new_v4()));
        let root = base.join("Ferrocast");
        std::fs::create_dir_all(root.join("clipboard-images")).unwrap();
        for f in ["snippets.json", "settings.json", "clipboard.json", "ferrocast.log", "clipboard-images/a.png"] {
            std::fs::write(root.join(f), "x").unwrap();
        }
        let out = export_backup(&root, &base.join("out"), "T1").unwrap();
        let list = String::from_utf8(std::process::Command::new("/usr/bin/tar").arg("-tzf").arg(&out).output().unwrap().stdout).unwrap();
        assert!(list.contains("snippets.json") && list.contains("settings.json"), "{list}");
        assert!(!list.contains("clipboard.json") && !list.contains("clipboard-images") && !list.contains("ferrocast.log"), "{list}");
        let _ = std::fs::remove_dir_all(base);
    }
}

#[cfg(test)]
mod ai_tests {
    use super::*;

    /// Warm session: two questions through one process. cargo test -p ferrocast claude_session_live -- --ignored --nocapture
    #[test]
    #[ignore]
    fn claude_session_live() {
        let ctx = egui::Context::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let bus = Bus { tx, ctx };
        let settings = Settings { ai_provider: AiProvider::ClaudeCode, ..Default::default() };
        let s = ClaudeSession::start(&settings, bus).unwrap();
        let answer = |run: u64, q: &str| {
            let t = std::time::Instant::now();
            s.ask(run, q).unwrap();
            let mut text = String::new();
            loop {
                match rx.recv_timeout(Duration::from_secs(60)).expect("answer in time") {
                    Msg::AiChunk { run: r, text: t2 } if r == run => text.push_str(&t2),
                    Msg::AiDone { run: r, error } if r == run => {
                        assert!(error.is_none(), "{error:?}");
                        break;
                    }
                    _ => {}
                }
            }
            println!("run {run}: {:?} in {:?}", text, t.elapsed());
            (text, t.elapsed())
        };
        let (a, t1) = answer(1, "Reply with exactly the word: ping");
        let (b, t2) = answer(2, "Now reply with exactly the word you said before, in uppercase.");
        assert!(a.to_lowercase().contains("ping"));
        assert!(b.contains("PING"), "{b}");
        assert!(t2 < t1, "second turn should skip the CLI boot: {t1:?} vs {t2:?}");
        s.kill();
    }

    /// Talks to the real Claude Code CLI; run by hand: cargo test -p ferrocast claude_code_live -- --ignored --nocapture
    #[test]
    #[ignore]
    fn claude_code_live() {
        let settings = Settings { ai_provider: AiProvider::ClaudeCode, ..Default::default() };
        let mut out = String::new();
        let r = ai_stream_with_key(&settings, None, &[], "Reply with exactly the word: pong", |t| out.push_str(t));
        println!("result={r:?} text={out:?}");
        assert!(r.is_ok(), "{r:?}");
        assert!(out.to_lowercase().contains("pong"), "{out}");
    }
    use std::io::{Read, Write};

    /// One-shot HTTP server returning `body`; yields the raw request it received.
    fn serve(content_type: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = vec![0u8; 16384];
            let mut n = 0;
            while n < buf.len() {
                let k = s.read(&mut buf[n..]).unwrap();
                if k == 0 {
                    break;
                }
                n += k;
                if buf[..n].ends_with(b"}") {
                    break;
                }
            }
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}");
            s.write_all(resp.as_bytes()).unwrap();
            String::from_utf8_lossy(&buf[..n]).to_string()
        });
        (url, h)
    }

    fn collect(url: &str, key: Option<&str>, history: &[(String, String)], q: &str) -> (Vec<String>, Result<(), String>) {
        let mut parts = vec![];
        let settings = Settings { ai_base_url: url.to_string(), ai_model: "test-model".into(), ..Default::default() };
        let r = ai_stream_with_key(&settings, key.map(str::to_string), history, q, |t| parts.push(t.to_string()));
        (parts, r)
    }

    #[test]
    fn streams_deltas_in_order_and_sends_key_and_history() {
        let body = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\ndata: [DONE]\n\n";
        let (url, h) = serve("text/event-stream", body);
        let hist = vec![("earlier q".to_string(), "earlier a".to_string())];
        let (parts, r) = collect(&url, Some("sk-test-123"), &hist, "say \"hi\"\nnow");
        assert_eq!(r, Ok(()));
        assert_eq!(parts, ["Hel", "lo"]);
        let req = h.join().unwrap();
        assert!(req.starts_with("POST /v1/chat/completions"), "{req}");
        assert!(req.to_lowercase().contains("authorization: bearer sk-test-123"));
        assert!(req.contains("\"stream\":true"));
        assert!(req.contains("earlier a"), "history must be sent: {req}");
        assert!(req.contains("say \\\"hi\\\"\\nnow"), "question must arrive JSON-escaped: {req}");
    }

    #[test]
    fn non_streaming_server_still_works() {
        let (url, h) = serve("application/json", r#"{"choices":[{"message":{"content":"whole answer"}}]}"#);
        let (parts, r) = collect(&url, None, &[], "hi");
        assert_eq!((parts.concat(), r), ("whole answer".to_string(), Ok(())));
        let _ = h.join();
    }

    #[test]
    fn provider_error_is_readable() {
        let (url, h) = serve("application/json", r#"{"error":{"message":"model 'x' not found"}}"#);
        assert_eq!(collect(&url, None, &[], "hi").1, Err("model 'x' not found".to_string()));
        let _ = h.join();
    }

    #[test]
    fn unreachable_server_explains_what_to_do() {
        let err = collect("http://127.0.0.1:1/v1", None, &[], "hi").1.unwrap_err();
        assert!(err.contains("Cannot reach"), "{err}");
    }
}

#[cfg(test)]
mod coverage_tests {
    use ferrocast_core::{convert::targets, matrix};

    /// Which of Tangerine's 275 (input → output) pairs does `targets()` offer today?
    #[test]
    fn report_matrix_coverage() {
        let tools = ferrocast_core::convert::Tools::detect();
        let (mut have, mut missing) = (0, vec![]);
        for i in matrix::inputs() {
            let probe = std::path::PathBuf::from(format!("/nonexistent/x.{i}"));
            let offered: Vec<String> = targets(&probe, &tools).iter().map(|t| t.ext.replace("tar.gz", "gz")).collect();
            for o in matrix::outputs_for(i) {
                if offered.iter().any(|x| x == o) {
                    have += 1;
                } else {
                    missing.push(format!("{i}→{o}"));
                }
            }
        }
        eprintln!("COVERAGE {have}/275; missing {}: {}", missing.len(), missing.join(" "));
    }
}
