//! Raycast extensions in the launcher: supervises the Node sidecar of the running command,
//! turns its render tree into launcher rows and actions, and answers its clipboard / storage / open
//! requests. Pure parts (manifests, protocol, view model, storage) live in `ferrocast_core::extensions`.

use super::*;
use ferrocast_core::extensions::hostcalls;
use ferrocast_core::extensions::manifest::{self, Manifest, Mode};
use ferrocast_core::extensions::node::{self, NodeInfo};
use ferrocast_core::extensions::protocol::{Environment, HostMsg, RuntimeMsg};
use ferrocast_core::extensions::sidecar::{Event, Sidecar};
use ferrocast_core::extensions::storage;
use ferrocast_core::extensions::view::{self, FormState, IconSpec, ListView, ViewModel};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A command that runs without a view is killed after this long; a view that never draws, after
/// `FIRST_RENDER_LIMIT`.
const NO_VIEW_LIMIT: Duration = Duration::from_secs(60);
const FIRST_RENDER_LIMIT: Duration = Duration::from_secs(30);

struct Session {
    sc: Sidecar,
    id: u64,
    ext: String,
    title: String,
    mode: Mode,
    assets: PathBuf,
    got_render: bool,
    /// Start of the time limit's clock; a no-view command relaunched by itself keeps the original.
    origin: Instant,
    /// Set when the session ends: the `osascript` children of its host calls are killed.
    cancel: Arc<AtomicBool>,
}

/// Why a launch did not happen (`NO_NODE` also sends the user to Settings).
const NO_NODE: &str = "Node.js 22 or newer is needed: install it or download it in Settings → Integrations";

impl ExtState {
    /// A discovery scan finished.
    pub(crate) fn node_found(&mut self, n: Option<NodeInfo>) {
        self.scanning = false;
        if n.is_some() {
            self.node = n;
        }
    }

    /// The download finished (`None`: it failed).
    pub(crate) fn download_finished(&mut self, n: Option<NodeInfo>) {
        self.downloading = false;
        if n.is_some() {
            self.node = n;
        }
    }

    /// Should the Settings section look for Node again (one that was installed since)?
    pub(crate) fn should_rescan(&self, now: Instant) -> bool {
        self.node.is_none()
            && !self.downloading
            && !self.scanning
            && self.last_scan.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(5))
    }

    pub(crate) fn begin_scan(&mut self, now: Instant) {
        self.scanning = true;
        self.last_scan = Some(now);
    }
}

/// Test hooks: a session around a process that does nothing, and a look at the private state.
#[cfg(test)]
impl ExtState {
    pub(crate) fn test_install_session(&mut self, id: u64, view: bool, age: Duration) {
        let sc = Sidecar::spawn(std::path::Path::new("/usr/bin/true"), std::path::Path::new("/nonexistent"), &std::env::temp_dir(), |_| {})
            .expect("spawn /usr/bin/true");
        self.sess = Some(Session {
            sc,
            id,
            ext: "e".into(),
            title: "E".into(),
            mode: if view { Mode::View } else { Mode::NoView },
            assets: PathBuf::new(),
            got_render: true,
            origin: Instant::now() - age,
            cancel: Arc::new(AtomicBool::new(false)),
        });
    }
    pub(crate) fn test_set_manifests(&mut self, m: Vec<Manifest>) {
        self.manifests = m;
    }
    pub(crate) fn test_set_base(&mut self, b: usize) {
        self.base = b;
    }
    pub(crate) fn test_has_session(&self) -> bool {
        self.sess.is_some()
    }
    pub(crate) fn test_pending(&self) -> hostcalls::Pending {
        self.pending.clone()
    }
    pub(crate) fn test_cancel_flag(&self) -> Option<Arc<AtomicBool>> {
        self.sess.as_ref().map(|s| s.cancel.clone())
    }
    pub(crate) fn test_set_nav_title(&mut self, t: &str) {
        if let Some(ViewModel::List(l)) = self.views.last_mut() {
            l.title = t.into();
        } else {
            self.views = vec![ViewModel::List(ListView { title: t.into(), ..Default::default() })];
        }
    }
    /// The extension renders the on-screen form again with `value` for field `id`.
    pub(crate) fn test_render_echo(&mut self, id: &str, value: Value) {
        if let Some(ViewModel::Form(f)) = self.views.last_mut() {
            f.fields.iter_mut().filter(|x| x.id == id).for_each(|x| x.initial = value.clone());
            let f = f.clone();
            self.form.sync(&f);
        }
    }
    /// A form on screen with fields `(id, on_change, controlled)`.
    pub(crate) fn test_set_form(&mut self, fields: Vec<(&str, bool, bool)>) {
        use view::{FieldKind, FormField, FormView};
        let fields = fields
            .into_iter()
            .map(|(id, on_change, controlled)| FormField {
                id: id.into(),
                kind: FieldKind::Text,
                title: id.into(),
                placeholder: String::new(),
                info: String::new(),
                error: String::new(),
                options: vec![],
                initial: Value::String(String::new()),
                controlled,
                on_change,
                text: String::new(),
            })
            .collect();
        self.views = vec![ViewModel::Form(FormView { fields, ..Default::default() })];
    }
}

#[derive(Default)]
pub struct ExtState {
    manifests: Vec<Manifest>,
    loaded: bool,
    node: Option<NodeInfo>,
    node_checked: bool,
    scanning: bool,
    last_scan: Option<Instant>,
    pub(crate) downloading: bool,
    sess: Option<Session>,
    next_id: u64,
    /// One view model per navigation depth of the running command.
    views: Vec<ViewModel>,
    /// Index in `App::stack` of the first view of the running command.
    base: usize,
    sent_query: String,
    sent_sel: Option<String>,
    /// What the user typed into the form on screen.
    pub(crate) form: FormState,
    /// Dropdown option the user picked, by the dropdown's `_id`.
    dropdown_choice: std::collections::HashMap<String, String>,
    /// Host calls that a thread is still answering, across relaunches (never reset by a launch).
    pending: hostcalls::Pending,
    /// `launchCommand` requests of the last few seconds.
    launches: hostcalls::LaunchLimiter,
    /// The preferences form of one extension while Settings → Integrations shows it.
    prefs: Option<PrefsEdit>,
}

