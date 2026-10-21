//! eframe::App implementation: event loop, keyboard handling and drawing.

use super::*;
use egui::TextureOptions;
use egui::{Event, Frame as EFrame, RichText, TextEdit};
use global_hotkey::hotkey::HotKey;
use std::str::FromStr;

/// Right-hand pane content: the thing itself plus `(label, value)` facts about it.
enum Detail {
    Text(String, Vec<(String, String)>),
    Image(String, Vec<(String, String)>),
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn logic(&mut self, ctx: &Context, _f: &mut eframe::Frame) {
        if !self.started {
            self.started = true;
            if self.visible {
                self.show();
            }
        }
        dev_screenshot(ctx, self);
        self.drain();
        self.tick_inline();
        self.sweep_timers();
        if matches!(self.view(), View::Timers) && !self.timers.is_empty() {
            self.invalidate_sections();
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        if self.rates_checked_at.elapsed() > Duration::from_secs(15 * 60) {
            self.rates_checked_at = Instant::now();
            if self.rates.is_stale(services::now_secs()) {
                services::refresh_rates(self.bus.clone());
            }
        }
        if self.toast.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_millis(2500)) {
            self.toast = None;
        }
        if self.visible {
            let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
            if focused {
                self.had_focus = true;
            } else if self.had_focus && self.shown_at.elapsed() > Duration::from_millis(400) {
                self.hide();
            }
            // compact mode
            let compact = self.settings.compact_mode && matches!(self.view(), View::Root) && self.query.is_empty() && self.panel.is_none();
            let want = if compact { ui::COMPACT_HEIGHT } else { ui::HEIGHT };
            if (want - self.cur_height).abs() > 0.5 {
                self.cur_height = want;
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(ui::WIDTH, want)));
            }
        }
        // debounced file search
        let q = self.query.trim().to_string();
        let wants = self.visible
            && q.chars().count() >= 2
            && match self.view() {
                View::Root => self.settings.file_search_in_root,
                View::Files => true,
                _ => false,
            };
        if wants && self.files_req != q {
            if self.query_changed_at.elapsed() >= Duration::from_millis(200) {
                self.files_req = q.clone();
                let bus = self.bus.clone();
                std::thread::spawn(move || {
                    let r = ferrocast_macos::files::search(&q, 30);
                    bus.send(Msg::Files(q, r));
                });
            } else {
                ctx.request_repaint_after(Duration::from_millis(220));
            }
        }
        if self.toast.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _f: &mut eframe::Frame) {
        if let Some(t) = self.show_t.take() {
            if std::env::var_os("FERROCAST_PROFILE").is_some() {
                eprintln!("show -> frame {:?}", t.elapsed());
            }
        }
        self.draw(ui);
    }
}

/// Dev aid: `FERROCAST_SCREENSHOT=/tmp/x.png` saves the window contents after a few frames
/// and exits. `FERROCAST_QUERY` / `FERROCAST_VIEW` preselect what is drawn. Needs no TCC permission.
fn dev_screenshot(ctx: &Context, app: &mut App) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static FRAME: AtomicU32 = AtomicU32::new(0);
    let Ok(path) = std::env::var("FERROCAST_SCREENSHOT") else { return };
    let n = FRAME.fetch_add(1, Ordering::Relaxed);
    if n == 0 && std::env::var_os("FERROCAST_PROFILE").is_some() {
        if let Some(t0) = crate::STARTED.get() {
            eprintln!("first frame after {:?}", t0.elapsed());
        }
    }
    if n == 2 {
        match std::env::var("FERROCAST_VIEW").as_deref() {
            Ok("clipboard") => app.push(View::Clipboard),
            Ok("snippets") => app.push(View::Snippets),
            Ok("quicklinks") => app.push(View::Quicklinks),
            Ok("emoji") => app.push(View::Emoji),
            Ok("settings") => app.push(View::Settings),
            Ok("notes") => app.push(View::Notes),
            Ok("welcome") => app.push(View::Welcome),
            Ok("shortcuts") => app.push(View::Shortcuts),
            Ok("about") => app.push(View::About),
            Ok("processes") => app.push(View::Processes),
            Ok("calchistory") => app.push(View::CalcHistory),
            Ok("shelf") => {
                app.shelf = std::env::var("FERROCAST_FILES")
                    .unwrap_or_default()
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect();
                app.push(View::Shelf);
            }
            Ok("convert") => app.push(View::Convert(
                std::env::var("FERROCAST_FILES")
                    .unwrap_or_default()
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect(),
            )),
            Ok("wheel") => {
                let files: Vec<std::path::PathBuf> = std::env::var("FERROCAST_FILES")
                    .unwrap_or_default()
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect();
                let kind = if std::env::var("FERROCAST_WHEEL").as_deref() == Ok("tools") {
                    ferrocast_core::wheel::WheelKind::Tools
                } else if std::env::var("FERROCAST_WHEEL").as_deref() == Ok("actions") {
                    ferrocast_core::wheel::WheelKind::Actions
                } else {
                    ferrocast_core::wheel::WheelKind::Convert
                };
                let segs = app.wheel_segments(kind, &files);
                app.push(View::Wheel { files, center: Pos2::new(-9999.0, -9999.0), segs, kind, sel: None });
            }
            Ok("settings-ai") => {
                ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 3usize));
                app.push(View::Settings);
            }
            Ok("settings-advanced") => {
                ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 5usize));
                app.push(View::Settings);
            }
            Ok("timers") => {
                app.timers.push(crate::app::RunningTimer {
                    ends_at: Instant::now() + Duration::from_secs(1500),
                    secs: 1500,
                    label: "Pomodoro".into(),
                    cancelled: Default::default(),
                });
                app.timers.push(crate::app::RunningTimer {
                    ends_at: Instant::now() + Duration::from_secs(600),
                    secs: 600,
                    label: String::new(),
                    cancelled: Default::default(),
                });
                app.push(View::Timers);
            }
            Ok("ai") => {
                app.run_builtin(Builtin::AskAi);
                if std::env::var("FERROCAST_AI_DEMO").is_ok() {
                    if let Some(View::Ai { history, asked, answer, .. }) = app.stack.last_mut() {
                        history.push(("What is a frecency score?".into(), "Frecency blends how often and how recently you used an item, so frequently and recently used results rank higher.".into()));
                        *asked = "And how is it stored?".into();
                        *answer = "As a small list of recent timestamps per item plus a counter, persisted in frecency.json.".into();
                    }
                }
            }
            _ => {}
        }
        if let Ok(q) = std::env::var("FERROCAST_QUERY") {
            app.query = q;
        }
    }
    if n == 8 && std::env::var("FERROCAST_PANEL").is_ok() {
        let sections = app.sections();
        if let Some(item) = sections.iter().flat_map(|s| s.items.iter()).next().cloned() {
            let actions = app.actions_for(&item);
            app.panel = Some(ActionPanel { actions, query: String::new(), sel: 0, item });
        }
    }
    if n == 40 {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
    }
    let shot = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let Some(img) = shot {
        let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        let _ = image::save_buffer(&path, &raw, img.width() as u32, img.height() as u32, image::ColorType::Rgba8);
        std::process::exit(0);
    }
    ctx.request_repaint();
}

/// Shorten `text` with an ellipsis so it fits `max_w` points.
fn elide(p: &egui::Painter, text: &str, font: FontId, max_w: f32) -> String {
    let width = |t: &str| p.layout_no_wrap(t.to_string(), font.clone(), Color32::WHITE).size().x;
    if width(text) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cand: String = chars[..mid].iter().collect::<String>() + "…";
        if width(&cand) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_string() + "…"
}

fn panel_filter(p: &ActionPanel) -> Vec<usize> {
    let q = p.query.trim().to_lowercase();
    p.actions.iter().enumerate().filter(|(_, a)| q.is_empty() || a.title.to_lowercase().contains(&q)).map(|(i, _)| i).collect()
}

