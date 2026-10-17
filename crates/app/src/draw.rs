//! eframe::App implementation: event loop, keyboard handling and drawing.

use super::*;
use egui::TextureOptions;
use egui::{Event, Frame as EFrame, RichText, TextEdit};
use global_hotkey::hotkey::HotKey;
use std::str::FromStr;

enum Detail {
    Text(String, String),
    Image(String, String),
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
                let meta = format!(
                    "{}Copied {} time{}",
                    e.source_app.as_ref().map(|a| format!("{a} · ")).unwrap_or_default(),
                    e.times_copied,
                    if e.times_copied == 1 { "" } else { "s" }
                );
                match e.kind {
                    ClipKind::Image => Some(Detail::Image(e.content, format!("{meta} · {}×{}", e.width, e.height))),
                    _ => Some(Detail::Text(e.content.chars().take(6000).collect(), meta)),
                }
            }
            Kind::Snippet(id) => {
                let s = self.snippets.iter().find(|s| &s.id == id)?;
                let kw = if s.keyword.is_empty() { "No keyword".to_string() } else { format!("Keyword {}", s.keyword) };
                Some(Detail::Text(s.text.clone(), kw))
            }
            Kind::Note(id) => {
                self.notes.iter().find(|n| &n.id == id).map(|n| Detail::Text(n.text.chars().take(6000).collect(), String::new()))
            }
            Kind::Quicklink { id, .. } => self.quicklinks.iter().find(|q| &q.id == id).map(|q| Detail::Text(q.link.clone(), "Link".into())),
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
        let keys = self.read_keys(&ctx);
        let shape = self.view().shape();
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

        if let Some(i) = clicked.or(double) {
            self.selected = i;
        }
        if let Some(i) = double {
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

    fn draw_detail(&mut self, ui: &mut Ui, rect: Rect, d: Detail, pal: &Palette) {
        let ctx = ui.ctx().clone();
        match d {
            Detail::Text(t, meta) => {
                let body = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.max.y - 22.0));
                ui.scope_builder(UiBuilder::new().max_rect(body), |ui| {
                    egui::ScrollArea::vertical().id_salt("detail").auto_shrink([false; 2]).show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(t).size(13.0).color(pal.text)).wrap());
                    });
                });
                ui.painter().text(Pos2::new(rect.min.x, rect.max.y - 8.0), Align2::LEFT_CENTER, meta, FontId::proportional(11.5), pal.sub);
            }
            Detail::Image(name, meta) => {
                let area = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.max.y - 22.0));
                let icon = Icon::ClipImage(name);
                if let Some(t) = self.icons.texture(&ctx, &icon) {
                    let r = ui::fit(area, t.size_vec2());
                    ui.painter().image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                ui.painter().text(Pos2::new(rect.min.x, rect.max.y - 8.0), Align2::LEFT_CENTER, meta, FontId::proportional(11.5), pal.sub);
            }
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
                            let r = ui.add(
                                TextEdit::singleline(prompt)
                                    .hint_text("Ask anything… (↵ to send, ⌘↵ copies the answer)")
                                    .font(FontId::proportional(16.0))
                                    .desired_width(f32::INFINITY),
                            );
                            if ff {
                                r.request_focus();
                            }
                            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false; 2]).show(ui, |ui| {
                                let turn = |ui: &mut Ui, q: &str, a: &str| {
                                    ui.label(RichText::new(q).size(12.5).strong().color(pal.accent));
                                    ui.add(egui::Label::new(RichText::new(a).size(14.0).color(pal.text)).selectable(true));
                                    ui.add_space(8.0);
                                };
                                for (q, a) in history.iter() {
                                    turn(ui, q, a);
                                }
                                if !asked.is_empty() {
                                    turn(ui, asked, if answer.is_empty() && *busy { "Thinking…" } else { answer });
                                }
                            });
                        }
                        View::Welcome => {
                            ui.label(RichText::new("Welcome to Ferrocast").size(20.0).strong().color(pal.text));
                            ui.label(
                                RichText::new(format!(
                                    "Open it anywhere with {launcher_hotkey}. Type to search apps, commands and files, or calculate."
                                ))
                                .color(pal.sub),
                            );
                            ui.add_space(6.0);
                            ui.label(RichText::new("Permissions").size(13.0).strong().color(pal.accent));
                            let (mark, txt) = if trusted {
                                ("✓", "Accessibility granted — window management, pasting and snippet expansion work.")
                            } else {
                                ("✗", "Accessibility not granted — needed for window management, pasting and snippet expansion.")
                            };
                            ui.label(RichText::new(format!("{mark}  {txt}")).color(if trusted { pal.text } else { pal.danger }));
                            if !trusted {
                                ui.horizontal(|ui| {
                                    if ui.button("Grant Accessibility…").clicked() {
                                        grant = true;
                                    }
                                    if ui.button("Open System Settings").clicked() {
                                        open_privacy = true;
                                    }
                                });
                                lbl(ui, "macOS asks you to add Ferrocast in Privacy & Security → Accessibility, then restart Ferrocast.");
                            }
                            ui.add_space(6.0);
                            ui.label(RichText::new("Tips").size(13.0).strong().color(pal.accent));
                            for t in [
                                "⌘K opens the actions of the selected item",
                                "Set aliases and hotkeys per command from ⌘K",
                                "Create snippets with keywords that expand while you type",
                                "Search “Ask AI” after configuring a server in Settings",
                                "Everything is stored locally; clipboard history never leaves this Mac",
                            ] {
                                ui.label(RichText::new(format!("•  {t}")).color(pal.text));
                            }
                            lbl(ui, "↵ grants Accessibility · esc closes · reopen from “Welcome & Permissions”");
                        }
                        View::Shortcuts => {
                            let rows = [
                                ("⌥ Space", "Open / close Ferrocast (configurable)"),
                                ("↑ ↓", "Move selection"),
                                ("↵", "Run the primary action"),
                                ("⌘ ↵", "Run the secondary action"),
                                ("⌘ K", "Open the action panel"),
                                ("⌘ ,", "Open settings"),
                                ("⌘ N", "New snippet / quicklink / note in those lists"),
                                ("⇧ ⌘ F", "Add or remove favorite"),
                                ("⇧ ⌘ ,", "Set hotkey for a command"),
                                ("esc", "Clear search, go back, or close"),
                                ("⌫ (empty search)", "Go back"),
                                ("⌃ X", "Delete the selected entry"),
                                ("⌘ Y", "Quick Look the selected file"),
                            ];
                            egui::Grid::new("shortcuts").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                                for (k, d) in rows {
                                    ui.label(RichText::new(k).strong().color(pal.accent));
                                    ui.label(RichText::new(d).color(pal.text));
                                    ui.end_row();
                                }
                            });
                        }
                        View::About => {
                            ui.label(RichText::new("Ferrocast").size(22.0).strong().color(pal.text));
                            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(pal.sub));
                            ui.add_space(6.0);
                            ui.label(RichText::new("A fast keyboard launcher for macOS, written in Rust.").color(pal.text));
                            ui.label(RichText::new("Not affiliated with Raycast. MIT licensed.").color(pal.sub));
                            ui.add_space(6.0);
                            ui.label(RichText::new(format!("Data folder: {data_dir}")).size(12.0).color(pal.sub));
                            ui.label(RichText::new("Currency rates: European Central Bank via frankfurter.dev").size(12.0).color(pal.sub));
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
        let mut s = self.settings.clone();
        let trusted = windows::accessibility_trusted(false);
        let mut rec = false;
        let mut req_acc = false;
        let mut open_data = false;
        let mut remove_dir = None;
        let mut add_dir = false;
        let mut enable = None;
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut key_scratch = std::mem::take(&mut self.key_scratch);
        let mut save_key = false;
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 9.0;
            let h = |ui: &mut Ui, t: &str| {
                ui.add_space(6.0);
                ui.label(RichText::new(t).size(13.0).strong().color(pal.accent));
            };
            h(ui, "General");
            ui.horizontal(|ui| {
                ui.label("Launcher hotkey");
                ui.label(RichText::new(services::hotkey_label(&s.launcher_hotkey)).strong());
                if ui.button("Record…").clicked() {
                    rec = true;
                }
            });
            egui::ComboBox::from_label("Theme").selected_text(format!("{:?}", s.theme)).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.theme, Theme::System, "System");
                ui.selectable_value(&mut s.theme, Theme::Dark, "Dark");
                ui.selectable_value(&mut s.theme, Theme::Light, "Light");
            });
            ui.checkbox(&mut s.compact_mode, "Compact mode (show only the search bar when empty)");
            egui::ComboBox::from_label("Root search sensitivity").selected_text(format!("{:?}", s.sensitivity)).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.sensitivity, ferrocast_core::fuzzy::Sensitivity::Low, "Low");
                ui.selectable_value(&mut s.sensitivity, ferrocast_core::fuzzy::Sensitivity::Medium, "Medium");
                ui.selectable_value(&mut s.sensitivity, ferrocast_core::fuzzy::Sensitivity::High, "High");
            });
            ui.checkbox(&mut s.file_search_in_root, "Include files in Root Search");
            ui.checkbox(&mut s.show_menu_bar_icon, "Show menu bar icon (restart required)");
            ui.checkbox(&mut s.launch_at_login, "Launch Ferrocast at login");
            h(ui, "Clipboard History");
            ui.checkbox(&mut s.clipboard_enabled, "Record clipboard history");
            egui::ComboBox::from_label("Keep history for").selected_text(s.clipboard_retention.label()).show_ui(ui, |ui| {
                for r in Retention::ALL {
                    ui.selectable_value(&mut s.clipboard_retention, r, r.label());
                }
            });
            h(ui, "Snippets");
            ui.checkbox(&mut s.snippet_expansion, "Expand snippet keywords automatically while typing");
            h(ui, "AI (OpenAI-compatible server)");
            ui.label(RichText::new("Default: local Ollama. Works with LM Studio, OpenAI, OpenRouter…").size(12.0).color(pal.sub));
            ui.add(TextEdit::singleline(&mut s.ai_base_url).desired_width(f32::INFINITY).hint_text("http://localhost:11434/v1"));
            ui.add(TextEdit::singleline(&mut s.ai_model).desired_width(300.0).hint_text("model, e.g. llama3.2"));
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut key_scratch).password(true).desired_width(300.0).hint_text("API key (saved to Keychain)"));
                if ui.button("Save key").clicked() {
                    save_key = true;
                }
            });
            h(ui, "Calculator & Windows");
            ui.horizontal(|ui| {
                ui.label("1 rem/em =");
                ui.add(egui::DragValue::new(&mut s.rem_px).range(8.0..=64.0).suffix(" px"));
                ui.label("   Window gap");
                ui.add(egui::DragValue::new(&mut s.window_gap).range(0.0..=64.0).suffix(" pt"));
            });
            ui.label(RichText::new("Web search fallback URL").size(12.0).color(pal.sub));
            ui.add(TextEdit::singleline(&mut s.fallback_search_url).desired_width(f32::INFINITY));
            h(ui, "Script Command directories");
            ui.label(RichText::new(format!("Default: {}", self.store.scripts_dir().display())).size(12.0).color(pal.sub));
            for (i, d) in s.script_directories.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(d);
                    if ui.small_button("Remove").clicked() {
                        remove_dir = Some(i);
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut scratch).desired_width(380.0).hint_text("~/scripts"));
                if ui.button("Add directory").clicked() {
                    add_dir = true;
                }
            });
            h(ui, "Permissions");
            ui.horizontal(|ui| {
                ui.label(if trusted {
                    "Accessibility: granted ✓"
                } else {
                    "Accessibility: not granted (needed for window management, pasting and snippet expansion)"
                });
                if !trusted && ui.button("Request").clicked() {
                    req_acc = true;
                }
            });
            if ui.button("Open data folder").clicked() {
                open_data = true;
            }
            let disabled: Vec<String> = s.commands.iter().filter(|(_, c)| c.disabled).map(|(k, _)| k.clone()).collect();
            if !disabled.is_empty() {
                h(ui, "Disabled commands");
                for d in disabled {
                    ui.horizontal(|ui| {
                        ui.label(&d);
                        if ui.small_button("Enable").clicked() {
                            enable = Some(d.clone());
                        }
                    });
                }
            }
            ui.add_space(10.0);
        });
        if add_dir && !scratch.trim().is_empty() {
            s.script_directories.push(scratch.trim().to_string());
            scratch.clear();
        }
        self.scratch = scratch;
        if save_key {
            let ok = services::set_ai_key(&key_scratch);
            key_scratch.clear();
            self.toast(if ok { "AI key saved to Keychain" } else { "Could not update Keychain" });
        }
        self.key_scratch = key_scratch;
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