/// What the user is editing in the preferences form of an extension.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PrefsEdit {
    pub ext: String,
    /// One value per preference name: a string, or a bool for a checkbox.
    pub values: serde_json::Map<String, Value>,
    /// Why the form was opened ("" when the user opened it).
    pub notice: String,
}

impl ExtState {
    #[cfg(test)]
    pub(crate) fn prefs_edit(&self) -> Option<&PrefsEdit> {
        self.prefs.as_ref()
    }
    #[cfg(test)]
    pub(crate) fn prefs_edit_mut(&mut self) -> Option<&mut PrefsEdit> {
        self.prefs.as_mut()
    }
    pub(crate) fn take_prefs(&mut self) -> Option<PrefsEdit> {
        self.prefs.take()
    }
    pub(crate) fn put_prefs(&mut self, p: Option<PrefsEdit>) {
        self.prefs = p;
    }
    pub(crate) fn manifests(&self) -> &[Manifest] {
        &self.manifests
    }
}

/// How a runtime request is answered.
pub(crate) enum Reply {
    /// Right away.
    Now(Result<Value, String>),
    /// A background thread sends `Event::Reply` when it has the answer.
    Later,
    /// The request replaced the running command; nobody is waiting any more.
    Replaced,
}

/// Folder Raycast installs extensions into (debug and test builds only: `FERROCAST_RAYCAST_EXT_DIR`
/// overrides it; a release build always reads `~/.config/raycast/extensions`).
fn raycast_extensions_dir() -> Option<PathBuf> {
    #[cfg(any(debug_assertions, test))]
    if let Some(d) = std::env::var_os("FERROCAST_RAYCAST_EXT_DIR") {
        return Some(PathBuf::from(d));
    }
    dirs::home_dir().map(|h| h.join(".config/raycast/extensions"))
}

pub(crate) fn map_shortcut(s: &view::Shortcut) -> Option<(Modifiers, Key)> {
    let mut m = Modifiers::NONE;
    for x in &s.modifiers {
        match x.as_str() {
            "cmd" => m.command = true,
            "shift" => m.shift = true,
            "opt" | "alt" => m.alt = true,
            "ctrl" => m.ctrl = true,
            _ => {}
        }
    }
    let name = match s.key.to_lowercase().as_str() {
        "return" | "enter" => "Enter".to_string(),
        "delete" | "backspace" => "Backspace".to_string(),
        "arrowdown" => "ArrowDown".into(),
        "arrowup" => "ArrowUp".into(),
        "arrowleft" => "ArrowLeft".into(),
        "arrowright" => "ArrowRight".into(),
        "escape" => "Escape".into(),
        "tab" => "Tab".into(),
        "space" => "Space".into(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        _ => return None,
    };
    Key::from_name(&name).map(|k| (m, k))
}

fn builtin_glyph(name: &str) -> &'static str {
    match name {
        "Star" | "StarCircle" => "sf:star",
        "Clipboard" => "sf:doc.on.clipboard",
        "Document" | "BlankDocument" | "Text" => "sf:doc.text",
        "Link" => "sf:link",
        "Globe" | "Network" => "sf:globe",
        "Terminal" | "Code" => "sf:terminal",
        "Folder" => "sf:folder",
        "Image" | "Camera" => "sf:photo",
        "Calendar" | "Clock" => "sf:calendar",
        "Person" | "PersonCircle" => "sf:person",
        "Trash" | "Xmark" => "sf:trash",
        "Pencil" => "sf:pencil",
        "MagnifyingGlass" => "sf:magnifyingglass",
        "Checkmark" | "CheckCircle" => "sf:checkmark.circle",
        "ExclamationMark" | "Warning" => "sf:exclamationmark.triangle",
        "Bookmark" => "sf:bookmark",
        "Gear" | "Cog" => "sf:gearshape",
        _ => "sf:square.grid.2x2",
    }
}

/// The Settings sentence next to the toggle (security, S5): everything an extension can reach.
pub(crate) const EXT_DISCLOSURE: &str =
    "Extensions are programs written by other people. They run on this Mac with your permissions: they can read your files, use the network, see what you copy, read the text you have selected in other apps and see the list of apps you use. Turn this on only for extensions you installed and trust.";

const EXT_GLYPH_COLOR: [u8; 3] = [150, 110, 255];

impl App {
    // ------------------------------------------------------------ discovery (opt-in)

    /// Load the installed manifests once, only while the setting is on. Nothing is read otherwise.
    /// Settings → Integrations is showing and no Node is known: look again (a Node installed since).
    pub(crate) fn ext_rescan_node_if_needed(&mut self) {
        if !self.settings.run_raycast_extensions || !self.ext.should_rescan(Instant::now()) {
            return;
        }
        self.ext.node_checked = true;
        self.ext.begin_scan(Instant::now());
        let (dir, bus) = (self.store.root.clone(), self.bus.clone());
        std::thread::spawn(move || bus.send(Msg::ExtNodeFound(node::find_node(&dir))));
    }

    pub(crate) fn ext_ensure_loaded(&mut self) {
        if !self.settings.run_raycast_extensions || self.ext.loaded {
            return;
        }
        self.ext.loaded = true;
        self.ext.manifests = raycast_extensions_dir().map(|d| manifest::discover(&d)).unwrap_or_default();
        if !self.ext.node_checked {
            self.ext_rescan_node_if_needed();
        }
    }

    /// The toggle changed.
    pub(crate) fn ext_settings_changed(&mut self) {
        self.ext_kill();
        self.ext.loaded = false;
        self.ext.manifests.clear();
        self.rebuild();
    }

    /// Is this extension switched on (they all are until the user turns one off)?
    pub(crate) fn ext_enabled(&self, name: &str) -> bool {
        !self.settings.raycast_extensions_disabled.iter().any(|n| n == name)
    }