impl App {
    fn read_keys(&mut self, ctx: &Context) -> Keys {
        let view = self.view();
        let shape = view.shape();
        let hotkey = matches!(view, View::Hotkey { .. });
        let query_empty = self.query.is_empty();
        let panel = self.panel.is_some();
        ctx.input_mut(|i| {
            let mut k = Keys::default();
            if hotkey {
                return k;
            }
            if shape == Shape::List || panel {
                k.cmd_k = i.consume_key(CMD, Key::K);
                k.up = i.consume_key(Modifiers::NONE, Key::ArrowUp);
                k.down = i.consume_key(Modifiers::NONE, Key::ArrowDown);
            }
            k.cmd_enter = i.consume_key(CMD, Key::Enter);
            if shape != Shape::Form || panel {
                k.enter = i.consume_key(Modifiers::NONE, Key::Enter);
            }
            k.esc = i.consume_key(Modifiers::NONE, Key::Escape);
            k.backspace = shape == Shape::List
                && !panel
                && query_empty
                && !i.modifiers.any()
                && i.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Backspace, pressed: true, repeat: false, .. }));
            k
        })
    }

    /// Returns true if state changed in a way that requires a fresh frame.
    fn handle_keys(&mut self, ctx: &Context, k: &Keys, total: usize, cur: Option<Item>) -> bool {
        if matches!(self.view(), View::Hotkey { .. }) {
            return self.record_hotkey(ctx);
        }
        if self.panel.is_some() {
            return self.panel_keys(k);
        }
        let shape = self.view().shape();
        if shape == Shape::List {
            if k.esc {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.selected = 0;
                } else {
                    self.pop();
                }
                return true;
            }
            if k.backspace {
                self.pop();
                return true;
            }
            if k.up && self.selected > 0 {
                self.selected -= 1;
                self.scroll_to_sel = true;
            }
            if k.down && self.selected + 1 < total {
                self.selected += 1;
                self.scroll_to_sel = true;
            }
            // View-level commands must also work when the library is empty.
            if ctx.input_mut(|i| i.consume_key(CMD, Key::Comma)) {
                self.push(View::Settings);
                return true;
            }
            if ctx.input_mut(|i| i.consume_key(CMD, Key::N)) {
                match self.view() {
                    View::Snippets => self.new_snippet(String::new()),
                    View::Quicklinks => self.run_builtin(Builtin::CreateQuicklink),
                    View::Notes => self.new_note(),
                    _ => return false,
                }
                return true;
            }
            let Some(item) = cur else { return false };
            if k.cmd_k {
                let actions = self.actions_for(&item);
                self.panel = Some(ActionPanel { actions, query: String::new(), sel: 0, item });
                return true;
            }
            if k.enter || k.cmd_enter {
                let acts = self.actions_for(&item);
                // ⌘↵ runs the action that declares that shortcut; when an item has none, it
                // behaves like ↵ rather than firing whatever happens to be listed second.
                let pick =
                    if k.enter { acts.first() } else { acts.iter().find(|a| a.shortcut == Some((CMD, Key::Enter))).or(acts.first()) };
                if let Some(a) = pick {
                    self.do_action(item, a.kind.clone());
                    return true;
                }
                return false;
            }
            if ctx.input(|i| i.modifiers.command || i.modifiers.ctrl) {
                let acts = self.actions_for(&item);
                let hit = ctx.input_mut(|i| acts.iter().find(|a| a.shortcut.is_some_and(|(m, key)| i.consume_key(m, key))).cloned());
                if let Some(a) = hit {
                    self.do_action(item, a.kind);
                    return true;
                }
            }
            return false;
        }
        // forms / single views
        if k.esc {
            if let View::NoteEdit(id) = self.view().clone() {
                if self.notes.iter().any(|n| n.id == id && n.text.trim().is_empty()) {
                    self.notes.retain(|n| n.id != id);
                    self.save_notes();
                }
            }
            self.pop();
            return true;
        }
        match self.view().clone() {
            View::Welcome if k.enter => {
                windows::accessibility_trusted(true);
                true
            }
            View::Ai { .. } if k.enter => {
                self.send_ai();
                true
            }
            View::Ai { answer, busy: false, .. } if k.cmd_enter && !answer.is_empty() => {
                self.copy_text(&answer);
                true
            }
            View::Args { .. } if k.enter || k.cmd_enter => {
                self.submit_args();
                true
            }
            View::Output { text, .. } if k.enter || k.cmd_enter => {
                self.copy_text(&script::strip_ansi(&text));
                true
            }
            View::Confirm { act, .. } if k.enter => {
                self.perform(act, true);
                if matches!(self.view(), View::Confirm { .. }) {
                    self.pop();
                }
                true
            }
            View::Alias { id, value, .. } if k.enter => {
                let v = value.trim().to_string();
                self.settings.command_mut(&id).alias = if v.is_empty() { None } else { Some(v) };
                self.save_settings();
                self.toast("Alias saved");
                self.pop();
                true
            }
            View::CreateScript { title, mode } if k.enter => {
                self.create_script(&title, mode);
                true
            }
            View::SnippetForm { .. } | View::QuicklinkForm { .. } if k.cmd_enter => {
                self.save_form();
                true
            }
            _ => false,
        }
    }

    fn panel_keys(&mut self, k: &Keys) -> bool {
        let Some(p) = self.panel.as_mut() else { return false };
        if k.esc || k.cmd_k {
            self.panel = None;
            return true;
        }
        let idx = panel_filter(p);
        if k.up && p.sel > 0 {
            p.sel -= 1;
        }
        if k.down && p.sel + 1 < idx.len() {
            p.sel += 1;
        }
        if p.sel >= idx.len() {
            p.sel = idx.len().saturating_sub(1);
        }
        if k.enter {
            if let Some(&i) = idx.get(p.sel) {
                let act = p.actions[i].kind.clone();
                let item = p.item.clone();
                self.panel = None;
                self.do_action(item, act);
                return true;
            }
        }
        false
    }

    fn do_action(&mut self, item: Item, act: Act) {
        self.invalidate_sections();
        self.act_item = Some(item.clone());
        match act {
            Act::Run => {
                if matches!(self.view(), View::Root) && !matches!(item.kind, Kind::Calc(_) | Kind::Prefilled(..) | Kind::WebSearch(_)) {
                    let q = self.query.clone();
                    self.frecency.record(&item.id, &q, now_secs());
                    self.save_frecency();
                }
                self.run_item(&item);
            }
            a => self.perform(a, false),
        }
    }

    fn record_hotkey(&mut self, ctx: &Context) -> bool {
        let View::Hotkey { id, .. } = self.view().clone() else { return false };
        let ev = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        let Some((key, m)) = ev else { return false };
        if key == Key::Escape {
            self.pop();
            return true;
        }
        let Some(code) = services::key_code_name(key) else { return false };
        let is_f = code.starts_with('F') && code.len() <= 3 && code[1..].chars().all(|c| c.is_ascii_digit());
        if !(m.ctrl || m.alt || m.mac_cmd || m.shift || is_f) {
            return false;
        }
        let mut parts = vec![];
        if m.ctrl {
            parts.push("Control".to_string());
        }
        if m.alt {
            parts.push("Alt".into());
        }
        if m.shift {
            parts.push("Shift".into());
        }
        if m.mac_cmd {
            parts.push("Super".into());
        }
        parts.push(code);
        let spec = parts.join("+");
        if HotKey::from_str(&spec).is_err() {
            if let Some(View::Hotkey { err, .. }) = self.stack.last_mut() {
                *err = format!("Unsupported combination {spec}");
            }
            return true;
        }
        if id == "__launcher" {
            self.settings.launcher_hotkey = spec.clone();
        } else {
            self.settings.command_mut(&id).hotkey = Some(spec.clone());
        }
        self.save_settings();
        self.apply_hotkeys();
        self.toast(format!("Hotkey set to {}", services::hotkey_label(&spec)));
        self.pop();
        true
    }

    fn create_script(&mut self, title: &str, mode: usize) {
        let t = title.trim();
        if t.is_empty() {
            self.toast("Enter a title");
            return;
        }
        let slug: String = t.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        let path = self.store.scripts_dir().join(format!("{}.sh", slug.trim_matches('-')));
        let mode = ["fullOutput", "compact", "silent"][mode.min(2)];
        let _ = std::fs::create_dir_all(self.store.scripts_dir());
        if std::fs::write(&path, script::template(t, mode, "Custom")).is_err() {
            self.toast("Could not write script");
            return;
        }
        services::set_exec(&path);
        self.reload_scripts();
        let _ = std::process::Command::new("/usr/bin/open").arg("-t").arg(&path).status();
        self.hide();
        self.reset_to_root();
    }

    fn save_form(&mut self) {
        match self.view().clone() {
            View::SnippetForm { id, name, text, keyword } => {
                if name.trim().is_empty() || text.is_empty() {
                    self.toast("Name and text are required");
                    return;
                }
                let kw = keyword.trim().to_string();
                if !kw.is_empty() && self.snippets.iter().any(|s| s.keyword == kw && Some(&s.id) != id.as_ref()) {
                    self.toast("Keyword already used by another snippet");
                    return;
                }
                if !kw.is_empty() {
                    if let Some(o) = self.snippets.iter().find(|s| {
                        Some(&s.id) != id.as_ref() && !s.keyword.is_empty() && (s.keyword.starts_with(&kw) || kw.starts_with(&s.keyword))
                    }) {
                        let msg = format!("Keyword conflicts with “{}”", o.keyword);
                        self.toast(&msg);
                        return;
                    }
                }
                match id.and_then(|i| self.snippets.iter_mut().find(|s| s.id == i)) {
                    Some(s) => {
                        s.name = name.trim().into();
                        s.text = text;
                        s.keyword = kw;
                    }
                    None => self.snippets.push(Snippet::new(name.trim(), &text, &kw)),
                }
                self.save_snippets();
                self.toast("Snippet saved");
                self.pop();
            }
            View::QuicklinkForm { id, name, link, open_with } => {
                if name.trim().is_empty() || link.trim().is_empty() {
                    self.toast("Name and link are required");
                    return;
                }
                let ow = Some(open_with.trim().to_string()).filter(|s| !s.is_empty());
                match id.and_then(|i| self.quicklinks.iter_mut().find(|q| q.id == i)) {
                    Some(q) => {
                        q.name = name.trim().into();
                        q.link = link.trim().into();
                        q.open_with = ow;
                    }
                    None => {
                        let mut q = Quicklink::new(name.trim(), link.trim());
                        q.open_with = ow;
                        self.quicklinks.push(q);
                    }
                }
                self.save_quicklinks();
                self.toast("Quicklink saved");
                self.pop();
            }
            _ => {}
        }
    }

    fn detail_for(&self, item: &Item) -> Option<Detail> {
        match &item.kind {
            Kind::Clip(id) => {
                let e = self.shared.clipboard.lock().unwrap().iter().find(|e| &e.id == id).cloned()?;
                let mut meta = vec![];
                if let Some(a) = &e.source_app {
                    meta.push(("Source".to_string(), a.clone()));
                }
                meta.push(("Copied".to_string(), services::ago(services::now_secs() - e.copied_at)));
                meta.push(("Times".to_string(), e.times_copied.to_string()));
                match e.kind {
                    ClipKind::Image => {
                        meta.push(("Size".to_string(), format!("{} × {} px", e.width, e.height)));
                        Some(Detail::Image(e.content, meta))
                    }
                    _ => {
                        let kind = match e.kind {
                            ClipKind::Link => "Link",
                            ClipKind::Color => "Color",
                            ClipKind::File => "File",
                            _ => "Text",
                        };
                        meta.insert(0, ("Type".to_string(), kind.to_string()));
                        let chars = e.content.chars().count();
                        meta.push(("Length".to_string(), format!("{chars} character{}", if chars == 1 { "" } else { "s" })));
                        if e.pinned {
                            meta.push(("Pinned".to_string(), "Yes".to_string()));
                        }
                        Some(Detail::Text(e.content.chars().take(6000).collect(), meta))
                    }
                }
            }
            Kind::Snippet(id) => {
                let s = self.snippets.iter().find(|s| &s.id == id)?;
                let mut meta = vec![("Keyword".to_string(), if s.keyword.is_empty() { "None".to_string() } else { s.keyword.clone() })];
                meta.push(("Length".to_string(), format!("{} characters", s.text.chars().count())));
                if s.text.contains('{') {
                    meta.push(("Placeholders".to_string(), "Yes".to_string()));
                }
                Some(Detail::Text(s.text.clone(), meta))
            }
            Kind::Note(id) => self.notes.iter().find(|n| &n.id == id).map(|n| {
                Detail::Text(
                    n.text.chars().take(6000).collect(),
                    vec![("Length".to_string(), format!("{} characters", n.text.chars().count()))],
                )
            }),
            Kind::Quicklink { id, .. } => self.quicklinks.iter().find(|q| &q.id == id).map(|q| {
                Detail::Text(
                    q.link.clone(),
                    vec![
                        ("Type".to_string(), "Quicklink".to_string()),
                        ("Arguments".to_string(), if q.link.contains('{') { "Yes".into() } else { "No".into() }),
                    ],
                )
            }),
            _ => None,
        }
    }

    // ---------------------------------------------------------------- drawing

    fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.icons.new_frame();
        self.dark = ctx.theme() == egui::Theme::Dark;
        let pal = palette(self.dark);
        // Brand-coloured caret and text selection instead of egui's default blue.
        if ui.visuals().text_cursor.stroke.color != pal.accent {
            let accent = pal.accent;
            ctx.all_styles_mut(|st| {
                st.visuals.text_cursor.stroke.color = accent;
                st.visuals.selection.bg_fill = accent.gamma_multiply(0.35);
                st.visuals.selection.stroke.color = accent;
            });
        }
        if let View::Wheel { center, segs, files, sel, .. } = self.view().clone() {
            // keyboard: arrows choose, Enter applies, Escape cancels
            let n = segs.len();
            let (next, prev, enter, esc) = ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::ArrowRight) || i.consume_key(Modifiers::NONE, Key::ArrowDown),
                    i.consume_key(Modifiers::NONE, Key::ArrowLeft) || i.consume_key(Modifiers::NONE, Key::ArrowUp),
                    i.consume_key(Modifiers::NONE, Key::Enter),
                    i.consume_key(Modifiers::NONE, Key::Escape),
                )
            });
            if esc {
                self.apply_wheel(None);
                return;
            }
            if enter {
                self.apply_wheel(sel);
                return;
            }
            if (next || prev) && n > 0 {
                let ns = ferrocast_core::wheel::step_sel(sel, n, next);
                if let Some(View::Wheel { sel, .. }) = self.stack.last_mut() {
                    *sel = Some(ns);
                }
            }
            let sel = if let Some(View::Wheel { sel, .. }) = self.stack.last() { *sel } else { None };
            self.draw_wheel(ui, &pal, center, &segs, files.len(), sel);
            return;
        }
        // Fade the launcher in when it appears (opacity only; one multiply per shape).
        let appear = ctx.animate_bool_with_time(Id::new("appear"), self.visible && self.shown_at.elapsed() > Duration::ZERO, 0.14);
        ui.set_opacity(appear);
        let full = ui.max_rect();
        ui.painter().rect_filled(full, ui::RADIUS, pal.bg);
        ui.painter().rect_stroke(full, ui::RADIUS, Stroke::new(1.0, pal.border), StrokeKind::Inside);

        if self.query != self.prev_query {
            self.prev_query = self.query.clone();
            self.selected = 0;
            self.scroll_to_sel = true;
            self.query_changed_at = Instant::now();
        }
        // Files dropped on the launcher open the converter.
        let dropped: Vec<std::path::PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        // a drop that ends a wheel gesture arrives after the wheel hid the window: ignore it
        if !dropped.is_empty() && self.visible && matches!(self.view(), View::Shelf) {
            // files dropped on an open shelf stay on the shelf
            self.perform(Act::AddToShelf(dropped), false);
            self.invalidate_sections();
            ctx.request_repaint();
            return;
        }
        if !dropped.is_empty() && self.visible {
            self.reset_to_root();
            self.push(View::Convert(dropped));
            ctx.request_repaint();
            return;
        }
        let keys = self.read_keys(&ctx);
        let shape = self.view().shape();
        let q_now = self.query.clone();
        self.ensure_wide_font(&q_now);
        ctx.data_mut(|d| d.insert_temp(Id::new("sel_salt"), self.stack.len()));
        let sections = if shape == Shape::List { self.sections_cached() } else { vec![] };
        let total: usize = sections.iter().map(|s| s.items.len()).sum();
        if self.selected >= total {
            self.selected = total.saturating_sub(1);
        }
        let cur = sections.iter().flat_map(|s| s.items.iter()).nth(self.selected).cloned();
        if self.handle_keys(&ctx, &keys, total, cur.clone()) {
            self.invalidate_sections();
            ctx.request_repaint();
            return;
        }

        let compact = self.settings.compact_mode && matches!(self.view(), View::Root) && self.query.is_empty() && self.panel.is_none();
        let search_rect = Rect::from_min_size(full.min, Vec2::new(full.width(), ui::SEARCH_H));
        let footer_rect = Rect::from_min_max(Pos2::new(full.min.x, full.max.y - ui::FOOTER_H), full.max);
        let content = Rect::from_min_max(Pos2::new(full.min.x, search_rect.max.y), Pos2::new(full.max.x, footer_rect.min.y));

        // ---- search bar
        let title = self.view().title();
        let placeholder = self.view().placeholder();
        let nested = self.stack.len() > 1;
        let mut back = false;
        ui.scope_builder(UiBuilder::new().max_rect(search_rect.shrink2(Vec2::new(18.0, 0.0)).translate(Vec2::new(0.0, 3.0))), |ui| {
            ui.horizontal_centered(|ui| {
                if nested && ui.add(egui::Label::new(RichText::new("←").size(18.0).color(pal.sub)).sense(Sense::click())).clicked() {
                    back = true;
                }
                if shape == Shape::List {
                    let te = TextEdit::singleline(&mut self.query)
                        .hint_text(RichText::new(placeholder).color(pal.sub))
                        .font(FontId::proportional(18.0))
                        .frame(EFrame::NONE)
                        .text_color(pal.text)
                        .desired_width(f32::INFINITY)
                        .id(Id::new("search"));
                    let r = ui.add(te);
                    if self.panel.is_none() {
                        r.request_focus();
                    }
                } else {
                    ui.label(RichText::new(title).size(16.0).strong().color(pal.text));
                }
            });
        });
        if back {
            self.pop();
            ctx.request_repaint();
            return;
        }
        if compact {
            return;
        }
        let line = |y: f32| {
            ui.painter().hline(full.min.x..=full.max.x, y, Stroke::new(1.0, pal.sep));
        };
        line(search_rect.max.y);
        line(footer_rect.min.y);

        // ---- content
        let mut clicked = None;
        let mut double = None;
        match shape {
            Shape::List => {
                let detail = if matches!(self.view(), View::Clipboard | View::Snippets | View::Notes | View::Quicklinks) {
                    cur.as_ref().and_then(|i| self.detail_for(i))
                } else {
                    None
                };
                let list_rect = if detail.is_some() {
                    Rect::from_min_max(content.min, Pos2::new(content.min.x + content.width() * 0.42, content.max.y))
                } else {
                    content
                };
                (clicked, double) = self.draw_list(ui, list_rect, &sections, &pal);
                if let Some(d) = detail {
                    let r = Rect::from_min_max(Pos2::new(list_rect.max.x, content.min.y), content.max);
                    ui.painter().vline(r.min.x, r.y_range(), Stroke::new(1.0, pal.sep));
                    self.draw_detail(ui, r.shrink(14.0), d, &pal);
                }
                if sections.is_empty() && matches!(self.view(), View::Files) && self.query.trim().chars().count() < 2 {
                    ui.painter().text(
                        content.center(),
                        Align2::CENTER_CENTER,
                        "Type at least 2 characters",
                        FontId::proportional(14.0),
                        pal.sub,
                    );
                } else if sections.is_empty() {
                    ui.painter().text(content.center(), Align2::CENTER_CENTER, "No results", FontId::proportional(14.0), pal.sub);
                }
            }
            _ => self.draw_single(ui, content.shrink2(Vec2::new(20.0, 14.0)), &pal),
        }
        self.scroll_to_sel = false;

        // ---- footer
        let primary = match shape {
            Shape::List => cur.as_ref().map(|i| self.primary_label(i)),
            _ => match self.view() {
                View::Args { .. } => Some("Run".into()),
                View::Output { .. } => Some("Copy Output".into()),
                View::Confirm { .. } => Some("Confirm".into()),
                View::Ai { .. } => Some("Ask".into()),
                View::Welcome => Some("Grant Accessibility".into()),
                View::SnippetForm { .. } | View::QuicklinkForm { .. } => Some("Save".into()),
                View::Alias { .. } | View::CreateScript { .. } => Some("Save".into()),
                _ => None,
            },
        };
        self.draw_footer(ui, footer_rect, &pal, primary, shape == Shape::List);
        // Keep the pop-in animation primed at 0 while the panel is closed.
        ctx.animate_bool_with_time(Id::new("panel_pop"), self.panel.is_some(), 0.12);
        if self.panel.is_some() {
            self.draw_panel(ui, content, &pal);
        }

        // Hovering already selects, so a single click runs the row, as in the original launcher.
        // Clicks right after a view change are ignored: the second half of a habitual double
        // click must not fire the first row of the screen that just opened.
        let _ = double;
        if let Some(i) = clicked.filter(|_| self.nav_at.elapsed() > Duration::from_millis(350)) {
            self.selected = i;
            if let Some(item) = sections.iter().flat_map(|s| s.items.iter()).nth(i).cloned() {
                if let Some(a) = self.actions_for(&item).first() {
                    self.do_action(item, a.kind.clone());
                }
            }
        }
    }

    fn draw_footer(&mut self, ui: &Ui, rect: Rect, pal: &Palette, primary: Option<String>, list: bool) {
        let p = ui.painter();
        let cy = rect.center().y;
        // App icon at the far left, as the original launcher does.
        let icon = self.app_icon.get_or_insert_with(|| {
            let img = services::decode_png(include_bytes!("../../../resources/icon-1024.png"))
                .unwrap_or_else(|| egui::ColorImage::filled([1, 1], Color32::TRANSPARENT));
            ui.ctx().load_texture("app-icon", img, TextureOptions::LINEAR)
        });
        let ir = Rect::from_center_size(Pos2::new(rect.min.x + 26.0, cy), Vec2::splat(18.0));
        p.image(icon.id(), ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        let tx = rect.min.x + 42.0;
        let q = self.query.trim();
        let searching = list
            && q.chars().count() >= 2
            && self.files.0 != q
            && match self.view() {
                View::Root => self.settings.file_search_in_root,
                View::Files => true,
                _ => false,
            };
        match &self.toast {
            Some((t, _)) => {
                p.text(Pos2::new(tx, cy), Align2::LEFT_CENTER, t, FontId::proportional(12.5), pal.accent);
            }
            None if searching => {
                // Three dots that pulse while Spotlight answers; cheap, and it stops on its own.
                let t = ui.ctx().input(|i| i.time);
                p.text(Pos2::new(tx, cy), Align2::LEFT_CENTER, "Searching files", FontId::proportional(12.5), pal.sub);
                let w = p.layout_no_wrap("Searching files".into(), FontId::proportional(12.5), pal.sub).size().x;
                for k in 0..3 {
                    let a = (((t * 3.0 - k as f64 * 0.6).sin() * 0.5 + 0.5) * 255.0) as u8;
                    p.circle_filled(
                        Pos2::new(tx + w + 6.0 + k as f32 * 7.0, cy + 3.0),
                        1.8,
                        Color32::from_rgba_unmultiplied(pal.sub.r(), pal.sub.g(), pal.sub.b(), a),
                    );
                }
                ui.ctx().request_repaint_after(Duration::from_millis(60));
            }
            None => {
                p.text(Pos2::new(tx, cy), Align2::LEFT_CENTER, self.view().title(), FontId::proportional(12.5), pal.sub);
            }
        }
        let mut x = rect.max.x - 16.0;
        let mut first = true;
        let mut hint = |label: &str, key: &str| {
            if !first {
                // thin separator between hint groups, as in the original launcher footers
                p.vline(x + 9.0, (cy - 9.0)..=(cy + 9.0), Stroke::new(1.0, pal.sep));
            }
            first = false;
            let kg = p.layout_no_wrap(key.to_string(), FontId::proportional(11.5), pal.text);
            let lg = p.layout_no_wrap(label.to_string(), FontId::proportional(12.5), pal.sub);
            let kw = (kg.size().x + 12.0).max(22.0);
            let kr = Rect::from_min_size(Pos2::new(x - kw, cy - 10.0), Vec2::new(kw, 20.0));
            p.rect_filled(kr, 5.0, pal.field);
            p.galley(Pos2::new(kr.center().x - kg.size().x / 2.0, cy - kg.size().y / 2.0), kg, pal.text);
            let lx = kr.min.x - 8.0 - lg.size().x;
            p.galley(Pos2::new(lx, cy - lg.size().y / 2.0), lg, pal.text);
            x = lx - 18.0;
        };
        if list {
            hint("Actions", "⌘K");
        }
        if let Some(pl) = primary {
            hint(&pl, "↵");
        }
        if !list && self.stack.len() > 1 {
            hint("Back", "esc");
        }
    }

    fn draw_list(&mut self, ui: &mut Ui, rect: Rect, sections: &[Section], pal: &Palette) -> (Option<usize>, Option<usize>) {
        let mut clicked = None;
        let mut double = None;
        let sel = self.selected;
        let scroll_sel = self.scroll_to_sel;
        let ctx = ui.ctx().clone();
        let moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO);
        let is_root = matches!(self.view(), View::Root);
        let mut hover_sel = None;
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(4.0);
                let mut idx = 0;
                for sec in sections {
                    let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
                    ui.painter().text(
                        Pos2::new(hr.min.x + 18.0, hr.center().y + 2.0),
                        Align2::LEFT_CENTER,
                        &sec.title,
                        FontId::proportional(12.0),
                        pal.sub,
                    );
                    for item in &sec.items {
                        let is_calc = matches!(item.kind, Kind::Calc(_));
                        let row_h = if is_calc { 86.0 } else { ui::ROW_H };
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                        let selected = idx == sel;
                        let painter = ui.painter().clone();
                        if selected {
                            // The highlight glides from its previous row to this one (y and height
                            // are animated; the row itself is painted at its real position).
                            let salt = ctx.data(|d| d.get_temp::<usize>(Id::new("sel_salt"))).unwrap_or(0);
                            let ay = ctx.animate_value_with_time(Id::new(("sel_y", salt)), r.min.y, 0.11);
                            let ah = ctx.animate_value_with_time(Id::new(("sel_h", salt)), r.height(), 0.11);
                            let hr = Rect::from_min_size(Pos2::new(r.min.x, ay), Vec2::new(r.width(), ah));
                            painter.rect_filled(hr.shrink2(Vec2::new(6.0, 1.0)), 8.0, pal.sel);
                            if scroll_sel {
                                ui.scroll_to_rect(r, None);
                            }
                        }
                        if resp.hovered() && moved && !selected {
                            hover_sel = Some(idx);
                        }
                        if resp.clicked() {
                            clicked = Some(idx);
                        }
                        if resp.double_clicked() {
                            double = Some(idx);
                        }
                        // Rows scrolled out of view still take part in layout and selection, but
                        // painting them (and fetching their icons) would be wasted work.
                        if !ui.is_rect_visible(r) {
                            idx += 1;
                            continue;
                        }
                        if is_calc {
                            // Calculator card: question on the left, big answer on the right.
                            let mid = r.center().y;
                            let half = r.width() / 2.0;
                            painter.text(
                                Pos2::new(r.min.x + half / 2.0, mid - 8.0),
                                Align2::CENTER_CENTER,
                                &item.subtitle,
                                FontId::proportional(20.0),
                                pal.text,
                            );
                            painter.text(
                                Pos2::new(r.min.x + half / 2.0, mid + 20.0),
                                Align2::CENTER_CENTER,
                                if item.accessory.is_empty() { "Calculator" } else { item.accessory.as_str() },
                                FontId::proportional(11.5),
                                pal.sub,
                            );
                            painter.text(Pos2::new(r.center().x, mid), Align2::CENTER_CENTER, "→", FontId::proportional(20.0), pal.sub);
                            painter.text(
                                Pos2::new(r.min.x + half * 1.5, mid - 8.0),
                                Align2::CENTER_CENTER,
                                &item.title,
                                FontId::proportional(34.0),
                                pal.text,
                            );
                            painter.text(
                                Pos2::new(r.min.x + half * 1.5, mid + 24.0),
                                Align2::CENTER_CENTER,
                                "Answer",
                                FontId::proportional(11.5),
                                pal.sub,
                            );
                            idx += 1;
                            continue;
                        }
                        let icon_rect = Rect::from_center_size(Pos2::new(r.min.x + 31.0, r.center().y), Vec2::splat(26.0));
                        ui::paint_icon(&mut self.icons, ui, icon_rect, &item.icon, pal);
                        let mut acc: Vec<String> = vec![];
                        if is_root {
                            if let Some(a) = self.settings.alias(&item.id) {
                                acc.push(format!("⌥ {a}"));
                            }
                            if let Some(h) = self.settings.hotkey(&item.id) {
                                acc.push(services::hotkey_label(h));
                            }
                            if self.settings.is_favorite(&item.id) {
                                acc.push("★".into());
                            }
                        }
                        if !item.accessory.is_empty() {
                            acc.push(item.accessory.clone());
                        }
                        let acc = acc.join("  ");
                        let right = r.max.x - 18.0;
                        let ag = painter.layout_no_wrap(acc, FontId::proportional(12.0), pal.sub);
                        let aw = ag.size().x;
                        painter.galley(Pos2::new(right - aw, r.center().y - ag.size().y / 2.0), ag, pal.sub);
                        let tx = r.min.x + 52.0;
                        let clip = Rect::from_min_max(Pos2::new(tx, r.min.y), Pos2::new((right - aw - 10.0).max(tx + 20.0), r.max.y));
                        let p = painter.with_clip_rect(clip);
                        let title = elide(&p, &item.title, FontId::proportional(14.0), clip.width());
                        let tg = p.layout_no_wrap(title, FontId::proportional(14.0), pal.text);
                        let tw = tg.size().x;
                        p.galley(Pos2::new(tx, r.center().y - tg.size().y / 2.0), tg, pal.text);
                        if !item.subtitle.is_empty() {
                            p.text(
                                Pos2::new(tx + tw + 8.0, r.center().y),
                                Align2::LEFT_CENTER,
                                &item.subtitle,
                                FontId::proportional(12.5),
                                pal.sub,
                            );
                        }
                        idx += 1;
                    }
                }
                ui.add_space(4.0);
            });
        });
        if let Some(i) = hover_sel {
            self.selected = i;
        }
        (clicked, double)
    }

    /// Preview on top (in a tone-stepped card), facts underneath as label/value rows.
    fn draw_detail(&mut self, ui: &mut Ui, rect: Rect, d: Detail, pal: &Palette) {
        let ctx = ui.ctx().clone();
        let (meta, is_image) = match &d {
            Detail::Text(_, m) => (m.clone(), false),
            Detail::Image(_, m) => (m.clone(), true),
        };
        let meta_h = 26.0 * meta.len() as f32 + 30.0;
        let card = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.max.y - meta_h));
        let p = ui.painter().clone();
        p.rect_filled(card, 10.0, pal.field);
        match d {
            Detail::Text(t, _) => {
                let inner = card.shrink(12.0);
                ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
                    egui::ScrollArea::vertical().id_salt("detail").auto_shrink([false; 2]).show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(t).size(13.5).color(pal.text)).wrap().selectable(true));
                    });
                });
            }
            Detail::Image(name, _) => {
                let icon = Icon::ClipImage(name);
                if let Some(t) = self.icons.texture(&ctx, &icon) {
                    let r = ui::fit(card.shrink(12.0), t.size_vec2());
                    p.image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
            }
        }
        let _ = is_image;
        // facts
        let mut y = card.max.y + 14.0;
        p.text(Pos2::new(rect.min.x + 2.0, y), Align2::LEFT_CENTER, "Information", FontId::proportional(11.5), pal.sub);
        y += 18.0;
        for (k, v) in &meta {
            p.text(Pos2::new(rect.min.x + 2.0, y), Align2::LEFT_CENTER, k, FontId::proportional(12.5), pal.sub);
            let vg = p.layout_no_wrap(v.clone(), FontId::proportional(12.5), pal.text);
            let max_w = rect.width() - 110.0;
            let shown = if vg.size().x > max_w { elide(&p, v, FontId::proportional(12.5), max_w) } else { v.clone() };
            p.text(Pos2::new(rect.max.x - 2.0, y), Align2::RIGHT_CENTER, shown, FontId::proportional(12.5), pal.text);
            p.hline((rect.min.x + 2.0)..=(rect.max.x - 2.0), y + 13.0, Stroke::new(1.0, pal.sep));
            y += 26.0;
        }
    }

    fn draw_panel(&mut self, ui: &mut Ui, content: Rect, pal: &Palette) {
        let Some(p) = self.panel.as_mut() else { return };
        let idx = panel_filter(p);
        let h = (idx.len() as f32 * 34.0 + 56.0).min(320.0);
        let rect = Rect::from_min_max(
            Pos2::new(content.max.x - 340.0, content.max.y - h - 6.0),
            Pos2::new(content.max.x - 12.0, content.max.y - 6.0),
        );
        // Pop in from the bottom-right corner: scale 0.96 → 1 and fade, 120 ms.
        let t = ui.ctx().animate_bool_with_time(Id::new("panel_pop"), true, 0.12);
        let rect = Rect::from_min_size(rect.max - rect.size() * (0.96 + 0.04 * t), rect.size() * (0.96 + 0.04 * t))
            .translate(Vec2::new(0.0, (1.0 - t) * 8.0));
        ui.set_opacity(t);
        let painter = ui.painter().clone();
        painter.rect_filled(rect.expand(1.0), 10.0, Color32::from_black_alpha(90));
        painter.rect_filled(rect, 10.0, if self.dark { Color32::from_rgb(34, 34, 38) } else { Color32::from_rgb(246, 246, 248) });
        painter.rect_stroke(rect, 10.0, Stroke::new(1.0, pal.border), StrokeKind::Inside);
        let list_rect = Rect::from_min_max(rect.min + Vec2::new(0.0, 6.0), Pos2::new(rect.max.x, rect.max.y - 44.0));
        let sel = p.sel;
        let mut click = None;
        ui.scope_builder(UiBuilder::new().max_rect(list_rect), |ui| {
            egui::ScrollArea::vertical().id_salt("actions").auto_shrink([false; 2]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (n, &i) in idx.iter().enumerate() {
                    let a = &p.actions[i];
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
                    if n == sel {
                        painter.rect_filled(r.shrink2(Vec2::new(6.0, 1.0)), 7.0, pal.sel);
                        ui.scroll_to_rect(r, None);
                    }
                    if resp.clicked() {
                        click = Some(i);
                    }
                    let col = if a.destructive { pal.danger } else { pal.text };
                    painter.text(Pos2::new(r.min.x + 16.0, r.center().y), Align2::LEFT_CENTER, &a.title, FontId::proportional(13.5), col);
                    if let Some((m, k)) = a.shortcut {
                        painter.text(
                            Pos2::new(r.max.x - 14.0, r.center().y),
                            Align2::RIGHT_CENTER,
                            shortcut_label(m, k),
                            FontId::proportional(12.0),
                            pal.sub,
                        );
                    }
                }
            });
        });
        let sep_y = rect.max.y - 44.0;
        painter.hline(rect.min.x..=rect.max.x, sep_y, Stroke::new(1.0, pal.sep));
        let fr = Rect::from_min_max(Pos2::new(rect.min.x + 14.0, sep_y), Pos2::new(rect.max.x - 14.0, rect.max.y));
        let old = p.query.clone();
        ui.scope_builder(UiBuilder::new().max_rect(fr), |ui| {
            ui.horizontal_centered(|ui| {
                let r = ui.add(
                    TextEdit::singleline(&mut p.query)
                        .hint_text(RichText::new("Search for actions…").color(pal.sub))
                        .frame(EFrame::NONE)
                        .desired_width(f32::INFINITY)
                        .id(Id::new("action-search")),
                );
                r.request_focus();
            });
        });
        if p.query != old {
            p.sel = 0;
        }
        if let Some(i) = click {
            let act = p.actions[i].kind.clone();
            let item = p.item.clone();
            self.panel = None;
            self.do_action(item, act);
        }
    }

    /// The drop wheel: a ring of segments around the window centre; the one under the
    /// pointer (sampled globally, since a drag owns the mouse) is highlighted.
    fn draw_wheel(&mut self, ui: &mut Ui, pal: &Palette, center: Pos2, segs: &[WheelSeg], nfiles: usize, sel: Option<usize>) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(Duration::from_millis(16));
        let c = ui.max_rect().center();
        let hover = mouse_top_left().and_then(|(m, _)| wheel_hit(m - center, segs.len())).or(sel);
        let p = ui.painter();
        let n = segs.len();
        let step = std::f32::consts::TAU / n as f32;
        // backdrop ring
        p.circle_filled(c, WHEEL_OUTER + 6.0, pal.bg);
        p.circle_stroke(c, WHEEL_OUTER + 6.0, Stroke::new(1.0, pal.border));
        let accent = Color32::from_rgb(255, 140, 60);
        for (i, seg) in segs.iter().enumerate() {
            // segment i is centred on i*step, measured clockwise from 12 o'clock
            let a0 = i as f32 * step - step / 2.0;
            let pt = |a: f32, r: f32| c + Vec2::new(a.sin(), -a.cos()) * r;
            if hover == Some(i) {
                let k = 24;
                let mut pts: Vec<Pos2> = (0..=k).map(|j| pt(a0 + step * j as f32 / k as f32, WHEEL_OUTER)).collect();
                pts.extend((0..=k).rev().map(|j| pt(a0 + step * j as f32 / k as f32, WHEEL_INNER)));
                // egui fills convex shapes only: draw the sector as quads
                for j in 0..k {
                    let q = vec![pts[j], pts[j + 1], pts[pts.len() - 2 - j], pts[pts.len() - 1 - j]];
                    p.add(egui::Shape::convex_polygon(q, accent.gamma_multiply(0.35), Stroke::NONE));
                }
            }
            p.line_segment([pt(a0, WHEEL_INNER), pt(a0, WHEEL_OUTER)], Stroke::new(1.0, pal.sep));
            let mid = pt(a0 + step / 2.0, (WHEEL_INNER + WHEEL_OUTER) / 2.0);
            let ir = Rect::from_center_size(mid - Vec2::new(0.0, 10.0), Vec2::splat(22.0));
            ui::paint_icon(&mut self.icons, ui, ir, &Icon::Glyph(seg.symbol, [255, 140, 60]), pal);
            let col = if hover == Some(i) { pal.text } else { pal.sub };
            ui.painter().text(mid + Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, &seg.label, FontId::proportional(11.5), col);
        }
        let p = ui.painter();
        p.circle_filled(c, WHEEL_INNER - 4.0, pal.field);
        let hint = match hover {
            Some(i) => segs[i].label.clone(),
            None => format!("{nfiles} file{}", if nfiles == 1 { "" } else { "s" }),
        };
        p.text(c, Align2::CENTER_CENTER, hint, FontId::proportional(12.0), pal.text);
    }

    fn draw_single(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette) {
        let ff = std::mem::take(&mut self.focus_first);
        let view = self.view().clone();
        match view {
            View::Settings => {
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| self.draw_settings(ui, pal));
            }
            View::NoteEdit(id) => {
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    let mut changed = false;
                    if let Some(n) = self.notes.iter_mut().find(|n| n.id == id) {
                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            let r = ui.add(
                                TextEdit::multiline(&mut n.text)
                                    .hint_text("Start typing… (markdown supported)")
                                    .frame(EFrame::NONE)
                                    .font(FontId::proportional(14.5))
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(14),
                            );
                            if ff {
                                r.request_focus();
                            }
                            if r.changed() {
                                n.modified = now_secs();
                                changed = true;
                            }
                        });
                    }
                    if changed {
                        self.save_notes();
                    }
                });
            }
            _ => {
                let trusted = windows::accessibility_trusted(false);
                let launcher_hotkey = services::hotkey_label(&self.settings.launcher_hotkey);
                let data_dir = self.store.root.display().to_string();
                let (mut grant, mut open_privacy) = (false, false);
                let app_icon = self.app_icon.clone();
                let provider_label = {
                    use ferrocast_core::models::AiProvider as P;
                    match self.settings.ai_provider {
                        P::OpenAiCompatible => {
                            let host = self
                                .settings
                                .ai_base_url
                                .trim_start_matches("http://")
                                .trim_start_matches("https://")
                                .split('/')
                                .next()
                                .unwrap_or("")
                                .to_string();
                            format!(
                                "{} · {}",
                                if host.starts_with("localhost") || host.starts_with("127.") { "Local".to_string() } else { host },
                                self.settings.ai_model
                            )
                        }
                        P::ChatGpt => format!(
                            "ChatGPT · {}",
                            if self.settings.chatgpt_model.is_empty() {
                                ferrocast_core::chatgpt::DEFAULT_MODEL
                            } else {
                                &self.settings.chatgpt_model
                            }
                        ),
                        P::Anthropic => format!(
                            "Anthropic · {}",
                            if self.settings.anthropic_model.is_empty() {
                                ferrocast_core::ai::DEFAULT_ANTHROPIC_MODEL
                            } else {
                                &self.settings.anthropic_model
                            }
                        ),
                        P::ClaudeCode => "Claude Code".to_string(),
                    }
                };
                // Providers the chat can switch to right now: configured, signed in or installed.
                // The Keychain checks cost a few ms each, so they are refreshed every 10 s at most.
                let active: Vec<(ferrocast_core::models::AiProvider, &'static str)> = {
                    use ferrocast_core::models::AiProvider as P;
                    let id = Id::new("ai_active");
                    let now = ui.ctx().input(|i| i.time);
                    let cached: Option<(f64, Vec<u8>)> = ui.ctx().data(|d| d.get_temp(id));
                    let flags: Vec<u8> = match cached {
                        Some((t, f)) if now - t < 10.0 => f,
                        _ => {
                            let f = vec![
                                1u8,
                                services::chatgpt_credentials().is_some() as u8,
                                services::ai_key_for(P::Anthropic).is_some() as u8,
                                services::claude_code_path().is_some() as u8,
                            ];
                            ui.ctx().data_mut(|d| d.insert_temp(id, (now, f.clone())));
                            f
                        }
                    };
                    [
                        (P::OpenAiCompatible, "OpenAI-compatible"),
                        (P::ChatGpt, "ChatGPT"),
                        (P::Anthropic, "Anthropic"),
                        (P::ClaudeCode, "Claude Code"),
                    ]
                    .into_iter()
                    .zip(flags)
                    .filter_map(|(p, on)| (on == 1).then_some(p))
                    .collect()
                };
                let models_key = services::models_key(&self.settings);
                let models_state = self.ai_models.get(&models_key).cloned();
                let cur_provider = self.settings.ai_provider;
                let cur_model = match cur_provider {
                    ferrocast_core::models::AiProvider::OpenAiCompatible => self.settings.ai_model.clone(),
                    ferrocast_core::models::AiProvider::ChatGpt => self.settings.chatgpt_model.clone(),
                    _ => self.settings.anthropic_model.clone(),
                };
                let mut pick_provider: Option<ferrocast_core::models::AiProvider> = None;
                let mut pick_model: Option<String> = None;
                let mut want_models = false;
                let mut open_url: Option<String> = None;
                // The match below borrows the view stack mutably; take the icon cache out so these
                // screens can still draw cached symbols.
                let mut icons = std::mem::replace(&mut self.icons, IconCache::new(self.store.images_dir()));
                let mut clear_hotkey = false;
                let mut set_hotkey = false;
                let mut mode_changed = None;
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    let lbl = |ui: &mut Ui, t: &str| {
                        ui.label(RichText::new(t).size(12.0).color(pal.sub));
                    };
                    match self.stack.last_mut().unwrap() {
                        View::SnippetForm { name, text, keyword, .. } => {
                            lbl(ui, "Name");
                            let r = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY).hint_text("e.g. Email signature"));
                            if ff {
                                r.request_focus();
                            }
                            lbl(ui, "Snippet");
                            ui.add(TextEdit::multiline(text).desired_width(f32::INFINITY).desired_rows(7).hint_text(
                                "Text to expand. Supports {clipboard}, {date}, {time}, {cursor}, {uuid}, {argument name=\"x\"}",
                            ));
                            lbl(ui, "Keyword (type it anywhere to expand)");
                            ui.add(TextEdit::singleline(keyword).desired_width(300.0).hint_text("e.g. !sig"));
                            lbl(ui, "Save with ⌘↵ · esc to cancel");
                        }
                        View::QuicklinkForm { name, link, open_with, .. } => {
                            lbl(ui, "Name");
                            let r = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY).hint_text("e.g. Search GitHub"));
                            if ff {
                                r.request_focus();
                            }
                            lbl(ui, "Link (URL, file path or deeplink)");
                            ui.add(
                                TextEdit::singleline(link)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("https://github.com/search?q={argument name=\"query\"}"),
                            );
                            lbl(ui, "Open with (optional app name)");
                            ui.add(TextEdit::singleline(open_with).desired_width(300.0).hint_text("e.g. Safari"));
                            lbl(ui, "Save with ⌘↵ · esc to cancel");
                        }
                        View::Args { defs, values, title, .. } => {
                            ui.label(RichText::new(title.as_str()).size(15.0).strong().color(pal.text));
                            for (i, d) in defs.iter().enumerate() {
                                lbl(ui, &format!("{}{}", d.name, if d.optional { " (optional)" } else { "" }));
                                if d.options.is_empty() {
                                    let r = ui.add(TextEdit::singleline(&mut values[i]).password(d.password).desired_width(f32::INFINITY));
                                    if ff && i == 0 {
                                        r.request_focus();
                                    }
                                } else {
                                    let cur = d.options.iter().find(|(_, v)| *v == values[i]).map(|(t, _)| t.clone()).unwrap_or_default();
                                    egui::ComboBox::from_id_salt(("arg", i)).selected_text(cur).show_ui(ui, |ui| {
                                        for (t, v) in &d.options {
                                            ui.selectable_value(&mut values[i], v.clone(), t);
                                        }
                                    });
                                }
                            }
                            lbl(ui, "Run with ↵ · Tab moves between fields");
                        }
                        View::Output { text, done, title, .. } => {
                            ui.label(
                                RichText::new(if *done { title.clone() } else { format!("{title} — running…") }).strong().color(pal.text),
                            );
                            egui::ScrollArea::both().auto_shrink([false; 2]).show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(script::strip_ansi(text)).monospace().size(12.5).color(pal.text))
                                        .selectable(true),
                                );
                            });
                        }
                        View::Ai { prompt, asked, answer, history, busy, .. } => {
                            // input row: field in a card, provider chip on the right
                            let w = ui.available_width();
                            ui.horizontal(|ui| {
                                let chip_w = 150.0;
                                egui::Frame::new().fill(pal.field).corner_radius(10.0).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                                    ui.set_width(w - chip_w - 36.0);
                                    let r = ui.add(TextEdit::singleline(prompt).hint_text("Ask anything…").font(FontId::proportional(15.0)).frame(EFrame::NONE).desired_width(f32::INFINITY));
                                    if ff {
                                        r.request_focus();
                                    }
                                });
                                ui.add_space(8.0);
                                let (r, resp) = ui.allocate_exact_size(Vec2::new(chip_w, 34.0), Sense::click());
                                let p = ui.painter();
                                p.rect_filled(r, 10.0, if resp.hovered() { pal.sel } else { pal.field });
                                let g = p.layout_no_wrap(provider_label.clone(), FontId::proportional(11.5), pal.sub);
                                let shown = if g.size().x > chip_w - 34.0 { elide(p, &provider_label, FontId::proportional(11.5), chip_w - 34.0) } else { provider_label.clone() };
                                p.text(Pos2::new(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, shown, FontId::proportional(11.5), pal.text);
                                p.text(Pos2::new(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, "⌄", FontId::proportional(12.0), pal.sub);
                                let mut menu = egui::Popup::menu(&resp);
                                if std::env::var_os("FERROCAST_AI_MENU").is_some() {
                                    menu = menu.open(true); // dev aid for screenshots
                                }
                                menu.show(|ui| {
                                    ui.set_min_width(260.0);
                                    ui.label(RichText::new("Provider").size(11.5).color(pal.sub));
                                    for (prov, name) in &active {
                                        if ui.selectable_label(*prov == cur_provider, *name).clicked() {
                                            pick_provider = Some(*prov);
                                            ui.close();
                                        }
                                    }
                                    ui.separator();
                                    ui.label(RichText::new("Model").size(11.5).color(pal.sub));
                                    if ui.selectable_label(cur_model.is_empty(), "Default").clicked() {
                                        pick_model = Some(String::new());
                                        ui.close();
                                    }
                                    match &models_state {
                                        Some(Some(Ok(list))) => {
                                            for m in list {
                                                if ui.selectable_label(cur_model == *m, m).clicked() {
                                                    pick_model = Some(m.clone());
                                                    ui.close();
                                                }
                                            }
                                        }
                                        Some(Some(Err(e))) => {
                                            ui.label(RichText::new(format!("List unavailable: {e}")).size(11.5).color(pal.sub));
                                        }
                                        _ => {
                                            ui.label(RichText::new("Loading…").size(11.5).color(pal.sub));
                                            want_models = true;
                                        }
                                    }
                                    ui.separator();
                                    ui.label(RichText::new("More in Settings → AI (⌘ ,)").size(11.0).color(pal.sub));
                                });
                            });
                            ui.add_space(6.0);
                            if history.is_empty() && asked.is_empty() {
                                // empty state: what this is, and a few prompts to try
                                ui.add_space(26.0);
                                ui.vertical_centered(|ui| {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                                    ui.painter().rect_filled(r, 16.0, Color32::from_rgb(168, 110, 255));
                                    ui::paint_icon(&mut icons, ui, r, &Icon::Glyph("sf:sparkles", [168, 110, 255]), pal);
                                    ui.add_space(12.0);
                                    ui.label(RichText::new("Ask anything").size(20.0).color(pal.text));
                                    ui.label(RichText::new("Short answers, in a conversation that lasts while this window is open.").size(12.5).color(pal.sub));
                                    ui.add_space(16.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                                        for ex in ["Explain this regex: ^\\d{3}-\\d{4}$", "Write a git commit message for a bug fix", "What is 18% of 240, step by step?", "Translate “see you tomorrow” to Spanish"] {
                                            if pill(ui, pal, ex, false) {
                                                *prompt = ex.to_string();
                                            }
                                        }
                                    });
                                });
                            } else {
                                egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false; 2]).show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 10.0;
                                    let t = ui.ctx().input(|i| i.time);
                                    let mut turn = |ui: &mut Ui, q: &str, a: &str, thinking: bool| {
                                        // question: right-aligned card
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                            egui::Frame::new().fill(pal.field).corner_radius(12.0).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                                                ui.set_max_width(ui.available_width() * 0.72);
                                                ui.add(egui::Label::new(RichText::new(q).size(13.5).color(pal.text)).wrap());
                                            });
                                        });
                                        // answer: sparkles tile + text
                                        ui.horizontal_top(|ui| {
                                            let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                                            ui.painter().rect_filled(r, 6.0, Color32::from_rgb(168, 110, 255));
                                            ui::paint_icon(&mut icons, ui, r, &Icon::Glyph("sf:sparkles", [168, 110, 255]), pal);
                                            ui.add_space(4.0);
                                            ui.vertical(|ui| {
                                                ui.set_max_width(ui.available_width() - 8.0);
                                                if thinking && a.is_empty() {
                                                    let (r, _) = ui.allocate_exact_size(Vec2::new(40.0, 18.0), Sense::hover());
                                                    for k in 0..3 {
                                                        let al = (((t * 3.0 - k as f64 * 0.6).sin() * 0.5 + 0.5) * 255.0) as u8;
                                                        ui.painter().circle_filled(Pos2::new(r.min.x + 6.0 + k as f32 * 10.0, r.center().y), 3.0, Color32::from_rgba_unmultiplied(pal.sub.r(), pal.sub.g(), pal.sub.b(), al));
                                                    }
                                                    ui.ctx().request_repaint_after(Duration::from_millis(60));
                                                } else {
                                                    ui.add(egui::Label::new(RichText::new(a).size(14.0).color(pal.text)).wrap().selectable(true));
                                                }
                                            });
                                        });
                                        ui.add_space(6.0);
                                    };
                                    for (q, a) in history.iter() {
                                        turn(ui, q, a, false);
                                    }
                                    if !asked.is_empty() {
                                        turn(ui, asked, answer, *busy);
                                    }
                                });
                            }
                        }
                        View::Welcome => {
                            egui::ScrollArea::vertical().id_salt("welcome").auto_shrink([false; 2]).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            ui.vertical_centered(|ui| {
                                if let Some(t) = &app_icon {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                                    ui.painter().image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                }
                                ui.add_space(6.0);
                                ui.label(RichText::new("Welcome to Ferrocast").size(22.0).color(pal.text));
                                ui.label(RichText::new(format!("Press {launcher_hotkey} anywhere. Type to launch, calculate, paste or ask.")).size(13.0).color(pal.sub));
                            });
                            ui.add_space(14.0);
                            group(ui, pal, |ui| {
                                row(ui, pal, "Accessibility", if trusted { "Granted. Window management, pasting and snippet expansion are ready." } else { "Needed to move windows, paste for you and expand snippets. Search, the calculator and AI work without it." }, |ui| {
                                    if trusted {
                                        ui.label(RichText::new("Granted ✓").size(13.0).color(Color32::from_rgb(89, 212, 153)));
                                    } else {
                                        if small_btn(ui, pal, "System Settings") {
                                            open_privacy = true;
                                        }
                                        if accent_btn(ui, pal, "Grant…") {
                                            grant = true;
                                        }
                                    }
                                });
                            });
                            ui.add_space(14.0);
                            section(ui, pal, "Three things to try");
                            let tips: [(&str, &str, &str); 3] = [
                                ("sf:command", "Press ⌘ K on anything", "Every item has more actions: copy, reveal, set an alias or a hotkey."),
                                ("sf:equal", "Type a sum or a conversion", "100 usd in eur · 5pm ldn in sf · 18% of 240 · timer 10m"),
                                ("sf:doc.on.clipboard", "Open Clipboard History", "Everything you copy, searchable, with pins. Password managers are skipped."),
                            ];
                            group(ui, pal, |ui| {
                                for (sym, title, desc) in tips {
                                    row(ui, pal, title, desc, |ui| {
                                        let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                                        ui::paint_icon(&mut icons, ui, r, &Icon::Glyph(sym, [90, 90, 105]), pal);
                                    });
                                }
                            });
                            ui.add_space(8.0);
                            lbl(ui, "esc closes this screen · it is always one search away as “Welcome & Permissions”");
                            });
                        }
                        View::Shortcuts => {
                            let groups: [(&str, &[(&str, &str)]); 3] = [
                                ("Navigate", &[("⌥ Space", "Open or close Ferrocast"), ("↑ ↓", "Move the selection"), ("esc", "Clear the search, go back, or close"), ("⌫", "Go back when the search is empty")]),
                                ("Act", &[("↵", "Run the primary action"), ("⌘ ↵", "Run the secondary action"), ("⌘ K", "Open the action panel"), ("⌘ Y", "Quick Look the selected file")]),
                                ("Manage", &[("⌘ ,", "Open Settings"), ("⌘ N", "New snippet, quicklink or note"), ("⌘ E", "Edit the selected snippet or quicklink"), ("⇧ ⌘ F", "Add or remove a favorite"), ("⇧ ⌘ ,", "Set a hotkey for a command"), ("⌘ .", "Pin or unpin a clipboard entry"), ("⌃ X", "Delete the selected entry")]),
                            ];
                            egui::ScrollArea::vertical().id_salt("shortcuts").auto_shrink([false; 2]).show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                for (name, rows) in groups {
                                    section(ui, pal, name);
                                    group(ui, pal, |ui| {
                                        for (keys, what) in rows.iter() {
                                            let w = ui.available_width();
                                            egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 9)).show(ui, |ui| {
                                                ui.set_width(w - 28.0);
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new(*what).size(13.5).color(pal.text));
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.spacing_mut().item_spacing.x = 4.0;
                                                        for k in keys.split(' ').rev() {
                                                            keycap(ui, pal, k);
                                                        }
                                                    });
                                                });
                                            });
                                        }
                                    });
                                    ui.add_space(14.0);
                                }
                            });
                        }
                        View::About => {
                            egui::ScrollArea::vertical().id_salt("about").auto_shrink([false; 2]).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            ui.add_space(4.0);
                            ui.vertical_centered(|ui| {
                                if let Some(t) = &app_icon {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
                                    ui.painter().image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                }
                                ui.add_space(8.0);
                                ui.label(RichText::new("Ferrocast").size(26.0).color(pal.text));
                                ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(13.0).color(pal.sub));
                                ui.add_space(10.0);
                                ui.label(RichText::new("A small, fast launcher for the Mac. Open source, no account, your data in files you can read.").size(13.5).color(pal.text));
                                ui.add_space(12.0);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    let total = 3.0 * 96.0;
                                    ui.add_space((ui.available_width() - total).max(0.0) / 2.0);
                                    if small_btn(ui, pal, "GitHub") {
                                        open_url = Some("https://github.com/drumst0ck/ferrocast".into());
                                    }
                                    if small_btn(ui, pal, "Changelog") {
                                        open_url = Some("https://github.com/drumst0ck/ferrocast/blob/main/CHANGELOG.md".into());
                                    }
                                    if small_btn(ui, pal, "Data folder") {
                                        open_url = Some(data_dir.clone());
                                    }
                                });
                            });
                            ui.add_space(18.0);
                            group(ui, pal, |ui| {
                                row(ui, pal, "Licence", "MIT. Fork it, fix it, ship it.", |ui| keycap(ui, pal, "MIT"));
                                row(ui, pal, "Currency rates", "European Central Bank, through frankfurter.dev, cached for 12 hours.", |_ui| {});
                                row(ui, pal, "Trademarks", "Raycast is a trademark of its owner. Ferrocast is an independent project.", |_ui| {});
                            });
                            });
                        }
                        View::Confirm { msg, .. } => {
                            ui.add_space(50.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(msg.as_str()).size(20.0).strong().color(pal.text));
                                ui.add_space(10.0);
                                ui.label(RichText::new("↵ Confirm      esc Cancel").color(pal.sub));
                            });
                        }
                        View::Alias { name, value, .. } => {
                            ui.label(RichText::new(format!("Alias for “{name}”")).size(15.0).strong().color(pal.text));
                            let r = ui.add(TextEdit::singleline(value).desired_width(300.0).hint_text("e.g. gc"));
                            if ff {
                                r.request_focus();
                            }
                            lbl(ui, "Type the alias in Root Search to jump to this command. Empty removes it. ↵ saves.");
                        }
                        View::Hotkey { id, name, err } => {
                            ui.add_space(30.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(format!("Hotkey for “{name}”")).size(16.0).strong().color(pal.text));
                                ui.add_space(8.0);
                                ui.label(RichText::new("Press the key combination now…").size(20.0).color(pal.accent));
                                if !err.is_empty() {
                                    ui.label(RichText::new(err.as_str()).color(pal.danger));
                                }
                                ui.add_space(8.0);
                                if id != "__launcher" && ui.button("Clear hotkey").clicked() {
                                    clear_hotkey = true;
                                }
                                lbl(ui, "esc to cancel · include ⌘ ⌥ ⌃ or ⇧");
                            });
                            set_hotkey = true;
                        }
                        View::CreateScript { title, mode } => {
                            lbl(ui, "Title");
                            let r = ui.add(TextEdit::singleline(title).desired_width(f32::INFINITY).hint_text("e.g. Deploy staging"));
                            if ff {
                                r.request_focus();
                            }
                            lbl(ui, "Output mode");
                            let names = ["Full Output", "Compact (toast)", "Silent (notification)"];
                            let before = *mode;
                            egui::ComboBox::from_id_salt("mode").selected_text(names[(*mode).min(2)]).show_ui(ui, |ui| {
                                for (i, n) in names.iter().enumerate() {
                                    ui.selectable_value(mode, i, *n);
                                }
                            });
                            if before != *mode {
                                mode_changed = Some(*mode);
                            }
                            lbl(ui, "Creates a bash script in your Script Commands folder and opens it in your editor. ↵ creates.");
                        }
                        _ => {}
                    }
                });
                let _ = (set_hotkey, mode_changed);
                self.icons = icons;
                if want_models && !self.ai_models.contains_key(&models_key) {
                    self.ai_models.insert(models_key.clone(), None);
                    services::fetch_models(&self.settings, self.bus.clone());
                }
                if let Some(p) = pick_provider.filter(|p| *p != self.settings.ai_provider) {
                    self.settings.ai_provider = p;
                    self.save_settings();
                    // a different brain: drop the warm Claude session, start one if Claude Code was chosen
                    if let Some(c) = self.claude.take() {
                        c.kill();
                    }
                    self.warm_claude();
                    let k = services::models_key(&self.settings);
                    if let std::collections::hash_map::Entry::Vacant(e) = self.ai_models.entry(k) {
                        e.insert(None);
                        services::fetch_models(&self.settings, self.bus.clone());
                    }
                }
                if let Some(m) = pick_model {
                    use ferrocast_core::models::AiProvider as P;
                    match self.settings.ai_provider {
                        P::OpenAiCompatible => self.settings.ai_model = m,
                        P::ChatGpt => self.settings.chatgpt_model = m,
                        _ => self.settings.anthropic_model = m,
                    }
                    self.save_settings();
                    if self.settings.ai_provider == P::ClaudeCode {
                        if let Some(c) = self.claude.take() {
                            c.kill();
                        }
                        self.warm_claude();
                    }
                }
                if let Some(u) = open_url {
                    let _ = std::process::Command::new("/usr/bin/open").arg(&u).status();
                }
                if grant {
                    windows::accessibility_trusted(true);
                }
                if open_privacy {
                    let _ = std::process::Command::new("/usr/bin/open")
                        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                        .status();
                }
                if clear_hotkey {
                    if let View::Hotkey { id, .. } = self.view().clone() {
                        self.settings.command_mut(&id).hotkey = None;
                        self.save_settings();
                        self.apply_hotkeys();
                        self.toast("Hotkey removed");
                        self.pop();
                    }
                }
            }
        }
    }

    fn draw_settings(&mut self, ui: &mut Ui, pal: &Palette) {
        const TABS: [(&str, &str); 6] =
            [("General", "⌘"), ("Clipboard", "⎘"), ("Snippets", "{ }"), ("AI", "AI"), ("Scripts", "›_"), ("Advanced", "···")];
        let ctx = ui.ctx().clone();
        let tab_id = Id::new("settings_tab");
        let mut tab: usize = ctx.data(|d| d.get_temp(tab_id)).unwrap_or(0);
        let mut s = self.settings.clone();
        let trusted = windows::accessibility_trusted(false);
        let (mut rec, mut req_acc, mut open_data, mut add_dir, mut save_key, mut test_ai) = (false, false, false, false, false, false);
        let (mut chatgpt_in, mut chatgpt_out, mut chatgpt_device) = (false, false, false);
        let mut fetch_models = false;
        let models_key = services::models_key(&self.settings);
        let models_state = self.ai_models.get(&models_key).cloned();
        let mut remove_dir = None;
        let mut enable = None;
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut key_scratch = std::mem::take(&mut self.key_scratch);
        let has_key = ctx.data(|d| d.get_temp::<bool>(Id::new("ai_has_key")));
        let has_key = match has_key {
            Some(v) => v,
            None => {
                let v = services::ai_key_for(self.settings.ai_provider).is_some();
                ctx.data_mut(|d| d.insert_temp(Id::new("ai_has_key"), v));
                v
            }
        };

        let full = ui.max_rect();
        let side = Rect::from_min_max(full.min, Pos2::new(full.min.x + 168.0, full.max.y));
        let pane = Rect::from_min_max(Pos2::new(side.max.x + 18.0, full.min.y), full.max);

        // ---- sidebar
        ui.scope_builder(UiBuilder::new().max_rect(side), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (i, (name, glyph)) in TABS.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(side.width(), 34.0), Sense::click());
                let on = i == tab;
                let p = ui.painter();
                if on {
                    p.rect_filled(r, 8.0, pal.sel);
                } else if resp.hovered() {
                    p.rect_filled(r, 8.0, pal.field);
                }
                let gr = Rect::from_center_size(Pos2::new(r.min.x + 20.0, r.center().y), Vec2::splat(20.0));
                p.rect_filled(gr, 5.0, if on { pal.accent } else { pal.field });
                p.text(gr.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(10.5), if on { Color32::WHITE } else { pal.sub });
                p.text(
                    Pos2::new(r.min.x + 38.0, r.center().y),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(13.5),
                    if on { pal.text } else { pal.sub },
                );
                if resp.clicked() {
                    tab = i;
                }
            }
        });
        ctx.data_mut(|d| d.insert_temp(tab_id, tab));
        ui.painter().vline(side.max.x + 9.0, pane.y_range(), Stroke::new(1.0, pal.sep));

        // ---- pane
        let acc = pal.accent;
        ui.scope_builder(UiBuilder::new().max_rect(pane), |ui| {
            egui::ScrollArea::vertical().id_salt("settings_pane").auto_shrink([false; 2]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(2.0);
                ui.label(RichText::new(TABS[tab].0).size(20.0).strong().color(pal.text));
                ui.add_space(14.0);
                match tab {
                    0 => {
                        group(ui, pal, |ui| {
                            row(ui, pal, "Launcher hotkey", "Press it from any app to open or close Ferrocast.", |ui| {
                                if small_btn(ui, pal, "Record…") {
                                    rec = true;
                                }
                                keycap(ui, pal, &services::hotkey_label(&s.launcher_hotkey));
                            });
                            row(ui, pal, "Appearance", "Follow the system, or force dark or light.", |ui| {
                                segmented(ui, pal, &mut s.theme, &[(Theme::System, "System"), (Theme::Dark, "Dark"), (Theme::Light, "Light")]);
                            });
                            row(ui, pal, "Compact mode", "Show only the search bar until you type.", |ui| toggle(ui, pal, &mut s.compact_mode));
                            row(ui, pal, "Launch at login", "Start Ferrocast when you sign in to your Mac.", |ui| toggle(ui, pal, &mut s.launch_at_login));
                            row(ui, pal, "Drop wheel", "While dragging files, tap a key to open a wheel of conversions or actions.", |ui| toggle(ui, pal, &mut s.drop_wheel));
                            row(ui, pal, "Convert wheel key", "Tap during a file drag for conversions.", |ui| {
                                let opts: Vec<(String, &str)> = ["Shift", "Control", "Option", "Command"].map(|v| (v.to_string(), v)).to_vec();
                                segmented(ui, pal, &mut s.wheel_convert_key, &opts);
                            });
                            row(ui, pal, "Tools wheel key", "Tap during a file drag for compress, crop, redact…", |ui| {
                                let opts: Vec<(String, &str)> = ["Shift", "Control", "Option", "Command"].map(|v| (v.to_string(), v)).to_vec();
                                segmented(ui, pal, &mut s.wheel_tools_key, &opts);
                            });
                            row(ui, pal, "Actions wheel key", "Tap during a file drag for copy, move, AirDrop…", |ui| {
                                let opts: Vec<(String, &str)> = ["Shift", "Control", "Option", "Command"].map(|v| (v.to_string(), v)).to_vec();
                                segmented(ui, pal, &mut s.wheel_actions_key, &opts);
                            });
                            row(ui, pal, "Menu bar icon", "A small icon to open Ferrocast with the mouse. Needs a restart.", |ui| toggle(ui, pal, &mut s.show_menu_bar_icon));
                        });
                        ui.add_space(14.0);
                        section(ui, pal, "Root Search");
                        group(ui, pal, |ui| {
                            row(ui, pal, "Sensitivity", "Low matches loosely; High needs every letter to start a word.", |ui| {
                                use ferrocast_core::fuzzy::Sensitivity as S;
                                segmented(ui, pal, &mut s.sensitivity, &[(S::Low, "Low"), (S::Medium, "Medium"), (S::High, "High")]);
                            });
                            row(ui, pal, "Include files", "Show Spotlight file results after apps and commands.", |ui| toggle(ui, pal, &mut s.file_search_in_root));
                            row(ui, pal, "Web search fallback", "Used by “Search the web for …”. {query} is replaced.", |ui| {
                                ui.add(TextEdit::singleline(&mut s.fallback_search_url).desired_width(280.0).font(FontId::proportional(12.5)));
                            });
                        });
                    }
                    1 => {
                        group(ui, pal, |ui| {
                            row(ui, pal, "Record history", "Keep what you copy so you can paste it later. Password managers are skipped.", |ui| toggle(ui, pal, &mut s.clipboard_enabled));
                            row(ui, pal, "Keep history for", "Older entries are removed automatically. Pinned entries stay.", |ui| {
                                egui::ComboBox::from_id_salt("retention").selected_text(s.clipboard_retention.label()).width(120.0).show_ui(ui, |ui| {
                                    for r in Retention::ALL {
                                        ui.selectable_value(&mut s.clipboard_retention, r, r.label());
                                    }
                                });
                            });
                        });
                        ui.add_space(10.0);
                        note(ui, pal, "History lives in clipboard.json inside the data folder and never leaves this Mac. Backups exclude it.");
                    }
                    2 => {
                        group(ui, pal, |ui| {
                            row(ui, pal, "Expand while typing", "Type a keyword such as !sig anywhere and it turns into the snippet.", |ui| toggle(ui, pal, &mut s.snippet_expansion));
                        });
                        ui.add_space(10.0);
                        note(ui, pal, if trusted { "Expansion is ready: Accessibility is granted." } else { "Expansion needs Accessibility. Grant it under Advanced." });
                    }
                    3 => {
                        use ferrocast_core::models::AiProvider as P;
                        let prov = s.ai_provider;
                        group(ui, pal, |ui| {
                            row_stacked(ui, pal, "Provider", match prov {
                                P::OpenAiCompatible => "Any server speaking the OpenAI API: Ollama on this Mac, LM Studio, OpenAI, OpenRouter…",
                                P::Anthropic => "Claude through Anthropic's API, billed to your API key. A ChatGPT or Claude subscription does not include API access.",
                                P::ClaudeCode => "Runs the official Claude Code command-line tool, signed in with your own Claude subscription. Slower to start (a few seconds) but no API key.",
                                P::ChatGpt => "Signs in with your ChatGPT account the way OpenAI's Codex CLI does and uses your ChatGPT plan. No API key.",
                            }, |ui| {
                                segmented(ui, pal, &mut s.ai_provider, &[(P::OpenAiCompatible, "OpenAI API"), (P::ChatGpt, "ChatGPT"), (P::Anthropic, "Anthropic"), (P::ClaudeCode, "Claude Code")]);
                            });
                        });
                        ui.add_space(14.0);
                        match prov {
                            P::OpenAiCompatible => {
                                section(ui, pal, "Server");
                                group(ui, pal, |ui| {
                                    row(ui, pal, "Preset", "Fills the server address below.", |ui| {
                                        let presets = [("Ollama", "http://localhost:11434/v1"), ("LM Studio", "http://localhost:1234/v1"), ("OpenAI", "https://api.openai.com/v1")];
                                        for &(name, url) in presets.iter().rev() {
                                            let on = s.ai_base_url.trim().trim_end_matches('/') == url;
                                            if pill(ui, pal, name, on) {
                                                s.ai_base_url = url.to_string();
                                            }
                                        }
                                    });
                                    row(ui, pal, "Server", "Base URL ending in /v1.", |ui| {
                                        ui.add(TextEdit::singleline(&mut s.ai_base_url).desired_width(280.0).font(FontId::proportional(12.5)).hint_text("http://localhost:11434/v1"));
                                    });
                                    row(ui, pal, "Model", "Listed by the server. Pick one, or Custom… to type a name.", |ui| {
                                        if model_picker(ui, pal, "openai", &mut s.ai_model, ferrocast_core::ai::DEFAULT_MODEL, models_state.as_ref()) {
                                            fetch_models = true;
                                        }
                                    });
                                    row(ui, pal, "API key", if has_key { "A key is stored in the Keychain. Enter a new one to replace it, or save empty to remove it." } else { "Only needed for hosted providers. Stored in the macOS Keychain, never in a file." }, |ui| {
                                        if small_btn(ui, pal, "Save") {
                                            save_key = true;
                                        }
                                        ui.add(TextEdit::singleline(&mut key_scratch).password(true).desired_width(180.0).font(FontId::proportional(12.5)).hint_text(if has_key { "••••••••" } else { "sk-…" }));
                                    });
                                });
                            }
                            P::Anthropic => {
                                section(ui, pal, "Anthropic API");
                                group(ui, pal, |ui| {
                                    row(ui, pal, "Model", "Listed by Anthropic once a key is saved. Pick one, or Custom… to type an id.", |ui| {
                                        if model_picker(ui, pal, "anthropic", &mut s.anthropic_model, ferrocast_core::ai::DEFAULT_ANTHROPIC_MODEL, models_state.as_ref()) {
                                            fetch_models = true;
                                        }
                                    });
                                    row(ui, pal, "API key", if has_key { "A key is stored in the Keychain. Enter a new one to replace it, or save empty to remove it." } else { "From console.anthropic.com. Stored in the macOS Keychain, never in a file." }, |ui| {
                                        if small_btn(ui, pal, "Save") {
                                            save_key = true;
                                        }
                                        ui.add(TextEdit::singleline(&mut key_scratch).password(true).desired_width(180.0).font(FontId::proportional(12.5)).hint_text(if has_key { "••••••••" } else { "sk-ant-…" }));
                                    });
                                });
                            }
                            P::ChatGpt => {
                                section(ui, pal, "ChatGPT account");
                                let signed = ctx.data(|d| d.get_temp::<bool>(Id::new("chatgpt_signed")));
                                let signed = match signed {
                                    Some(v) => v,
                                    None => {
                                        let v = services::chatgpt_credentials().is_some();
                                        ctx.data_mut(|d| d.insert_temp(Id::new("chatgpt_signed"), v));
                                        v
                                    }
                                };
                                group(ui, pal, |ui| {
                                    row(ui, pal, "Account", if signed { "Signed in. The session is stored in the macOS Keychain and refreshed automatically." } else { "Sign in opens your browser and listens on port 1455 for the redirect. If that port is taken (Codex, Pi), it switches to a one-time code you type on openai.com." }, |ui| {
                                        if signed {
                                            if small_btn(ui, pal, "Sign out") {
                                                chatgpt_out = true;
                                            }
                                            ui.label(RichText::new("Signed in ✓").size(13.0).color(Color32::from_rgb(89, 212, 153)));
                                        } else {
                                            if small_btn(ui, pal, "Use a code instead") {
                                                chatgpt_device = true;
                                            }
                                            if accent_btn(ui, pal, "Sign in with ChatGPT…") {
                                                chatgpt_in = true;
                                            }
                                        }
                                    });
                                    row(ui, pal, "Model", "Models the ChatGPT backend serves. Only the ones your plan includes will answer.", |ui| {
                                        if model_picker(ui, pal, "chatgpt", &mut s.chatgpt_model, ferrocast_core::chatgpt::DEFAULT_MODEL, models_state.as_ref()) {
                                            fetch_models = true;
                                        }
                                    });
                                });
                                ui.add_space(10.0);
                                note(ui, pal, "Plain talk: this uses the same sign-in and backend as OpenAI's Codex tool, presenting itself as that client. OpenAI has not blessed third-party apps doing so. If that is a problem for you, use the OpenAI API with a key instead.");
                            }
                            P::ClaudeCode => {
                                section(ui, pal, "Claude Code");
                                let path = services::claude_code_path();
                                group(ui, pal, |ui| {
                                    row(ui, pal, "Command-line tool", &path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not found. Install from claude.ai/code, then run `claude` once in Terminal to sign in.".into()), |ui| {
                                        ui.label(RichText::new(if path.is_some() { "Installed ✓" } else { "Missing" }).size(13.0).color(if path.is_some() { Color32::from_rgb(89, 212, 153) } else { pal.accent }));
                                    });
                                    row(ui, pal, "Model", "Claude Code's default, one of its aliases, or Custom… for a full model id.", |ui| {
                                        if model_picker(ui, pal, "claudecode", &mut s.anthropic_model, "Claude Code default", models_state.as_ref()) {
                                            fetch_models = true;
                                        }
                                    });
                                });
                                ui.add_space(10.0);
                                note(ui, pal, "Ferrocast runs `claude -p` with its tools switched off and your question on stdin, exactly as you would in Terminal. Whether that use fits your Claude plan is between you and Anthropic.");
                            }
                        }
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if accent_btn(ui, pal, "Test connection") {
                                test_ai = true;
                            }
                            ui.add_space(10.0);
                            ui.label(RichText::new(match prov { P::ClaudeCode => "Checks that the tool runs.", P::ChatGpt => "Checks the session and refreshes it if needed.", _ => "Asks the server for its model list." }).size(12.0).color(pal.sub));
                        });
                        ui.add_space(10.0);
                        note(ui, pal, "Ask AI streams answers and keeps the conversation while the window is open. With Ollama or Claude Code nothing goes through Ferrocast's servers: there are none.");
                    }
                    4 => {
                        group(ui, pal, |ui| {
                            row(ui, pal, "Default folder", "Scripts with Raycast metadata here become commands.", |ui| {
                                ui.label(RichText::new(self.store.scripts_dir().display().to_string()).size(12.0).color(pal.sub));
                            });
                            for (i, d) in s.script_directories.clone().iter().enumerate() {
                                row(ui, pal, d, "Extra directory.", |ui| {
                                    if small_btn(ui, pal, "Remove") {
                                        remove_dir = Some(i);
                                    }
                                });
                            }
                            row(ui, pal, "Add directory", "Use ~ for your home folder.", |ui| {
                                if small_btn(ui, pal, "Add") {
                                    add_dir = true;
                                }
                                ui.add(TextEdit::singleline(&mut scratch).desired_width(220.0).font(FontId::proportional(12.5)).hint_text("~/scripts"));
                            });
                        });
                        ui.add_space(10.0);
                        note(ui, pal, "Supported modes: fullOutput, compact, silent and inline. Scripts marked needsConfirmation ask before running.");
                    }
                    _ => {
                        section(ui, pal, "Permissions");
                        group(ui, pal, |ui| {
                            row(ui, pal, "Accessibility", if trusted { "Granted. Window management, pasting and snippet expansion work." } else { "Needed for window management, pasting and snippet expansion." }, |ui| {
                                if trusted {
                                    ui.label(RichText::new("Granted ✓").size(13.0).color(Color32::from_rgb(89, 212, 153)));
                                } else if accent_btn(ui, pal, "Grant…") {
                                    req_acc = true;
                                }
                            });
                        });
                        ui.add_space(14.0);
                        section(ui, pal, "Units");
                        group(ui, pal, |ui| {
                            row(ui, pal, "1 rem", "Used by the calculator for rem and em conversions.", |ui| {
                                ui.add(egui::DragValue::new(&mut s.rem_px).range(8.0..=64.0).suffix(" px"));
                            });
                            row(ui, pal, "Window gap", "Space left between windows and screen edges by window commands.", |ui| {
                                ui.add(egui::DragValue::new(&mut s.window_gap).range(0.0..=64.0).suffix(" pt"));
                            });
                        });
                        ui.add_space(14.0);
                        section(ui, pal, "Data");
                        group(ui, pal, |ui| {
                            row(ui, pal, "Data folder", &self.store.root.display().to_string(), |ui| {
                                if small_btn(ui, pal, "Open") {
                                    open_data = true;
                                }
                            });
                        });
                        let disabled: Vec<String> = s.commands.iter().filter(|(_, c)| c.disabled).map(|(k, _)| k.clone()).collect();
                        if !disabled.is_empty() {
                            ui.add_space(14.0);
                            section(ui, pal, "Disabled commands");
                            group(ui, pal, |ui| {
                                for d in disabled {
                                    row(ui, pal, &d, "Hidden from Root Search.", |ui| {
                                        if small_btn(ui, pal, "Enable") {
                                            enable = Some(d.clone());
                                        }
                                    });
                                }
                            });
                        }
                    }
                }
                ui.add_space(16.0);
            });
        });
        let _ = acc;

        if add_dir && !scratch.trim().is_empty() {
            s.script_directories.push(scratch.trim().to_string());
            scratch.clear();
        }
        self.scratch = scratch;
        if save_key {
            let ok = services::set_ai_key_for(s.ai_provider, &key_scratch);
            let now_has = !key_scratch.trim().is_empty();
            key_scratch.clear();
            ctx.data_mut(|d| d.insert_temp(Id::new("ai_has_key"), now_has));
            self.toast(if !ok {
                "Could not update Keychain"
            } else if now_has {
                "AI key saved to Keychain"
            } else {
                "AI key removed"
            });
        }
        self.key_scratch = key_scratch;
        let need_models = tab == 3 && !self.ai_models.contains_key(&models_key);
        if fetch_models || need_models {
            self.ai_models.insert(models_key.clone(), None);
            services::fetch_models(&s, self.bus.clone());
        }
        if chatgpt_in {
            ctx.data_mut(|d| d.remove::<bool>(Id::new("chatgpt_signed")));
            services::chatgpt_sign_in(self.bus.clone());
        }
        if chatgpt_device {
            ctx.data_mut(|d| d.remove::<bool>(Id::new("chatgpt_signed")));
            services::chatgpt_sign_in_device(self.bus.clone());
        }
        if chatgpt_out {
            let ok = services::chatgpt_sign_out();
            ctx.data_mut(|d| d.insert_temp(Id::new("chatgpt_signed"), false));
            self.toast(if ok { "Signed out of ChatGPT" } else { "Could not remove the session from the Keychain" });
        }
        if test_ai {
            self.toast("Testing connection…");
            services::test_ai_connection(&s, self.bus.clone());
        }
        if let Some(i) = remove_dir {
            s.script_directories.remove(i);
        }
        if let Some(id) = enable {
            s.command_mut(&id).disabled = false;
        }
        if req_acc {
            windows::accessibility_trusted(true);
        }
        if open_data {
            let _ = std::process::Command::new("/usr/bin/open").arg(&self.store.root).status();
        }
        if rec {
            self.push(View::Hotkey { id: "__launcher".into(), name: "Ferrocast launcher".into(), err: String::new() });
            return;
        }
        if s.ai_provider != self.settings.ai_provider {
            ctx.data_mut(|d| d.remove::<bool>(Id::new("ai_has_key")));
        }
        if s != self.settings {
            if s.launch_at_login != self.settings.launch_at_login {
                match services::set_launch_at_login(s.launch_at_login) {
                    Ok(()) => self.toast(if s.launch_at_login { "Will launch at login" } else { "Login item removed" }),
                    Err(e) => {
                        s.launch_at_login = self.settings.launch_at_login;
                        self.toast(format!("Could not change login item: {e}"));
                    }
                }
            }
            let theme_changed = s.theme != self.settings.theme;
            let dirs_changed = s.script_directories != self.settings.script_directories;
            self.settings = s;
            self.save_settings();
            if theme_changed {
                self.set_theme();
            }
            if dirs_changed {
                self.reload_scripts();
            }
        }
    }
}

