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
    Rates(ferrocast_core::currency::Rates),
    ImportFile(PathBuf),
    AiChunk { run: u64, text: String },
    AiDone { run: u64, error: Option<String> },
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

pub fn ai_key() -> Option<String> {
    let out =
        std::process::Command::new("/usr/bin/security").args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|k| !k.is_empty())
}

/// Store (or with an empty key, delete) the API key. The key is briefly visible in `security`'s argv.
pub fn set_ai_key(key: &str) -> bool {
    let key = key.trim();
    let mut cmd = std::process::Command::new("/usr/bin/security");
    if key.is_empty() {
        cmd.args(["delete-generic-password", "-s", KEYCHAIN_SERVICE]);
    } else {
        cmd.args(["add-generic-password", "-U", "-s", KEYCHAIN_SERVICE, "-a", "ferrocast", "-w", key]);
    }
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// Streaming chat completion; `on_delta` receives text as it arrives. The request
/// (including the key) goes to curl via stdin, never argv.
pub fn ai_stream(
    base_url: &str,
    model: &str,
    api_key: Option<&str>,
    history: &[(String, String)],
    question: &str,
    mut on_delta: impl FnMut(&str),
) -> Result<(), String> {
    use ferrocast_core::ai::{self, Sse};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::process::{Command, Stdio};
    let url = ai::endpoint(base_url);
    let cfg = ai::curl_config(&url, api_key, &ai::build_chat_request(model, history, question));
    let mut child = Command::new("/usr/bin/curl")
        .args(["-sS", "-N", "-m", "300", "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start curl: {e}"))?;
    child.stdin.take().ok_or("no stdin")?.write_all(cfg.as_bytes()).map_err(|e| e.to_string())?;
    let mut got = false;
    let mut raw = String::new();
    for line in BufReader::new(child.stdout.take().ok_or("no stdout")?).lines().map_while(Result::ok) {
        match ai::parse_sse_line(&line) {
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
                if !line.starts_with("data:") && !line.starts_with(':') {
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
    // No SSE deltas: a plain JSON answer or error body, or a connection failure.
    if !raw.trim().is_empty() {
        return ai::parse_response(&raw).map(|t| on_delta(&t));
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    Err(if err.contains("Failed to connect") || err.contains("Connection refused") {
        format!("Cannot reach {base_url}. Is your AI server (e.g. Ollama) running? Change it in Settings.")
    } else if err.trim().is_empty() {
        "The AI server returned an empty answer".to_string()
    } else {
        err.trim().to_string()
    })
}

pub fn ask_ai(settings: &Settings, history: Vec<(String, String)>, question: String, run: u64, bus: Bus) {
    let (base, model) = (settings.ai_base_url.clone(), settings.ai_model.clone());
    std::thread::spawn(move || {
        let key = ai_key();
        let b = bus.clone();
        let result = ai_stream(&base, &model, key.as_deref(), &history, &question, |t| b.send(Msg::AiChunk { run, text: t.to_string() }));
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

/// Ask the user for a JSON file with a native dialog (blocks; call from a thread).
pub fn choose_json_file() -> Option<PathBuf> {
    let out = ferrocast_macos::system::osascript(
        "POSIX path of (choose file of type {\"public.json\"} with prompt \"Choose a snippets JSON file\")",
    )
    .ok()?;
    (!out.is_empty()).then(|| PathBuf::from(out))
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
        let r = ai_stream(url, "test-model", key, history, q, |t| parts.push(t.to_string()));
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