    /// Titles of the required preferences of a command that have no value yet.
    pub(crate) fn ext_missing_required(&self, ext: &str, cmd: &str) -> Vec<String> {
        let Some(m) = self.ext.manifests.iter().find(|m| m.name == ext) else { return vec![] };
        let Some(c) = m.commands.iter().find(|c| c.name == cmd) else { return vec![] };
        m.missing_required(c, &storage::load_preferences(&self.store.root, &m.name))
    }

    /// Show the preferences form of an extension in Settings → Integrations (`notice`: why, or "").
    pub(crate) fn ext_open_prefs(&mut self, ext: &str, notice: &str) {
        let Some(m) = self.ext.manifests.iter().find(|m| m.name == ext) else { return };
        let saved = storage::load_preferences(&self.store.root, &m.name);
        let values = m.all_preferences().into_iter().map(|p| (p.name.clone(), p.form_value(&saved))).collect();
        self.ext.prefs = Some(PrefsEdit { ext: ext.to_string(), values, notice: notice.to_string() });
    }

    /// Write the form to `extensions/<name>/preferences.json` and close it. Only key names are logged, never values.
    pub(crate) fn ext_save_prefs(&mut self) {
        let Some(edit) = self.ext.prefs.take() else { return };
        match storage::save_preferences(&self.store.root, &edit.ext, &edit.values) {
            Ok(()) => {
                let line = self.ext.manifests.iter().find(|m| m.name == edit.ext).map(|m| m.logged_keys(&edit.values)).unwrap_or_default();
                self.ext_log(&format!("preferences of {} saved (keys: {line})", edit.ext));
                self.toast("Preferences saved");
            }
            Err(e) => {
                self.toast(format!("Could not save the preferences: {e}"));
                self.ext.prefs = Some(edit);
            }
        }
    }

    /// Launcher rows for the commands of installed extensions.
    pub(crate) fn ext_command_items(&self) -> Vec<Item> {
        if !self.settings.run_raycast_extensions {
            return vec![];
        }
        self.ext
            .manifests
            .iter()
            .filter(|m| self.ext_enabled(&m.name))
            .flat_map(|m| {
                m.commands.iter().map(move |c| Item {
                    id: format!("ext:{}:{}", m.name, c.name),
                    title: c.title.clone(),
                    subtitle: m.title.clone(),
                    icon: Icon::Glyph("sf:puzzlepiece.extension", EXT_GLYPH_COLOR),
                    accessory: "Raycast Extension".into(),
                    keywords: vec![m.title.clone(), m.name.clone()],
                    kind: Kind::ExtCommand { ext: m.name.clone(), cmd: c.name.clone() },
                })
            })
            .collect()
    }

    pub(crate) fn ext_node_status(&self) -> (String, bool) {
        if self.ext.downloading {
            return ("Downloading and checking Node.js…".into(), false);
        }
        match (&self.ext.node, self.ext.node_checked) {
            (Some(n), _) => (format!("Node.js {}.{}.{} · {}", n.version.0, n.version.1, n.version.2, n.path.display()), true),
            (None, true) => ("Node.js 22 or newer was not found on this Mac.".into(), false),
            (None, false) => ("Turn the setting on to look for Node.js.".into(), false),
        }
    }

    /// Settings button: fetch the official Node.js into the app's data folder. Never automatic.
    pub(crate) fn ext_download_node(&mut self) {
        if self.ext.downloading {
            return;
        }
        self.ext.downloading = true;
        let (dir, bus) = (self.store.root.clone(), self.bus.clone());
        std::thread::spawn(move || {
            let r = node::download_node(&dir).map(|p| node::probe(&p));
            bus.send(Msg::ExtNodeDownloaded(r.and_then(|n| n.ok_or_else(|| "the downloaded node does not run".to_string()))));
        });
    }

    // ------------------------------------------------------------ launching and supervising

    fn ext_log(&self, line: &str) {
        instance::log(&self.store.root, &format!("extension: {line}"));
    }

    pub(crate) fn ext_launch(&mut self, ext: &str, cmd: &str) {
        let missing = self.ext_missing_required(ext, cmd);
        if !missing.is_empty() {
            let title = self.ext.manifests.iter().find(|m| m.name == ext).map(|m| m.title.clone()).unwrap_or_default();
            self.ext_open_prefs(ext, &format!("{title} needs a value for: {}", missing.join(", ")));
            self.notion_open_settings_tab();
            return;
        }
        if let Err(e) = self.ext_launch_with(ext, cmd, Default::default()) {
            if e == NO_NODE {
                self.notion_open_settings_tab();
            }
            self.toast(e);
        }
    }