// ---- settings widgets: a tone-stepped card of rows, each with a title, a description and its control.

fn section(ui: &mut Ui, pal: &Palette, title: &str) {
    ui.label(RichText::new(title).size(12.5).color(pal.sub));
    ui.add_space(6.0);
}

fn note(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(pal.sub));
}

fn group(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui)) {
    let fill = pal.field;
    egui::Frame::new().fill(fill).corner_radius(10.0).inner_margin(egui::Margin::symmetric(0, 2)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        add(ui);
    });
}

fn row(ui: &mut Ui, pal: &Palette, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    let text_w = (w * 0.46).min(w - 340.0).max(150.0);
    let inner = egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 11)).show(ui, |ui| {
        ui.set_width(w - 28.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(text_w);
                ui.set_max_width(text_w);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).size(13.5).color(pal.text));
                ui.label(RichText::new(desc).size(11.5).color(pal.sub));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                control(ui);
            });
        });
    });
    // hairline between rows, inset like the original launcher's settings
    let r = inner.response.rect;
    ui.painter().hline((r.min.x + 14.0)..=(r.max.x - 14.0), r.max.y, Stroke::new(1.0, pal.sep));
}

/// Model dropdown: Default, every model the provider lists, and Custom… (a free text field).
/// Returns true when the list should be (re)fetched.
fn model_picker(
    ui: &mut Ui,
    pal: &Palette,
    id: &str,
    value: &mut String,
    default_label: &str,
    state: Option<&Option<Result<Vec<String>, String>>>,
) -> bool {
    let custom_id = Id::new(("model_custom", id));
    let mut custom: bool = ui.ctx().data(|d| d.get_temp(custom_id)).unwrap_or(false);
    let models: &[String] = match state {
        Some(Some(Ok(m))) => m,
        _ => &[],
    };
    // a saved value the server does not list is shown through the custom field
    if !custom && !value.is_empty() && !models.is_empty() && !models.iter().any(|m| m == value) {
        custom = true;
    }
    let mut refresh = false;
    if small_btn(ui, pal, "↻") {
        refresh = true;
    }
    if custom {
        ui.add(TextEdit::singleline(value).desired_width(170.0).font(FontId::proportional(12.5)).hint_text(default_label));
    }
    let shown = match state {
        None | Some(None) => "Loading…".to_string(),
        _ => {
            if custom {
                "Custom".to_string()
            } else if value.is_empty() {
                format!("Default ({default_label})")
            } else {
                value.clone()
            }
        }
    };
    egui::ComboBox::from_id_salt(("model_combo", id))
        .selected_text(RichText::new(shown).size(12.5))
        .width(if custom { 110.0 } else { 220.0 })
        .show_ui(ui, |ui| {
            ui.set_min_width(240.0);
            if ui.selectable_label(!custom && value.is_empty(), format!("Default ({default_label})")).clicked() {
                value.clear();
                custom = false;
            }
            for m in models {
                if ui.selectable_label(!custom && value == m, m).clicked() {
                    *value = m.clone();
                    custom = false;
                }
            }
            if let Some(Some(Err(e))) = state {
                ui.label(RichText::new(format!("List unavailable: {e}")).size(11.5).color(pal.sub));
            }
            if ui.selectable_label(custom, "Custom…").clicked() {
                custom = true;
            }
        });
    ui.ctx().data_mut(|d| d.insert_temp(custom_id, custom));
    refresh
}

