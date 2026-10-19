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
            let cmd = match plan(f, target, &out, tools) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            let res = std::process::Command::new(&cmd[0]).args(&cmd[1..]).output();
            match res {
                Ok(o) if o.status.success() => {
                    if target.engine == Engine::Soffice {
                        let written = soffice_written_path(f, &out, target.ext);
                        if written != out && written.exists() {
                            let _ = std::fs::rename(&written, &out);
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
                    errors.push(format!(
                        "{}: {}",
                        f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                        err.lines().last().unwrap_or("conversion failed").trim()
                    ));
                }
                Err(e) => errors.push(format!("could not run {}: {e}", cmd[0])),
            }
        }
        (outputs, errors)
    }
}

/// Pixel size of an image via sips.
fn image_size(p: &std::path::Path) -> Option<(u32, u32)> {
    let o = std::process::Command::new("/usr/bin/sips").args(["-g", "pixelWidth", "-g", "pixelHeight"]).arg(p).output().ok()?;
    let t = String::from_utf8_lossy(&o.stdout);
    let get = |k: &str| t.lines().find_map(|l| l.trim().strip_prefix(k).map(|v| v.trim().parse::<u32>().ok())).flatten();
    Some((get("pixelWidth:")?, get("pixelHeight:")?))
}

fn sips(args: &[&str]) -> Result<(), String> {
    let o = std::process::Command::new("/usr/bin/sips").args(args).output().map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("sips failed").trim().to_string())
    }
}

/// Run one image tool on each file (or once for Combine). Returns outputs and failures.
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
            ImageOp::CombinePdf => unreachable!(),
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
        if let Some(f) = outs.first() {
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

    #[test]
    fn image_tools_on_real_files() {
        use ferrocast_core::convert::ImageOp;
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
    }

    #[test]
    fn missing_tool_reports_instead_of_running() {
        let d = tmp("missing");
        let src = d.join("p.png");
        std::fs::write(&src, b"x").unwrap();
        let t = targets(&src, &Tools::default()).into_iter().find(|t| t.ext == "webp").unwrap();
        let (out, err) = run_conversion(&[src], &t, &Tools::default());
        assert!(out.is_empty());
        assert!(err[0].contains("ffmpeg"));
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