    /// Start a command. Nothing of the running one (its screens, its process) is touched until every
    /// check has passed: a failed launch leaves it as it was and returns the reason.
    pub(crate) fn ext_launch_with(&mut self, ext: &str, cmd: &str, arguments: serde_json::Map<String, Value>) -> Result<(), String> {
        let m = self.ext.manifests.iter().find(|m| m.name == ext).cloned().ok_or_else(|| format!("unknown extension {ext}"))?;
        if !self.ext_enabled(&m.name) {
            return Err(format!("{} is turned off in Settings → Integrations", m.title));
        }
        let c = m.commands.iter().find(|c| c.name == cmd).cloned().ok_or_else(|| format!("{} has no command {cmd}", m.title))?;
        let node = self.ext.node.clone().ok_or_else(|| NO_NODE.to_string())?;
        let entry = m.entry(&c).map_err(|e| format!("{}: {e}", m.title))?;
        if !entry.is_file() {
            return Err(format!("{} is not built: {}.js is missing", m.title, c.name));
        }
        let saved = storage::load_preferences(&self.store.root, &m.name);
        let missing = m.missing_required(&c, &saved);
        if !missing.is_empty() {
            return Err(format!("Required preferences are not set ({}): fill them in at Settings → Integrations", missing.join(", ")));
        }
        let runtime =
            ferrocast_core::extensions::write_runtime(&self.store.root).map_err(|e| format!("Cannot write the extensions runtime: {e}"))?;
        let view_mode = c.mode == Mode::View;
        let prev = self.ext.sess.as_ref().map(|s| (s.mode == Mode::View, s.origin));
        let origin = hostcalls::carried_origin(prev, view_mode, Instant::now());
        if self.ext.sess.is_some() {
            // a command started from inside another one replaces its screens
            self.stack.truncate(self.ext.base.max(1));
        }
        self.ext_kill();
        let support = storage::support_dir(&self.store.root, &m.name);
        let _ = std::fs::create_dir_all(&support);
        let assets = m.dir.join("assets");
        if view_mode && !self.visible {
            // started by another command while the launcher was hidden
            self.show();
        }
        if !view_mode {
            self.hide();
            self.reset_to_root();
        }
        self.ext.next_id += 1;
        let id = self.ext.next_id;
        let bus = self.bus.clone();
        let sc = Sidecar::spawn(&node.path, &runtime, &support, move |e| bus.send(Msg::Ext(id, e)));
        let mut sc = match sc {
            Ok(sc) => sc,
            Err(e) => return Err(format!("Cannot start Node.js: {e}")),
        };
        let launch = HostMsg::Launch {
            entry: entry.to_string_lossy().into_owned(),
            extension: m.name.clone(),
            command: c.name.clone(),
            title: c.title.clone(),
            mode: if view_mode { "view" } else { "no-view" }.into(),
            arguments,
            preferences: m.preference_values(&c, &saved),
            environment: Environment {
                extension_name: m.name.clone(),
                command_name: c.name.clone(),
                support_path: support.to_string_lossy().into_owned(),
                assets_path: assets.to_string_lossy().into_owned(),
                is_development: false,
                raycast_version: "1.100.0".into(),
                launch_type: "userInitiated".into(),
            },
        };
        if let Err(e) = sc.send(&launch) {
            return Err(format!("Cannot talk to Node.js: {e}"));
        }
        self.ext_log(&format!("started {}/{} (session {id})", m.name, c.name));
        self.ext.sess = Some(Session {
            sc,
            id,
            ext: m.name.clone(),
            title: c.title.clone(),
            mode: c.mode,
            assets,
            got_render: false,
            origin,
            cancel: Arc::new(AtomicBool::new(false)),
        });
        self.ext.views.clear();
        self.ext.sent_query.clear();
        self.ext.sent_sel = None;
        self.ext.form = FormState::default();
        self.ext.dropdown_choice.clear();
        if view_mode {
            self.ext.base = self.stack.len();
            self.push(View::ExtList);
        }
        Ok(())
    }

    /// Stop the running command's process and forget its screens.
    pub(crate) fn ext_kill(&mut self) {
        if let Some(s) = self.ext.sess.take() {
            // the session's `osascript` children (alerts wait up to 300 s) are killed with it
            s.cancel.store(true, Ordering::SeqCst);
            self.ext_log(&format!("stopped {} (session {})", s.ext, s.id));
        }
        self.ext.views.clear();
    }

    /// Back to the root: a command with a view dies with its screens (a no-view command keeps going).
    pub(crate) fn ext_leave_root(&mut self) {
        if self.ext.sess.as_ref().is_some_and(|s| s.mode == Mode::View) {
            self.ext_kill();
        }
    }

    fn ext_top_depth(&self) -> Option<usize> {
        matches!(self.view(), View::ExtList | View::ExtDetail | View::ExtForm).then(|| self.stack.len().saturating_sub(self.ext.base))
    }

    /// Called by `pop()` before it leaves an extension screen.
    pub(crate) fn ext_before_pop(&mut self) {
        let Some(depth) = self.ext_top_depth() else { return };
        if depth > 1 {
            self.ext.views.truncate(depth - 1);
            self.ext.sent_query.clear();
            self.ext.sent_sel = None;
            if let Some(s) = self.ext.sess.as_mut() {
                let _ = s.sc.send(&HostMsg::Pop);
            }
        } else {
            self.ext_kill();
        }
    }

    /// Per-frame housekeeping: timeouts, and forwarding search text / selection to the runtime.
    pub(crate) fn ext_tick(&mut self) {
        let Some(s) = self.ext.sess.as_mut() else { return };
        let age = s.origin.elapsed();
        let (view_mode, got) = (s.mode == Mode::View, s.got_render);
        if hostcalls::view_expired(view_mode, self.visible, self.hidden_at.elapsed()) {
            // hidden longer than the launcher keeps its screens (see `show`): nothing may keep running
            let title = s.title.clone();
            self.ext_log(&format!("{title}: hidden for over {} s, stopped", hostcalls::HIDDEN_VIEW_LIMIT.as_secs()));
            self.reset_to_root();
            return;
        }
        if (!view_mode && age > NO_VIEW_LIMIT) || (view_mode && !got && age > FIRST_RENDER_LIMIT) {
            let title = s.title.clone();
            self.ext_kill();
            self.toast(format!("{title} did not finish in time and was stopped"));
            return;
        }
        if view_mode && self.ext_top_depth().is_none() {
            // its screens are gone (root reached some other way)
            self.ext_kill();
            return;
        }
        if !matches!(self.view(), View::ExtList) {
            return;
        }
        let q = self.query.clone();
        if q != self.ext.sent_query {
            self.ext.sent_query = q.clone();
            if let Some(s) = self.ext.sess.as_mut() {
                let _ = s.sc.send(&HostMsg::Search { text: q.clone() });
            }
        }
        let sel = self.ext_sections(q.trim()).into_iter().flat_map(|s| s.items).nth(self.selected).and_then(|i| match i.kind {
            Kind::ExtItem(id) if !id.is_empty() => Some(id),
            _ => None,
        });
        if sel != self.ext.sent_sel {
            self.ext.sent_sel = sel.clone();
            if let Some(s) = self.ext.sess.as_mut() {
                let _ = s.sc.send(&HostMsg::Selection { id: sel });
            }
        }
    }

    // ------------------------------------------------------------ runtime -> app