/// Like `row`, but the control sits on its own line under the text, for wide controls.
fn row_stacked(ui: &mut Ui, pal: &Palette, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    let inner = egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 11)).show(ui, |ui| {
        ui.set_width(w - 28.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(RichText::new(title).size(13.5).color(pal.text));
        ui.label(RichText::new(desc).size(11.5).color(pal.sub));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            control(ui);
        });
    });
    let r = inner.response.rect;
    ui.painter().hline((r.min.x + 14.0)..=(r.max.x - 14.0), r.max.y, Stroke::new(1.0, pal.sep));
}

/// A pill switch that slides in 120 ms.
fn toggle(ui: &mut Ui, pal: &Palette, on: &mut bool) {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(40.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.12);
    let off = if pal.text.r() > 128 { Color32::from_gray(74) } else { Color32::from_gray(200) };
    let bg = lerp_color(off, pal.accent, t);
    let p = ui.painter();
    p.rect_filled(r, 12.0, bg);
    let kx = r.min.x + 12.0 + (r.width() - 24.0) * t;
    p.circle_filled(Pos2::new(kx, r.center().y), 9.5, Color32::WHITE);
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

fn small_btn(ui: &mut Ui, pal: &Palette, label: &str) -> bool {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.5), pal.text);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 20.0, 26.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 7.0, if resp.hovered() { pal.sel } else { pal.field.gamma_multiply(1.6) });
    p.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, pal.text);
    resp.clicked()
}

