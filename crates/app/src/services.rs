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
    Script { run: u64, text: String, code: i32 },
    Toast(String),
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
    let (settings_retention, ) = (shared.settings.read().unwrap().clipboard_retention,);
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

pub fn start_clipboard(shared: Arc<Shared>, bus: Bus) {
    std::thread::Builder::new()
        .name("clipboard-monitor".into())
        .spawn(move || {
            let mut last = clipboard::change_count();
            loop {
                std::thread::sleep(Duration::from_millis(400));
                objc2::rc::autoreleasepool(|_| {
                    let c = clipboard::change_count();
                    let record_change = crate::clipboard_policy::should_record(
                        c, last, shared.ignored_clipboard_count.load(Ordering::SeqCst) as isize,
                    );
                    last = c;
                    if !record_change { return; }
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
        if let Some(t) = clipboard::read_text() { v.push(t); }
    }
    for e in shared.clipboard.lock().unwrap().iter().filter(|e| matches!(e.kind, ClipKind::Text | ClipKind::Link | ClipKind::Color)).take(10) {
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
        if clipboard::is_concealed() { return; }
        let Some(sn) = s2.snippets.read().unwrap().iter().find(|s| s.keyword == kw).cloned() else { return };
        let ctx = placeholders::Context { clipboard: clipboard_texts(&s2), ..Default::default() };
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
    spec.split('+')
        .map(|t| match t.to_uppercase().as_str() {
            "ALT" | "OPTION" => "⌥".to_string(),
            "SHIFT" => "⇧".to_string(),
            "CONTROL" | "CTRL" => "⌃".to_string(),
            "SUPER" | "CMD" | "COMMAND" => "⌘".to_string(),
            "SPACE" => "Space".to_string(),
            _ => t.trim_start_matches("Key").trim_start_matches("Digit").to_string(),
        })
        .collect()
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
    tray_icon::TrayIconBuilder::new().with_menu(Box::new(menu)).with_icon(icon).with_icon_as_template(true).with_tooltip("Ferrocast").build().ok()
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
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
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