    pub(crate) fn ext_event(&mut self, id: u64, ev: Event) {
        let Some(s) = self.ext.sess.as_ref().filter(|s| s.id == id) else { return };
        let name = s.ext.clone();
        match ev {
            Event::Stderr(l) => self.ext_log(&format!("{name}: {l}")),
            Event::BadLine(l) => self.ext_log(&format!("{name}: unexpected output: {l}")),
            Event::TooLong => {
                self.ext_log(&format!("{name}: stdout line over 16 MB, stopping the command"));
                self.ext_kill();
                self.toast(format!("{name} sent a message that was too large and was stopped"));
            }
            Event::Closed => {
                self.ext_log(&format!("{name}: process ended"));
                self.ext_finished();
            }
            Event::Reply { call, result } => self.ext_answer(id, call, result),
            Event::Msg(m) => match m {
                RuntimeMsg::Render { depth, tree } => self.ext_render(depth, &tree),
                RuntimeMsg::Toast { style, title, message } => {
                    let sep = if title.is_empty() || message.is_empty() { "" } else { ": " };
                    let text = format!("{title}{sep}{message}");
                    let text = if style == "failure" { format!("⚠ {text}") } else { text };
                    if self.visible {
                        self.toast(text);
                    } else {
                        services::notify("Ferrocast", &text);
                    }
                }
                RuntimeMsg::Hud { title } => {
                    self.toast(title);
                    self.hide();
                    self.reset_to_root();
                }
                RuntimeMsg::Call { id: call, method, params } => match self.ext_call(&name, id, call, &method, &params) {
                    Reply::Now(r) => self.ext_answer(id, call, r),
                    Reply::Later | Reply::Replaced => {}
                },
                RuntimeMsg::PopToRoot | RuntimeMsg::CloseMainWindow => {
                    self.hide();
                    self.reset_to_root();
                }
                RuntimeMsg::Exit => self.ext_finished(),
            },
        }
    }

    fn ext_answer(&mut self, session: u64, call: u64, r: Result<Value, String>) {
        if let Some(s) = self.ext.sess.as_mut().filter(|s| s.id == session) {
            let (value, error) = match r {
                Ok(v) => (v, None),
                Err(e) => (Value::Null, Some(e)),
            };
            let _ = s.sc.send(&HostMsg::Result { id: call, value, error });
        }
    }