fn accent_btn(ui: &mut Ui, pal: &Palette, label: &str) -> bool {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.5), Color32::WHITE);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 24.0, 28.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 7.0, if resp.hovered() { pal.accent.gamma_multiply(1.15) } else { pal.accent });
    p.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
    resp.clicked()
}

fn pill(ui: &mut Ui, pal: &Palette, label: &str, on: bool) -> bool {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.0), pal.text);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 18.0, 24.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(
        r,
        12.0,
        if on {
            pal.accent
        } else if resp.hovered() {
            pal.sel
        } else {
            pal.field.gamma_multiply(1.6)
        },
    );
    p.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, if on { Color32::WHITE } else { pal.text });
    resp.clicked()
}

/// Segmented control: one filled segment slides under the chosen option.
fn segmented<T: PartialEq + Clone>(ui: &mut Ui, pal: &Palette, value: &mut T, options: &[(T, &str)]) {
    let seg_w = options
        .iter()
        .map(|(_, l)| ui.painter().layout_no_wrap(l.to_string(), FontId::proportional(12.0), pal.text).size().x + 22.0)
        .fold(56.0f32, f32::max);
    let (r, base) = ui.allocate_exact_size(Vec2::new(seg_w * options.len() as f32 + 4.0, 26.0), Sense::hover());
    let id = base.id;
    let p = ui.painter().clone();
    p.rect_filled(r, 8.0, pal.field.gamma_multiply(1.3));
    let idx = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let x = ui.ctx().animate_value_with_time(id.with("seg"), idx as f32, 0.14);
    let sr = Rect::from_min_size(Pos2::new(r.min.x + 2.0 + x * seg_w, r.min.y + 2.0), Vec2::new(seg_w, 22.0));
    p.rect_filled(sr, 6.0, pal.sel.gamma_multiply(1.8));
    for (i, (v, label)) in options.iter().enumerate() {
        let cell = Rect::from_min_size(Pos2::new(r.min.x + 2.0 + i as f32 * seg_w, r.min.y), Vec2::new(seg_w, 26.0));
        let resp = ui.interact(cell, id.with(("cell", i)), Sense::click());
        if resp.clicked() {
            *value = v.clone();
        }
        p.text(cell.center(), Align2::CENTER_CENTER, *label, FontId::proportional(12.0), if i == idx { pal.text } else { pal.sub });
    }
}

fn keycap(ui: &mut Ui, pal: &Palette, label: &str) {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.0), pal.text);
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 16.0, 24.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 6.0, pal.field.gamma_multiply(1.6));
    p.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, pal.text);
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