    /// Run `f` off the UI thread and deliver its result as the answer to runtime call `call`.
    /// `f` gets the session's cancel flag (set when the session ends) to stop what it runs.
    fn ext_later(&mut self, session: u64, call: u64, f: impl FnOnce(&AtomicBool) -> Result<Value, String> + Send + 'static) -> Reply {
        let Some(cancel) = self.ext.sess.as_ref().filter(|s| s.id == session).map(|s| s.cancel.clone()) else {
            return Reply::Now(Err("the command is not running".into()));
        };
        let Some(slot) = self.ext.pending.acquire() else {
            return Reply::Now(Err("too many requests in flight, try again".into()));
        };
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let result = f(&cancel);
            // the slot is given back when the work is over, not when the answer is read
            drop(slot);
            bus.send(Msg::Ext(session, Event::Reply { call, result }));
        });
        Reply::Later
    }

    /// The process ended (it asked to, or it died).
    fn ext_finished(&mut self) {
        let Some(s) = self.ext.sess.take() else { return };
        if s.mode == Mode::View && self.ext_top_depth().is_some() {
            if !s.got_render {
                self.toast(format!("{} stopped without showing anything", s.title));
                self.ext.views.clear();
                self.stack.truncate(self.ext.base.max(1));
                self.query.clear();
                self.selected = 0;
            } else {
                self.toast(format!("{} stopped", s.title));
            }
        }
    }

    fn ext_render(&mut self, depth: usize, tree: &ferrocast_core::extensions::protocol::Node) {
        if depth == 0 || depth > self.ext.views.len() + 1 {
            return;
        }
        let Some(s) = self.ext.sess.as_mut() else { return };
        s.got_render = true;
        let vm = view::build(tree);
        let changed_depth = depth != self.ext.views.len();
        self.ext.views.truncate(depth - 1);
        self.ext.views.push(vm);
        let want = self.ext.base + depth;
        if self.stack.len() > want {
            self.stack.truncate(want);
            self.query.clear();
            self.selected = 0;
        }
        while self.stack.len() < want {
            let v = ext_view_for(self.ext.views.get(self.stack.len() - self.ext.base));
            self.push(v);
        }
        let kind = ext_view_for(self.ext.views.last());
        if let Some(top) = self.stack.last_mut() {
            *top = kind;
        }
        if changed_depth {
            self.ext.sent_query.clear();
            self.ext.sent_sel = None;
            self.ext.form = FormState::default();
        }
        if let Some(ViewModel::Form(f)) = self.ext.views.last() {
            let f = f.clone();
            self.ext.form.sync(&f);
        }
        self.sec_dirty = true;
    }

    pub(crate) fn ext_call(&mut self, ext: &str, session: u64, call: u64, method: &str, params: &Value) -> Reply {
        let is_view = self.ext.sess.as_ref().is_some_and(|s| s.mode == Mode::View);
        if let Err(e) = hostcalls::call_allowed(method, is_view, self.visible) {
            return Reply::Now(Err(e));
        }
        match method {
            "apps.list" => Reply::Now(Ok(Value::Array(
                self.apps.iter().map(|a| hostcalls::application_json(&a.name, &a.path.to_string_lossy(), a.bundle_id.as_deref())).collect(),
            ))),
            "apps.frontmost" => {
                let pid = self.prev_pid;
                let app = running::running_apps().into_iter().find(|a| Some(a.pid) == pid).or_else(running::frontmost);
                Reply::Now(match app.and_then(|a| Some((a.name, a.path?, a.bundle_id))) {
                    Some((n, p, b)) => Ok(hostcalls::application_json(&n, &p, b.as_deref())),
                    None => Err("Unable to determine the frontmost application".into()),
                })
            }
            "apps.default" => {
                let path = params.get("path").and_then(Value::as_str).unwrap_or("").to_string();
                match hostcalls::default_app_script(&path) {
                    Err(e) => Reply::Now(Err(e)),
                    Ok(script) => self.ext_later(session, call, move |cancel| {
                        let out = ferrocast_macos::system::osascript_raw(&script, 5, Some(cancel))
                            .map_err(|_| "No default application for this file".to_string())?;
                        let app = out.trim().trim_end_matches('/').to_string();
                        let name = hostcalls::app_name_from_path(&app).ok_or("No default application for this file")?;
                        let bundle = std::process::Command::new("/usr/bin/mdls")
                            .args(["-name", "kMDItemCFBundleIdentifier", "-raw"])
                            .arg(&app)
                            .output()
                            .ok()
                            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                            .filter(|b| !b.is_empty() && b != "(null)");
                        Ok(hostcalls::application_json(&name, &app, bundle.as_deref()))
                    }),
                }
            }
            "selection.text" => match self.prev_pid {
                None => Reply::Now(Err("Unable to get selected text".into())),
                Some(pid) => self.ext_later(session, call, move |cancel| {
                    // verbatim: the selection keeps its leading / trailing whitespace
                    let out = ferrocast_macos::system::osascript_raw(&hostcalls::selection_script(pid), 3, Some(cancel))
                        .map_err(|_| "Unable to get selected text".to_string())?;
                    hostcalls::parse_selection(&out).map(Value::String)
                }),
            },
            "alert.confirm" => match hostcalls::confirm_script(params) {
                Err(e) => Reply::Now(Err(e)),
                Ok((script, primary)) => self.ext_later(session, call, move |cancel| {
                    let r = ferrocast_macos::system::osascript_raw(&script, 300, Some(cancel)).map_err(|_| ());
                    Ok(Value::Bool(hostcalls::parse_confirm(r, &primary)))
                }),
            },
            "launchCommand" => {
                let owners: Vec<String> = self
                    .ext
                    .manifests
                    .iter()
                    .find(|m| m.name == ext)
                    .map(|m| m.owner_names().into_iter().map(str::to_string).collect())
                    .unwrap_or_default();
                let owners: Vec<&str> = owners.iter().map(String::as_str).collect();
                match hostcalls::parse_launch(params, ext, &owners) {
                    Err(e) => Reply::Now(Err(e)),
                    Ok((cmd, args)) => {
                        if !self.ext.manifests.iter().any(|m| m.name == ext && m.commands.iter().any(|c| c.name == cmd)) {
                            return Reply::Now(Err(format!("launchCommand: {ext} has no command {cmd}")));
                        }
                        if !self.ext.launches.allow(Instant::now()) {
                            return Reply::Now(Err("launchCommand: too many launches, try again in a few seconds".into()));
                        }
                        match self.ext_launch_with(ext, &cmd, args) {
                            Ok(()) => Reply::Replaced,
                            Err(e) => Reply::Now(Err(format!("launchCommand: {e}"))),
                        }
                    }
                }
            }
            _ => Reply::Now(self.ext_call_simple(ext, method, params)),
        }
    }

    fn ext_call_simple(&mut self, ext: &str, method: &str, params: &Value) -> Result<Value, String> {
        let text = || params.get("text").and_then(Value::as_str).unwrap_or("").to_string();
        match method {
            "clipboard.copy" => {
                notion_io::write_clipboard(&text());
                self.shared.mark_clipboard_write();
                Ok(Value::Null)
            }
            "clipboard.paste" => {
                self.paste_text(&text());
                Ok(Value::Null)
            }
            "clipboard.readText" => Ok(clipboard::read_text().map(Value::String).unwrap_or(Value::Null)),
            "clipboard.clear" => {
                clipboard::write_text("");
                self.shared.mark_clipboard_write();
                Ok(Value::Null)
            }
            "open" => {
                let target = params.get("target").and_then(Value::as_str).ok_or("open: no target")?;
                let with = params.get("application").and_then(Value::as_str);
                ferrocast_core::extensions::validate_open(target, with)?;
                // /usr/bin/open can take a while: never on the UI thread
                let (target, with) = (target.to_string(), with.map(str::to_string));
                std::thread::spawn(move || {
                    let _ = apps::open(&target, with.as_deref());
                });
                Ok(Value::Null)
            }
            "showInFinder" => {
                let p = params.get("path").and_then(Value::as_str).ok_or("showInFinder: no path")?;
                ferrocast_core::extensions::validate_open(p, None)?;
                let p = PathBuf::from(p);
                std::thread::spawn(move || {
                    let _ = apps::reveal(&p);
                });
                Ok(Value::Null)
            }
            m if m.starts_with("localStorage.") => storage::local_storage(&self.store.root, ext, m, params),
            other => Err(format!("not supported in Ferrocast yet: {other}")),
        }
    }

    // ------------------------------------------------------------ app -> screen

    fn ext_current(&self) -> Option<&ViewModel> {
        let depth = self.ext_top_depth()?;
        self.ext.views.get(depth - 1)
    }

    fn ext_icon(&self, spec: &IconSpec) -> Icon {
        let assets = self.ext.sess.as_ref().map(|s| s.assets.clone());
        match spec {
            IconSpec::Builtin(n) => Icon::Glyph(builtin_glyph(n), EXT_GLYPH_COLOR),
            IconSpec::Emoji(e) => Icon::Emoji(e.clone()),
            IconSpec::Path(p) => {
                let path = PathBuf::from(p);
                let full = if path.is_absolute() { Some(path) } else { assets.map(|a| a.join(p)) };
                match full.filter(|f| f.exists()) {
                    Some(f) => Icon::App(f.to_string_lossy().into_owned()),
                    None => Icon::Glyph("sf:square.grid.2x2", EXT_GLYPH_COLOR),
                }
            }
            IconSpec::Url(_) => Icon::Glyph("sf:globe", EXT_GLYPH_COLOR),
            IconSpec::None => Icon::Glyph("sf:square.grid.2x2", [90, 90, 105]),
        }
    }

    fn ext_note_item(id: &str, text: &str) -> Item {
        Item {
            id: format!("ext-note:{id}"),
            title: text.to_string(),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:info.circle", [90, 90, 105]),
            accessory: String::new(),
            keywords: vec![],
            kind: Kind::ExtItem(String::new()),
        }
    }

    /// Rows of the running command's current list.
    pub(crate) fn ext_sections(&self, q: &str) -> Vec<Section> {
        let note = |id: &str, t: &str| vec![Section { title: String::new(), items: vec![Self::ext_note_item(id, t)] }];
        let Some(vm) = self.ext_current() else { return note("loading", "Starting the extension…") };
        let ViewModel::List(l) = vm else { return vec![] };
        let l: &ListView = l;
        let mut out: Vec<Section> = vec![];
        for s in &l.sections {
            let items: Vec<Item> = s
                .items
                .iter()
                .filter(|i| !l.filtering || self.matches(q, &i.title, &format!("{} {}", i.subtitle, i.keywords.join(" "))).is_some())
                .map(|i| Item {
                    id: format!("ext-item:{}", i.id),
                    title: i.title.clone(),
                    subtitle: i.subtitle.clone(),
                    icon: self.ext_icon(&i.icon),
                    accessory: i.accessories.join(" · "),
                    keywords: i.keywords.clone(),
                    kind: Kind::ExtItem(i.id.clone()),
                })
                .collect();
            if !items.is_empty() {
                out.push(Section { title: s.title.clone(), items });
            }
        }
        if out.is_empty() {
            out =
                if l.is_loading { note("loading", "Loading…") } else { note("empty", l.empty_title.as_deref().unwrap_or("No results")) };
        }
        if !l.unsupported.is_empty() {
            out.push(Section {
                title: "Not supported".into(),
                items: l
                    .unsupported
                    .iter()
                    .map(|u| Self::ext_note_item(u, &format!("{} ({})", view::unsupported_line(u), self.ext_running_title())))
                    .collect(),
            });
        }
        out
    }

    fn ext_to_actions(&self, v: &[view::ActionModel]) -> Vec<Action> {
        v.iter()
            .map(|a| {
                let mut x = Action::new(a.title.clone(), Act::ExtAction(a.id.clone()));
                if let Some((m, k)) = a.shortcut.as_ref().and_then(map_shortcut) {
                    x = x.key(m, k);
                }
                if a.destructive {
                    x = x.danger();
                }
                x
            })
            .collect()
    }

    /// Actions of a list row (`item_id` is the extension's item id), then one per option of the
    /// list's dropdown (the launcher has no dropdown widget in the search bar: picking an option is an
    /// action, the current one is ticked).
    pub(crate) fn ext_actions(&self, item_id: &str) -> Vec<Action> {
        match self.ext_current() {
            Some(ViewModel::List(l)) => {
                let mut v = l
                    .sections
                    .iter()
                    .flat_map(|s| &s.items)
                    .find(|i| i.id == item_id)
                    .map(|i| self.ext_to_actions(&i.actions))
                    .unwrap_or_default();
                v.extend(self.ext_dropdown_actions(l));
                v
            }
            _ => vec![],
        }
    }

    fn ext_dropdown_actions(&self, l: &ListView) -> Vec<Action> {
        let Some(d) = l.dropdown.as_ref().filter(|d| d.on_change && d.options.len() > 1) else { return vec![] };
        let current = self.ext.dropdown_choice.get(&d.id).unwrap_or(&d.selected);
        let label = if d.tooltip.is_empty() { "Show".to_string() } else { d.tooltip.clone() };
        d.options
            .iter()
            .map(|o| {
                let tick = if &o.value == current { "✓ " } else { "" };
                Action::new(format!("{tick}{label}: {}", o.title), Act::ExtAction(view::dropdown_action_id(&d.id, &o.value)))
            })
            .collect()
    }

    /// The side pane of a list row, when the extension gave it a `List.Item.Detail`.
    pub(crate) fn ext_item_detail(&self, item_id: &str) -> Option<view::ItemDetail> {
        match self.ext_current() {
            Some(ViewModel::List(l)) => l.sections.iter().flat_map(|s| &s.items).find(|i| i.id == item_id).and_then(|i| i.detail.clone()),
            _ => None,
        }
    }

    /// Actions of the current Detail or Form screen.
    pub(crate) fn ext_detail_actions(&self) -> Vec<Action> {
        match self.ext_current() {
            Some(ViewModel::Detail(d)) => self.ext_to_actions(&d.actions),
            Some(ViewModel::Form(f)) => self.ext_to_actions(&f.actions),
            _ => vec![],
        }
    }

    /// The form on screen.
    pub(crate) fn ext_form(&self) -> Option<view::FormView> {
        match self.ext_current() {
            Some(ViewModel::Form(f)) => Some(f.clone()),
            _ => None,
        }
    }

    /// A field of the on-screen form changed: remember it and tell the extension (it may be
    /// controlling the value, or listening to `onChange`).
    /// Returns whether the extension was told.
    pub(crate) fn ext_form_changed(&mut self, id: &str, value: Value) -> bool {
        let wants = self.ext_form().is_some_and(|f| f.wants_field(id));
        let sending = wants && self.ext.sess.is_some();
        self.ext.form.edit(id, value.clone(), sending);
        if !sending {
            return false;
        }
        match self.ext.sess.as_mut() {
            Some(s) => {
                let _ = s.sc.send(&HostMsg::Field { id: id.to_string(), value });
                true
            }
            None => false,
        }
    }

    /// Header text of an extension screen: the screen's `navigationTitle`, else the command's title.
    pub(crate) fn ext_header_title(&self) -> String {
        let nav = self.ext_current().map(|v| v.nav_title().to_string()).unwrap_or_default();
        if !nav.is_empty() {
            return nav;
        }
        self.ext.sess.as_ref().map(|s| s.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| "Raycast Extension".into())
    }

    /// Title of the current view for the header and footer.
    pub(crate) fn view_title(&self) -> String {
        match self.view() {
            View::ExtList | View::ExtDetail | View::ExtForm => self.ext_header_title(),
            v => tr(v.title()).into_owned(),
        }
    }

    /// Label of the Enter action of a row.
    pub(crate) fn ext_primary(&self, item_id: &str) -> String {
        self.ext_actions(item_id).first().map(|a| a.title.clone()).unwrap_or_else(|| "No actions".into())
    }

    /// Tell the runtime the user triggered an action.
    pub(crate) fn ext_trigger(&mut self, id: String) {
        if let Some((dd, value)) = view::parse_dropdown_action(&id) {
            self.ext.dropdown_choice.insert(dd.to_string(), value.to_string());
            if let Some(s) = self.ext.sess.as_mut() {
                let _ = s.sc.send(&HostMsg::Field { id: dd.to_string(), value: Value::String(value.to_string()) });
            }
            self.sec_dirty = true;
            return;
        }
        let values = self.ext_form().map(|f| self.ext.form.values(&f));
        match self.ext.sess.as_mut() {
            Some(s) => {
                if let Err(e) = s.sc.send(&HostMsg::Action { id, values }) {
                    self.toast(format!("The extension is not running: {e}"));
                }
            }
            None => self.toast("The extension is not running"),
        }
    }

    /// Title of the extension that is running ("" when none).
    fn ext_running_title(&self) -> String {
        let name = self.ext.sess.as_ref().map(|s| s.ext.as_str());
        self.ext.manifests.iter().find(|m| Some(m.name.as_str()) == name).map(|m| m.title.clone()).unwrap_or_default()
    }

    /// What the Detail screen shows: markdown (or a "not supported" line) and whether it is loading.
    pub(crate) fn ext_detail_content(&self) -> (String, bool) {
        match self.ext_current() {
            Some(ViewModel::Detail(d)) => (d.markdown.clone(), d.is_loading),
            Some(ViewModel::Unsupported(n)) => (view::unsupported_message(n, &self.ext_running_title()), false),
            Some(ViewModel::List(_)) | Some(ViewModel::Form(_)) => (String::new(), false),
            Some(ViewModel::Empty) | None => ("Starting the extension…".into(), true),
        }
    }

    /// First action of the Detail screen, for Enter; `⌘K` opens them all.
    pub(super) fn ext_detail_keys(&mut self, k: &Keys) -> bool {
        let form = matches!(self.view(), View::ExtForm);
        // in a form Enter belongs to the fields; ⌘↵ submits
        if !(k.enter || k.cmd_k || (form && k.cmd_enter)) {
            return false;
        }
        let actions = self.ext_detail_actions();
        if actions.is_empty() {
            return false;
        }
        let item = Self::ext_note_item("detail", "Extension");
        if k.cmd_k {
            self.panel = Some(ActionPanel { actions, query: String::new(), sel: 0, item });
        } else if let Act::ExtAction(id) = actions[0].kind.clone() {
            self.ext_trigger(id);
        }
        true
    }
}

fn ext_view_for(vm: Option<&ViewModel>) -> View {
    match vm {
        Some(ViewModel::Form(_)) => View::ExtForm,
        Some(ViewModel::Detail(_)) | Some(ViewModel::Unsupported(_)) => View::ExtDetail,
        _ => View::ExtList,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrocast_core::extensions::view::Shortcut;

    fn sc(mods: &[&str], key: &str) -> Shortcut {
        Shortcut { modifiers: mods.iter().map(|s| s.to_string()).collect(), key: key.into() }
    }

    #[test]
    fn shortcuts_map_to_egui() {
        let (m, k) = map_shortcut(&sc(&["cmd", "shift"], "c")).unwrap();
        assert!(m.command && m.shift && !m.alt && !m.ctrl);
        assert_eq!(k, Key::C);
        assert_eq!(map_shortcut(&sc(&["opt"], "return")).unwrap().1, Key::Enter);
        assert_eq!(map_shortcut(&sc(&["ctrl"], "arrowDown")).unwrap().1, Key::ArrowDown);
        assert_eq!(map_shortcut(&sc(&["cmd"], "delete")).unwrap().1, Key::Backspace);
        assert!(map_shortcut(&sc(&["cmd"], "nonsense-key")).is_none());
    }

    fn ni() -> NodeInfo {
        NodeInfo { path: PathBuf::from("/x/node"), version: (22, 1, 0) }
    }

    #[test]
    fn a_discovery_result_does_not_end_a_running_download() {
        let mut st = ExtState { downloading: true, ..Default::default() };
        st.node_found(None);
        assert!(st.downloading, "button must stay disabled while downloading");
        st.node_found(Some(ni()));
        assert!(st.downloading && st.node.is_some());
        st.download_finished(None);
        assert!(!st.downloading);
    }

    #[test]
    fn rescan_only_when_node_is_missing_idle_and_not_too_often() {
        let t0 = Instant::now();
        let mut st = ExtState::default();
        assert!(st.should_rescan(t0));
        st.begin_scan(t0);
        assert!(!st.should_rescan(t0 + Duration::from_secs(1)), "scan in flight");
        st.node_found(None);
        assert!(!st.should_rescan(t0 + Duration::from_secs(1)), "throttled");
        assert!(st.should_rescan(t0 + Duration::from_secs(6)));
        st.downloading = true;
        assert!(!st.should_rescan(t0 + Duration::from_secs(60)));
        st.downloading = false;
        st.node_found(Some(ni()));
        assert!(!st.should_rescan(t0 + Duration::from_secs(60)), "node already known");
    }

    #[test]
    fn view_kind_follows_the_root_component() {
        use ferrocast_core::extensions::view::DetailView;
        assert!(matches!(ext_view_for(Some(&ViewModel::Detail(DetailView::default()))), View::ExtDetail));
        assert!(matches!(ext_view_for(Some(&ViewModel::Unsupported("MenuBarExtra".into()))), View::ExtDetail));
        assert!(matches!(ext_view_for(Some(&ViewModel::Form(view::FormView::default()))), View::ExtForm));
        assert!(matches!(ext_view_for(Some(&ViewModel::List(ListView::default()))), View::ExtList));
        assert!(matches!(ext_view_for(None), View::ExtList));
    }
}
