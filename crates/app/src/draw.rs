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

/// Per-open animation state of the drop wheel, kept in the egui context (not in `App`).
/// Times are egui seconds. It starts fresh when the wheel was not drawn for 0.3 s or is removed.
#[derive(Clone)]
pub(crate) struct WheelAnim {
    /// Reduce Motion, read once when the wheel opened: every duration is 0.
    reduce: bool,
    /// Dev aid `FERROCAST_WHEEL_ANIM_T=<ms>`: every animation is frozen at this elapsed time.
    frozen: Option<f32>,
    last: f64,
    open_t0: f64,
    kind: ferrocast_core::wheel::WheelKind,
    from_rgb: [f32; 3],
    to_rgb: [f32; 3],
    kind_t0: f64,
    /// per segment: (from, to, start time) of the highlight
    hl: Vec<(f32, f32, f64)>,
    hub_cur: String,
    hub_prev: String,
    hub_t0: f64,
    mouse: Option<Pos2>,
}

impl WheelAnim {
    fn open(now: f64, kind: ferrocast_core::wheel::WheelKind, rgb: [f32; 3], hub: &str, reduce: bool) -> Self {
        let frozen = std::env::var("FERROCAST_WHEEL_ANIM_T").ok().and_then(|v| v.parse::<f32>().ok()).filter(|_| !reduce);
        WheelAnim {
            reduce,
            frozen,
            last: now,
            open_t0: now,
            kind,
            from_rgb: rgb,
            to_rgb: rgb,
            kind_t0: f64::NEG_INFINITY,
            hl: Vec::new(),
            hub_cur: hub.to_string(),
            hub_prev: String::new(),
            hub_t0: f64::NEG_INFINITY,
            mouse: None,
        }
    }
}

/// Reduce Motion for a wheel that is opening: a test hook in the context, then
/// `FERROCAST_REDUCE_MOTION=1|0`, then the system setting.
fn wheel_reduce_motion(ctx: &Context) -> bool {
    if let Some(v) = ctx.data(|d| d.get_temp::<bool>(Id::new("wheel_reduce_motion"))) {
        return v;
    }
    match std::env::var("FERROCAST_REDUCE_MOTION").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => ferrocast_macos::system::reduce_motion(),
    }
}

impl App {
    /// Drop the toast after 2.5 s (the "Tab reset · Undo" toast lives as long as Undo works), and
    /// the undo state with it.
    fn expire_toasts(&mut self) {
        // the "Tab reset · Undo" toast stays as long as Undo works
        let life = if self.toast.as_ref().is_some_and(|(t, _)| t == RESET_TOAST) { UNDO_SECS * 1000 } else { 2500 };
        if self.toast.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_millis(life)) {
            if life > 2500 {
                self.settings_undo = None;
            }
            self.toast = None;
        }
        if self.settings_undo.as_ref().is_some_and(|(_, _, t)| t.elapsed() > Duration::from_secs(UNDO_SECS)) {
            self.settings_undo = None;
        }
    }
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
        self.effect_tick(Instant::now());
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
        self.expire_toasts();
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
        let now = ctx.input(|i| i.time);
        self.tick_notion_view(now);
        self.tick_obsidian_view(now);
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
        let visible = self.visible;
        let ctx = ui.ctx().clone();
        crate::perf::frame(visible, || ctx.repaint_causes().iter().map(|c| c.to_string()).collect(), || self.draw_if_visible(ui));
    }
}

impl App {
    /// While the launcher window is hidden nothing is laid out or painted: a focused search box
    /// would otherwise blink its caret (a repaint every ~0.5 s), pending icons would keep
    /// requesting frames, and every frame would flush an invisible GL surface.
    pub(crate) fn draw_if_visible(&mut self, ui: &mut Ui) {
        if self.visible {
            self.draw(ui);
            return;
        }
        // keep the fade-in of the next show() starting from transparent
        ui.ctx().animate_bool_with_time(Id::new("appear"), false, 0.0);
    }
}

/// Dev aid: `FERROCAST_SCREENSHOT=/tmp/x.png` saves the window contents after a few frames
/// and exits. `FERROCAST_QUERY` / `FERROCAST_VIEW` preselect what is drawn. Needs no TCC permission.
/// With `FERROCAST_VIEW=settings-tab-N`, `FERROCAST_SETTINGS_OPEN=<row title>` scrolls to and flashes that row.
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
        // FERROCAST_THEME=light|dark applies to every screenshot, not only the wheel
        match std::env::var("FERROCAST_THEME").as_deref() {
            Ok("light") => ctx.set_theme(egui::ThemePreference::Light),
            Ok("dark") => ctx.set_theme(egui::ThemePreference::Dark),
            _ => {}
        }
        match std::env::var("FERROCAST_VIEW").as_deref() {
            Ok("clipboard") => app.push(View::Clipboard),
            Ok("snippets") => app.push(View::Snippets),
            Ok("quicklinks") => app.push(View::Quicklinks),
            Ok("emoji") => app.push(View::Emoji),
            Ok("settings") => app.push(View::Settings),
            Ok("notes") => app.push(View::Notes),
            // design-review shots of Search Notion: `notion` reads the real Keychain (no token: the connect
            // state), `notion-list` shows a fixed list without any request
            Ok("notion") => app.run_builtin(Builtin::SearchNotion),
            // the Add to Notion form with a fixed database target, no Keychain read and no request
            Ok("notion-add") => app.push(View::NotionAdd {
                title: "Call Ana about the offer".into(),
                text: "Ask for the revised numbers.\n\n- budget\n- start date".into(),
                target: ferrocast_core::models::NotionTarget { id: "d1".into(), kind: "data_source".into(), title: "Tasks".into() },
            }),
            // the New Obsidian Note form with a fixed vault and text
            Ok("obsidian-new") => {
                app.settings.obsidian_vault = Some(std::path::PathBuf::from("/Users/me/Work Notes"));
                app.push(View::ObsidianNew {
                    title: "Call Ana about the offer".into(),
                    text: "Ask for the revised numbers.\n\n- budget\n- start date".into(),
                })
            }
            Ok("notion-list") => {
                app.run_builtin(Builtin::SearchNotion);
                let ago = |h: i64| (chrono::Utc::now() - chrono::Duration::hours(h)).to_rfc3339();
                let json = format!(
                    r#"{{"results":[
                    {{"object":"page","id":"a","url":"https://www.notion.so/a","last_edited_time":"{}","icon":{{"type":"emoji","emoji":"🚀"}},"properties":{{"title":{{"type":"title","title":[{{"plain_text":"Product roadmap Q3"}}]}}}}}},
                    {{"object":"data_source","id":"b","url":"https://www.notion.so/b","last_edited_time":"{}","icon":{{"type":"emoji","emoji":"✅"}},"title":[{{"plain_text":"Tasks"}}]}},
                    {{"object":"page","id":"c","url":"https://www.notion.so/c","last_edited_time":"{}","properties":{{"title":{{"type":"title","title":[{{"plain_text":"Meeting notes: design review"}}]}}}}}},
                    {{"object":"data_source","id":"d","url":"https://www.notion.so/d","last_edited_time":"{}","title":[{{"plain_text":"Reading list"}}]}},
                    {{"object":"page","id":"e","url":"https://www.notion.so/e","last_edited_time":"{}","icon":{{"type":"emoji","emoji":"🏠"}},"properties":{{"title":{{"type":"title","title":[{{"plain_text":"Home"}}]}}}}}}
                    ],"has_more":true,"next_cursor":"c"}}"#,
                    ago(0),
                    ago(2),
                    ago(30),
                    ago(24 * 9),
                    ago(24 * 400)
                );
                if let Ok(page) = ferrocast_core::notion::parse_search(&json) {
                    app.nv.items = page.items;
                    app.nv.has_more = true;
                    app.nv.next_cursor = page.next_cursor;
                    app.nv.connected = Some(true);
                    app.nv.asked = Some(String::new());
                }
            }
            // design-review shots of Search Obsidian: `obsidian` has no vault chosen (the choose-vault state),
            // `obsidian-list` builds a throwaway vault in the temp folder and fills the list and the preview
            // without any thread
            Ok("obsidian") => app.run_builtin(Builtin::SearchObsidian),
            Ok("obsidian-list") => {
                let root = std::env::temp_dir().join("ferrocast-shot-vault").join("Notes");
                let notes: [(&str, &str, u64); 5] = [
                    ("Project kickoff.md", "---\ntags: [work]\n---\n# Project kickoff\n\nAgenda for the first week, see [[Roadmap|the roadmap]].\n\n## Goals\n\n- Ship the preview pane\n- Write the docs\n  - English first\n- [x] Pick the vault\n- [ ] Index the notes\n\n> Notes are plain files. Nothing leaves this Mac.\n\n```rust\nlet entries = index_vault(&root);\n```\n\n---\n\nSee you Monday.\n", 600),
                    ("Daily/2026-10-22.md", "# Thursday\n\nStandup at 10.\n", 3 * 3600),
                    ("Recipes/Pasta al limón.md", "# Pasta al limón\n\n- 200 g spaghetti\n", 30 * 3600),
                    ("Roadmap.md", "# Roadmap\n\nQ3 plan.\n", 9 * 86_400),
                    ("Reading list.md", "# Reading list\n", 400 * 86_400),
                ];
                for (rel, body, ago) in notes {
                    let p = root.join(rel);
                    let _ = std::fs::create_dir_all(p.parent().unwrap());
                    let _ = std::fs::write(&p, body);
                    if let Ok(f) = std::fs::File::options().write(true).open(&p) {
                        let _ = f.set_modified(std::time::SystemTime::now() - Duration::from_secs(ago));
                    }
                }
                app.settings.obsidian_vault = Some(root.clone());
                app.obsidian.installed = Some(true);
                app.run_builtin(Builtin::SearchObsidian);
                app.ov.indexing = false;
                app.ov.cache = Some((root.clone(), Arc::new(ferrocast_core::obsidian::index_vault(&root))));
                app.ov.preview_for = Some("Project kickoff.md".into());
                app.ov.preview = Some(("Project kickoff.md".into(), crate::services::read_preview(&root.join("Project kickoff.md"))));
                app.invalidate_sections();
            }
            Ok("compress") => {
                let files: Vec<std::path::PathBuf> = std::env::var("FERROCAST_FILES")
                    .unwrap_or_default()
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .filter(|p| p.exists())
                    .collect();
                let orig_bytes = files.first().and_then(|f| std::fs::metadata(f).ok()).map(|m| m.len()).unwrap_or(0);
                // audio and video files open the media mode (by file type, or FERROCAST_COMPRESS=media)
                let family = files.first().map(|f| ferrocast_core::convert::family_of(f));
                let media_mode = std::env::var("FERROCAST_COMPRESS").as_deref() == Ok("media")
                    || matches!(family, Some(ferrocast_core::convert::Family::Audio | ferrocast_core::convert::Family::Video));
                if media_mode {
                    let info = app.tools.ffmpeg.zip(files.first()).and_then(|(ff, f)| crate::media_tools::media_info(ff, f));
                    let video = family == Some(ferrocast_core::convert::Family::Video);
                    let kb = std::env::var("FERROCAST_KB").ok().and_then(|v| v.parse().ok()).unwrap_or(2048);
                    app.push(View::CompressImage {
                        files,
                        orig_bytes,
                        size: None,
                        mode: std::env::var("FERROCAST_MODE").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
                        kb,
                        percent: 40,
                        preset: 1,
                        max_edge: None,
                        resize: false,
                        media: Some(MediaWin { video, loaded: true, info }),
                    });
                } else {
                    let size = files.first().and_then(|f| services::image_size(f));
                    app.push(View::CompressImage {
                        files,
                        orig_bytes,
                        size,
                        mode: 0,
                        kb: 500,
                        percent: 40,
                        preset: 1,
                        max_edge: None,
                        resize: false,
                        media: None,
                    });
                }
            }
            Ok("vspeed") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let Some(ff) = app.tools.ffmpeg {
                    let d = crate::media_tools::video_info(ff, &file).map(|(d, _)| d);
                    app.push(View::VideoSpeed { file, duration: d, factor: 2.0 });
                }
            }
            Ok("vcrop") | Ok("vredact") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let Some(ff) = app.tools.ffmpeg {
                    if let Some(dims) = crate::media_tools::video_dims(ff, &file) {
                        let duration = crate::media_tools::video_info(ff, &file).map(|(d, _)| d).unwrap_or(0.0);
                        let ref_t = ferrocast_core::videoplan::frame_time(duration, 2.0);
                        if let Some(img) = crate::media_tools::preview_frame(ff, &file, ref_t).and_then(|b| services::decode_png(&b)) {
                            let tex = ctx.load_texture("crop-view", img, egui::TextureOptions::LINEAR);
                            ctx.data_mut(|d| d.insert_temp(Id::new("crop-tex"), tex));
                            ctx.data_mut(|d| d.insert_temp(Id::new("ref-shown"), (file.clone(), ref_t)));
                        }
                        app.push(if std::env::var("FERROCAST_VIEW").as_deref() == Ok("vredact") {
                            let area = |x: f64, y: f64, w: f64, h: f64, start: f64, end: f64, blur: bool| crate::app::Area {
                                rect: ferrocast_core::croprect::Rect {
                                    x: (dims.0 as f64 * x) as u32,
                                    y: (dims.1 as f64 * y) as u32,
                                    w: (dims.0 as f64 * w) as u32,
                                    h: (dims.1 as f64 * h) as u32,
                                },
                                start,
                                end,
                                blur,
                            };
                            View::VideoRedact {
                                file,
                                size: Some(dims),
                                aspect: None,
                                duration,
                                areas: vec![
                                    area(0.08, 0.1, 0.25, 0.3, 0.0, duration, false),
                                    area(0.4, 0.3, 0.3, 0.35, 1.0, 3.0, true),
                                    area(0.72, 0.62, 0.22, 0.3, 2.0, duration * 0.9, false),
                                ],
                                sel: 1,
                                ref_t,
                            }
                        } else {
                            View::VideoCrop {
                                file,
                                size: Some(dims),
                                rect: ferrocast_core::croprect::fit_aspect(dims.0, dims.1, 1, 1),
                                aspect: Some((1, 1)),
                                duration,
                                ref_t,
                            }
                        });
                    }
                }
            }
            Ok("meta") => {
                // the tags are read synchronously here (screenshot mode only)
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let Some(kind) = crate::app::meta_kind(&file) {
                    let fields = match kind {
                        crate::app::MetaKind::Media => app.tools.ffmpeg.map(|ff| crate::media_tools::inspect_metadata(ff, &file)),
                        crate::app::MetaKind::Pdf => {
                            Some(ferrocast_macos::pdf::metadata(&file).map(|p| ferrocast_core::metaform::pdf_fields(&p)))
                        }
                    };
                    let (original, error) = match fields {
                        Some(Ok(f)) => (Some(f), None),
                        Some(Err(e)) => (None, Some(e)),
                        None => (None, Some("ffmpeg is not installed".to_string())),
                    };
                    app.push(View::EditMetadata {
                        file,
                        kind,
                        working: original.clone().unwrap_or_default(),
                        original,
                        remove_all: false,
                        error,
                    });
                }
            }
            Ok("vtrim") | Ok("vsnapshot") | Ok("vsplit") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let Some(ff) = app.tools.ffmpeg {
                    if let Some((d, r)) = crate::media_tools::video_info(ff, &file) {
                        let mut t = ferrocast_core::videoplan::Trim::new(d, r).unwrap();
                        t.seek(d * 0.3);
                        t.mark_start();
                        t.seek(d * 0.7);
                        t.mark_end();
                        t.seek(d * 0.45);
                        if let Some(img) = crate::media_tools::preview_frame(ff, &file, t.cursor).and_then(|b| services::decode_png(&b)) {
                            let tex = ctx.load_texture("vt-view", img, egui::TextureOptions::LINEAR);
                            ctx.data_mut(|d| {
                                d.insert_temp(Id::new("vt-tex"), tex);
                                d.insert_temp(Id::new("vt-asked"), t.cursor);
                            });
                        }
                        app.push(match std::env::var("FERROCAST_VIEW").as_deref() {
                            Ok("vsnapshot") => View::VideoSnapshot { file, trim: Some(t) },
                            Ok("vsplit") => {
                                let mut points = vec![];
                                for at in [0.3, 0.7] {
                                    let mut c = t;
                                    c.seek(d * at);
                                    c.add_cut(&mut points);
                                }
                                View::VideoSplit { file, trim: Some(t), points, parts: 2 }
                            }
                            _ => View::VideoTrim { file, trim: Some(t) },
                        });
                    }
                }
            }
            Ok("anorm") | Ok("achan") => {
                // measured / probed synchronously here (screenshot mode only)
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let Some(ff) = app.tools.ffmpeg {
                    app.push(if std::env::var("FERROCAST_VIEW").as_deref() == Ok("anorm") {
                        let l = crate::media_tools::loudness(ff, &file);
                        View::NormalizeAudio { file, loudness: l, target: -16.0, range: 11.0, peak: -1.5 }
                    } else {
                        let n = crate::media_tools::audio_channel_count(ff, &file);
                        View::ChannelsAudio { file, in_channels: n, stereo: true, left_db: -3.0, right_db: 0.0 }
                    });
                }
            }
            Ok("avis") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                app.push(View::VisualizeAudio {
                    file,
                    orient: ferrocast_core::media::Canvas::Portrait,
                    style: crate::media_tools::VisStyle::Waveform,
                    image: None,
                    picking: false,
                });
            }
            Ok("bleep") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let Some(ff) = app.tools.ffmpeg {
                    match crate::media_tools::audio_samples(ff, &file, crate::media_tools::AUDIO_CAP) {
                        Ok(a) => {
                            let bars = ferrocast_core::waveform::peaks(&a.samples, 800);
                            let ranges = vec![(0.8, 1.6), (2.5, 3.5), (4.4, 5.2)];
                            app.push(View::BleepAudio {
                                file,
                                duration: a.duration,
                                peaks: Some(std::sync::Arc::new(bars)),
                                ranges,
                                sel: 1,
                            });
                        }
                        Err(e) => app.toast(e),
                    }
                }
            }
            Ok("atrim") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let Some(ff) = app.tools.ffmpeg {
                    match crate::media_tools::audio_samples(ff, &file, crate::media_tools::AUDIO_CAP) {
                        Ok(a) => {
                            let mut t = ferrocast_core::videoplan::Trim::new(a.duration, 100.0).unwrap();
                            t.set_range(1.0, 4.0);
                            t.seek(2.2);
                            app.push(View::AudioTrim { file, trim: Some(t), audio: Some(std::sync::Arc::new(a)) });
                        }
                        Err(e) => app.toast(e),
                    }
                }
            }
            Ok("collage") => {
                let files: Vec<std::path::PathBuf> = std::env::var("FERROCAST_FILES")
                    .unwrap_or_default()
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect();
                let imgs: Vec<image::RgbaImage> = files
                    .iter()
                    .filter_map(|f| services::display_png(f, 300).and_then(|p| image::load_from_memory(&p).ok()).map(|i| i.to_rgba8()))
                    .collect();
                if imgs.len() == files.len() && files.len() >= 2 {
                    ctx.data_mut(|d| d.insert_temp(Id::new("collage-base"), std::sync::Arc::new(imgs)));
                    let opts = ferrocast_core::collage::CollageOpts {
                        layout: ferrocast_core::collage::Layout::Featured,
                        gap: 24,
                        radius: 40,
                        bg: Some(ferrocast_core::annotate::COLORS[4]),
                        ..Default::default()
                    };
                    app.push(View::CollageImage { files, opts, color: 4 });
                }
            }
            Ok("frame") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let (Some(size), Some(png)) = (services::image_size(&file), services::display_png(&file, 600)) {
                    let rgba = image::load_from_memory(&png).unwrap().to_rgba8();
                    ctx.data_mut(|d| d.insert_temp(Id::new("frame-base"), std::sync::Arc::new(rgba)));
                    let opts = ferrocast_core::frame::FrameOpts {
                        pad: size.0 / 8,
                        radius: size.0 / 14,
                        shadow: 40,
                        backdrop: ferrocast_core::frame::Backdrop::Color(ferrocast_core::annotate::COLORS[4]),
                    };
                    let mut opts = opts;
                    // FERROCAST_FRAME_IMAGE=/path/pic.png preselects the picture backdrop
                    if let Ok(bg) = std::env::var("FERROCAST_FRAME_IMAGE") {
                        if let Some(rgba) =
                            services::display_png(std::path::Path::new(&bg), 600).and_then(|p| image::load_from_memory(&p).ok())
                        {
                            ctx.data_mut(|d| d.insert_temp(Id::new("frame-bg"), std::sync::Arc::new((bg.clone(), rgba.to_rgba8()))));
                            opts.backdrop = ferrocast_core::frame::Backdrop::Image(bg);
                        }
                    }
                    app.push(View::FrameImage { file, size, opts, color: 4, picking: false });
                }
            }
            Ok("redact") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let (Some(size), Some(png)) = (services::image_size(&file), services::display_png(&file, 1000)) {
                    if let Some(img) = services::decode_png(&png) {
                        let tex = ctx.load_texture("redact-view", img, egui::TextureOptions::LINEAR);
                        ctx.data_mut(|d| d.insert_temp(Id::new("redact-tex"), tex));
                    }
                    let areas = vec![
                        ferrocast_core::croprect::Rect { x: size.0 / 8, y: size.1 / 5, w: size.0 / 3, h: size.1 / 5 },
                        ferrocast_core::croprect::Rect { x: size.0 / 2, y: size.1 / 2, w: size.0 / 4, h: size.1 / 4 },
                    ];
                    let blur = std::env::var("FERROCAST_REDACT_BLUR").is_ok();
                    app.push(View::RedactImage { file, size, areas, sel: Some(1), blur, radius: 24, color: 5 });
                }
            }
            Ok("crop") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let (Some(size), Some(png)) = (services::image_size(&file), services::display_png(&file, 1000)) {
                    if let Some(img) = services::decode_png(&png) {
                        let tex = ctx.load_texture("crop-view", img, egui::TextureOptions::LINEAR);
                        ctx.data_mut(|d| d.insert_temp(Id::new("crop-tex"), tex));
                    }
                    let rect = ferrocast_core::croprect::fit_aspect(size.0, size.1, 4, 3);
                    app.push(View::CropImage { file, size, rect, aspect: Some((4, 3)) });
                }
            }
            Ok("edit") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let (Some(size), Some(png)) = (services::image_size(&file), services::display_png(&file, 800)) {
                    let rgba = image::load_from_memory(&png).unwrap().to_rgba8();
                    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                    ctx.data_mut(|d| d.insert_temp(Id::new("edit-base"), std::sync::Arc::new((rgba.into_raw(), w, h))));
                    let opts = ferrocast_core::photo::PhotoOpts {
                        brightness: 15,
                        contrast: 20,
                        saturation: -35,
                        warmth: 25,
                        grain: 20,
                        ..Default::default()
                    };
                    app.push(View::EditPhoto { file, size, opts });
                }
            }
            Ok("annotate") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let (Some(size), Some(png)) = (services::image_size(&file), services::display_png(&file, 1600)) {
                    let rgba = image::load_from_memory(&png).unwrap().to_rgba8();
                    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                    ctx.data_mut(|d| d.insert_temp(Id::new("annot-base"), std::sync::Arc::new((rgba.into_raw(), w, h))));
                    use ferrocast_core::annotate::{Doc, Shape, Style, Tool, COLORS};
                    let mut doc = Doc::default();
                    let st = Style { color: COLORS[0], width: 8.0, fill: false };
                    let (cw, ch) = (size.0 as f32, size.1 as f32);
                    doc.add(Shape::Arrow((cw * 0.1, ch * 0.9), (cw * 0.4, ch * 0.5)), st);
                    doc.add(Shape::Rect((cw * 0.5, ch * 0.2), (cw * 0.85, ch * 0.5)), Style { color: COLORS[4], ..st });
                    doc.add(Shape::Number((cw * 0.5, ch * 0.2), 1), st);
                    doc.add(
                        Shape::Callout { at: (cw * 0.55, ch * 0.62), tip: (cw * 0.72, ch * 0.75), text: "Zoom here".into() },
                        Style { color: COLORS[4], width: 6.0, fill: false },
                    );
                    doc.add(Shape::Text((cw * 0.08, ch * 0.08), "Check this".into()), Style { color: COLORS[2], width: 10.0, fill: false });
                    doc.add(
                        Shape::Magnifier { center: (cw * 0.7, ch * 0.75), radius: cw * 0.1, zoom: 2.5 },
                        Style { color: COLORS[3], ..st },
                    );
                    app.push(View::Annotate { file, size, doc, tool: Tool::Arrow, color: 0, sel: Some(1) });
                }
            }
            Ok("reorder") => {
                let files: Vec<std::path::PathBuf> = std::env::var("FERROCAST_FILES")
                    .unwrap_or_default()
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect();
                let purpose = match std::env::var("FERROCAST_REORDER").as_deref() {
                    Ok("audio") => ReorderPurpose::MergeAudio,
                    Ok("video") => ReorderPurpose::JoinVideos,
                    Ok("images") => ReorderPurpose::CreatePdf,
                    _ => ReorderPurpose::MergePdf,
                };
                app.push(View::Reorder { purpose, files, cursor: 0 });
            }
            Ok("organize") => {
                let file = std::path::PathBuf::from(std::env::var("FERROCAST_FILES").unwrap_or_default());
                if let Ok(n) = ferrocast_macos::pdf::page_count(&file) {
                    let mut plan = ferrocast_core::pdfplan::PdfPlan::new(n);
                    plan.move_cursor(1);
                    plan.rotate(1);
                    plan.duplicate();
                    for i in 0..n {
                        if let Ok(png) = ferrocast_macos::pdf::thumbnail(&file, i, 240) {
                            if let Some(img) = services::decode_png(&png) {
                                let tex = ctx.load_texture(format!("pdfthumb-{i}"), img, egui::TextureOptions::LINEAR);
                                ctx.data_mut(|d| d.insert_temp(Id::new(("pdfthumb", i)), tex));
                            }
                        }
                    }
                    app.push(View::PdfOrganize { file, n, plan });
                }
            }
            Ok("welcome") => {
                app.push(View::Welcome);
                // dev aid: FERROCAST_TOUR_SLIDE=<0..6> renders that slide
                if let Some(n) = std::env::var("FERROCAST_TOUR_SLIDE").ok().and_then(|v| v.parse().ok()) {
                    app.tour.go(n);
                }
            }
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
                let mut segs = app.wheel_segments(kind, &files);
                // screenshot aid: show a ring of N segments by repeating the real ones
                if let Some(want) =
                    std::env::var("FERROCAST_WHEEL_N").ok().and_then(|v| v.parse::<usize>().ok()).filter(|n| (1..=12).contains(n))
                {
                    let base = segs.clone();
                    if !base.is_empty() {
                        segs = (0..want).map(|i| base[i % base.len()].clone()).collect();
                    }
                }
                let sel = std::env::var("FERROCAST_WHEEL_SEL").ok().and_then(|v| v.parse::<usize>().ok()).filter(|i| *i < segs.len());
                match std::env::var("FERROCAST_THEME").as_deref() {
                    Ok("light") => ctx.set_theme(egui::ThemePreference::Light),
                    Ok("dark") => ctx.set_theme(egui::ThemePreference::Dark),
                    _ => {}
                }
                app.push(View::Wheel { files, center: Pos2::new(-9999.0, -9999.0), segs, kind, sel });
            }
            // `settings-tab-N`: open Settings on tab N (0 General … 8 Advanced), for design review shots
            Ok(v) if v.starts_with("settings-tab-") => {
                let n = v.trim_start_matches("settings-tab-").parse::<usize>().unwrap_or(0);
                ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), n));
                if let Ok(t) = std::env::var("FERROCAST_SETTINGS_OPEN") {
                    if let Some(en) = ferrocast_core::settings_index::ENTRIES.iter().find(|e| e.title == t) {
                        let (mut tab, mut q) = (n, String::new());
                        open_settings_result(ctx, en, &mut tab, &mut q);
                    }
                }
                // `FERROCAST_SETTINGS_QUERY=text` shows the search results state
                if let Ok(q) = std::env::var("FERROCAST_SETTINGS_QUERY") {
                    ctx.data_mut(|d| d.insert_temp(Id::new("settings_query"), q));
                }
                app.push(View::Settings);
            }
            Ok("settings-ai") => {
                ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 3usize));
                app.push(View::Settings);
            }
            // Settings → Integrations scrolled to the Obsidian group, with fixed scan results (no scan)
            Ok("settings-obsidian") => {
                ctx.data_mut(|d| {
                    d.insert_temp(Id::new("settings_tab"), INTEGRATIONS_TAB);
                    d.insert_temp(Id::new("dev-settings-scroll"), 330.0f32);
                });
                let mk = |n: &str| ferrocast_core::obsidian::Vault {
                    id: String::new(),
                    name: n.into(),
                    path: std::path::PathBuf::from("/Users/me").join(n),
                };
                app.obsidian.started = true;
                app.obsidian.vaults = Some(vec![mk("Personal"), mk("Work Notes")]);
                app.obsidian.open = Some(std::path::PathBuf::from("/Users/me/Personal"));
                app.obsidian.installed = Some(false);
                app.settings.obsidian_vault = Some(std::path::PathBuf::from("/Users/me/Work Notes"));
                app.push(View::Settings);
            }
            Ok("settings-advanced") => {
                ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 8usize));
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
        let tour = matches!(view, View::Welcome);
        let query_empty = self.query.is_empty();
        let panel = self.panel.is_some();
        // egui drops the focus of a text field at the start of the frame in which Escape arrives,
        // so "a field is being edited" must also look at the previous frame; otherwise the first
        // Escape would cancel the edit and close the view at once
        let kbd_now = ctx.egui_wants_keyboard_input();
        let kbd_edit = kbd_now || ctx.data(|d| d.get_temp::<bool>(Id::new("kbd-prev"))).unwrap_or(false);
        ctx.data_mut(|d| d.insert_temp(Id::new("kbd-prev"), kbd_now));
        // typing a label on the annotation canvas: the text field owns Enter and Escape
        let typing = (matches!(view, View::Annotate { .. }) && ctx.data(|d| d.get_temp::<bool>(Id::new("annot-typing"))).unwrap_or(false))
            // a number box (crop W/H, slider value) being edited owns Enter and Escape too
            || (matches!(view, View::CropImage { .. } | View::CompressImage { .. } | View::EditPhoto { .. } | View::FrameImage { .. } | View::RedactImage { .. } | View::CollageImage { .. } | View::VideoCrop { .. } | View::VideoSpeed { .. } | View::VideoRedact { .. } | View::VideoSplit { .. } | View::EditMetadata { .. } | View::AudioTrim { .. } | View::BleepAudio { .. } | View::NormalizeAudio { .. } | View::ChannelsAudio { .. }) && kbd_edit);
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
            if tour {
                k.left = i.consume_key(Modifiers::NONE, Key::ArrowLeft);
                k.right = i.consume_key(Modifiers::NONE, Key::ArrowRight);
            }
            if (shape != Shape::Form || panel) && !typing {
                k.repeat = i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, repeat: true, .. }));
                k.enter = i.consume_key(Modifiers::NONE, Key::Enter);
            }
            if !typing {
                k.esc = i.consume_key(Modifiers::NONE, Key::Escape);
            }
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
                    View::Notes => self.open_notes_window(|c, d| c.create_note(d)),
                    _ => return false,
                }
                return true;
            }
            if matches!(self.view(), View::ObsidianSearch) && ctx.input_mut(|i| i.consume_key(CMD, Key::Y)) {
                self.obsidian_toggle_preview();
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
            if matches!(self.view(), View::Settings) {
                // the first Escape of a Settings search only clears the query
                let id = Id::new("settings_query");
                if ctx.data(|d| d.get_temp::<String>(id)).is_some_and(|q| !q.is_empty()) {
                    ctx.data_mut(|d| d.insert_temp(id, String::new()));
                    return true;
                }
            }
            self.pop();
            return true;
        }
        match self.view().clone() {
            View::CompressImage { files, orig_bytes, mode, kb, percent, preset, max_edge, resize, media: Some(m), .. } if k.enter => {
                match compress_media_job(m.video, orig_bytes, m.info.as_ref(), mode, kb, percent, preset, max_edge, resize) {
                    _ if files.is_empty() => self.toast("Compress: no files selected"),
                    Err(e) => self.toast(e),
                    Ok(tool) => match self.tools.ffmpeg {
                        None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                        Some(_) => {
                            self.toast(format!("{}…", tool.label()));
                            services::file_tool_files(files, tool, self.tools.ffmpeg, self.bus.clone());
                            self.pop();
                        }
                    },
                }
                true
            }
            View::CompressImage { files, mode, kb, percent, preset, max_edge, resize, .. } if k.enter => {
                match compress_goal(mode, kb, percent, preset, max_edge, resize) {
                    _ if files.is_empty() => self.toast("Compress: no images selected"),
                    Err(e) => self.toast(e),
                    Ok((goal, r)) => {
                        let op = ferrocast_core::convert::ImageOp::CompressWith(goal, r);
                        self.toast(format!("{}…", op.label()));
                        services::image_op_files(files, op, self.bus.clone());
                        self.pop();
                    }
                }
                true
            }
            View::Reorder { purpose, files, .. } if k.enter => {
                if files.len() < 2 {
                    self.toast("Choose order: at least two files are needed");
                } else {
                    self.toast("Working…");
                    match reorder_job(purpose, files) {
                        ReorderJob::Tool(tool, files) => services::file_tool_files(files, tool, self.tools.ffmpeg, self.bus.clone()),
                        ReorderJob::Image(op, files) => services::image_op_files(files, op, self.bus.clone()),
                    }
                    self.pop();
                }
                true
            }
            View::PdfOrganize { file, n, plan } if k.enter => {
                if plan.is_identity(n) {
                    self.toast("Nothing changed: reorder, rotate, duplicate or remove a page first");
                } else {
                    self.toast("Organizing PDF…");
                    services::file_tool_files(
                        vec![file],
                        ferrocast_core::convert::FileTool::OrganizePdf(plan.selection()),
                        None,
                        self.bus.clone(),
                    );
                    self.pop();
                }
                true
            }
            View::EditMetadata { file, original: Some(orig), working, remove_all, .. } if k.enter => {
                // GIFs cannot store tags: the executor refuses edits, so only "remove all" is offered
                let gif = ferrocast_core::convert::ext(&file) == "gif";
                let edits = if gif { vec![] } else { ferrocast_core::metaform::build_edits(&orig, &working) };
                if edits.is_empty() && !remove_all {
                    self.toast("Nothing changed");
                } else {
                    self.toast(if remove_all { "Removing metadata…" } else { "Saving metadata…" });
                    services::file_tool_files(
                        vec![file],
                        ferrocast_core::convert::FileTool::EditMetadata(edits, remove_all),
                        self.tools.ffmpeg,
                        self.bus.clone(),
                    );
                    self.pop();
                }
                true
            }
            View::VideoSpeed { file, duration: Some(_), factor } if k.enter => {
                if (factor - 1.0).abs() < 1e-9 {
                    self.toast("Nothing to change: pick a speed other than 1×");
                } else {
                    self.toast("Changing speed…");
                    services::file_tool_files(
                        vec![file],
                        ferrocast_core::convert::FileTool::Speed(factor),
                        self.tools.ffmpeg,
                        self.bus.clone(),
                    );
                    self.pop();
                }
                true
            }
            View::VideoCrop { file, size: Some(size), rect, .. } if k.enter => {
                if rect == ferrocast_core::croprect::full(size.0, size.1) {
                    self.toast("Nothing to crop: drag the rectangle first");
                } else {
                    let (x, y, w, h) = ferrocast_core::croprect::tuple(rect);
                    self.toast("Cropping video…");
                    services::file_tool_files(
                        vec![file],
                        ferrocast_core::convert::FileTool::CropVideo(x, y, w, h),
                        self.tools.ffmpeg,
                        self.bus.clone(),
                    );
                    self.pop();
                }
                true
            }
            View::VideoRedact { file, size: Some(size), areas, duration, .. } if k.enter => {
                match redact_job(&areas, duration, size) {
                    Err(m) => self.toast(m),
                    Ok(_) if self.tools.ffmpeg.is_none() => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                    Ok(tool) => {
                        self.toast("Redacting video…");
                        services::file_tool_files(vec![file], tool, self.tools.ffmpeg, self.bus.clone());
                        self.pop();
                    }
                }
                true
            }
            View::BleepAudio { file, duration, ranges, peaks: Some(_), .. } if k.enter => {
                match bleep_job(&ranges, duration) {
                    Err(m) => self.toast(m),
                    Ok(_) if self.tools.ffmpeg.is_none() => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                    Ok(tool) => {
                        self.toast("Bleeping…");
                        services::file_tool_files(vec![file], tool, self.tools.ffmpeg, self.bus.clone());
                        self.pop();
                    }
                }
                true
            }
            View::BleepAudio { .. } if k.enter => true,
            View::VideoSnapshot { trim: Some(_), .. } if k.enter && k.repeat => true,
            View::VideoSnapshot { file, trim: Some(t) } if k.enter => {
                self.toast("Saving full-resolution frame…");
                services::file_tool_files(
                    vec![file],
                    ferrocast_core::convert::FileTool::Snapshot(t.cursor),
                    self.tools.ffmpeg,
                    self.bus.clone(),
                );
                true
            }
            View::VideoSplit { file, trim: Some(t), points, parts } if k.enter => {
                match split_job(t.duration, &points, parts) {
                    Err(m) => self.toast(m),
                    Ok(_) if self.tools.ffmpeg.is_none() => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                    Ok(tool) => {
                        self.toast("Splitting…");
                        services::file_tool_files(vec![file], tool, self.tools.ffmpeg, self.bus.clone());
                        self.pop();
                    }
                }
                true
            }
            View::AudioTrim { file, trim: Some(t), .. } if k.enter => {
                if t.is_whole() {
                    self.toast("Nothing to trim: set a start (I) or an end (O) first");
                } else {
                    self.toast("Trimming…");
                    services::file_tool_files(
                        vec![file],
                        ferrocast_core::convert::FileTool::Trim(t.start, t.end),
                        self.tools.ffmpeg,
                        self.bus.clone(),
                    );
                    self.pop();
                }
                true
            }
            View::NormalizeAudio { file, target, range, peak, .. } if k.enter => {
                match crate::media_tools::normalize_params(target, range, peak) {
                    Err(m) => self.toast(m),
                    Ok(tool) => {
                        self.toast("Normalizing…");
                        services::file_tool_files(vec![file], tool, self.tools.ffmpeg, self.bus.clone());
                        self.pop();
                    }
                }
                true
            }
            View::ChannelsAudio { file, stereo, left_db, right_db, .. } if k.enter => {
                match crate::media_tools::channels_params(stereo, left_db, right_db) {
                    Err(m) => self.toast(m),
                    Ok(tool) => {
                        self.toast(if stereo { "Converting channels to stereo…" } else { "Converting channels to mono…" });
                        services::file_tool_files(vec![file], tool, self.tools.ffmpeg, self.bus.clone());
                        self.pop();
                    }
                }
                true
            }
            View::VisualizeAudio { file, orient, style, image, .. } if k.enter => {
                match crate::media_tools::visualize_job(orient, style, image.as_deref()) {
                    Err(m) => self.toast(m),
                    Ok(_) if self.tools.ffmpeg.is_none() => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                    Ok(tool) => {
                        self.toast("Making the video…");
                        services::file_tool_files(vec![file], tool, self.tools.ffmpeg, self.bus.clone());
                        self.pop();
                    }
                }
                true
            }
            View::VideoTrim { file, trim: Some(t) } if k.enter => {
                if t.is_whole() {
                    self.toast("Nothing to trim: set a start (I) or an end (O) first");
                } else {
                    self.toast("Trimming…");
                    services::file_tool_files(
                        vec![file],
                        ferrocast_core::convert::FileTool::TrimExact(t.start, t.end),
                        self.tools.ffmpeg,
                        self.bus.clone(),
                    );
                    self.pop();
                }
                true
            }
            View::CollageImage { files, opts, .. } if k.enter => {
                self.toast("Saving collage…");
                services::image_op_files(files, ferrocast_core::convert::ImageOp::CollageWith(opts), self.bus.clone());
                self.pop();
                true
            }
            View::FrameImage { file, opts, .. } if k.enter => {
                match crate::app::frame_job(&opts) {
                    Ok(op) => {
                        self.toast("Saving framed copy…");
                        services::image_op_files(vec![file], op, self.bus.clone());
                        self.pop();
                    }
                    Err(m) => self.toast(&m),
                }
                true
            }
            View::RedactImage { file, areas, blur, radius, color, .. } if k.enter => {
                match crate::app::redact_job_image(&areas, blur, radius, color) {
                    Ok(op) => {
                        self.toast("Saving redacted copy…");
                        services::image_op_files(vec![file], op, self.bus.clone());
                        self.pop();
                    }
                    Err(m) => self.toast(&m),
                }
                true
            }
            View::CropImage { file, size, rect, .. } if k.enter => {
                if rect == ferrocast_core::croprect::full(size.0, size.1) {
                    self.toast("Nothing to crop: drag the rectangle first");
                } else {
                    let (x, y, w, h) = ferrocast_core::croprect::tuple(rect);
                    self.toast("Saving cropped copy…");
                    services::image_op_files(vec![file], ferrocast_core::convert::ImageOp::CropRect(x, y, w, h), self.bus.clone());
                    self.pop();
                }
                true
            }
            View::EditPhoto { file, opts, .. } if k.enter => {
                if opts.is_identity() {
                    self.toast("Nothing changed: move a slider first");
                } else {
                    self.toast("Saving edited copy…");
                    services::image_op_files(vec![file], ferrocast_core::convert::ImageOp::EditWith(opts), self.bus.clone());
                    self.pop();
                }
                true
            }
            View::Annotate { file, doc, .. } if k.enter => {
                if doc.items.is_empty() {
                    self.toast("Nothing drawn yet");
                } else {
                    self.toast("Saving annotated copy…");
                    let bus = self.bus.clone();
                    std::thread::spawn(move || {
                        let msg = match services::annotate_save(&file, &doc) {
                            Ok(o) => {
                                let _ = ferrocast_macos::apps::reveal(&o);
                                format!(
                                    "Annotated copy saved: {}",
                                    o.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                                )
                            }
                            Err(e) => format!("Annotate failed: {e}"),
                        };
                        bus.send(Msg::Toast(msg));
                    });
                    self.pop();
                }
                true
            }
            View::Update { release } if k.enter => {
                self.pop();
                let exe = instance::running_exe().unwrap_or_default();
                self.start_update(release, ferrocast_core::update::channel_of(&exe), services::parent_writable_cached());
                true
            }
            // the grant is a button: Enter only moves through the deck, and a held key does not run through it
            View::Welcome if k.enter => {
                if !k.repeat {
                    self.tour_enter();
                }
                true
            }
            View::Welcome if k.left || k.right => {
                if k.left {
                    self.tour.prev();
                } else {
                    self.tour.next();
                }
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
            View::NotionAdd { .. } if k.cmd_enter => {
                self.notion_add_submit();
                true
            }
            View::ObsidianNew { .. } if k.cmd_enter => {
                self.obsidian_new_submit();
                true
            }
            View::ObsidianDaily { .. } if k.cmd_enter => {
                self.obsidian_daily_submit();
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
        self.intro_input(ui.ctx());
        self.draw_main(ui);
        self.intro_paint(ui);
    }

    fn draw_main(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.icons.new_frame();
        self.dark = ctx.theme() == egui::Theme::Dark;
        let pal = palette(self.dark);
        // Brand-coloured caret and text selection instead of egui's default blue.
        if ui.visuals().text_cursor.stroke.color != pal.accent {
            let accent = pal.accent;
            ctx.all_styles_mut(|st| {
                // the slider rail is `widgets.inactive.bg_fill`; egui's grey 230 vanishes on a light card
                st.visuals.widgets.inactive.bg_fill = palette(st.visuals.dark_mode).sel.gamma_multiply(2.0);
                st.visuals.text_cursor.stroke.color = accent;
                st.visuals.selection.bg_fill = accent.gamma_multiply(0.35);
                st.visuals.selection.stroke.color = accent;
            });
        }
        if let View::Wheel { center, segs, files, sel, kind } = self.view().clone() {
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
            // Shift, Command or Control tapped while the wheel is open switches wheels
            let mods = ctx.input(|i| i.modifiers);
            let was: (bool, bool, bool) = ctx.data(|d| d.get_temp(Id::new("wheel_mods"))).unwrap_or((true, true, true));
            ctx.data_mut(|d| d.insert_temp(Id::new("wheel_mods"), (mods.shift, mods.command, mods.ctrl)));
            use ferrocast_core::wheel::WheelKind;
            let pressed = |now: bool, before: bool| now && !before;
            if pressed(mods.shift, was.0) {
                self.switch_wheel(WheelKind::Convert);
            } else if pressed(mods.command, was.1) && !mods.ctrl {
                self.switch_wheel(WheelKind::Tools);
            } else if pressed(mods.ctrl, was.2) && !mods.command {
                self.switch_wheel(WheelKind::Actions);
            }
            if esc {
                self.apply_wheel(None);
                return;
            }
            if enter {
                self.apply_wheel(sel);
                return;
            }
            // A click picks the segment under the pointer, and a click outside the ring cancels.
            // During a drag the pick happens on the mouse RELEASE (the drag watcher sends it), but a
            // wheel opened without a drag (Enter on a Finder selection) has no release event and used
            // to look locked. The press of a drag happens in Finder, so no click reaches here then.
            if ctx.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    let d = pos - ui.max_rect().center();
                    if let Some(i) = wheel_hit(d, n) {
                        self.apply_wheel(Some(i));
                        return;
                    } else if d.length() > WHEEL_OUTER {
                        self.apply_wheel(None);
                        return;
                    }
                }
            }
            if (next || prev) && n > 0 {
                let ns = ferrocast_core::wheel::step_sel(sel, n, next);
                if let Some(View::Wheel { sel, .. }) = self.stack.last_mut() {
                    *sel = Some(ns);
                }
            }
            let sel = if let Some(View::Wheel { sel, .. }) = self.stack.last() { *sel } else { None };
            self.draw_wheel(ui, &pal, center, &segs, &files, kind, sel);
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
        // The drop that ends a wheel gesture arrives right after the wheel closed. When the picked
        // tool showed its own window, that window is visible by now: the drop must not replace it
        // with the converter.
        let after_wheel = ctx.data(|d| d.get_temp::<Instant>(Id::new("wheel_end"))).is_some_and(|t| t.elapsed() < WHEEL_DROP_GRACE);
        let dropped = if after_wheel { Vec::new() } else { dropped };
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
        // the tour is a full-bleed deck: it has no search bar
        let tour_view = matches!(self.view(), View::Welcome);
        let search_rect = Rect::from_min_size(full.min, Vec2::new(full.width(), if tour_view { 0.0 } else { ui::SEARCH_H }));
        let footer_rect = Rect::from_min_max(Pos2::new(full.min.x, full.max.y - ui::FOOTER_H), full.max);
        let content = Rect::from_min_max(Pos2::new(full.min.x, search_rect.max.y), Pos2::new(full.max.x, footer_rect.min.y));

        // ---- search bar
        let title = self.view().title();
        let placeholder = self.view().placeholder();
        let nested = self.stack.len() > 1;
        let mut back = false;
        if !tour_view {
            ui.scope_builder(UiBuilder::new().max_rect(search_rect.shrink2(Vec2::new(18.0, 0.0)).translate(Vec2::new(0.0, 3.0))), |ui| {
                ui.horizontal_centered(|ui| {
                    if nested && matches!(self.view(), View::Settings) {
                        // a 28 pt hit target for the back arrow
                        let (br, bresp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                        record_ctl(&ctx, "back", bresp.id, br, true);
                        record_text(&ctx, &pal, "back-arrow", pal.sub_strong, &[], 1.0);
                        let col = if bresp.hovered() { pal.text } else { pal.sub_strong };
                        ui.painter().text(br.center(), Align2::CENTER_CENTER, "←", FontId::proportional(18.0), col);
                        focus_ring(ui, &pal, &bresp, br);
                        if bresp.clicked() {
                            back = true;
                        }
                    } else if nested
                        && ui.add(egui::Label::new(RichText::new("←").size(18.0).color(pal.sub)).sense(Sense::click())).clicked()
                    {
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
                    } else if matches!(self.view(), View::Settings) {
                        // breadcrumb: "Settings /" muted, the current tab in the normal colour
                        let tab: usize = ctx.data(|d| d.get_temp(Id::new("settings_tab"))).unwrap_or(0);
                        let searching = ctx.data(|d| d.get_temp::<String>(Id::new("settings_query"))).is_some_and(|q| !q.trim().is_empty());
                        let name = if searching { "Search" } else { SETTINGS_TABS[tab.min(SETTINGS_TABS.len() - 1)].0 };
                        ui.spacing_mut().item_spacing.x = 6.0;
                        record_text(&ctx, &pal, "crumb-parent", pal.sub_strong, &[], 1.0);
                        record_text(&ctx, &pal, "crumb-current", pal.text, &[], 1.0);
                        ui.label(RichText::new("Settings /").size(16.0).color(pal.sub_strong));
                        ui.label(RichText::new(name).size(16.0).strong().color(pal.text));
                        #[cfg(test)]
                        ctx.data_mut(|d| d.insert_temp(Id::new("settings-breadcrumb"), format!("Settings / {name}")));
                    } else {
                        ui.label(RichText::new(title).size(16.0).strong().color(pal.text));
                    }
                });
            });
        }
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
        if !tour_view {
            line(search_rect.max.y);
        }
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
                // Search Obsidian: the selected note's preview on the right (⌘Y hides it)
                let note = match (self.view(), &cur) {
                    (View::ObsidianSearch, Some(Item { kind: Kind::ObsidianNote(rel), .. })) if !self.ov.hide_preview => Some(rel.clone()),
                    _ => None,
                };
                let list_rect = if detail.is_some() {
                    Rect::from_min_max(content.min, Pos2::new(content.min.x + content.width() * 0.42, content.max.y))
                } else if note.is_some() {
                    Rect::from_min_max(content.min, Pos2::new(content.min.x + OBSIDIAN_LIST_W, content.max.y))
                } else {
                    content
                };
                (clicked, double) = self.draw_list(ui, list_rect, &sections, &pal);
                if let Some(rel) = note {
                    let r = Rect::from_min_max(Pos2::new(list_rect.max.x, content.min.y), content.max);
                    ui.painter().vline(r.min.x, r.y_range(), Stroke::new(1.0, pal.sep));
                    self.draw_obsidian_preview(ui, r, &rel, &pal);
                }
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
                } else if sections.is_empty() && matches!(self.view(), View::ObsidianSearch) && self.ov.indexing {
                    ui.painter().text(content.center(), Align2::CENTER_CENTER, "Indexing your vault…", FontId::proportional(14.0), pal.sub);
                } else if sections.is_empty() && !(matches!(self.view(), View::NotionSearch) && self.nv.searching) {
                    ui.painter().text(content.center(), Align2::CENTER_CENTER, "No results", FontId::proportional(14.0), pal.sub);
                }
            }
            // under the intro the deck waits, so its entrance plays at the hand-off
            _ if tour_view && self.intro_covers() => {}
            _ if tour_view => self.draw_tour(ui, content, &pal),
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
                View::Update { .. } => Some("Install and relaunch".into()),
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
        // Settings only: stronger muted/accent text colours (see Palette::sub_strong)
        let settings = matches!(self.view(), View::Settings);
        let fsub = if settings { pal.sub_strong } else { pal.sub };
        let facc = if settings { pal.accent_text } else { pal.accent };
        let q = self.query.trim();
        let searching = list
            && q.chars().count() >= 2
            && self.files.0 != q
            && match self.view() {
                View::Root => self.settings.file_search_in_root,
                View::Files => true,
                _ => false,
            };
        let obsidian_busy = matches!(self.view(), View::ObsidianSearch) && self.ov.indexing;
        let notion_busy = (matches!(self.view(), View::NotionSearch) && self.nv.searching) || obsidian_busy;
        let busy_label = if obsidian_busy {
            "Indexing vault"
        } else if notion_busy {
            "Searching Notion"
        } else {
            "Searching files"
        };
        #[cfg(test)]
        ui.ctx().data_mut(|d| d.remove::<Rect>(Id::new("settings-undo-rect")));
        let mut undo_clicked = false;
        match &self.toast {
            Some((t, _)) if t == RESET_TOAST && self.settings_undo.is_some() => {
                // "Tab reset ·" then a clickable "Undo"
                let head = p.layout_no_wrap("Tab reset · ".into(), FontId::proportional(12.5), facc);
                let hw = head.size().x;
                p.galley(Pos2::new(tx, cy - head.size().y / 2.0), head, facc);
                record_text(ui.ctx(), pal, "footer-toast", facc, &[], 1.0);
                let ug = p.layout_no_wrap("Undo".into(), FontId::proportional(12.5), pal.text);
                let ur = Rect::from_min_size(Pos2::new(tx + hw - 4.0, cy - 14.0), Vec2::new(ug.size().x + 8.0, 28.0));
                let resp = ui.interact(ur, Id::new("settings-undo"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                record_ctl(ui.ctx(), "undo", resp.id, ur, true);
                let col = if resp.hovered() { facc } else { pal.text };
                p.text(Pos2::new(ur.min.x + 4.0, cy), Align2::LEFT_CENTER, "Undo", FontId::proportional(12.5), col);
                p.hline((ur.min.x + 4.0)..=(ur.max.x - 4.0), cy + 7.0, Stroke::new(1.0, col));
                #[cfg(test)]
                ui.ctx().data_mut(|d| d.insert_temp(Id::new("settings-undo-rect"), ur));
                undo_clicked = resp.clicked();
            }
            Some((t, _)) => {
                p.text(Pos2::new(tx, cy), Align2::LEFT_CENTER, t, FontId::proportional(12.5), facc);
                record_text(ui.ctx(), pal, "footer-toast", facc, &[], 1.0);
            }
            None if searching || notion_busy => {
                // Three dots that pulse while Spotlight answers; cheap, and it stops on its own.
                let t = ui.ctx().input(|i| i.time);
                p.text(Pos2::new(tx, cy), Align2::LEFT_CENTER, busy_label, FontId::proportional(12.5), pal.sub);
                let w = p.layout_no_wrap(busy_label.into(), FontId::proportional(12.5), pal.sub).size().x;
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
                // Settings shows the app name next to the icon (the header's breadcrumb already says "Settings")
                if settings {
                    p.text(Pos2::new(tx, cy), Align2::LEFT_CENTER, "Ferrocast", FontId::proportional(12.5), fsub);
                    record_text(ui.ctx(), pal, "footer-left", fsub, &[], 1.0);
                } else {
                    p.text(Pos2::new(tx, cy), Align2::LEFT_CENTER, self.view().title(), FontId::proportional(12.5), pal.sub);
                }
            }
        }
        let mut x = rect.max.x - 16.0;
        let mut window_chips = if matches!(self.view(), View::Welcome) { self.tour.hints() } else { self.view().footer_hints() };
        // The left text (title or toast) wins: drop the least important chips until both fit.
        let left_w = match &self.toast {
            Some((t, _)) => p.layout_no_wrap(t.clone(), FontId::proportional(12.5), pal.accent).size().x,
            None if settings => p.layout_no_wrap("Ferrocast".into(), FontId::proportional(12.5), fsub).size().x,
            None => p.layout_no_wrap(self.view().title().into(), FontId::proportional(12.5), pal.sub).size().x,
        };
        let chip_w = |(l, k): &(&str, &str)| {
            let kw = (p.layout_no_wrap(k.to_string(), FontId::proportional(11.5), pal.text).size().x + 12.0).max(22.0);
            kw + 8.0 + p.layout_no_wrap(l.to_string(), FontId::proportional(12.5), pal.sub).size().x + 18.0
        };
        while window_chips.len() > 1 && tx + left_w + 16.0 + window_chips.iter().map(chip_w).sum::<f32>() > rect.max.x - 16.0 {
            window_chips.remove(if window_chips.len() > 2 { window_chips.len() - 2 } else { window_chips.len() - 1 });
        }
        let mut first = true;
        #[cfg(test)]
        let mut drawn: Vec<(String, String)> = vec![];
        let mut hint = |label: &str, key: &str| {
            #[cfg(test)]
            drawn.insert(0, (label.to_string(), key.to_string()));
            if !first {
                // thin separator between hint groups, as in the original launcher footers
                p.vline(x + 9.0, (cy - 9.0)..=(cy + 9.0), Stroke::new(1.0, pal.sep));
            }
            first = false;
            let kg = p.layout_no_wrap(key.to_string(), FontId::proportional(11.5), pal.text);
            let lg = p.layout_no_wrap(label.to_string(), FontId::proportional(12.5), fsub);
            record_text(ui.ctx(), pal, "footer-label", fsub, &[], 1.0);
            record_text(ui.ctx(), pal, "footer-key", pal.text, &[pal.field], 1.0);
            let kw = (kg.size().x + 12.0).max(22.0);
            let kr = Rect::from_min_size(Pos2::new(x - kw, cy - 10.0), Vec2::new(kw, 20.0));
            p.rect_filled(kr, 5.0, pal.field);
            p.galley(Pos2::new(kr.center().x - kg.size().x / 2.0, cy - kg.size().y / 2.0), kg, pal.text);
            let lx = kr.min.x - 8.0 - lg.size().x;
            p.galley(Pos2::new(lx, cy - lg.size().y / 2.0), lg, pal.text);
            x = lx - 18.0;
        };
        if !window_chips.is_empty() {
            // tool windows: chips in reading order, so the primary action ends up leftmost
            for (label, key) in window_chips.iter().rev() {
                hint(label, key);
            }
        } else {
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
        #[cfg(test)]
        ui.ctx().data_mut(|d| d.insert_temp(Id::new("footer-chips"), drawn));
        if undo_clicked {
            self.undo_settings_reset();
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

    /// Search Obsidian's right pane: the note title and path, then the parsed blocks of its first 200 KB.
    fn draw_obsidian_preview(&self, ui: &mut Ui, rect: Rect, rel: &str, pal: &Palette) {
        let title = self.obsidian_entry(rel).map(|e| e.title).unwrap_or_else(|| rel.to_string());
        let title_text = title.clone();
        let inner = rect.shrink2(Vec2::new(18.0, 14.0));
        ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
            egui::ScrollArea::vertical().id_salt("obsidian-preview").auto_shrink([false; 2]).show(ui, |ui| {
                ui.set_width(inner.width());
                ui.spacing_mut().item_spacing.y = 6.0;
                let label = |t: RichText| egui::Label::new(t).wrap().selectable(false);
                ui.add(label(RichText::new(title).size(20.0).strong().color(pal.text)));
                ui.add(label(RichText::new(rel).size(11.5).color(pal.sub)));
                ui.add_space(4.0);
                ui.painter().hline(inner.x_range(), ui.cursor().min.y, Stroke::new(1.0, pal.sep));
                ui.add_space(6.0);
                match &self.ov.preview {
                    Some((r, Ok(blocks))) if r == rel => {
                        // the title is already shown above: a leading heading with the same text would repeat it
                        let skip = matches!(blocks.first(), Some(ferrocast_core::obsidian::Block::Heading(_, h)) if *h == title_text);
                        draw_blocks(ui, pal, &blocks[skip as usize..]);
                    }
                    Some((r, Err(()))) if r == rel => {
                        ui.add(label(RichText::new("Could not read this note.").size(13.5).color(pal.sub)));
                    }
                    _ => {
                        ui.add(label(RichText::new("Loading…").size(13.5).color(pal.sub)));
                    }
                }
            });
        });
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
    /// pointer (sampled globally, since a drag owns the mouse) is highlighted. Highlight, open,
    /// switch and hub-text changes are animated from per-wheel state in the egui context
    /// (`WheelAnim`); Reduce Motion (read once per open) makes every state jump to its end value.
    #[allow(clippy::too_many_arguments)]
    fn draw_wheel(
        &mut self,
        ui: &mut Ui,
        pal: &Palette,
        center: Pos2,
        segs: &[WheelSeg],
        files: &[std::path::PathBuf],
        kind: ferrocast_core::wheel::WheelKind,
        sel: Option<usize>,
    ) {
        use ferrocast_core::wheel::{
            anim_progress, crossfade_alpha, hub_text, lerp, ring_label, sector_points, segment_center, stagger_ms, Geometry, WheelKind,
        };
        let ctx = ui.ctx().clone();
        let g0 = Geometry::DEFAULT;
        let c = ui.max_rect().center();
        let n = segs.len();
        // A wheel opened without a drag (Enter on a Finder selection, "Format Wheel") parks its
        // centre off screen, so only the pointer inside the window is meaningful there. During a
        // real drag the window gets no pointer events (egui could even hold a stale position), so a
        // wheel with a real centre keeps using the global mouse position as before.
        let parked = center.x < -5000.0;
        let mouse = mouse_top_left().map(|(m, _)| m);
        let under = if parked { ctx.input(|i| i.pointer.hover_pos()).map(|p| p - c) } else { mouse.map(|m| m - center) };
        let hover = if n == 0 { None } else { under.and_then(|d| wheel_hit(d, n)).or(sel) };
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(Id::new("wheel-hover"), hover));
        let dark = self.dark;
        let (name, target_rgb) = match kind {
            WheelKind::Convert => ("CONVERT", [255.0f32, 140.0, 60.0]),
            WheelKind::Tools => ("TOOLS", [86.0, 194.0, 255.0]),
            WheelKind::Actions => ("ACTIONS", [89.0, 212.0, 153.0]),
            WheelKind::Translate => ("TRANSLATE", [180.0, 120.0, 255.0]),
        };
        let (count, kind_line) = match (kind, &self.wheel_text) {
            (WheelKind::Translate, Some(t)) => ferrocast_core::translate::hub_text(t),
            _ => hub_text(files),
        };
        let hub_label = match hover.and_then(|i| segs.get(i)) {
            Some(sg) if sg.label.chars().count() > 16 => format!("{}…", sg.label.chars().take(15).collect::<String>()),
            Some(sg) => sg.label.clone(),
            None => count,
        };

        // ---- animation state -------------------------------------------------------------
        let now = ctx.input(|i| i.time);
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1) as f64;
        let key = Id::new("wheel_anim");
        let old = ctx.data(|d| d.get_temp::<WheelAnim>(key)).filter(|s| now - s.last < 0.3);
        let fresh = old.is_none();
        let mut st = old.unwrap_or_else(|| WheelAnim::open(now, kind, target_rgb, &hub_label, wheel_reduce_motion(&ctx)));
        // the start of a transition is the previous frame, so its first frame already moves
        let prev = if fresh { now - dt } else { st.last };
        st.hl.resize(n, (0.0, 0.0, f64::NEG_INFINITY));
        // an animation that never started (`-inf`) is finished; a frozen dev clock only holds started ones
        let el = |t0: f64| match st.frozen {
            Some(ms) if t0.is_finite() => ms,
            _ => ((now - t0) * 1000.0) as f32,
        };
        let dur = |ms: f32| if st.reduce { 0.0 } else { ms };
        // wheel kind switched: cross-fade the accent, bring the new segments in
        if st.kind != kind {
            let p = anim_progress(el(st.kind_t0), dur(120.0));
            for (k, v) in st.from_rgb.iter_mut().enumerate() {
                *v = lerp(*v, st.to_rgb[k], p);
            }
            st.kind = kind;
            st.to_rgb = target_rgb;
            st.kind_t0 = prev;
        }
        let p_kind = anim_progress(el(st.kind_t0), dur(120.0));
        let rgb_f = [0, 1, 2].map(|k| lerp(st.from_rgb[k], st.to_rgb[k], p_kind));
        let rgb = rgb_f.map(|v| v.round().clamp(0.0, 255.0) as u8);
        let accent = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        // per-segment highlight: (from, to, start); value eased between from and to
        let mut hv = vec![0.0f32; n];
        let mut animating = false;
        for (i, h) in st.hl.iter_mut().enumerate() {
            let target = if hover == Some(i) { 1.0 } else { 0.0 };
            let cur = lerp(h.0, h.1, anim_progress(el(h.2), dur(120.0)));
            if target != h.1 {
                *h = (cur, target, prev);
            }
            let pr = anim_progress(el(h.2), dur(120.0));
            hv[i] = lerp(h.0, h.1, pr);
            animating |= pr < 1.0;
        }
        // open: the ring grows from 0.92 and fades in
        let p_open = anim_progress(el(st.open_t0), dur(140.0));
        let (scale, open_alpha) = (lerp(0.92, 1.0, p_open), p_open);
        animating |= p_open < 1.0;
        // hub text cross-fade
        if st.hub_cur != hub_label {
            st.hub_prev = std::mem::replace(&mut st.hub_cur, hub_label);
            st.hub_t0 = prev;
        }
        let p_hub = anim_progress(el(st.hub_t0), dur(80.0));
        let (hub_out, hub_in) = crossfade_alpha(p_hub);
        animating |= p_hub < 1.0;
        // switch: fade and scale each new segment in with a stagger (the last one ends at 120 + 96 ms)
        let seg_in: Vec<f32> = (0..n).map(|i| anim_progress(el(st.kind_t0) - dur(stagger_ms(i)), dur(120.0))).collect();
        animating |= p_kind < 1.0 || seg_in.iter().any(|p| *p < 1.0);
        // repaint policy: 60 fps only while something moves; otherwise 30 fps to poll the pointer
        let moved = st.mouse != mouse;
        st.mouse = mouse;
        ctx.request_repaint_after(Duration::from_millis(if animating || moved { 16 } else { 33 }));
        st.last = now;
        #[cfg(test)]
        {
            let probe = Id::new("wheel-anim-probe");
            let which = sel.or(hover);
            let mut v: Vec<(f32, f32, f32)> = ctx.data(|d| d.get_temp(probe)).unwrap_or_default();
            v.push((which.and_then(|i| hv.get(i).copied()).unwrap_or(0.0), scale, open_alpha));
            ctx.data_mut(|d| d.insert_temp(probe, v));
        }
        ctx.data_mut(|d| d.insert_temp(key, st.clone()));

        // ---- drawing ---------------------------------------------------------------------
        let g = Geometry { r_in: g0.r_in * scale, r_out: g0.r_out * scale, ..g0 };
        let outer = g.r_out + 10.0 * scale;
        let (white, black) = (Color32::from_white_alpha, Color32::from_black_alpha);
        ui.set_opacity(open_alpha);
        let p = ui.painter().clone();
        // soft shadow, then the translucent disc and its rim
        for (k, a) in if dark { [(16.0, 10u8), (10.0, 14), (5.0, 20)] } else { [(16.0, 6), (10.0, 9), (5.0, 12)] } {
            p.circle_filled(c + Vec2::new(0.0, 6.0), outer + k * scale, black(a));
        }
        p.circle_filled(
            c,
            outer,
            if dark { Color32::from_rgba_unmultiplied(24, 24, 27, 222) } else { Color32::from_rgba_unmultiplied(250, 250, 251, 226) },
        );
        p.circle_stroke(c, outer - 0.5, Stroke::new(1.0, if dark { white(34) } else { black(40) }));
        let top: Vec<Pos2> = (0..=24)
            .map(|j| {
                let a = (-60.0f32 + 120.0 * j as f32 / 24.0).to_radians();
                c + Vec2::new(a.sin(), -a.cos()) * (outer - 1.5)
            })
            .collect();
        p.add(egui::Shape::line(top, Stroke::new(1.0, if dark { white(46) } else { white(120) })));
        // ring track
        p.circle_stroke(c, (g.r_in + g.r_out) / 2.0, Stroke::new(g.r_out - g.r_in, if dark { white(8) } else { black(6) }));
        let at = |q: [f32; 2]| c + Vec2::new(q[0], q[1]);
        // while one segment is highlighted the others dim a little
        let any_on = hv.iter().cloned().fold(0.0f32, f32::max);
        for (i, seg) in segs.iter().enumerate() {
            let h = hv[i];
            let sa = seg_in[i];
            let seg_alpha = open_alpha * sa * (1.0 - 0.3 * any_on * (1.0 - h));
            ui.set_opacity(open_alpha * sa);
            let p = ui.painter().clone();
            if h > 0.003 {
                let gh = Geometry { r_out: lerp(g.r_out - 8.0 * scale, g.r_out, h), ..g };
                let pts = sector_points(i, n, &gh, 16);
                let k = pts.len() / 2 - 1;
                let mut mesh = egui::Mesh::default();
                for j in 0..=k {
                    mesh.colored_vertex(at(pts[j]), accent.gamma_multiply(0.34 * h));
                    mesh.colored_vertex(at(pts[pts.len() - 1 - j]), accent.gamma_multiply(0.10 * h));
                }
                for j in 0..k as u32 {
                    let b = j * 2;
                    mesh.add_triangle(b, b + 1, b + 2);
                    mesh.add_triangle(b + 1, b + 3, b + 2);
                }
                p.add(egui::Shape::mesh(mesh));
                p.add(egui::Shape::closed_line(pts.iter().map(|q| at(*q)).collect(), Stroke::new(1.5, accent.gamma_multiply(h))));
                // glow along the outer edge
                let edge: Vec<Pos2> = pts[..=k].iter().map(|q| at(*q)).collect();
                for (w, a) in [(10.0, 18.0f32), (6.0, 32.0), (3.0, 70.0)] {
                    p.add(egui::Shape::line(
                        edge.clone(),
                        Stroke::new(w * lerp(0.6, 1.0, h), Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], (a * h) as u8)),
                    ));
                }
            }
            if n > 1 && h < 0.99 {
                let a = (i as f32 - 0.5) * std::f32::consts::TAU / n as f32;
                let d = Vec2::new(a.sin(), -a.cos());
                p.line_segment(
                    [c + d * (g.r_in + 2.0 * scale), c + d * (g.r_out - 2.0 * scale)],
                    Stroke::new(1.0, if dark { white(16) } else { black(14) }),
                );
            }
            // tile, icon and label: dimmed with the segment, popped in by the switch animation
            ui.set_opacity(seg_alpha.clamp(0.0, 1.0));
            let pop = lerp(0.85, 1.0, sa) * scale;
            let r_mid = (g.r_in + g.r_out) / 2.0;
            let mid = at(segment_center(i, n, r_mid));
            let ring_text = ring_label(&seg.label, n, (g0.r_in + g0.r_out) / 2.0);
            let fits = ring_text.is_some();
            let tile = (if n > 8 { 24.0 } else { 28.0 }) * lerp(1.0, 1.12, h) * pop;
            let ir = Rect::from_center_size(if fits { mid - Vec2::new(0.0, 8.0 * pop) } else { mid }, Vec2::splat(tile));
            ui.painter().rect_filled(
                ir.expand(4.0 * pop),
                9.0 * pop,
                Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], lerp(22.0, 46.0, h) as u8),
            );
            let tone = if seg.needs.is_some() { [rgb[0] / 3 + 60, rgb[1] / 3 + 60, rgb[2] / 3 + 60] } else { rgb };
            ui::paint_icon(&mut self.icons, ui, ir, &Icon::Glyph(seg.symbol, tone), pal);
            if let Some((label, size)) = ring_text {
                let col = pal.sub.lerp_to_gamma(pal.text, h);
                ui.painter().text(mid + Vec2::new(0.0, 22.0 * pop), Align2::CENTER_CENTER, label, FontId::proportional(size * pop), col);
            }
        }
        // hub: wheel name, hovered label or file summary (cross-faded), kind line
        ui.set_opacity(open_alpha);
        let p = ui.painter();
        let hub_font = FontId::proportional(13.0 * scale);
        p.circle_filled(c, g.r_in - 6.0 * scale, if dark { white(10) } else { black(6) });
        p.circle_stroke(c, g.r_in - 6.0 * scale, Stroke::new(1.0, pal.border));
        p.text(c - Vec2::new(0.0, 30.0 * scale), Align2::CENTER_CENTER, name, FontId::proportional(9.5 * scale), accent);
        if hub_out > 0.0 {
            p.text(c, Align2::CENTER_CENTER, &st.hub_prev, hub_font.clone(), pal.text.gamma_multiply(hub_out));
        }
        p.text(c, Align2::CENTER_CENTER, &st.hub_cur, hub_font, pal.text.gamma_multiply(hub_in));
        p.text(c + Vec2::new(0.0, 18.0 * scale), Align2::CENTER_CENTER, kind_line, FontId::proportional(10.5 * scale), pal.sub);
        // a text wheel has no other wheel to switch to, so no key legend
        let legend: &[(&str, WheelKind)] = if kind == WheelKind::Translate {
            &[]
        } else {
            &[("⇧", WheelKind::Convert), ("⌘", WheelKind::Tools), ("⌃", WheelKind::Actions)]
        };
        for (j, (sym, k)) in legend.iter().copied().enumerate() {
            let x = c.x + (j as f32 - 1.0) * 16.0 * scale;
            p.text(
                Pos2::new(x, c.y + 38.0 * scale),
                Align2::CENTER_CENTER,
                sym,
                FontId::proportional(10.0 * scale),
                if k == kind { accent } else { pal.sub },
            );
        }
    }
    /// Compress window: goal (size, percent or preset), optional resize and a rough size estimate.
    /// `state` is (mode, kb, percent, preset, max edge, resize).
    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    fn draw_compress(
        &mut self,
        ui: &mut Ui,
        area_in: Rect,
        pal: &Palette,
        files: &[PathBuf],
        orig_bytes: u64,
        size: Option<(u32, u32)>,
        state: (usize, u32, u8, usize, Option<u32>, bool),
        media: Option<&MediaWin>,
    ) {
        use ferrocast_core::compress::estimate_bytes;
        let ctx = ui.ctx().clone();
        let (mut mode, mut kb, mut percent, mut preset, max_edge, mut resize) = state;
        let mut edge = max_edge.unwrap_or(1600);
        ui.scope_builder(UiBuilder::new().max_rect(form_rect(area_in)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let first = files.first().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let dims = match media {
                None => size.map(|(w, h)| format!(" · {w}×{h} px")).unwrap_or_else(|| " · reading size…".into()),
                Some(m) if !m.loaded => " · reading the file…".into(),
                Some(MediaWin { info: None, .. }) => " · could not read the file".into(),
                Some(MediaWin { info: Some(i), video, .. }) => {
                    let px = i.dims.filter(|_| *video).map(|(w, h)| format!(" · {w}×{h} px")).unwrap_or_default();
                    format!(
                        " · {}{px}{}",
                        ferrocast_core::convert::format_secs(i.duration),
                        if i.has_audio || !*video { "" } else { " · no sound" }
                    )
                }
            };
            let more = if files.len() > 1 { format!(" and {} more", files.len() - 1) } else { String::new() };
            ui.label(RichText::new(format!("{first}{more}{dims}")).size(12.5).color(pal.sub));
            ui.add_space(8.0);
            card(ui, pal, |ui| {
                form_row(ui, pal, "Goal", "Choose a size, a percentage or a preset.", |ui| {
                    seg_index(ui, pal, &mut mode, &["Size", "Percent", "Preset"]);
                });
                match mode {
                    0 => form_row(ui, pal, "Target size", "The copy aims for this file size.", |ui| {
                        let max_kb = if media.is_some() { crate::app::MEDIA_MAX_KB } else { 100_000 };
                        ui.add(egui::DragValue::new(&mut kb).range(10..=max_kb).clamp_existing_to_range(false).speed(10.0).suffix(" KB"));
                    }),
                    1 => form_row(ui, pal, "Reduction", "How much smaller the copy should be.", |ui| {
                        ui.add(
                            egui::DragValue::new(&mut percent).range(1..=95).clamp_existing_to_range(false).speed(0.5).suffix(" % smaller"),
                        );
                    }),
                    _ => form_row(ui, pal, "Preset", "Light keeps detail, Strong saves the most.", |ui| {
                        seg_index(ui, pal, &mut preset, &["Light", "Balanced", "Strong"]);
                    }),
                }
                // an audio file has nothing to resize
                if media.is_none_or(|m| m.video) {
                    form_row(ui, pal, "Resize", "Shrink the longest edge while compressing.", |ui| {
                        toggle(ui, pal, &mut resize);
                        ui.add_enabled(
                            resize,
                            egui::DragValue::new(&mut edge)
                                .range(64..=16_000)
                                .clamp_existing_to_range(false)
                                .speed(10.0)
                                .prefix("longest edge ")
                                .suffix(" px"),
                        );
                    });
                }
            });
            ui.add_space(10.0);
            let line = if let Some(m) = media {
                // bitrate arithmetic only (no encoding): what Enter would use, or why it cannot run
                match (&m.info, compress_media_job(m.video, orig_bytes, m.info.as_ref(), mode, kb, percent, preset, Some(edge), resize)) {
                    (None, _) => {
                        if m.loaded {
                            "The file could not be read".to_string()
                        } else {
                            "Reading the file…".to_string()
                        }
                    }
                    (Some(_), Err(e)) => e.trim_start_matches("Compress: ").to_string(),
                    (Some(i), Ok(ferrocast_core::convert::FileTool::Compress(goal, r))) => {
                        ferrocast_core::compress::plan(m.video, orig_bytes, i, goal, &r)
                            .map(|p| ferrocast_core::compress::describe(&p))
                            .unwrap_or_default()
                    }
                    _ => String::new(),
                }
            } else {
                match compress_goal(mode, kb, percent, preset, Some(edge), resize) {
                    Ok((goal, _)) if orig_bytes > 0 => {
                        let est = estimate_bytes(orig_bytes, goal);
                        let kbs = |b: u64| b.div_ceil(1024);
                        let share = est * 100 / orig_bytes;
                        let note = if resize { " · resizing may make it smaller" } else { "" };
                        format!("≈ about {} KB ({share} % of {} KB) · rough estimate, not measured{note}", kbs(est), kbs(orig_bytes))
                    }
                    Ok(_) => "Size of the original unknown".to_string(),
                    Err(_) => "Enter a value in range to see an estimate".to_string(),
                }
            };
            note(ui, pal, &line);
            ui.add_space(4.0);
            let nt = match media {
                None => "The copy is a JPEG next to the original; the original is never changed.",
                Some(m) if m.video => "The copy is an MP4 next to the original; the original is never changed.",
                Some(_) => "The copy keeps mp3, m4a, ogg or opus and is an m4a otherwise; the original is never changed.",
            };
            note(ui, pal, nt);
        });
        let edge_changed = edge != max_edge.unwrap_or(1600);
        if (mode, kb, percent, preset, resize) != (state.0, state.1, state.2, state.3, state.5) || edge_changed {
            if let Some(View::CompressImage { mode: m, kb: k, percent: p, preset: pr, max_edge: e, resize: r, .. }) = self.stack.last_mut()
            {
                (*m, *k, *p, *pr, *r) = (mode, kb, percent, preset, resize);
                if edge_changed {
                    *e = Some(edge);
                }
            }
            ctx.request_repaint();
        }
    }

    /// Edit Metadata window: rows grouped by file, track and chapter, a text box per editable row
    /// with a "clear" button, and a "Remove all metadata" switch that dims the rows.
    #[allow(clippy::too_many_arguments)]
    fn draw_edit_metadata(
        &mut self,
        ui: &mut Ui,
        area: Rect,
        pal: &Palette,
        file: &std::path::Path,
        kind: crate::app::MetaKind,
        original: Option<Vec<ferrocast_core::metaform::Field>>,
        mut working: Vec<ferrocast_core::metaform::Field>,
        mut remove_all: bool,
        error: Option<String>,
    ) {
        use ferrocast_core::metaform::{build_edits, Group};
        let ctx = ui.ctx().clone();
        let Some(original) = original else {
            let (msg, col) = match &error {
                Some(e) => (format!("Could not read the metadata: {e}"), pal.danger),
                None => ("Reading the metadata…".to_string(), pal.sub),
            };
            ui.painter().text(area.center(), egui::Align2::CENTER_CENTER, msg, FontId::proportional(15.0), col);
            return;
        };
        let gif = ferrocast_core::convert::ext(file) == "gif";
        let edits = if gif { vec![] } else { build_edits(&original, &working) };
        let (before, before_all) = (working.clone(), remove_all);
        let inner = form_rect(area);
        ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
            ui.set_clip_rect(inner.expand(2.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            let summary = if remove_all {
                "All tags and chapters will be removed.".to_string()
            } else {
                match edits.len() {
                    0 => "No changes yet.".to_string(),
                    1 => "1 change.".to_string(),
                    n => format!("{n} changes."),
                }
            };
            card(ui, pal, |ui| {
                form_row(ui, pal, "Remove all metadata", &summary, |ui| toggle(ui, pal, &mut remove_all));
            });
            ui.add_space(6.0);
            if remove_all {
                note(ui, pal, "Container brands and some muxer tags stay: the format itself writes them.");
            }
            if gif {
                note(ui, pal, "GIF files cannot store tags: you can only remove all metadata.");
            }
            if kind == crate::app::MetaKind::Pdf {
                note(ui, pal, "Creator and Producer are shown but not written back.");
            }
            ui.add_space(6.0);
            let list_h = (inner.max.y - ui.cursor().min.y - 22.0).max(40.0);
            let mut first_edit = true;
            egui::ScrollArea::vertical().id_salt("meta-rows").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
                ui.add_enabled_ui(!remove_all, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if working.is_empty() {
                        note(ui, pal, "This file has no metadata.");
                    }
                    // one card per group (file, track, chapter), fields in file order
                    let mut start = 0;
                    while start < working.len() {
                        let group = working[start].group.clone();
                        let end = working[start..].iter().position(|f| f.group != group).map_or(working.len(), |n| start + n);
                        let head = match &group {
                            Group::File => "File".to_string(),
                            Group::Track { index, kind } => format!("Track {} · {kind}", index + 1),
                            Group::Chapter(i) => format!("Chapter {}", i + 1),
                        };
                        ui.add_space(if start == 0 { 2.0 } else { 12.0 });
                        section(ui, pal, &head);
                        card(ui, pal, |ui| {
                            for f in working[start..end].iter_mut() {
                                let key = elide(ui.painter(), &f.key, FontId::proportional(13.5), 128.0);
                                form_row_compact(ui, pal, &key, 140.0, |ui| {
                                    if f.editable && !gif {
                                        if small_btn(ui, pal, "clear") {
                                            f.value.clear();
                                        }
                                        let r = ui
                                            .add(TextEdit::singleline(&mut f.value).desired_width(230.0).font(FontId::proportional(13.0)));
                                        if first_edit {
                                            first_edit = false;
                                            ctx.data_mut(|d| d.insert_temp(Id::new("meta-first-edit"), r.rect));
                                        }
                                    } else {
                                        let shown = elide(ui.painter(), &f.value, FontId::proportional(13.0), 270.0);
                                        ui.label(RichText::new(shown).size(13.0).color(pal.sub));
                                    }
                                });
                            }
                        });
                        start = end;
                    }
                });
            });
        });
        if working != before || remove_all != before_all {
            if let Some(View::EditMetadata { working: w, remove_all: r, .. }) = self.stack.last_mut() {
                (*w, *r) = (working, remove_all);
            }
            ctx.request_repaint();
        }
        note_line(ui, Pos2::new(area.center().x, area.max.y - 14.0), "edit a value or clear it", pal);
    }

    /// Speed window: presets, a logarithmic slider and the resulting length. Pitch is preserved.
    fn draw_video_speed(&mut self, ui: &mut Ui, area_in: Rect, pal: &Palette, duration: Option<f64>, factor: f64) {
        use ferrocast_core::videoplan::{sped_duration, SPEED_MAX, SPEED_MIN};
        let ctx = ui.ctx().clone();
        let Some(d) = duration else {
            ui.painter().text(area_in.center(), egui::Align2::CENTER_CENTER, "Reading the video…", FontId::proportional(16.0), pal.sub);
            return;
        };
        let mut f = factor;
        ui.scope_builder(UiBuilder::new().max_rect(form_rect(area_in)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            card(ui, pal, |ui| {
                form_row_stacked(ui, pal, "Speed", "Pick a preset, or drag the slider below.", |ui| {
                    const PRESETS: [f64; 7] = [0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0];
                    let labels = PRESETS.map(|p| format!("{p}×"));
                    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
                    let sel = PRESETS.iter().position(|p| (f - p).abs() < 1e-9);
                    if let (Some(i), _) = segmented_bar(ui, pal, sel, &labels, 56.0, 22.0) {
                        f = PRESETS[i];
                    }
                });
                form_row(ui, pal, "Fine tune", "Any speed from slow motion to fast forward.", |ui| {
                    ui.spacing_mut().slider_width = 170.0;
                    ui.add(egui::Slider::new(&mut f, SPEED_MIN..=SPEED_MAX).logarithmic(true).suffix("×"));
                });
                let len = sped_duration(d, f.clamp(SPEED_MIN, SPEED_MAX)).unwrap_or(d);
                form_row(ui, pal, "New length", "The video after the change.", |ui| {
                    ui.label(RichText::new(format!("{d:.1} s  →  {len:.1} s")).size(15.0).color(pal.text));
                });
            });
            ui.add_space(10.0);
            note(ui, pal, "The sound keeps its pitch. A video without sound just plays faster or slower.");
        });
        if (f - factor).abs() > 1e-12 {
            if let Some(View::VideoSpeed { factor: x, .. }) = self.stack.last_mut() {
                *x = f.clamp(SPEED_MIN, SPEED_MAX);
            }
            ctx.request_repaint();
        }
        note_line(ui, Pos2::new(area_in.center().x, area_in.max.y - 14.0), "pick a preset or drag the slider", pal);
    }

    /// Normalize window: measured loudness versus the target, with loudness, range and true-peak controls.
    #[allow(clippy::too_many_arguments)]
    fn draw_normalize_audio(&mut self, ui: &mut Ui, area: Rect, pal: &Palette, loudness: Option<f64>, target: f64, range: f64, peak: f64) {
        let ctx = ui.ctx().clone();
        let (mut t, mut r, mut p) = (target, range, peak);
        ui.scope_builder(UiBuilder::new().max_rect(form_rect(area)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            card(ui, pal, |ui| {
                form_row(ui, pal, "Target loudness", "The level the copy is levelled to.", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut t)
                            .range(-30.0..=-5.0)
                            .clamp_existing_to_range(false)
                            .speed(0.1)
                            .suffix(" LUFS")
                            .max_decimals(1),
                    );
                });
                form_row(ui, pal, "Preset", "Broadcast, podcast or music levels.", |ui| {
                    const GOALS: [(f64, &str); 3] = [(-23.0, "-23 broadcast"), (-16.0, "-16 podcast"), (-14.0, "-14 music")];
                    let sel = GOALS.iter().position(|(v, _)| (t - v).abs() < 1e-9);
                    if let (Some(i), _) = segmented_bar(ui, pal, sel, &GOALS.map(|(_, n)| n), 0.0, 16.0) {
                        t = GOALS[i].0;
                    }
                });
                form_row(ui, pal, "Loudness range", "How much the level may vary.", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut r)
                            .range(1.0..=20.0)
                            .clamp_existing_to_range(false)
                            .speed(0.1)
                            .suffix(" LU")
                            .max_decimals(1),
                    );
                });
                form_row(ui, pal, "True peak", "The loudest peak allowed.", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut p)
                            .range(-9.0..=0.0)
                            .clamp_existing_to_range(false)
                            .speed(0.1)
                            .suffix(" dBTP")
                            .max_decimals(1),
                    );
                });
            });
            ui.add_space(10.0);
            note(ui, pal, &crate::media_tools::normalize_line(loudness, t));
            ui.add_space(4.0);
            note(
                ui,
                pal,
                "The copy is levelled to the target; the original stays untouched. The result is measured again when it is done.",
            );
        });
        if (t, r, p) != (target, range, peak) {
            if let Some(View::NormalizeAudio { target, range, peak, .. }) = self.stack.last_mut() {
                (*target, *range, *peak) = (t, r, p);
            }
            ctx.request_repaint();
        }
        note_line(ui, Pos2::new(area.center().x, area.max.y - 14.0), "pick a target or type one", pal);
    }

    /// Channels window: mono or stereo output and the volume of each side in dB.
    #[allow(clippy::too_many_arguments)]
    fn draw_channels_audio(
        &mut self,
        ui: &mut Ui,
        area: Rect,
        pal: &Palette,
        in_channels: Option<u32>,
        stereo: bool,
        left_db: f64,
        right_db: f64,
    ) {
        let ctx = ui.ctx().clone();
        let (mut st, mut l, mut r) = (stereo, left_db, right_db);
        ui.scope_builder(UiBuilder::new().max_rect(form_rect(area)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            card(ui, pal, |ui| {
                form_row(ui, pal, "Output", "Mono mixes both sides into one channel.", |ui| {
                    segmented(ui, pal, &mut st, &[(false, "Mono"), (true, "Stereo")]);
                });
                form_row(ui, pal, "Left volume", "Gain of the left channel.", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut l)
                            .range(-30.0..=12.0)
                            .clamp_existing_to_range(false)
                            .speed(0.1)
                            .suffix(" dB")
                            .max_decimals(1),
                    );
                });
                form_row(ui, pal, "Right volume", "Gain of the right channel.", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut r)
                            .range(-30.0..=12.0)
                            .clamp_existing_to_range(false)
                            .speed(0.1)
                            .suffix(" dB")
                            .max_decimals(1),
                    );
                });
                form_row(ui, pal, "Reset", "Back to 0 dB on both sides.", |ui| {
                    if small_btn(ui, pal, "reset") {
                        (l, r) = (0.0, 0.0);
                    }
                });
            });
            ui.add_space(10.0);
            let msg = match (in_channels, st) {
                (Some(n), _) if n <= 1 && st => "This file is already mono: Stereo copies it to both sides, each with its own volume.",
                (Some(n), _) if n <= 1 => "This file is already mono: Mono keeps it as one channel at the average of both volumes.",
                (_, true) => "Stereo keeps the left and right channels, each at its own volume.",
                _ => "Mono mixes left and right into one channel; each volume scales its own side.",
            };
            note(ui, pal, if in_channels.is_none() { "Reading the file…" } else { msg });
            ui.add_space(4.0);
            note(ui, pal, "0 dB leaves a channel unchanged; the copy is saved as a new file.");
        });
        if (st, l, r) != (stereo, left_db, right_db) {
            if let Some(View::ChannelsAudio { stereo, left_db, right_db, .. }) = self.stack.last_mut() {
                (*stereo, *left_db, *right_db) = (st, l, r);
            }
            ctx.request_repaint();
        }
        note_line(ui, Pos2::new(area.center().x, area.max.y - 14.0), "pick Mono or Stereo, set the volumes", pal);
    }

    /// Audio to video window: waveform or still picture, and the canvas shape. No number or text
    /// field, so it needs no entry in the kbd_edit list.
    #[allow(clippy::too_many_arguments)]
    fn draw_visualize_audio(
        &mut self,
        ui: &mut Ui,
        area: Rect,
        pal: &Palette,
        file: &std::path::Path,
        orient: ferrocast_core::media::Canvas,
        style: crate::media_tools::VisStyle,
        image: Option<std::path::PathBuf>,
        picking: bool,
    ) {
        use crate::media_tools::VisStyle;
        use ferrocast_core::media::Canvas;
        let ctx = ui.ctx().clone();
        let (mut o, mut st) = (orient, style);
        let mut choose = false;
        let mut rects = [Rect::NOTHING; 5];
        ui.scope_builder(UiBuilder::new().max_rect(form_rect(area)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(
                RichText::new(file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).size(12.5).color(pal.sub),
            );
            ui.add_space(8.0);
            card(ui, pal, |ui| {
                form_row(ui, pal, "Picture", "A moving waveform, or one still image.", |ui| {
                    let sel = [VisStyle::Waveform, VisStyle::Still].iter().position(|v| *v == st);
                    let (hit, cells) = segmented_bar(ui, pal, sel, &["Waveform", "Still image"], 56.0, 22.0);
                    if let Some(i) = hit {
                        st = [VisStyle::Waveform, VisStyle::Still][i];
                    }
                    rects[..2].copy_from_slice(&cells);
                });
                form_row(ui, pal, "Shape", "The canvas of the video.", |ui| {
                    let all = [Canvas::Landscape, Canvas::Portrait, Canvas::Square];
                    let (hit, cells) =
                        segmented_bar(ui, pal, all.iter().position(|v| *v == o), &["Landscape", "Portrait", "Square"], 56.0, 22.0);
                    if let Some(i) = hit {
                        o = all[i];
                    }
                    rects[2..].copy_from_slice(&cells);
                });
                if st == VisStyle::Still {
                    form_row(ui, pal, "Image", "The picture shown while the audio plays.", |ui| {
                        choose =
                            ui.add_enabled(!picking, egui::Button::new(if picking { "Choosing…" } else { "Choose picture…" })).clicked();
                        let (text, color) = match &image {
                            Some(p) if p.is_file() => {
                                (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), pal.text)
                            }
                            Some(p) => (format!("{} (not found)", p.display()), pal.sub),
                            None => ("none yet".to_string(), pal.sub),
                        };
                        let text = elide(ui.painter(), &text, FontId::proportional(12.5), 120.0);
                        ui.label(RichText::new(text).size(12.5).color(color));
                    });
                }
            });
            ui.add_space(10.0);
            note(ui, pal, &crate::media_tools::visualize_line(o, st));
            ui.add_space(4.0);
            note(ui, pal, "The video is saved as a new MP4 next to the audio; nothing is overwritten.");
        });
        ctx.data_mut(|d| d.insert_temp(Id::new("avis-rects"), rects));
        if choose && !picking {
            let (bus, f) = (self.bus.clone(), file.to_path_buf());
            std::thread::spawn(move || bus.send(Msg::VisualPicked(f, services::choose_picture_file())));
        }
        if (o, st) != (orient, style) || (choose && !picking) {
            if let Some(View::VisualizeAudio { orient, style, picking, .. }) = self.stack.last_mut() {
                (*orient, *style) = (o, st);
                if choose {
                    *picking = true;
                }
            }
            ctx.request_repaint();
        }
        note_line(ui, Pos2::new(area.center().x, area.max.y - 14.0), "pick the picture and the shape", pal);
    }

    /// Video trim viewer: the frame at the cursor, a timeline with start and end markers, and
    /// stepping keys. Frames are extracted on a worker thread, one at a time, newest wins.
    fn draw_video_trim(
        &mut self,
        ui: &mut Ui,
        area_in: Rect,
        pal: &Palette,
        file: &std::path::Path,
        trim: Option<ferrocast_core::videoplan::Trim>,
    ) {
        let ctx = ui.ctx().clone();
        let Some(before) = trim else {
            ui.painter().text(area_in.center(), egui::Align2::CENTER_CENTER, "Reading the video…", FontId::proportional(16.0), pal.sub);
            return;
        };
        let snapshot = matches!(self.view(), View::VideoSnapshot { .. });
        let (split, points0, parts0) = match self.view() {
            View::VideoSplit { points, parts, .. } => (true, points.clone(), *parts),
            _ => (false, vec![], 2),
        };
        let (mut points, mut parts) = (points0.clone(), parts0);
        let mut t = before;
        // a number box being edited keeps the letters, arrows and Backspace
        let editing = split && ctx.egui_wants_keyboard_input();
        ctx.input_mut(|i| {
            let none = Modifiers::NONE;
            if editing {
                return;
            }
            if split {
                if i.consume_key(none, Key::M) {
                    t.add_cut(&mut points);
                }
                if i.consume_key(none, Key::Backspace) {
                    points.pop();
                }
            }
            for (key, frames) in [(Key::ArrowLeft, -1i64), (Key::ArrowRight, 1)] {
                if i.consume_key(Modifiers::SHIFT, key) {
                    t.step_secs(frames as f64);
                } else if i.consume_key(none, key) {
                    t.step_frames(frames);
                }
            }
            if !snapshot && i.consume_key(none, Key::I) {
                t.mark_start();
            }
            if !snapshot && i.consume_key(none, Key::O) {
                t.mark_end();
            }
            if i.consume_key(none, Key::Home) {
                t.seek(t.start);
            }
            if i.consume_key(none, Key::End) {
                t.seek((t.end - t.frame()).max(0.0));
            }
        });
        let bar = Rect::from_min_size(Pos2::new(area_in.min.x + 24.0, area_in.max.y - 92.0), Vec2::new(area_in.width() - 48.0, 26.0));
        let resp = ui.interact(bar, Id::new("vt-bar"), egui::Sense::click_and_drag());
        if let (true, Some(p)) = (resp.clicked() || resp.dragged(), resp.interact_pointer_pos()) {
            t.seek(((p.x - bar.min.x) / bar.width()).clamp(0.0, 1.0) as f64 * t.duration);
        }
        let x_of = |s: f64| bar.min.x + (s / t.duration) as f32 * bar.width();
        ui.painter().rect_filled(bar, 6.0, pal.field);
        if split {
            for p in &points {
                ui.painter().rect_filled(
                    Rect::from_center_size(Pos2::new(x_of(*p), bar.center().y), Vec2::new(3.0, 34.0)),
                    1.5,
                    pal.accent,
                );
            }
        } else {
            let sel = Rect::from_min_max(Pos2::new(x_of(t.start), bar.min.y), Pos2::new(x_of(t.end), bar.max.y));
            ui.painter().rect_filled(sel, 6.0, pal.accent.gamma_multiply(0.45));
            for m in [t.start, t.end] {
                ui.painter().rect_filled(Rect::from_center_size(Pos2::new(x_of(m), bar.center().y), Vec2::new(4.0, 34.0)), 2.0, pal.accent);
            }
        }
        ui.painter().rect_filled(Rect::from_center_size(Pos2::new(x_of(t.cursor), bar.center().y), Vec2::new(2.0, 34.0)), 1.0, pal.text);
        // newest frame wanted: ask for it unless one is still being extracted
        let busy: bool = ctx.data(|d| d.get_temp(Id::new("vt-busy"))).unwrap_or(false);
        let asked: Option<f64> = ctx.data(|d| d.get_temp(Id::new("vt-asked")));
        if !busy && asked != Some(t.cursor) {
            if let Some(ff) = self.tools.ffmpeg {
                ctx.data_mut(|d| {
                    d.insert_temp(Id::new("vt-busy"), true);
                    d.insert_temp(Id::new("vt-asked"), t.cursor);
                });
                let (bus, f, at) = (self.bus.clone(), file.to_path_buf(), t.cursor);
                std::thread::spawn(move || bus.send(Msg::VideoFrame(f.clone(), at, crate::media_tools::preview_frame(ff, &f, at))));
            }
        }
        let view = Rect::from_min_max(area_in.min + Vec2::new(24.0, 16.0), Pos2::new(area_in.max.x - 24.0, bar.min.y - 40.0));
        match ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("vt-tex"))) {
            Some(tex) => {
                let sz = tex.size_vec2();
                let k = (view.width() / sz.x).min(view.height() / sz.y);
                egui::Image::new(&tex).paint_at(ui, Rect::from_center_size(view.center(), sz * k));
            }
            None => {
                ui.painter().text(view.center(), egui::Align2::CENTER_CENTER, "Loading frame…", FontId::proportional(16.0), pal.sub);
            }
        }
        ui.painter().text(
            Pos2::new(bar.min.x, bar.min.y - 18.0),
            egui::Align2::LEFT_CENTER,
            if snapshot {
                format!("at {} · PNG saves at original resolution", t.label(t.cursor))
            } else if split {
                format!("at {}   ·   {} cut{}", t.label(t.cursor), points.len(), if points.len() == 1 { "" } else { "s" })
            } else {
                format!(
                    "at {}   ·   start {}   ·   end {}   ·   clip {:.2} s",
                    t.label(t.cursor),
                    t.label(t.start),
                    t.label(t.end),
                    t.clip()
                )
            },
            FontId::proportional(12.5),
            pal.text,
        );
        note_line(ui, Pos2::new(area_in.center().x, area_in.max.y - 14.0), "click the bar to jump", pal);
        if split {
            let clips = ferrocast_core::videoplan::split_ranges(t.duration, &points, parts).len();
            let row = Rect::from_min_size(Pos2::new(bar.min.x, bar.max.y + 8.0), Vec2::new(bar.width(), 24.0));
            ui.scope_builder(UiBuilder::new().max_rect(row), |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{clips} clip{}", if clips == 1 { "" } else { "s" })).size(13.0).color(pal.text));
                    ui.label(RichText::new("·  Equal parts").size(12.5).color(pal.sub));
                    ui.add(egui::DragValue::new(&mut parts).range(2..=50));
                    ui.label(
                        RichText::new(if points.is_empty() { "(used while there are no cuts)" } else { "(ignored: cuts are set)" })
                            .size(11.5)
                            .color(pal.sub),
                    );
                    if ui.add_enabled(!points.is_empty(), egui::Button::new("Clear")).clicked() {
                        points.clear();
                    }
                });
            });
        }
        if t != before || points != points0 || parts != parts0 {
            if let Some(View::VideoTrim { trim, .. } | View::VideoSnapshot { trim, .. } | View::VideoSplit { trim, .. }) =
                self.stack.last_mut()
            {
                *trim = Some(t);
            }
            if let Some(View::VideoSplit { points: p, parts: n, .. }) = self.stack.last_mut() {
                (*p, *n) = (points, parts.clamp(2, 50));
            }
            ctx.request_repaint();
        }
    }

    /// Audio trim window: the waveform with the selected range, markers and cursor, exact Start
    /// and End boxes and a "Trim silence" button. Samples are read off the UI thread.
    fn draw_audio_trim(
        &mut self,
        ui: &mut Ui,
        area_in: Rect,
        pal: &Palette,
        trim: Option<ferrocast_core::videoplan::Trim>,
        audio: Option<std::sync::Arc<crate::media_tools::AudioSamples>>,
    ) {
        let ctx = ui.ctx().clone();
        let (Some(before), Some(audio)) = (trim, audio) else {
            ui.painter().text(area_in.center(), egui::Align2::CENTER_CENTER, "Reading the audio…", FontId::proportional(16.0), pal.sub);
            return;
        };
        let mut t = before;
        // a number box being edited keeps the letters and arrows
        let editing = ctx.egui_wants_keyboard_input();
        ctx.input_mut(|i| {
            let none = Modifiers::NONE;
            if editing {
                return;
            }
            // Shift first: egui matches Modifiers::NONE even while Shift is held
            for (key, steps) in [(Key::ArrowLeft, -1i64), (Key::ArrowRight, 1)] {
                if i.consume_key(Modifiers::SHIFT, key) {
                    t.step_secs(steps as f64);
                } else if i.consume_key(none, key) {
                    t.step_frames(steps);
                }
            }
            if i.consume_key(none, Key::I) {
                t.mark_start();
            }
            if i.consume_key(none, Key::O) {
                t.mark_end();
            }
            if i.consume_key(none, Key::Home) {
                t.seek(t.start);
            }
            if i.consume_key(none, Key::End) {
                t.seek((t.end - t.frame()).max(0.0));
            }
        });
        let wave = Rect::from_min_max(area_in.min + Vec2::new(24.0, 16.0), Pos2::new(area_in.max.x - 24.0, area_in.max.y - 150.0));
        let resp = ui.interact(wave, Id::new("at-wave"), egui::Sense::click_and_drag());
        if let (true, Some(p)) = (resp.clicked() || resp.dragged(), resp.interact_pointer_pos()) {
            t.seek(((p.x - wave.min.x) / wave.width()).clamp(0.0, 1.0) as f64 * t.duration);
        }
        let x_of = |s: f64| wave.min.x + (s / t.duration) as f32 * wave.width();
        let painter = ui.painter();
        painter.rect_filled(wave, 6.0, pal.field);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x_of(t.start), wave.min.y), Pos2::new(x_of(t.end), wave.max.y)),
            0.0,
            pal.accent.gamma_multiply(0.22),
        );
        // at most 800 buckets, recomputed only when the samples or the width change
        let buckets = (wave.width() as usize).clamp(1, 800);
        let key = (std::sync::Arc::as_ptr(&audio) as usize, audio.samples.len(), buckets);
        let cached: Option<crate::media_tools::PeaksCache> = ctx.data(|d| d.get_temp(Id::new("at-peaks")));
        let bars = match cached {
            Some((k, p)) if k == key => p,
            _ => {
                let p = std::sync::Arc::new(ferrocast_core::waveform::peaks(&audio.samples, buckets));
                ctx.data_mut(|d| d.insert_temp(Id::new("at-peaks"), (key, p.clone())));
                p
            }
        };
        // the samples may cover only the start of a long file
        let covered = (audio.samples.len() as f64 / audio.rate.max(1) as f64 / t.duration).clamp(0.0, 1.0) as f32;
        let (mid, half) = (wave.center().y, wave.height() / 2.0 - 4.0);
        for (i, (lo, hi)) in bars.iter().enumerate() {
            let frac = (i as f32 + 0.5) / bars.len() as f32;
            let x = wave.min.x + frac * covered * wave.width();
            let inside = (x_of(t.start)..=x_of(t.end)).contains(&x);
            let top = (mid - *hi as f32 / 32768.0 * half).min(mid - 0.5);
            let bottom = (mid - *lo as f32 / 32768.0 * half).max(mid + 0.5);
            painter.line_segment([Pos2::new(x, top), Pos2::new(x, bottom)], Stroke::new(1.0, if inside { pal.text } else { pal.sub }));
        }
        for m in [t.start, t.end] {
            painter.rect_filled(
                Rect::from_center_size(Pos2::new(x_of(m), wave.center().y), Vec2::new(3.0, wave.height())),
                1.5,
                pal.accent,
            );
        }
        painter.rect_filled(
            Rect::from_center_size(Pos2::new(x_of(t.cursor), wave.center().y), Vec2::new(2.0, wave.height())),
            1.0,
            pal.text,
        );
        let note = if audio.truncated {
            format!("   ·   waveform covers the first {:.0} min", audio.samples.len() as f64 / audio.rate.max(1) as f64 / 60.0)
        } else {
            String::new()
        };
        painter.text(
            Pos2::new(wave.min.x, wave.max.y + 14.0),
            egui::Align2::LEFT_CENTER,
            format!(
                "at {}   ·   start {}   ·   end {}   ·   clip {:.2} s{note}",
                t.label(t.cursor),
                t.label(t.start),
                t.label(t.end),
                t.clip()
            ),
            FontId::proportional(12.5),
            pal.text,
        );
        note_line(ui, Pos2::new(area_in.center().x, area_in.max.y - 14.0), "click the waveform to jump", pal);
        let (mut start, mut end, mut silence) = (t.start, t.end, false);
        let row = Rect::from_min_size(Pos2::new(wave.min.x, wave.max.y + 30.0), Vec2::new(wave.width(), 26.0));
        ui.scope_builder(UiBuilder::new().max_rect(row), |ui| {
            ui.horizontal(|ui| {
                ui.label("Start s");
                ui.add(egui::DragValue::new(&mut start).range(0.0..=t.duration).speed(0.01).fixed_decimals(2));
                ui.label("End s");
                ui.add(egui::DragValue::new(&mut end).range(0.0..=t.duration).speed(0.01).fixed_decimals(2));
                silence = ui.button("Trim silence").clicked();
            });
        });
        if (start - t.start).abs() > 1e-9 {
            t.set_start(start);
        } else if (end - t.end).abs() > 1e-9 {
            t.set_end(end);
        }
        if silence {
            match ferrocast_core::waveform::silence_bounds(&audio.samples, audio.rate, -40.0) {
                Some((a, b)) if t.set_range(a, b) => {}
                _ => self.toast("No silence to trim"),
            }
        }
        if t != before {
            if let Some(View::AudioTrim { trim, .. }) = self.stack.last_mut() {
                *trim = Some(t);
            }
            ctx.request_repaint();
        }
    }

    /// Bleep window: the ranges on the left (add, select, delete), the waveform with every range
    /// shaded (the selected one stronger) and the selected range's Start and End boxes.
    fn draw_bleep(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette) {
        let View::BleepAudio { duration, peaks: Some(peaks), ranges, sel, .. } = self.view().clone() else {
            return;
        };
        if ranges.is_empty() {
            // cannot happen once the audio is read; keep the window usable if it ever does
            if let Some(View::BleepAudio { ranges, sel, .. }) = self.stack.last_mut() {
                (*ranges, *sel) = (vec![crate::app::new_bleep_range(duration)], 0);
            }
            ui.ctx().request_repaint();
            return;
        }
        let sel_now = sel.min(ranges.len() - 1);
        let mut ranges_new = ranges.clone();
        let mut sel_new = sel_now;
        // left column: "+ Add range" first (fixed place), then one row per range
        let col = Rect::from_min_size(rect.min + Vec2::new(12.0, 8.0), Vec2::new(206.0, rect.height() - 16.0));
        let full = ranges.len() >= crate::app::MAX_BLEEP_RANGES;
        let add = Rect::from_min_size(col.min, Vec2::new(col.width(), 26.0));
        let add_resp = ui.interact(add, Id::new("bleep-add"), if full { egui::Sense::hover() } else { egui::Sense::click() });
        ui.painter().rect_filled(add, 6.0, if add_resp.hovered() && !full { pal.sel } else { pal.field });
        ui.painter().text(
            add.center(),
            egui::Align2::CENTER_CENTER,
            if full { format!("Limit of {} ranges", crate::app::MAX_BLEEP_RANGES) } else { "+ Add range".to_string() },
            FontId::proportional(13.0),
            if full { pal.sub } else { pal.text },
        );
        if add_resp.clicked() && !full {
            ranges_new.push(crate::app::new_bleep_range(duration));
            sel_new = ranges_new.len() - 1;
        }
        let mut delete = None;
        let can_delete = ranges.len() > 1;
        for (i, (a, b)) in ranges.iter().enumerate() {
            let row = Rect::from_min_size(col.min + Vec2::new(0.0, 34.0 + 30.0 * i as f32), Vec2::new(col.width(), 26.0));
            if row.max.y > rect.max.y - 4.0 {
                break;
            }
            let label_rect = Rect::from_min_max(row.min, Pos2::new(row.max.x - 26.0, row.max.y));
            let del_rect = Rect::from_min_max(Pos2::new(row.max.x - 26.0, row.min.y), row.max);
            let resp = ui.interact(label_rect, Id::new(("bleep-row", i)), egui::Sense::click());
            let del =
                ui.interact(del_rect, Id::new(("bleep-del", i)), if can_delete { egui::Sense::click() } else { egui::Sense::hover() });
            if i == sel_now {
                ui.painter().rect_filled(row, 6.0, pal.sel);
                ui.painter().rect_stroke(row, 6.0, Stroke::new(1.0, pal.accent), StrokeKind::Inside);
            } else if resp.hovered() || del.hovered() {
                ui.painter().rect_filled(row, 6.0, pal.field);
            }
            ui.painter().text(
                label_rect.left_center() + Vec2::new(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("Range {} · {}–{}", i + 1, crate::app::clock_tenths(*a), crate::app::clock_tenths(*b)),
                FontId::proportional(12.0),
                pal.text,
            );
            let x_color = if !can_delete {
                pal.sep
            } else if del.hovered() {
                pal.danger
            } else {
                pal.sub
            };
            let c = del_rect.center();
            ui.painter().line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], Stroke::new(1.5, x_color));
            ui.painter().line_segment([c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)], Stroke::new(1.5, x_color));
            if del.clicked() && can_delete {
                delete = Some(i);
            } else if resp.clicked() {
                sel_new = i;
            }
        }
        if let Some(i) = delete {
            ranges_new.remove(i);
            sel_new = if sel_new > i || sel_new >= ranges_new.len() { sel_new.saturating_sub(1) } else { sel_new };
        }
        // right side: Start and End of the selected range, then the waveform
        let right = Rect::from_min_max(Pos2::new(rect.min.x + 232.0, rect.min.y), rect.max);
        if sel_new == sel_now && sel_now < ranges_new.len() {
            let (mut a, mut b) = ranges_new[sel_now];
            let strip = Rect::from_min_size(right.min + Vec2::new(12.0, 4.0), Vec2::new(right.width() - 24.0, 30.0));
            ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Range {}", sel_now + 1));
                    ui.label("Start s");
                    ui.add(egui::DragValue::new(&mut a).range(0.0..=duration).speed(0.02).fixed_decimals(2));
                    ui.label("End s");
                    ui.add(egui::DragValue::new(&mut b).range(0.0..=duration).speed(0.02).fixed_decimals(2));
                });
            });
            ranges_new[sel_now] = (a, b);
        }
        let wave = Rect::from_min_max(right.min + Vec2::new(12.0, 46.0), Pos2::new(right.max.x - 12.0, right.max.y - 56.0));
        if wave.width() > 8.0 && wave.height() > 8.0 && duration > 0.0 {
            let x_of = |s: f64| wave.min.x + (s / duration).clamp(0.0, 1.0) as f32 * wave.width();
            let resp = ui.interact(wave, Id::new("bleep-wave"), egui::Sense::click());
            if let (true, Some(p)) = (resp.clicked(), resp.interact_pointer_pos()) {
                // clicking inside a range selects it
                let t = ((p.x - wave.min.x) / wave.width()).clamp(0.0, 1.0) as f64 * duration;
                if let Some(i) = ranges_new.iter().position(|(a, b)| (*a..=*b).contains(&t)) {
                    sel_new = i;
                }
            }
            let painter = ui.painter();
            painter.rect_filled(wave, 6.0, pal.field);
            for (i, (a, b)) in ranges_new.iter().enumerate() {
                let (lo, hi) = (a.min(*b), a.max(*b));
                let strong = i == sel_new.min(ranges_new.len() - 1);
                let r = Rect::from_min_max(Pos2::new(x_of(lo), wave.min.y), Pos2::new(x_of(hi).max(x_of(lo) + 1.0), wave.max.y));
                painter.rect_filled(r, 0.0, pal.accent.gamma_multiply(if strong { 0.45 } else { 0.2 }));
                if strong {
                    painter.rect_stroke(r, 0.0, Stroke::new(1.5, pal.accent), StrokeKind::Inside);
                }
                painter.text(
                    r.min + Vec2::new(3.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}", i + 1),
                    FontId::proportional(11.0),
                    pal.text,
                );
            }
            let (mid, half) = (wave.center().y, wave.height() / 2.0 - 4.0);
            for (i, (lo, hi)) in peaks.iter().enumerate() {
                let x = wave.min.x + (i as f32 + 0.5) / peaks.len() as f32 * wave.width();
                let top = (mid - *hi as f32 / 32768.0 * half).min(mid - 0.5);
                let bottom = (mid - *lo as f32 / 32768.0 * half).max(mid + 0.5);
                painter.line_segment([Pos2::new(x, top), Pos2::new(x, bottom)], Stroke::new(1.0, pal.text));
            }
            painter.text(
                Pos2::new(wave.min.x, wave.max.y + 14.0),
                egui::Align2::LEFT_CENTER,
                format!(
                    "{} range{} · clip {}",
                    ranges_new.len(),
                    if ranges_new.len() == 1 { "" } else { "s" },
                    crate::app::clock_tenths(duration)
                ),
                FontId::proportional(12.5),
                pal.text,
            );
        }
        note_line(
            ui,
            Pos2::new(right.center().x, rect.max.y - 14.0),
            "click a shaded range to select it · overlapping ranges are merged",
            pal,
        );
        if ranges_new != ranges || sel_new != sel {
            if let Some(View::BleepAudio { ranges, sel, .. }) = self.stack.last_mut() {
                (*ranges, *sel) = (ranges_new, sel_new);
            }
            ui.ctx().request_repaint();
        }
    }

    /// Collage editor: layout buttons, width/gap/corner sliders and a background; the preview is
    /// laid out on small copies of the pictures with the sizes scaled to match.
    fn draw_collage(
        &mut self,
        ui: &mut Ui,
        area_in: Rect,
        pal: &Palette,
        files: Vec<PathBuf>,
        opts: ferrocast_core::collage::CollageOpts,
        color: usize,
    ) {
        use ferrocast_core::collage::{layout, Layout};
        let n = files.len();
        let ctx = ui.ctx().clone();
        let (mut o, mut color) = (opts, color);
        let panel = Rect::from_min_max(Pos2::new(area_in.max.x - 300.0, area_in.min.y + 12.0), area_in.max - Vec2::new(16.0, 34.0));
        ui.scope_builder(UiBuilder::new().max_rect(panel), |ui| {
            ui.spacing_mut().slider_width = 96.0;
            ui.spacing_mut().item_spacing.y = 0.0;
            card(ui, pal, |ui| {
                form_row_stacked(ui, pal, "Layout", "How the pictures are arranged.", |ui| {
                    // the side card is 256 pt wide inside: a tight padding keeps the four cells in it
                    let all = [Layout::Grid, Layout::Row, Layout::Column, Layout::Featured];
                    let sel = all.iter().position(|l| *l == o.layout);
                    if let (Some(i), _) = segmented_bar(ui, pal, sel, &["Grid", "Row", "Column", "Featured"], 0.0, 8.0) {
                        o.layout = all[i];
                    }
                });
                let mut auto = o.width.is_none();
                form_row_compact(ui, pal, "Auto width", 110.0, |ui| {
                    if toggle_changed(ui, pal, &mut auto) {
                        o.width = if auto { None } else { Some(1600) };
                    }
                });
                if let Some(w) = o.width.as_mut() {
                    form_row_compact(ui, pal, "Width", 62.0, |ui| {
                        ui.add(egui::Slider::new(w, 200..=6000));
                    });
                }
                for (label, v, max) in [("Spacing", &mut o.gap, 100u32), ("Corners", &mut o.radius, 300)] {
                    form_row_compact(ui, pal, label, 62.0, |ui| {
                        ui.add(egui::Slider::new(v, 0..=max));
                    });
                }
                form_row_stacked(ui, pal, "Background", "Colour behind the pictures.", |ui| {
                    for (i, k) in ferrocast_core::annotate::COLORS.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                        swatch_dot(ui.painter(), r.center(), egui::Color32::from_rgb(k[0], k[1], k[2]), pal);
                        if o.bg == Some(*k) && i == color {
                            ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(1.5, pal.text));
                        }
                        if resp.clicked() {
                            color = i;
                            o.bg = Some(*k);
                        }
                    }
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                    swatch_dot(ui.painter(), r.center(), egui::Color32::WHITE, pal);
                    if resp.clicked() {
                        o.bg = Some([255, 255, 255]);
                    }
                    if ui.selectable_label(o.bg.is_none(), "None").clicked() {
                        o.bg = None;
                    }
                });
            });
        });
        if o != opts {
            if let Some(View::CollageImage { opts: p, color: c, .. }) = self.stack.last_mut() {
                (*p, *c) = (o, color);
            }
            ctx.request_repaint();
        }
        let mut area = Rect::from_min_max(area_in.min + Vec2::new(16.0, 14.0), Pos2::new(area_in.max.x - 316.0, area_in.max.y - 34.0));
        let base: Option<std::sync::Arc<Vec<image::RgbaImage>>> = ctx.data(|d| d.get_temp(Id::new("collage-base")));
        if n >= 2 {
            // one small chip per picture, draggable: the order here is the order of the collage
            let strip = Rect::from_min_size(area.min, Vec2::new(area.width(), 70.0));
            area.min.y += 78.0;
            let chips: Option<std::sync::Arc<Vec<egui::TextureHandle>>> =
                match (&base, ctx.data(|d| d.get_temp::<std::sync::Arc<Vec<egui::TextureHandle>>>(Id::new("collage-strip")))) {
                    (_, Some(c)) if c.len() == n => Some(c),
                    (Some(b), _) if b.len() == n => {
                        let v: Vec<egui::TextureHandle> = b
                            .iter()
                            .enumerate()
                            .map(|(i, img)| {
                                let ci =
                                    egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                                ctx.load_texture(format!("collage-chip-{i}"), ci, egui::TextureOptions::LINEAR)
                            })
                            .collect();
                        let v = std::sync::Arc::new(v);
                        ctx.data_mut(|d| d.insert_temp(Id::new("collage-strip"), v.clone()));
                        Some(v)
                    }
                    _ => None,
                };
            let mut dropped = None;
            ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
                egui::ScrollArea::horizontal().id_salt("collage-strip-scroll").auto_shrink([false, true]).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        dropped = drag_reorder_cells(
                            ui,
                            "collage-strip",
                            n,
                            true,
                            pal,
                            |ui, i, lifted| {
                                let (r, _) = ui.allocate_exact_size(Vec2::new(64.0, 52.0), egui::Sense::hover());
                                ui.painter().rect_filled(r, 8.0, if lifted { pal.sel } else { pal.field });
                                match chips.as_ref().and_then(|c| c.get(i)) {
                                    Some(t) => {
                                        let s = t.size_vec2();
                                        let k = ((r.width() - 8.0) / s.x).min((r.height() - 8.0) / s.y);
                                        egui::Image::new(t).paint_at(ui, Rect::from_center_size(r.center(), s * k));
                                    }
                                    None => {
                                        let name = files[i].file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                                        let p = ui.painter().with_clip_rect(r.shrink(3.0));
                                        p.text(r.center(), egui::Align2::CENTER_CENTER, name, FontId::proportional(10.5), pal.sub);
                                    }
                                }
                                // the index sits on a small dark pill fully inside the chip
                                let galley = ui.painter().layout_no_wrap((i + 1).to_string(), FontId::proportional(10.5), Color32::WHITE);
                                let pill = Rect::from_min_size(
                                    Pos2::new(r.min.x + 4.0, r.max.y - 4.0 - (galley.size().y + 2.0)),
                                    galley.size() + Vec2::new(8.0, 2.0),
                                );
                                ui.painter().rect_filled(pill, pill.height() / 2.0, Color32::from_black_alpha(190));
                                ui.painter().galley(pill.center() - galley.size() / 2.0, galley, Color32::WHITE);
                                if lifted {
                                    ui.painter().rect_stroke(r, 8.0, Stroke::new(1.5, pal.accent), StrokeKind::Inside);
                                }
                            },
                            |_, _, _| {},
                        )
                        .moved;
                    });
                });
            });
            if let Some((from, to)) = dropped {
                use ferrocast_core::reorder::move_item;
                if let Some(View::CollageImage { files: f, .. }) = self.stack.last_mut() {
                    move_item(f, from, to);
                }
                ctx.data_mut(|d| {
                    if let Some(b) = d.get_temp::<std::sync::Arc<Vec<image::RgbaImage>>>(Id::new("collage-base")) {
                        let mut v = (*b).clone();
                        move_item(&mut v, from, to);
                        d.insert_temp(Id::new("collage-base"), std::sync::Arc::new(v));
                    }
                    if let Some(c) = d.get_temp::<std::sync::Arc<Vec<egui::TextureHandle>>>(Id::new("collage-strip")) {
                        let mut v = (*c).clone();
                        move_item(&mut v, from, to);
                        d.insert_temp(Id::new("collage-strip"), std::sync::Arc::new(v));
                    }
                    d.remove::<ferrocast_core::collage::CollageOpts>(Id::new("collage-last"));
                });
                ctx.request_repaint();
            }
        }
        let base: Option<std::sync::Arc<Vec<image::RgbaImage>>> = ctx.data(|d| d.get_temp(Id::new("collage-base")));
        if let Some(b) = base {
            let last: Option<ferrocast_core::collage::CollageOpts> = ctx.data(|d| d.get_temp(Id::new("collage-last")));
            if last != Some(o) && b.len() == n {
                // final size first, then shrink every length by the same factor for the preview
                let ((fw, _), _) = layout(n, &o);
                let k = 480.0 / fw.max(1) as f32;
                let small = ferrocast_core::collage::CollageOpts {
                    width: Some(480),
                    gap: (o.gap as f32 * k).round() as u32,
                    radius: (o.radius as f32 * k).round() as u32,
                    ..o
                };
                let out = services::collage_compose(&b, &small);
                let (w, h) = (out.width() as usize, out.height() as usize);
                let tex = ctx.load_texture(
                    "collage-view",
                    egui::ColorImage::from_rgba_unmultiplied([w, h], out.as_raw()),
                    egui::TextureOptions::LINEAR,
                );
                ctx.data_mut(|d| {
                    d.insert_temp(Id::new("collage-tex"), tex);
                    d.insert_temp(Id::new("collage-last"), o);
                });
            }
        }
        match ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("collage-tex"))) {
            Some(tex) => {
                let t = tex.size_vec2();
                let k = (area.width() / t.x).min(area.height() / t.y);
                egui::Image::new(&tex).paint_at(ui, Rect::from_center_size(area.center(), t * k));
            }
            None => {
                ui.painter().text(area.center(), egui::Align2::CENTER_CENTER, "Loading…", FontId::proportional(16.0), pal.sub);
            }
        }
        note_line(ui, Pos2::new(area_in.center().x, area_in.max.y - 14.0), format!("{n} pictures · the preview is a reduced copy"), pal);
    }

    /// Background editor: sliders for padding, corners and shadow, a colour or blurred backdrop;
    /// the preview is composed on a 600 px copy with the sizes scaled down.
    #[allow(clippy::too_many_arguments)]
    fn draw_frame(
        &mut self,
        ui: &mut Ui,
        area_in: Rect,
        pal: &Palette,
        file: &std::path::Path,
        size: (u32, u32),
        opts: ferrocast_core::frame::FrameOpts,
        color: usize,
        picking: bool,
    ) {
        use ferrocast_core::frame::Backdrop;
        let ctx = ui.ctx().clone();
        let (mut o, mut color) = (opts.clone(), color);
        let mut choose = false;
        let blur_on = matches!(o.backdrop, Backdrop::Blur(_));
        let image_on = matches!(o.backdrop, Backdrop::Image(_));
        let panel = Rect::from_min_max(Pos2::new(area_in.max.x - 300.0, area_in.min.y + 12.0), area_in.max - Vec2::new(16.0, 34.0));
        ui.scope_builder(UiBuilder::new().max_rect(panel), |ui| {
            ui.spacing_mut().slider_width = 96.0;
            ui.spacing_mut().item_spacing.y = 0.0;
            let cap = size.0.max(size.1) / 3;
            card(ui, pal, |ui| {
                for (label, v, max) in
                    [("Padding", &mut o.pad, cap.max(8)), ("Corners", &mut o.radius, cap.max(8)), ("Shadow", &mut o.shadow, 100)]
                {
                    form_row_compact(ui, pal, label, 62.0, |ui| {
                        ui.add(egui::Slider::new(v, 0..=max));
                    });
                }
                form_row_compact(ui, pal, "Backdrop", 70.0, |ui| {
                    let sel = if blur_on {
                        1
                    } else if image_on {
                        2
                    } else {
                        0
                    };
                    match segmented_bar(ui, pal, Some(sel), &["Colour", "Blur", "Image"], 0.0, 12.0).0 {
                        Some(0) if sel != 0 => o.backdrop = Backdrop::Color(ferrocast_core::annotate::COLORS[color.min(5)]),
                        Some(1) if sel != 1 => o.backdrop = Backdrop::Blur(30.0),
                        Some(2) if sel != 2 => {
                            // reuse the picture already loaded for the preview, if there is one
                            let kept: Option<std::sync::Arc<(String, image::RgbaImage)>> = ctx.data(|d| d.get_temp(Id::new("frame-bg")));
                            o.backdrop = Backdrop::Image(kept.map(|k| k.0.clone()).unwrap_or_default());
                        }
                        _ => {}
                    }
                });
                match &mut o.backdrop {
                    Backdrop::Blur(r) => {
                        form_row_compact(ui, pal, "Blur", 62.0, |ui| {
                            ui.add(egui::Slider::new(r, 1.0..=100.0));
                        });
                    }
                    Backdrop::Color(c) => {
                        form_row_compact(ui, pal, "Colour", 40.0, |ui| {
                            // right to left: white first, then the palette in reverse, so it reads left to right
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                            swatch_dot(ui.painter(), r.center(), egui::Color32::WHITE, pal);
                            if *c == [255, 255, 255] {
                                ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(1.5, pal.text));
                            }
                            if resp.clicked() {
                                *c = [255, 255, 255];
                            }
                            for (i, k) in ferrocast_core::annotate::COLORS.iter().enumerate().rev() {
                                let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                                swatch_dot(ui.painter(), r.center(), egui::Color32::from_rgb(k[0], k[1], k[2]), pal);
                                // ring the swatch that matches the actual backdrop
                                if *c == *k {
                                    ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(1.5, pal.text));
                                }
                                if resp.clicked() {
                                    color = i;
                                    *c = *k;
                                }
                            }
                        });
                    }
                    Backdrop::Image(path) => {
                        let name = std::path::Path::new(path.as_str()).file_name().map(|n| n.to_string_lossy().to_string());
                        let text = match name {
                            Some(n) => n,
                            None if picking => "choosing…".to_string(),
                            None => "no picture chosen yet".to_string(),
                        };
                        form_row_stacked(ui, pal, "Picture", "Shown behind the photo.", |ui| {
                            if ui.add_enabled(!picking, egui::Button::new("Choose image…")).clicked() {
                                choose = true;
                            }
                            let text = elide(ui.painter(), &text, FontId::proportional(12.5), 120.0);
                            ui.label(RichText::new(text).size(12.5).color(pal.sub));
                        });
                    }
                }
            });
        });
        if choose && !picking {
            let (bus, f) = (self.bus.clone(), file.to_path_buf());
            std::thread::spawn(move || {
                bus.send(Msg::FramePicked(f, services::choose_picture_with_prompt("Choose a picture for the background")))
            });
        }
        if o != opts || (choose && !picking) {
            if let Some(View::FrameImage { opts: p, color: c, picking: pk, .. }) = self.stack.last_mut() {
                (*p, *c) = (o.clone(), color);
                if choose {
                    *pk = true;
                }
            }
            ctx.request_repaint();
        }
        let area = Rect::from_min_max(area_in.min + Vec2::new(16.0, 14.0), Pos2::new(area_in.max.x - 316.0, area_in.max.y - 34.0));
        let base: Option<std::sync::Arc<image::RgbaImage>> = ctx.data(|d| d.get_temp(Id::new("frame-base")));
        if let Some(b) = base {
            let last: Option<(ferrocast_core::frame::FrameOpts, usize)> = ctx.data(|d| d.get_temp(Id::new("frame-last")));
            if last.as_ref().map(|l| &l.0) != Some(&o) {
                let k = b.width() as f32 / size.0.max(1) as f32;
                let sc = |v: u32| (v as f32 * k).round() as u32;
                let small = ferrocast_core::frame::FrameOpts {
                    pad: sc(o.pad),
                    radius: sc(o.radius),
                    shadow: sc(o.shadow),
                    // the blur radius is relative to the canvas width, so it scales like the rest
                    backdrop: match &o.backdrop {
                        Backdrop::Blur(r) => Backdrop::Blur((r * k).max(1.0)),
                        other => other.clone(),
                    },
                };
                // the chosen backdrop picture, once it is loaded (a white backdrop until then)
                let bg: Option<std::sync::Arc<(String, image::RgbaImage)>> = match &o.backdrop {
                    Backdrop::Image(p) => {
                        ctx.data(|d| d.get_temp::<std::sync::Arc<(String, image::RgbaImage)>>(Id::new("frame-bg"))).filter(|b| b.0 == *p)
                    }
                    _ => None,
                };
                let out = services::frame_compose(&b, bg.as_deref().map(|b| &b.1), &small);
                let (w, h) = (out.width() as usize, out.height() as usize);
                let tex = ctx.load_texture(
                    "frame-view",
                    egui::ColorImage::from_rgba_unmultiplied([w, h], out.as_raw()),
                    egui::TextureOptions::LINEAR,
                );
                ctx.data_mut(|d| {
                    d.insert_temp(Id::new("frame-tex"), tex);
                    d.insert_temp(Id::new("frame-last"), (o.clone(), color));
                });
            }
        }
        match ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("frame-tex"))) {
            Some(tex) => {
                let t = tex.size_vec2();
                let k = (area.width() / t.x).min(area.height() / t.y);
                egui::Image::new(&tex).paint_at(ui, Rect::from_center_size(area.center(), t * k));
            }
            None => {
                ui.painter().text(area.center(), egui::Align2::CENTER_CENTER, "Loading…", FontId::proportional(16.0), pal.sub);
            }
        }
        note_line(ui, Pos2::new(area_in.center().x, area_in.max.y - 14.0), "drag a slider · the preview is a reduced copy", pal);
    }

    /// Redaction editor: drag on the picture to add an area, drag an area to move or resize it,
    /// Delete removes the selected one. The preview shows where the areas are; the covering itself
    /// (solid or blur) is applied to the full-size copy on Enter.
    #[allow(clippy::too_many_arguments)]
    fn draw_redact(
        &mut self,
        ui: &mut Ui,
        area_in: Rect,
        pal: &Palette,
        size: (u32, u32),
        areas: Vec<ferrocast_core::croprect::Rect>,
        sel: Option<usize>,
        style: (bool, u32),
        color: usize,
    ) {
        use ferrocast_core::croprect as cr;
        let ctx = ui.ctx().clone();
        let before = (areas.clone(), sel, style, color);
        let (mut areas, mut sel, (mut blur, mut radius), mut color) = (areas, sel, style, color);
        let mut deleted = false;
        let editing = ctx.egui_wants_keyboard_input();
        ctx.input_mut(|i| {
            // Backspace belongs to the blur box while it is being typed in
            if !editing && (i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace)) {
                if let Some(s) = sel.take().filter(|s| *s < areas.len()) {
                    areas.remove(s);
                    deleted = true;
                }
            }
        });
        if deleted {
            // a drag in progress holds an area index that no longer means the same area
            ctx.data_mut(|d| d.remove::<(cr::Handle, cr::Rect, Pos2, usize)>(Id::new("redact-drag")));
        }
        let strip = Rect::from_min_size(area_in.min + Vec2::new(0.0, 6.0), Vec2::new(area_in.width(), 90.0));
        let strip_bottom = ui
            .scope_builder(UiBuilder::new().max_rect(form_rect(strip)), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                card(ui, pal, |ui| {
                    form_row_compact(ui, pal, "Cover", 70.0, |ui| {
                        // right to left: the count first, the Solid / Blur choice last
                        ui.label(
                            RichText::new(format!("{} area{}", areas.len(), if areas.len() == 1 { "" } else { "s" }))
                                .size(12.0)
                                .color(pal.sub),
                        );
                        ui.add_space(6.0);
                        if blur {
                            ui.spacing_mut().slider_width = 120.0;
                            ui.add(egui::Slider::new(&mut radius, crate::app::REDACT_BLUR_RANGE).suffix(" px"));
                            ui.label(RichText::new("Blur").size(12.0).color(pal.sub));
                        } else {
                            for (i, c) in ferrocast_core::annotate::COLORS.iter().enumerate().rev() {
                                let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                                swatch_dot(ui.painter(), r.center(), egui::Color32::from_rgb(c[0], c[1], c[2]), pal);
                                if i == color {
                                    ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(1.5, pal.text));
                                }
                                if resp.clicked() {
                                    color = i;
                                }
                            }
                        }
                        ui.add_space(6.0);
                        segmented(ui, pal, &mut blur, &[(false, "Solid"), (true, "Blur")]);
                    });
                });
            })
            .response
            .rect
            .max
            .y;
        let area = Rect::from_min_max(Pos2::new(area_in.min.x + 16.0, strip_bottom + 8.0), area_in.max - Vec2::new(16.0, 34.0));
        let scale = (area.width() / size.0 as f32).min(area.height() / size.1 as f32).max(0.01);
        let img_rect = Rect::from_center_size(area.center(), Vec2::new(size.0 as f32 * scale, size.1 as f32 * scale));
        let resp = ui.interact(img_rect.expand(8.0), Id::new("redact-canvas"), egui::Sense::click_and_drag());
        ctx.data_mut(|d| d.insert_temp(Id::new("redact-img-rect"), img_rect));
        let to_img = |p: Pos2| ((p.x - img_rect.min.x) / scale, (p.y - img_rect.min.y) / scale);
        type Drag = (cr::Handle, cr::Rect, Pos2, usize);
        if resp.clicked() {
            // a plain click selects the topmost area under it, or clears the selection on empty space
            if let Some(p) = resp.interact_pointer_pos() {
                let q = to_img(p);
                sel = areas.iter().enumerate().rev().find(|(_, r)| cr::hit(**r, q.0, q.1, scale, 10.0).is_some()).map(|(i, _)| i);
            }
        }
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                let q = to_img(p);
                // topmost area under the pointer first, else start a new one at this corner
                let hit = areas.iter().enumerate().rev().find_map(|(i, r)| cr::hit(*r, q.0, q.1, scale, 10.0).map(|h| (i, h)));
                let state: Option<Drag> = match hit {
                    Some((i, h)) => {
                        sel = Some(i);
                        Some((h, areas[i], p, i))
                    }
                    None if areas.len() < 64 && q.0 >= 0.0 && q.1 >= 0.0 && q.0 < size.0 as f32 && q.1 < size.1 as f32 => {
                        let start = cr::clamp(cr::Rect { x: q.0 as u32, y: q.1 as u32, w: cr::MIN, h: cr::MIN }, size.0, size.1);
                        areas.push(start);
                        sel = Some(areas.len() - 1);
                        Some((cr::Handle::SE, start, p, areas.len() - 1))
                    }
                    None => None,
                };
                ctx.data_mut(|d| {
                    d.remove::<Drag>(Id::new("redact-drag"));
                    if let Some(s) = state {
                        d.insert_temp(Id::new("redact-drag"), s);
                    }
                });
            }
        }
        if resp.dragged() {
            let state: Option<Drag> = ctx.data(|d| d.get_temp(Id::new("redact-drag")));
            if let (Some((h, start, origin, i)), Some(p)) = (state, resp.interact_pointer_pos()) {
                if let Some(slot) = areas.get_mut(i) {
                    let (dx, dy) = ((p.x - origin.x) / scale, (p.y - origin.y) / scale);
                    *slot = cr::drag(start, h, dx.round() as i64, dy.round() as i64, size.0, size.1, None);
                }
            }
        }
        if resp.drag_stopped() {
            ctx.data_mut(|d| d.remove::<Drag>(Id::new("redact-drag")));
        }
        if let Some(tex) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("redact-tex"))) {
            egui::Image::new(&tex).paint_at(ui, img_rect);
        } else {
            ui.painter().text(img_rect.center(), egui::Align2::CENTER_CENTER, "Loading…", FontId::proportional(16.0), pal.sub);
        }
        let c = ferrocast_core::annotate::COLORS[color.min(5)];
        for (i, r) in areas.iter().enumerate() {
            let sr = Rect::from_min_size(
                Pos2::new(img_rect.min.x + r.x as f32 * scale, img_rect.min.y + r.y as f32 * scale),
                Vec2::new(r.w as f32 * scale, r.h as f32 * scale),
            );
            let fill =
                if blur { egui::Color32::from_white_alpha(70) } else { egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 190) };
            ui.painter().rect_filled(sr, 0.0, fill);
            ui.painter().rect_stroke(sr, 0.0, Stroke::new(if sel == Some(i) { 2.0 } else { 1.0 }, pal.accent), StrokeKind::Outside);
            if sel == Some(i) {
                for p in [sr.left_top(), sr.right_top(), sr.left_bottom(), sr.right_bottom()] {
                    ui.painter().rect_filled(Rect::from_center_size(p, Vec2::splat(8.0)), 1.0, pal.accent);
                }
            }
        }
        note_line(
            ui,
            Pos2::new(area_in.center().x, area_in.max.y - 14.0),
            "drag to add an area · drag an area to move, corners to resize · the preview only marks areas; the cover is applied on save",
            pal,
        );
        if (areas.clone(), sel, (blur, radius), color) != before {
            if let Some(View::RedactImage { areas: a, sel: s, blur: b, radius: r, color: c, .. }) = self.stack.last_mut() {
                (*a, *s, *b, *r, *c) = (areas, sel, blur, radius, color);
            }
            ctx.request_repaint();
        }
    }

    /// "Reference frame" slider of the crop and redact windows: moving it asks (off the UI thread,
    /// newest wins) for the frame at that time; the rectangles are not touched.
    fn draw_ref_frame(&mut self, ui: &mut Ui, strip: Rect, pal: &Palette, duration: f64, ref_t: f64) {
        let max = ferrocast_core::videoplan::frame_time(duration, duration);
        if max <= 0.0 {
            return;
        }
        let mut t = ref_t.clamp(0.0, max);
        ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Reference frame").size(12.0).color(pal.sub));
                ui.spacing_mut().slider_width = 220.0;
                ui.add(egui::Slider::new(&mut t, 0.0..=max).suffix(" s").fixed_decimals(2));
            });
        });
        if t != ref_t {
            if let Some(View::VideoCrop { ref_t: r, .. } | View::VideoRedact { ref_t: r, .. }) = self.stack.last_mut() {
                *r = t;
            }
            ui.ctx().request_repaint();
        }
        self.pump_ref_frame();
    }

    /// Redact video: the areas on the left (add, select, delete), the picture with the selected
    /// area's rectangle on the right, and the selected area's time range and style.
    fn draw_video_redact(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette) {
        let View::VideoRedact { size: Some(size), duration, areas, sel, ref_t, .. } = self.view().clone() else {
            return;
        };
        if areas.is_empty() {
            // cannot happen once the video is read; keep the window usable if it ever does
            if let Some(View::VideoRedact { areas, sel, .. }) = self.stack.last_mut() {
                (*areas, *sel) = (vec![crate::app::new_area(size, duration)], 0);
            }
            ui.ctx().request_repaint();
            return;
        }
        let sel_now = sel.min(areas.len() - 1);
        let mut areas_new = areas.clone();
        let mut sel_new = sel_now;
        // left column: "+ Add area" first (fixed place), then one row per area
        let col = Rect::from_min_size(rect.min + Vec2::new(12.0, 8.0), Vec2::new(206.0, rect.height() - 16.0));
        let full = areas.len() >= crate::app::MAX_REDACT_AREAS;
        let add = Rect::from_min_size(col.min, Vec2::new(col.width(), 26.0));
        let add_resp = ui.interact(add, Id::new("redact-add"), if full { egui::Sense::hover() } else { egui::Sense::click() });
        let add_fill = if add_resp.hovered() && !full { pal.sel } else { pal.field };
        ui.painter().rect_filled(add, 6.0, add_fill);
        ui.painter().text(
            add.center(),
            egui::Align2::CENTER_CENTER,
            if full { format!("Limit of {} areas", crate::app::MAX_REDACT_AREAS) } else { "+ Add area".to_string() },
            FontId::proportional(13.0),
            if full { pal.sub } else { pal.text },
        );
        if add_resp.clicked() && !full {
            areas_new.push(crate::app::new_area(size, duration));
            sel_new = areas_new.len() - 1;
        }
        let mut delete = None;
        let can_delete = areas.len() > 1;
        for (i, a) in areas.iter().enumerate() {
            let row = Rect::from_min_size(col.min + Vec2::new(0.0, 34.0 + 30.0 * i as f32), Vec2::new(col.width(), 26.0));
            if row.max.y > rect.max.y - 4.0 {
                break;
            }
            let label_rect = Rect::from_min_max(row.min, Pos2::new(row.max.x - 26.0, row.max.y));
            let del_rect = Rect::from_min_max(Pos2::new(row.max.x - 26.0, row.min.y), row.max);
            let resp = ui.interact(label_rect, Id::new(("redact-row", i)), egui::Sense::click());
            let del =
                ui.interact(del_rect, Id::new(("redact-del", i)), if can_delete { egui::Sense::click() } else { egui::Sense::hover() });
            if i == sel_now {
                ui.painter().rect_filled(row, 6.0, pal.sel);
                ui.painter().rect_stroke(row, 6.0, Stroke::new(1.0, pal.accent), StrokeKind::Inside);
            } else if resp.hovered() || del.hovered() {
                ui.painter().rect_filled(row, 6.0, pal.field);
            }
            ui.painter().text(
                label_rect.left_center() + Vec2::new(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("Area {} · {}–{} · {}", i + 1, clock(a.start), clock(a.end), if a.blur { "blur" } else { "solid" }),
                FontId::proportional(12.0),
                pal.text,
            );
            let x_color = if !can_delete {
                pal.sep
            } else if del.hovered() {
                pal.danger
            } else {
                pal.sub
            };
            let c = del_rect.center();
            ui.painter().line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], Stroke::new(1.5, x_color));
            ui.painter().line_segment([c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)], Stroke::new(1.5, x_color));
            if del.clicked() && can_delete {
                delete = Some(i);
            } else if resp.clicked() {
                sel_new = i;
            }
        }
        if let Some(i) = delete {
            areas_new.remove(i);
            sel_new = if sel_new > i || sel_new >= areas_new.len() { sel_new.saturating_sub(1) } else { sel_new };
        }
        // right side: range and style of the selected area, the reference frame, then the picture
        let right = Rect::from_min_max(Pos2::new(rect.min.x + 232.0, rect.min.y), rect.max);
        if sel_new == sel_now && sel_now < areas_new.len() {
            let mut a = areas_new[sel_now];
            let strip = Rect::from_min_size(right.min + Vec2::new(12.0, 4.0), Vec2::new(right.width() - 24.0, 30.0));
            ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Area {}", sel_now + 1));
                    ui.label("Start s");
                    ui.add(egui::DragValue::new(&mut a.start).range(0.0..=duration).speed(0.04));
                    ui.label("End s");
                    ui.add(egui::DragValue::new(&mut a.end).range(0.0..=duration).speed(0.04));
                    segmented(ui, pal, &mut a.blur, &[(false, "Solid"), (true, "Blur")]);
                });
            });
            areas_new[sel_now] = a;
        }
        if areas_new != areas || sel_new != sel {
            if let Some(View::VideoRedact { areas, sel, .. }) = self.stack.last_mut() {
                (*areas, *sel) = (areas_new.clone(), sel_new);
            }
            ui.ctx().request_repaint();
        }
        let ref_strip = Rect::from_min_size(right.min + Vec2::new(12.0, 36.0), Vec2::new(right.width() - 24.0, 28.0));
        self.draw_ref_frame(ui, ref_strip, pal, duration, ref_t);
        let sel_rect = areas_new.get(sel_new).map(|a| a.rect).unwrap_or_else(|| ferrocast_core::croprect::full(size.0, size.1));
        let aspect = match self.view() {
            View::VideoRedact { aspect, .. } => *aspect,
            _ => None,
        };
        let area = Rect::from_min_max(right.min + Vec2::new(0.0, 64.0), right.max);
        self.draw_crop(ui, area, pal, size, sel_rect, aspect);
    }

    /// Crop editor: the picture with a draggable rectangle, aspect presets and exact size boxes.
    fn draw_crop(
        &mut self,
        ui: &mut Ui,
        area_in: Rect,
        pal: &Palette,
        size: (u32, u32),
        rect: ferrocast_core::croprect::Rect,
        aspect: Option<(u32, u32)>,
    ) {
        use ferrocast_core::croprect as cr;
        let ctx = ui.ctx().clone();
        let (mut rect, mut aspect) = (rect, aspect);
        let before = (rect, aspect);
        let strip = Rect::from_min_size(area_in.min + Vec2::new(12.0, 8.0), Vec2::new(area_in.width() - 24.0, 30.0));
        ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
            ui.horizontal(|ui| {
                // tighter cells: in the video windows the strip shares its line with the W and H boxes
                const ASPECTS: [Option<(u32, u32)>; 6] = [None, Some((1, 1)), Some((4, 3)), Some((3, 2)), Some((16, 9)), Some((9, 16))];
                let sel = ASPECTS.iter().position(|a| *a == aspect);
                if let (Some(i), _) = segmented_bar(ui, pal, sel, &["Free", "1:1", "4:3", "3:2", "16:9", "9:16"], 0.0, 16.0) {
                    aspect = ASPECTS[i];
                    if let Some((w, h)) = aspect {
                        rect = cr::fit_aspect(size.0, size.1, w, h);
                    }
                }
                ui.add_space(10.0);
                let (mut w, mut h) = (rect.w, rect.h);
                ui.label(RichText::new("W").size(12.0).color(pal.sub));
                ui.add(egui::DragValue::new(&mut w).range(cr::MIN..=size.0).speed(2.0));
                ui.label(RichText::new("H").size(12.0).color(pal.sub));
                ui.add(egui::DragValue::new(&mut h).range(cr::MIN..=size.1).speed(2.0));
                if (w, h) != (rect.w, rect.h) {
                    // a locked ratio follows whichever box moved
                    if let Some((rw, rh)) = aspect {
                        if w != rect.w {
                            h = ((w as f64) * rh as f64 / rw as f64).round() as u32;
                        } else {
                            w = ((h as f64) * rw as f64 / rh as f64).round() as u32;
                        }
                    }
                    rect = cr::with_size(rect, w, h, size.0, size.1);
                }
                let note = if matches!(self.view(), View::VideoRedact { .. }) {
                    format!("{}×{} px", rect.w, rect.h)
                } else {
                    format!("{}×{} of {}×{} px", rect.w, rect.h, size.0, size.1)
                };
                ui.label(RichText::new(note).size(12.0).color(pal.sub));
            });
        });
        let area = Rect::from_min_max(area_in.min + Vec2::new(16.0, 46.0), area_in.max - Vec2::new(16.0, 34.0));
        let scale = (area.width() / size.0 as f32).min(area.height() / size.1 as f32).max(0.01);
        let img_rect = Rect::from_center_size(area.center(), Vec2::new(size.0 as f32 * scale, size.1 as f32 * scale));
        let resp = ui.interact(img_rect.expand(8.0), Id::new("crop-canvas"), egui::Sense::click_and_drag());
        let to_img = |p: Pos2| ((p.x - img_rect.min.x) / scale, (p.y - img_rect.min.y) / scale);
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                let q = to_img(p);
                if let Some(hd) = cr::hit(rect, q.0, q.1, scale, 10.0) {
                    ctx.data_mut(|d| d.insert_temp(Id::new("crop-drag"), (hd, rect, p)));
                }
            }
        }
        if resp.dragged() {
            let state: Option<(cr::Handle, cr::Rect, Pos2)> = ctx.data(|d| d.get_temp(Id::new("crop-drag")));
            if let (Some((hd, start, origin)), Some(p)) = (state, resp.interact_pointer_pos()) {
                let (dx, dy) = ((p.x - origin.x) / scale, (p.y - origin.y) / scale);
                rect = cr::drag(start, hd, dx.round() as i64, dy.round() as i64, size.0, size.1, aspect);
            }
        }
        if resp.drag_stopped() {
            ctx.data_mut(|d| d.remove::<(cr::Handle, cr::Rect, Pos2)>(Id::new("crop-drag")));
        }
        if let Some(tex) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("crop-tex"))) {
            egui::Image::new(&tex).paint_at(ui, img_rect);
        } else {
            ui.painter().text(img_rect.center(), egui::Align2::CENTER_CENTER, "Loading…", FontId::proportional(16.0), pal.sub);
        }
        let sr = Rect::from_min_size(
            Pos2::new(img_rect.min.x + rect.x as f32 * scale, img_rect.min.y + rect.y as f32 * scale),
            Vec2::new(rect.w as f32 * scale, rect.h as f32 * scale),
        );
        let shade = egui::Color32::from_black_alpha(150);
        for r in [
            Rect::from_min_max(img_rect.min, Pos2::new(img_rect.max.x, sr.min.y)),
            Rect::from_min_max(Pos2::new(img_rect.min.x, sr.max.y), img_rect.max),
            Rect::from_min_max(Pos2::new(img_rect.min.x, sr.min.y), Pos2::new(sr.min.x, sr.max.y)),
            Rect::from_min_max(Pos2::new(sr.max.x, sr.min.y), Pos2::new(img_rect.max.x, sr.max.y)),
        ] {
            ui.painter().rect_filled(r, 0.0, shade);
        }
        ui.painter().rect_stroke(sr, 0.0, Stroke::new(1.5, pal.accent), StrokeKind::Outside);
        // the other redaction areas, as dim numbered outlines
        if let View::VideoRedact { areas, sel, .. } = self.view() {
            for (i, a) in areas.iter().enumerate().filter(|(i, _)| i != sel) {
                let r = Rect::from_min_size(
                    Pos2::new(img_rect.min.x + a.rect.x as f32 * scale, img_rect.min.y + a.rect.y as f32 * scale),
                    Vec2::new(a.rect.w as f32 * scale, a.rect.h as f32 * scale),
                );
                ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, egui::Color32::from_white_alpha(130)), StrokeKind::Outside);
                ui.painter().text(
                    r.min + Vec2::new(4.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}", i + 1),
                    FontId::proportional(11.0),
                    egui::Color32::from_white_alpha(170),
                );
            }
        }
        for p in [
            sr.left_top(),
            sr.right_top(),
            sr.left_bottom(),
            sr.right_bottom(),
            sr.center_top(),
            sr.center_bottom(),
            sr.left_center(),
            sr.right_center(),
        ] {
            ui.painter().rect_filled(Rect::from_center_size(p, Vec2::splat(8.0)), 1.0, pal.accent);
        }
        note_line(
            ui,
            Pos2::new(area_in.center().x, area_in.max.y - 14.0),
            if matches!(self.view(), View::VideoRedact { .. }) {
                "drag to move/resize the selected area · the preview only marks it"
            } else {
                "drag inside to move · drag an edge or corner to resize · presets lock the ratio"
            },
            pal,
        );
        if (rect, aspect) != before {
            match self.stack.last_mut() {
                Some(View::CropImage { rect: r, aspect: a, .. } | View::VideoCrop { rect: r, aspect: a, .. }) => (*r, *a) = (rect, aspect),
                Some(View::VideoRedact { areas, sel, aspect: a, .. }) => {
                    if let Some(area) = areas.get_mut(*sel) {
                        area.rect = rect;
                    }
                    *a = aspect;
                }
                _ => {}
            }
            ctx.request_repaint();
        }
    }

    /// Photo editor: the picture on the left (recomputed on a downscaled copy while a slider moves)
    /// and one slider per control on the right. The full-size copy is written on Enter.
    fn draw_edit_photo(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette, size: (u32, u32), opts: ferrocast_core::photo::PhotoOpts) {
        let ctx = ui.ctx().clone();
        let mut o = opts;
        let panel = Rect::from_min_max(Pos2::new(rect.max.x - 300.0, rect.min.y + 12.0), rect.max - Vec2::new(16.0, 34.0));
        ui.scope_builder(UiBuilder::new().max_rect(panel), |ui| {
            ui.spacing_mut().slider_width = 96.0;
            ui.spacing_mut().item_spacing.y = 0.0;
            card(ui, pal, |ui| {
                for (label, v, signed) in [
                    ("Brightness", &mut o.brightness, true),
                    ("Contrast", &mut o.contrast, true),
                    ("Saturation", &mut o.saturation, true),
                    ("Warmth", &mut o.warmth, true),
                    ("Clarity", &mut o.clarity, false),
                    ("Sharpen", &mut o.sharpen, false),
                    ("Grain", &mut o.grain, false),
                    ("Denoise", &mut o.denoise, false),
                    ("Dehaze", &mut o.dehaze, false),
                ] {
                    form_row_compact(ui, pal, label, 76.0, |ui| {
                        ui.add(egui::Slider::new(v, if signed { -100..=100 } else { 0..=100 }).show_value(true));
                    });
                }
            });
            ui.add_space(8.0);
            if small_btn(ui, pal, "Reset") {
                o = Default::default();
            }
        });
        if o != opts {
            if let Some(View::EditPhoto { opts: p, .. }) = self.stack.last_mut() {
                *p = o;
            }
            ctx.request_repaint();
        }
        let area = Rect::from_min_max(rect.min + Vec2::new(16.0, 14.0), Pos2::new(rect.max.x - 316.0, rect.max.y - 34.0));
        let scale = (area.width() / size.0 as f32).min(area.height() / size.1 as f32).max(0.01);
        let img_rect = Rect::from_center_size(area.center(), Vec2::new(size.0 as f32 * scale, size.1 as f32 * scale));
        let base: Option<std::sync::Arc<(Vec<u8>, usize, usize)>> = ctx.data(|d| d.get_temp(Id::new("edit-base")));
        if let Some(b) = base {
            let last: Option<ferrocast_core::photo::PhotoOpts> = ctx.data(|d| d.get_temp(Id::new("edit-last")));
            if last != Some(o) {
                let mut buf = b.0.clone();
                ferrocast_core::photo::apply(&mut buf, b.1, b.2, &o);
                let tex =
                    ctx.load_texture("edit-view", egui::ColorImage::from_rgba_unmultiplied([b.1, b.2], &buf), egui::TextureOptions::LINEAR);
                ctx.data_mut(|d| {
                    d.insert_temp(Id::new("edit-tex"), tex);
                    d.insert_temp(Id::new("edit-last"), o);
                });
            }
        }
        match ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("edit-tex"))) {
            Some(tex) => {
                egui::Image::new(&tex).paint_at(ui, img_rect);
            }
            None => {
                ui.painter().text(img_rect.center(), egui::Align2::CENTER_CENTER, "Loading…", FontId::proportional(16.0), pal.sub);
            }
        }
        note_line(ui, Pos2::new(rect.center().x, rect.max.y - 14.0), "drag a slider · the preview is a reduced copy", pal);
    }

    /// Annotation canvas: tool strip, the picture with the drawing composed on it, mouse editing.
    #[allow(clippy::too_many_arguments)]
    fn draw_annotate(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        pal: &Palette,
        file: &std::path::Path,
        size: (u32, u32),
        doc: ferrocast_core::annotate::Doc,
        tool: ferrocast_core::annotate::Tool,
        color: usize,
        sel: Option<usize>,
    ) {
        use ferrocast_core::annotate::{self as an, Doc, Style, Tool};
        let ctx = ui.ctx().clone();
        let before = (doc.clone(), tool, color, sel);
        let (mut doc, mut tool, mut color, mut sel) = (doc, tool, color, sel);
        let mut pending: Option<((f32, f32), String)> = ctx.data(|d| d.get_temp(Id::new("annot-text")));
        ctx.input_mut(|i| {
            if pending.is_some() {
                return;
            }
            if i.consume_key(CMD, Key::Z) {
                doc.undo();
                sel = None;
            }
            if i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace) {
                if let Some(s) = sel.take() {
                    doc.remove(s);
                }
            }
        });
        let strip = Rect::from_min_size(rect.min + Vec2::new(12.0, 6.0), Vec2::new(rect.width() - 24.0, 60.0));
        let strip_bottom = ui
            .scope_builder(UiBuilder::new().max_rect(strip), |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                // the tools wrap if the window is narrow; the colours get a row of their own
                ui.horizontal_wrapped(|ui| {
                    for (t, l) in [
                        (Tool::Select, "Select"),
                        (Tool::Arrow, "Arrow"),
                        (Tool::Line, "Line"),
                        (Tool::Box, "Box"),
                        (Tool::Ellipse, "Ellipse"),
                        (Tool::Highlight, "Highlight"),
                        (Tool::Pen, "Pen"),
                        (Tool::Number, "Number"),
                        (Tool::Magnifier, "Magnifier"),
                        (Tool::Text, "Text"),
                        (Tool::Callout, "Callout"),
                        (Tool::Stamp, "Image"),
                    ] {
                        let resp = ui.selectable_label(tool == t, l);
                        record_strip(&ctx, resp.rect);
                        if resp.clicked() {
                            tool = t;
                            sel = None;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    for (i, c) in an::COLORS.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                        record_strip(&ctx, r.expand(2.0));
                        swatch_dot(ui.painter(), r.center(), egui::Color32::from_rgb(c[0], c[1], c[2]), pal);
                        if i == color {
                            ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(1.5, pal.text));
                        }
                        if resp.clicked() {
                            color = i;
                        }
                    }
                });
            })
            .response
            .rect
            .max
            .y;
        let area = Rect::from_min_max(Pos2::new(rect.min.x + 12.0, strip_bottom + 6.0), rect.max - Vec2::new(12.0, 30.0));
        let scale = (area.width() / size.0 as f32).min(area.height() / size.1 as f32).max(0.01);
        let img_rect = Rect::from_center_size(area.center(), Vec2::new(size.0 as f32 * scale, size.1 as f32 * scale));
        let to_img = |p: Pos2| ((p.x - img_rect.min.x) / scale, (p.y - img_rect.min.y) / scale);
        let width = (size.0.max(size.1) as f32 / 300.0).clamp(3.0, 24.0);
        let style = Style { color: an::COLORS[color.min(an::COLORS.len() - 1)], width, fill: false };
        let resp = ui.interact(img_rect, Id::new("annot-canvas"), egui::Sense::click_and_drag());
        let mut pts: Vec<(f32, f32)> = ctx.data(|d| d.get_temp(Id::new("annot-drag"))).unwrap_or_default();
        if resp.drag_started() {
            pts.clear();
            if let Some(p) = resp.interact_pointer_pos() {
                let q = to_img(p);
                if tool == Tool::Select {
                    sel = doc.hit(q, 8.0 / scale);
                }
                pts.push(q);
            }
        }
        if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let q = to_img(p);
                if tool == Tool::Select {
                    if let (Some(s), Some(last)) = (sel, pts.last().copied()) {
                        doc.move_item(s, q.0 - last.0, q.1 - last.1);
                    }
                    pts = vec![q];
                } else if pts.len() < 4000 {
                    pts.push(q);
                }
            }
        }
        if resp.drag_stopped() {
            if tool == Tool::Callout {
                if let (Some(a), Some(b)) = (pts.first().copied(), pts.last().copied()) {
                    if an::text_available() && pending.is_none() && (b.0 - a.0).hypot(b.1 - a.1) >= 3.0 {
                        ctx.data_mut(|d| d.insert_temp(Id::new("annot-tip"), a));
                        pending = Some((b, String::new()));
                    }
                }
            } else if tool != Tool::Select {
                if let Some(shape) = an::build(tool, &pts, doc.next_number()) {
                    doc.add(shape, style);
                }
            }
            pts.clear();
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let q = to_img(p);
                match tool {
                    Tool::Number => {
                        let n = doc.next_number();
                        doc.add(an::Shape::Number(q, n), style);
                    }
                    Tool::Stamp => {
                        let (bus, f) = (self.bus.clone(), file.to_path_buf());
                        std::thread::spawn(move || bus.send(Msg::StampChosen(f, q, services::choose_stamp_image())));
                    }
                    Tool::Text if an::text_available() && pending.is_none() => pending = Some((q, String::new())),
                    Tool::Text if !an::text_available() => self.toast("Text needs the system font Arial, which was not found"),
                    Tool::Select => sel = doc.hit(q, 8.0 / scale),
                    _ => {}
                }
            }
        }
        let mut shown = doc.clone();
        if resp.dragged() && tool != Tool::Select {
            if let Some(shape) = an::build(tool, &pts, doc.next_number()) {
                shown.add(shape, style);
            }
        }
        ctx.data_mut(|d| d.insert_temp(Id::new("annot-drag"), pts));
        let base: Option<std::sync::Arc<(Vec<u8>, usize, usize)>> = ctx.data(|d| d.get_temp(Id::new("annot-base")));
        if let Some(b) = base {
            let last: Option<Doc> = ctx.data(|d| d.get_temp(Id::new("annot-last")));
            if last.as_ref() != Some(&shown) {
                let mut buf = b.0.clone();
                shown.scaled(b.1 as f32 / size.0 as f32).render(&mut buf, b.1, b.2);
                let ci = egui::ColorImage::from_rgba_unmultiplied([b.1, b.2], &buf);
                let tex = ctx.load_texture("annot-view", ci, egui::TextureOptions::LINEAR);
                ctx.data_mut(|d| {
                    d.insert_temp(Id::new("annot-tex"), tex);
                    d.insert_temp(Id::new("annot-last"), shown.clone());
                });
            }
        }
        match ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("annot-tex"))) {
            Some(tex) => {
                egui::Image::new(&tex).paint_at(ui, img_rect);
            }
            None => {
                ui.painter().text(img_rect.center(), egui::Align2::CENTER_CENTER, "Loading…", FontId::proportional(16.0), pal.sub);
            }
        }
        // inline text field at the clicked point; Enter adds the text, Escape drops it
        if let Some((at, mut text)) = pending.take() {
            let pos = Pos2::new(img_rect.min.x + at.0 * scale, img_rect.min.y + at.1 * scale);
            let col = an::COLORS[color.min(an::COLORS.len() - 1)];
            let field = egui::Area::new(Id::new("annot-text-area")).order(egui::Order::Foreground).fixed_pos(pos).show(&ctx, |ui| {
                ui.add(
                    TextEdit::singleline(&mut text)
                        .id(Id::new("annot-text-field"))
                        .desired_width(220.0)
                        .text_color(egui::Color32::from_rgb(col[0], col[1], col[2]))
                        .hint_text("Type, then Enter"),
                )
            });
            let r = field.inner;
            if !r.has_focus() && !r.lost_focus() {
                r.request_focus();
            }
            let enter = r.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter));
            let esc = ctx.input(|i| i.key_pressed(Key::Escape));
            if esc {
                ctx.data_mut(|d| d.remove::<(f32, f32)>(Id::new("annot-tip")));
            } else if enter {
                if !text.trim().is_empty() {
                    let tip: Option<(f32, f32)> = ctx.data(|d| d.get_temp(Id::new("annot-tip")));
                    match (tool, tip) {
                        (Tool::Callout, Some(tip)) => doc.add(an::Shape::Callout { at, tip, text: text.trim().to_string() }, style),
                        _ => doc.add(an::Shape::Text(at, text.trim().to_string()), style),
                    }
                }
                ctx.data_mut(|d| d.remove::<(f32, f32)>(Id::new("annot-tip")));
            } else {
                pending = Some((at, text));
            }
        }
        ctx.data_mut(|d| {
            d.insert_temp(Id::new("annot-text"), pending.clone());
            d.insert_temp(Id::new("annot-typing"), pending.is_some());
        });
        if let Some(it) = sel.and_then(|s| doc.items.get(s)) {
            let (x0, y0, x1, y1) = it.shape.bounds_with(it.style.width);
            let pad = it.style.width / 2.0
                + 6.0 / scale
                + if matches!(it.shape, an::Shape::Number(..)) { an::label_radius(it.style.width) } else { 0.0 };
            let r = Rect::from_min_max(
                Pos2::new(img_rect.min.x + (x0 - pad) * scale, img_rect.min.y + (y0 - pad) * scale),
                Pos2::new(img_rect.min.x + (x1 + pad) * scale, img_rect.min.y + (y1 + pad) * scale),
            );
            ui.painter().rect_stroke(r, 2.0, Stroke::new(1.5, pal.accent), StrokeKind::Outside);
        }
        note_line(
            ui,
            Pos2::new(rect.center().x, rect.max.y - 14.0),
            "drag to draw · click a Number to place · Select: click and drag to move",
            pal,
        );
        if (doc.clone(), tool, color, sel) != before {
            if let Some(View::Annotate { doc: d, tool: t, color: c, sel: s, .. }) = self.stack.last_mut() {
                (*d, *t, *c, *s) = (doc, tool, color, sel);
            }
            ctx.request_repaint();
        }
    }

    /// Ordered list of files: ↑/↓ choose, ⌘↑/⌘↓ (or the row buttons) move, ↵ runs.
    fn draw_reorder(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette, purpose: ReorderPurpose, files: Vec<PathBuf>, cursor: usize) {
        use ferrocast_core::reorder::{move_down, move_item, move_up};
        let ctx = ui.ctx().clone();
        let (mut files, mut cursor) = (files, cursor);
        let before = (files.clone(), cursor);
        let last = files.len().saturating_sub(1);
        cursor = cursor.min(last);
        ctx.input_mut(|i| {
            // ⌘ variants first: egui matches Modifiers::NONE even when other modifiers are held
            if i.consume_key(CMD, Key::ArrowUp) {
                if let Some(n) = move_up(&mut files, cursor) {
                    cursor = n;
                }
            } else if i.consume_key(CMD, Key::ArrowDown) {
                if let Some(n) = move_down(&mut files, cursor) {
                    cursor = n;
                }
            } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                cursor = cursor.saturating_sub(1);
            } else if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                cursor = (cursor + 1).min(last);
            }
        });
        let mut click: Option<(usize, i8)> = None; // (row, 0 select / -1 up / 1 down)
        let mut dropped: Option<(usize, usize)> = None;
        ui.scope_builder(UiBuilder::new().max_rect(rect.shrink2(Vec2::new(16.0, 12.0))), |ui| {
            ui.label(RichText::new(format!("{} files · first to last", files.len())).size(13.0).color(pal.text));
            let hint = match purpose {
                ReorderPurpose::JoinVideos => "First clip sets the canvas · drag a row to move",
                ReorderPurpose::CreatePdf => "One page per picture · drag a row to move",
                _ => "Drag a row to move",
            };
            record_note(ui.ctx(), hint);
            ui.label(RichText::new(hint).size(12.0).color(pal.sub));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().id_salt("reorder-list").auto_shrink([false, false]).show(ui, |ui| {
                let res = drag_reorder_cells(
                    ui,
                    "reorder-rows",
                    files.len(),
                    false,
                    pal,
                    |ui, pos, lifted| {
                        let f = &files[pos];
                        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), egui::Sense::hover());
                        let on = pos == cursor;
                        ui.painter().rect_filled(r, 8.0, if on || lifted { pal.sel } else { pal.field });
                        if on || lifted {
                            ui.painter().rect_stroke(r, 8.0, Stroke::new(1.5, pal.accent), StrokeKind::Inside);
                        }
                        if on && !lifted {
                            ui.scroll_to_rect(r, None);
                        }
                        ui.painter().text(
                            Pos2::new(r.min.x + 22.0, r.center().y),
                            egui::Align2::CENTER_CENTER,
                            (pos + 1).to_string(),
                            FontId::proportional(14.0),
                            pal.sub,
                        );
                        let text_rect = Rect::from_min_max(Pos2::new(r.min.x + 42.0, r.min.y), Pos2::new(r.max.x - 76.0, r.max.y));
                        let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| f.display().to_string());
                        let kind = f.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default();
                        let p = ui.painter().with_clip_rect(text_rect);
                        p.text(
                            Pos2::new(text_rect.min.x, r.center().y - 8.0),
                            egui::Align2::LEFT_CENTER,
                            name,
                            FontId::proportional(14.0),
                            pal.text,
                        );
                        p.text(
                            Pos2::new(text_rect.min.x, r.center().y + 10.0),
                            egui::Align2::LEFT_CENTER,
                            kind,
                            FontId::proportional(11.5),
                            pal.sub,
                        );
                    },
                    |ui, pos, r| {
                        for (b, (glyph, dir)) in [("↑", -1i8), ("↓", 1i8)].into_iter().enumerate() {
                            let br =
                                Rect::from_center_size(Pos2::new(r.max.x - 50.0 + b as f32 * 32.0, r.center().y), Vec2::new(28.0, 28.0));
                            let enabled = if dir < 0 { pos > 0 } else { pos < last };
                            let bresp = ui.interact(br, Id::new(("reorder-btn", pos, dir)), egui::Sense::click());
                            let fill = if bresp.hovered() && enabled { pal.sel } else { pal.bg };
                            ui.painter().rect_filled(br, 6.0, fill);
                            ui.painter().text(
                                br.center(),
                                egui::Align2::CENTER_CENTER,
                                glyph,
                                FontId::proportional(14.0),
                                if enabled { pal.text } else { pal.sub.gamma_multiply(0.4) },
                            );
                            if bresp.clicked() && enabled {
                                click = Some((pos, dir));
                            }
                        }
                    },
                );
                if let Some(c) = res.clicked {
                    click = Some((c, 0));
                }
                dropped = res.moved;
            });
        });
        if let Some((from, to)) = dropped {
            if move_item(&mut files, from, to) {
                cursor = to;
            }
        }
        match click {
            Some((pos, 0)) => cursor = pos,
            Some((pos, -1)) => {
                if let Some(n) = move_up(&mut files, pos) {
                    cursor = n;
                }
            }
            Some((pos, _)) => {
                if let Some(n) = move_down(&mut files, pos) {
                    cursor = n;
                }
            }
            None => {}
        }
        if (&files, cursor) != (&before.0, before.1) {
            if let Some(View::Reorder { files: f, cursor: c, .. }) = self.stack.last_mut() {
                *f = files;
                *c = cursor;
            }
            ctx.request_repaint();
        }
    }

    /// Strip of page thumbnails with keyboard editing; the plan lives in the view on the stack.
    fn draw_pdf_organizer(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        pal: &Palette,
        file: &std::path::Path,
        n: usize,
        plan: ferrocast_core::pdfplan::PdfPlan,
    ) {
        use ferrocast_core::pdfplan::PdfPlan;
        let ctx = ui.ctx().clone();
        let (mut plan, before) = (plan.clone(), plan);
        ctx.input_mut(|i| {
            let none = Modifiers::NONE;
            if i.consume_key(CMD, Key::ArrowLeft) {
                plan.shift(-1);
            } else if i.consume_key(CMD, Key::ArrowRight) {
                plan.shift(1);
            }
            if i.consume_key(none, Key::ArrowLeft) {
                plan.move_cursor(-1);
            }
            if i.consume_key(none, Key::ArrowRight) {
                plan.move_cursor(1);
            }
            // Shift+R first: egui matches Modifiers::NONE even when Shift is held, so plain R
            // would otherwise consume Shift+R and turn the page the wrong way
            if i.consume_key(Modifiers::SHIFT, Key::R) {
                plan.rotate(-1);
            } else if i.consume_key(none, Key::R) {
                plan.rotate(1);
            }
            if i.consume_key(none, Key::D) {
                plan.duplicate();
            }
            if i.consume_key(none, Key::Backspace) || i.consume_key(none, Key::Delete) {
                plan.remove();
            }
        });
        if plan != before {
            if let Some(View::PdfOrganize { plan: p, .. }) = self.stack.last_mut() {
                *p = plan.clone();
            }
            ctx.request_repaint();
        }
        ui.scope_builder(UiBuilder::new().max_rect(rect.shrink2(Vec2::new(16.0, 12.0))), |ui| {
            ui.label(
                RichText::new(file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).size(14.0).color(pal.text),
            );
            let changed = !plan.is_identity(n);
            ui.label(
                RichText::new(format!("{} of {n} pages{}", plan.pages.len(), if changed { " · edited" } else { "" }))
                    .size(12.0)
                    .color(pal.sub),
            );
            ui.add_space(8.0);
            let mut res = CellDrag::default();
            egui::ScrollArea::horizontal().id_salt("pdf-strip").auto_shrink([false, true]).show(ui, |ui| {
                ui.horizontal(|ui| {
                    // each cell is one page entry (rotation included) and can be dragged to a new place
                    res = drag_reorder_cells(
                        ui,
                        "pdf-strip",
                        plan.pages.len(),
                        true,
                        pal,
                        |ui, pos, lifted| {
                            let page = &plan.pages[pos];
                            let (r, _) = ui.allocate_exact_size(Vec2::new(116.0, 168.0), egui::Sense::hover());
                            let on = pos == plan.cursor;
                            ui.painter().rect_filled(r, 8.0, if on || lifted { pal.sel } else { pal.field });
                            if on || lifted {
                                ui.painter().rect_stroke(r, 8.0, Stroke::new(1.5, pal.accent), StrokeKind::Inside);
                            }
                            if on && !lifted {
                                ui.scroll_to_rect(r, None);
                            }
                            let boxr = Rect::from_min_size(r.min + Vec2::new(8.0, 8.0), Vec2::new(100.0, 128.0));
                            match ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new(("pdfthumb", page.src)))) {
                                Some(tex) => {
                                    let odd = page.turns % 2 == 1;
                                    let t = tex.size_vec2();
                                    // the page as shown: swapped sides when turned a quarter
                                    let shown = if odd { Vec2::new(t.y, t.x) } else { t };
                                    let k = (boxr.width() / shown.x).min(boxr.height() / shown.y);
                                    let size = t * k;
                                    let img = egui::Image::new(&tex)
                                        .fit_to_exact_size(size)
                                        .rotate(page.turns as f32 * std::f32::consts::FRAC_PI_2, Vec2::splat(0.5));
                                    img.paint_at(ui, Rect::from_center_size(boxr.center(), size));
                                }
                                None => {
                                    ui.painter().text(boxr.center(), egui::Align2::CENTER_CENTER, "…", FontId::proportional(18.0), pal.sub);
                                }
                            }
                            ui.painter().text(
                                Pos2::new(r.center().x, r.max.y - 14.0),
                                egui::Align2::CENTER_CENTER,
                                PdfPlan::label(page),
                                FontId::proportional(12.0),
                                if on { pal.text } else { pal.sub },
                            );
                        },
                        |_, _, _| {},
                    );
                });
            });
            let mut edited = plan.clone();
            if let Some(c) = res.clicked {
                edited.cursor = c;
            }
            if let Some((from, to)) = res.moved {
                edited.move_page(from, to);
            }
            if edited != plan {
                if let Some(View::PdfOrganize { plan: p, .. }) = self.stack.last_mut() {
                    *p = edited;
                }
                ctx.request_repaint();
            }
        });
    }

    fn draw_single(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette) {
        let ff = std::mem::take(&mut self.focus_first);
        let view = self.view().clone();
        match view {
            View::Settings => {
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| self.draw_settings(ui, pal));
            }
            View::VideoRedact { size: Some(_), .. } => {
                self.draw_video_redact(ui, rect, pal);
            }
            View::VideoRedact { .. } => {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Reading the video…", FontId::proportional(16.0), pal.sub);
            }
            View::VideoSpeed { duration, factor, .. } => {
                self.draw_video_speed(ui, rect, pal, duration, factor);
            }
            View::NormalizeAudio { loudness, target, range, peak, .. } => {
                self.draw_normalize_audio(ui, rect, pal, loudness, target, range, peak);
            }
            View::ChannelsAudio { in_channels, stereo, left_db, right_db, .. } => {
                self.draw_channels_audio(ui, rect, pal, in_channels, stereo, left_db, right_db);
            }
            View::EditMetadata { file, kind, original, working, remove_all, error } => {
                self.draw_edit_metadata(ui, rect, pal, &file, kind, original, working, remove_all, error);
            }
            View::VideoCrop { size: Some(size), rect: crop, aspect, duration, ref_t, .. } => {
                let strip = Rect::from_min_size(rect.min + Vec2::new(12.0, 4.0), Vec2::new(rect.width() - 24.0, 30.0));
                self.draw_ref_frame(ui, strip, pal, duration, ref_t);
                self.draw_crop(ui, Rect::from_min_max(rect.min + Vec2::new(0.0, 30.0), rect.max), pal, size, crop, aspect);
            }
            View::VideoCrop { .. } => {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Reading the video…", FontId::proportional(16.0), pal.sub);
            }
            View::VideoTrim { file, trim } | View::VideoSnapshot { file, trim } | View::VideoSplit { file, trim, .. } => {
                self.draw_video_trim(ui, rect, pal, &file, trim);
            }
            View::AudioTrim { trim, audio, .. } => {
                self.draw_audio_trim(ui, rect, pal, trim, audio);
            }
            View::BleepAudio { peaks: Some(_), .. } => {
                self.draw_bleep(ui, rect, pal);
            }
            View::BleepAudio { .. } => {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Reading the audio…", FontId::proportional(16.0), pal.sub);
            }
            View::VisualizeAudio { file, orient, style, image, picking } => {
                self.draw_visualize_audio(ui, rect, pal, &file, orient, style, image, picking);
            }
            View::CollageImage { files, opts, color } => {
                self.draw_collage(ui, rect, pal, files, opts, color);
            }
            View::FrameImage { file, size, opts, color, picking } => {
                self.draw_frame(ui, rect, pal, &file, size, opts, color, picking);
            }
            View::RedactImage { size, areas, sel, blur, radius, color, .. } => {
                self.draw_redact(ui, rect, pal, size, areas, sel, (blur, radius), color);
            }
            View::CropImage { size, rect: crop, aspect, .. } => {
                self.draw_crop(ui, rect, pal, size, crop, aspect);
            }
            View::CompressImage { files, orig_bytes, size, mode, kb, percent, preset, max_edge, resize, media } => {
                self.draw_compress(ui, rect, pal, &files, orig_bytes, size, (mode, kb, percent, preset, max_edge, resize), media.as_ref());
            }
            View::EditPhoto { size, opts, .. } => {
                self.draw_edit_photo(ui, rect, pal, size, opts);
            }
            View::Annotate { file, size, doc, tool, color, sel } => {
                self.draw_annotate(ui, rect, pal, &file, size, doc, tool, color, sel);
            }
            View::PdfOrganize { file, n, plan } => {
                self.draw_pdf_organizer(ui, rect, pal, &file, n, plan);
            }
            View::Reorder { purpose, files, cursor } => {
                self.draw_reorder(ui, rect, pal, purpose, files, cursor);
            }
            _ => {
                let data_dir = self.store.root.display().to_string();
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
                    ferrocast_core::models::AiProvider::ClaudeCode => self.settings.claude_code_model.clone(),
                    _ => self.settings.anthropic_model.clone(),
                };
                let mut pick_provider: Option<ferrocast_core::models::AiProvider> = None;
                let mut pick_model: Option<String> = None;
                let mut want_models = false;
                let mut open_url: Option<String> = None;
                let mut check_now = false;
                // 1 install, 2 later, 3 skip (View::Update buttons)
                let mut update_act: u8 = 0;
                // 1 copy, 2 paste into the app, 3 close (View::Translation buttons)
                let mut translation_act: u8 = 0;
                // The match below borrows the view stack mutably; take the icon cache out so these
                // screens can still draw cached symbols.
                let na_sending = self.na.sending;
                let oc_sending = self.oc.sending;
                let oc_vault = self.obsidian_vault().map(|v| v.name).unwrap_or_default();
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
                        View::NotionAdd { title, text, target } => {
                            let (title_id, text_id) = (Id::new("na_title"), Id::new("na_text"));
                            lbl(
                                ui,
                                &format!("Adds to {} ({})", target.title, if target.kind == "data_source" { "database" } else { "page" }),
                            );
                            lbl(ui, "Title (optional)");
                            let r =
                                ui.add(TextEdit::singleline(title).id(title_id).desired_width(f32::INFINITY).hint_text(
                                    if target.kind == "data_source" { "Defaults to the first line" } else { "Added as a heading" },
                                ));
                            // Enter in the title moves on to the text
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                ui.memory_mut(|m| m.request_focus(text_id));
                            }
                            lbl(ui, "Text");
                            let r = ui.add(
                                TextEdit::multiline(text)
                                    .id(text_id)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(9)
                                    .hint_text("Type or paste. Markdown is supported."),
                            );
                            if ff {
                                r.request_focus();
                            }
                            lbl(ui, if na_sending { "Sending…" } else { "Send with ⌘↵ · esc to go back" });
                        }
                        View::ObsidianNew { title, text } => {
                            let text_id = Id::new("on_text");
                            lbl(ui, &format!("Creates a note in {oc_vault}"));
                            lbl(ui, "Title (optional)");
                            let r = ui.add(
                                TextEdit::singleline(title)
                                    .id(Id::new("on_title"))
                                    .desired_width(f32::INFINITY)
                                    .hint_text("Defaults to the first line"),
                            );
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                ui.memory_mut(|m| m.request_focus(text_id));
                            }
                            lbl(ui, "Text");
                            let r = ui.add(
                                TextEdit::multiline(text)
                                    .id(text_id)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(9)
                                    .hint_text("Type or paste. Markdown is supported."),
                            );
                            if ff {
                                r.request_focus();
                            }
                            lbl(ui, if oc_sending { "Sending…" } else { "Send with ⌘↵ · esc to go back" });
                        }
                        View::ObsidianDaily { text } => {
                            lbl(ui, &format!("Adds to today's daily note in {oc_vault}"));
                            lbl(ui, "Text");
                            let r = ui.add(
                                TextEdit::multiline(text)
                                    .id(Id::new("od_text"))
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(11)
                                    .hint_text("Type or paste. Markdown is supported."),
                            );
                            if ff {
                                r.request_focus();
                            }
                            lbl(ui, if oc_sending { "Sending…" } else { "Send with ⌘↵ · esc to go back" });
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
                                egui::Frame::new().fill(pal.field).corner_radius(10.0).inner_margin(egui::Margin::symmetric(12, 8)).show(
                                    ui,
                                    |ui| {
                                        ui.set_width(w - chip_w - 36.0);
                                        let r = ui.add(
                                            TextEdit::singleline(prompt)
                                                .hint_text("Ask anything…")
                                                .font(FontId::proportional(15.0))
                                                .frame(EFrame::NONE)
                                                .desired_width(f32::INFINITY),
                                        );
                                        if ff {
                                            r.request_focus();
                                        }
                                    },
                                );
                                ui.add_space(8.0);
                                let (r, resp) = ui.allocate_exact_size(Vec2::new(chip_w, 34.0), Sense::click());
                                let p = ui.painter();
                                p.rect_filled(r, 10.0, if resp.hovered() { pal.sel } else { pal.field });
                                let g = p.layout_no_wrap(provider_label.clone(), FontId::proportional(11.5), pal.sub);
                                let shown = if g.size().x > chip_w - 34.0 {
                                    elide(p, &provider_label, FontId::proportional(11.5), chip_w - 34.0)
                                } else {
                                    provider_label.clone()
                                };
                                p.text(
                                    Pos2::new(r.min.x + 12.0, r.center().y),
                                    Align2::LEFT_CENTER,
                                    shown,
                                    FontId::proportional(11.5),
                                    pal.text,
                                );
                                p.text(
                                    Pos2::new(r.max.x - 12.0, r.center().y),
                                    Align2::RIGHT_CENTER,
                                    "⌄",
                                    FontId::proportional(12.0),
                                    pal.sub,
                                );
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
                                    ui.label(
                                        RichText::new("Short answers, in a conversation that lasts while this window is open.")
                                            .size(12.5)
                                            .color(pal.sub),
                                    );
                                    ui.add_space(16.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                                        for ex in [
                                            "Explain this regex: ^\\d{3}-\\d{4}$",
                                            "Write a git commit message for a bug fix",
                                            "What is 18% of 240, step by step?",
                                            "Translate “see you tomorrow” to Spanish",
                                        ] {
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
                                            egui::Frame::new()
                                                .fill(pal.field)
                                                .corner_radius(12.0)
                                                .inner_margin(egui::Margin::symmetric(12, 8))
                                                .show(ui, |ui| {
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
                                                        ui.painter().circle_filled(
                                                            Pos2::new(r.min.x + 6.0 + k as f32 * 10.0, r.center().y),
                                                            3.0,
                                                            Color32::from_rgba_unmultiplied(pal.sub.r(), pal.sub.g(), pal.sub.b(), al),
                                                        );
                                                    }
                                                    ui.ctx().request_repaint_after(Duration::from_millis(60));
                                                } else {
                                                    ui.add(
                                                        egui::Label::new(RichText::new(a).size(14.0).color(pal.text))
                                                            .wrap()
                                                            .selectable(true),
                                                    );
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
                        View::Shortcuts => {
                            let groups: [(&str, &[(&str, &str)]); 4] = [
                                (
                                    "Navigate",
                                    &[
                                        ("⌥ Space", "Open or close Ferrocast"),
                                        ("↑ ↓", "Move the selection"),
                                        ("esc", "Clear the search, go back, or close"),
                                        ("⌫", "Go back when the search is empty"),
                                    ],
                                ),
                                (
                                    "Act",
                                    &[
                                        ("↵", "Run the primary action"),
                                        ("⌘ ↵", "Run the secondary action"),
                                        ("⌘ K", "Open the action panel"),
                                        ("⌘ Y", "Quick Look the selected file"),
                                    ],
                                ),
                                (
                                    "Manage",
                                    &[
                                        ("⌘ ,", "Open Settings"),
                                        ("⌘ N", "New snippet, quicklink or note"),
                                        ("⌘ E", "Edit the selected snippet or quicklink"),
                                        ("⇧ ⌘ F", "Add or remove a favorite"),
                                        ("⇧ ⌘ ,", "Set a hotkey for a command"),
                                        ("⌘ .", "Pin or unpin a clipboard entry"),
                                        ("⌃ X", "Delete the selected entry"),
                                    ],
                                ),
                                (
                                    "Notes",
                                    &[
                                        ("⌘ N", "New note"),
                                        ("⌘ P", "Browse Notes"),
                                        ("⌘ K", "Open the action panel"),
                                        ("⇧ ⌘ P", "Pin or unpin the note"),
                                        ("⌘ [", "Go back to the previous note"),
                                        ("⌘ ]", "Go forward to the next note"),
                                    ],
                                ),
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
                        View::Update { release } => {
                            egui::ScrollArea::vertical().id_salt("update").auto_shrink([false; 2]).show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 6.0;
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("Ferrocast {} is available", release.tag.trim_start_matches('v')))
                                        .size(20.0)
                                        .color(pal.text),
                                );
                                ui.label(RichText::new(format!("You have {}.", env!("CARGO_PKG_VERSION"))).size(13.0).color(pal.sub));
                                ui.add_space(8.0);
                                let notes: String = release.notes.lines().take(40).collect::<Vec<_>>().join("\n");
                                let notes = if notes.trim().is_empty() { "No release notes.".to_string() } else { notes };
                                group(ui, pal, |ui| {
                                    ui.label(RichText::new(notes).size(13.0).color(pal.text));
                                });
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    if accent_btn(ui, pal, "Install and relaunch") {
                                        update_act = 1;
                                    }
                                    if small_btn(ui, pal, "Later") {
                                        update_act = 2;
                                    }
                                    if small_btn(ui, pal, "Skip this version") {
                                        update_act = 3;
                                    }
                                });
                            });
                        }
                        View::Translation { lang, text } => {
                            ui.label(RichText::new(format!("Translated to {lang}")).size(15.0).strong().color(pal.text));
                            ui.add_space(6.0);
                            egui::ScrollArea::vertical()
                                .id_salt("translation")
                                .auto_shrink([false; 2])
                                .max_height((ui.available_height() - 44.0).max(60.0))
                                .show(ui, |ui| {
                                    ui.add(egui::Label::new(RichText::new(text.as_str()).size(14.0).color(pal.text)).selectable(true));
                                });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                if accent_btn(ui, pal, "Copy") {
                                    translation_act = 1;
                                }
                                if small_btn(ui, pal, "Paste into app") {
                                    translation_act = 2;
                                }
                                if small_btn(ui, pal, "Close") {
                                    translation_act = 3;
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
                                    ui.label(
                                        RichText::new(
                                            "A small, fast launcher for the Mac. Open source, no account, your data in files you can read.",
                                        )
                                        .size(13.5)
                                        .color(pal.text),
                                    );
                                    ui.add_space(12.0);
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        let total = 4.0 * 96.0 + 3.0 * 8.0;
                                        ui.add_space((ui.available_width() - total).max(0.0) / 2.0);
                                        if small_btn(ui, pal, "GitHub") {
                                            open_url = Some("https://github.com/drumst0ck/ferrocast".into());
                                        }
                                        if small_btn(ui, pal, "Changelog") {
                                            open_url = Some("https://ferrocast.app/changelog/".into());
                                        }
                                        if small_btn(ui, pal, "Data folder") {
                                            open_url = Some(data_dir.clone());
                                        }
                                        if small_btn(ui, pal, "Check for Updates…") {
                                            check_now = true;
                                        }
                                    });
                                });
                                ui.add_space(18.0);
                                group(ui, pal, |ui| {
                                    row(ui, pal, "Licence", "MIT. Fork it, fix it, ship it.", |ui| keycap(ui, pal, "MIT"));
                                    row(
                                        ui,
                                        pal,
                                        "Currency rates",
                                        "European Central Bank, through frankfurter.dev, cached for 12 hours.",
                                        |_ui| {},
                                    );
                                    row(
                                        ui,
                                        pal,
                                        "Trademarks",
                                        "Raycast is a trademark of its owner. Ferrocast is an independent project.",
                                        |_ui| {},
                                    );
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
                        P::ClaudeCode => self.settings.claude_code_model = m,
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
                if check_now {
                    self.check_updates();
                }
                if translation_act != 0 {
                    if let View::Translation { text, .. } = self.view().clone() {
                        match translation_act {
                            1 => {
                                clipboard::write_text(&text);
                                self.shared.mark_clipboard_write();
                                self.toast("Copied");
                            }
                            2 => self.paste_text(&text),
                            _ => self.reset_to_root(),
                        }
                    }
                }
                if update_act != 0 {
                    if let View::Update { release } = self.view().clone() {
                        self.pop();
                        match update_act {
                            1 => {
                                let exe = instance::running_exe().unwrap_or_default();
                                self.start_update(release, ferrocast_core::update::channel_of(&exe), services::parent_writable_cached());
                            }
                            3 => self.skip_update(&release),
                            _ => {}
                        }
                    }
                }
                if clear_hotkey {
                    if let View::Hotkey { id, .. } = self.view().clone() {
                        self.settings.command_mut(&id).hotkey = Some(String::new());
                        self.save_settings();
                        self.apply_hotkeys();
                        self.toast("Hotkey removed");
                        self.pop();
                    }
                }
            }
        }
    }

    /// The content pane while the Settings search has a query: up to 8 result rows (tab chip,
    /// title with the match in the accent colour), or the empty state. Returns the clicked result.
    fn draw_settings_results(
        &mut self,
        ui: &mut Ui,
        pal: &Palette,
        query: &str,
        hits: &[&'static ferrocast_core::settings_index::Entry],
        sel: &mut usize,
    ) -> Option<&'static ferrocast_core::settings_index::Entry> {
        let ctx = ui.ctx().clone();
        ui.label(RichText::new("Search results").size(20.0).strong().color(pal.text));
        ui.add_space(14.0);
        if hits.is_empty() {
            let text = format!("No setting matches \u{201c}{}\u{201d}", query.trim());
            ui.add_space(26.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(&text).size(14.5).color(pal.text));
                ui.add_space(6.0);
                record_text(&ctx, pal, "note", pal.sub_strong, &[], 1.0);
                ui.label(RichText::new("Try a shorter word, for example hotkey, model or clipboard.").size(12.0).color(pal.sub_strong));
            });
            #[cfg(test)]
            ctx.data_mut(|d| d.insert_temp(Id::new("settings-empty-state"), text));
            return None;
        }
        #[cfg(test)]
        ctx.data_mut(|d| {
            d.insert_temp(Id::new("settings-results"), hits.iter().map(|e| e.title.to_string()).collect::<Vec<_>>());
            d.insert_temp(Id::new("settings-result-rects"), Vec::<(String, Rect)>::new());
        });
        let moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO);
        let mut clicked = None;
        ui.spacing_mut().item_spacing.y = 4.0;
        for (i, en) in hits.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            focus_ring(ui, pal, &resp, r);
            #[cfg(test)]
            ctx.data_mut(|d| {
                d.get_temp_mut_or_default::<Vec<(String, Rect)>>(Id::new("settings-result-rects")).push((en.title.to_string(), r))
            });
            if resp.hovered() && moved {
                *sel = i;
            }
            record_ctl(&ctx, "result", resp.id, r, true);
            let on = i == *sel;
            let row_fill = if on { pal.sel } else { pal.field };
            record_text(&ctx, pal, "result-chip", pal.sub_strong, &[row_fill, pal.bg.gamma_multiply(0.55)], 1.0);
            record_text(&ctx, pal, "result-title", pal.text, &[row_fill], 1.0);
            record_text(&ctx, pal, "result-accent", pal.accent_text, &[row_fill], 1.0);
            ui.painter().rect_filled(r, 10.0, row_fill);
            if on {
                ui.painter().rect_stroke(r.shrink(0.5), 10.0, Stroke::new(1.0, pal.accent.gamma_multiply(0.7)), egui::StrokeKind::Inside);
            }
            let chip = Rect::from_min_size(Pos2::new(r.min.x + 12.0, r.center().y - 10.0), Vec2::new(74.0, 20.0));
            ui.painter().rect_filled(chip, 6.0, pal.bg.gamma_multiply(0.55));
            ui.painter().text(chip.center(), Align2::CENTER_CENTER, SETTINGS_TABS[en.tab].0, FontId::proportional(11.0), pal.sub_strong);
            // the title, its match in the accent colour
            let font = FontId::proportional(13.5);
            let mut job = egui::text::LayoutJob::default();
            let chars: Vec<char> = en.title.chars().collect();
            let (ms, ml) = ferrocast_core::settings_index::match_range(en.title, query).unwrap_or((0, 0));
            // the match is accented only when it is a proper part of the title: a title that is
            // the whole match stays in the normal colour
            let whole = ms == 0 && ml == chars.len();
            let hl = if whole { pal.text } else { pal.accent_text };
            #[cfg(test)]
            ctx.data_mut(|d| {
                let part = (!whole && ml > 0).then(|| chars[ms..ms + ml].iter().collect::<String>());
                d.get_temp_mut_or_default::<Vec<(String, Option<String>)>>(Id::new("settings-result-accent"))
                    .push((en.title.to_string(), part))
            });
            for (from, to, color) in [(0, ms, pal.text), (ms, ms + ml, hl), (ms + ml, chars.len(), pal.text)] {
                if from < to {
                    let part: String = chars[from..to].iter().collect();
                    job.append(&part, 0.0, egui::text::TextFormat::simple(font.clone(), color));
                }
            }
            let galley = ui.painter().layout_job(job);
            ui.painter().galley(Pos2::new(chip.max.x + 12.0, r.center().y - galley.size().y / 2.0), galley, pal.text);
            if on {
                ui.painter().text(
                    Pos2::new(r.max.x - 14.0, r.center().y),
                    Align2::RIGHT_CENTER,
                    "↵",
                    FontId::proportional(13.0),
                    pal.sub_strong,
                );
            }
            if resp.clicked() {
                clicked = Some(*en);
            }
        }
        clicked
    }

    fn draw_settings(&mut self, ui: &mut Ui, pal: &Palette) {
        let ctx = ui.ctx().clone();
        let tab_id = Id::new("settings_tab");
        let mut tab: usize = ctx.data(|d| d.get_temp(tab_id)).unwrap_or(0);
        let tab_before = tab;
        let mut s = self.settings.clone();
        let trusted = windows::accessibility_trusted(false);
        let (mut rec, mut req_acc, mut open_data, mut add_dir, mut save_key, mut test_ai) = (false, false, false, false, false, false);
        let (mut chatgpt_in, mut chatgpt_out, mut chatgpt_device) = (false, false, false);
        let mut fetch_models = false;
        let (mut check_now, mut install_now) = (false, false);
        let (mut new_snippet, mut create_script) = (false, false);
        let mut reset_req: Option<usize> = None;
        let update_channel = {
            let id = Id::new("update_channel");
            match ctx.data(|d| d.get_temp::<ferrocast_core::update::Channel>(id)) {
                Some(c) => c,
                None => {
                    let c = ferrocast_core::update::channel_of(&crate::instance::running_exe().unwrap_or_default());
                    ctx.data_mut(|d| d.insert_temp(id, c));
                    c
                }
            }
        };
        let update_available = self.update_available.clone();
        let models_key = services::models_key(&self.settings);
        let models_state = self.ai_models.get(&models_key).cloned();
        // The Translate tab lists the models of ITS provider, with the chat's server address and key.
        let translate_models_state =
            fetched_translate_settings(&self.settings).and_then(|ts| self.ai_models.get(&services::models_key(&ts)).cloned());
        let mut fetch_translate_models = false;
        let apple_ok = cached_apple_available(&ctx, self.fx.apple_available);
        let mut open_language_settings = false;
        let mut recheck_apple = false;
        let apple_states = self.apple_langs.clone();
        let mut remove_dir = None;
        let mut enable = None;
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut key_scratch = std::mem::take(&mut self.key_scratch);
        let mut ntoken = std::mem::take(&mut self.notion.token);
        let mut nquery = std::mem::take(&mut self.notion.query);
        let (mut notion_save, mut notion_test, mut notion_search, mut notion_clear, mut notion_open) = (false, false, false, false, false);
        let mut notion_pick: Option<usize> = None;
        let has_notion = if tab == INTEGRATIONS_TAB {
            match ctx.data(|d| d.get_temp::<bool>(Id::new("notion_has_token"))) {
                Some(v) => v,
                None => {
                    let v = notion_io::has_token();
                    ctx.data_mut(|d| d.insert_temp(Id::new("notion_has_token"), v));
                    v
                }
            }
        } else {
            false
        };
        let notion_rows: Vec<(String, &'static str)> = self
            .notion
            .results
            .iter()
            .map(|i| (i.title.clone(), if i.kind == ferrocast_core::notion::ItemKind::Page { "page" } else { "database" }))
            .collect();
        let notion_busy = self.notion.searching;
        let notion_target = self.settings.notion_target.clone();
        // the first time the tab is drawn the vaults and the app are looked up on a thread (tests start it by hand)
        if tab == INTEGRATIONS_TAB && !self.obsidian.started && !cfg!(test) {
            self.obsidian_scan();
        }
        let obs_vaults = self.obsidian.vaults.clone();
        let obs_open = self.obsidian.open.clone();
        let obs_installed = self.obsidian.installed;
        let obs_picking = self.obsidian.picking;
        let obs_vault = self.obsidian_vault();
        let (mut obs_pick, mut obs_choose, mut obs_clear): (Option<usize>, bool, bool) = (None, false, false);
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

        // ---- search state: the query and the selected result live in egui temp data
        let query_id = Id::new("settings_query");
        let sel_id = Id::new("settings-selected");
        let search_id = Id::new("settings-search");
        let mut query: String = ctx.data(|d| d.get_temp(query_id)).unwrap_or_default();
        let query_before = query.clone();
        let mut sel: usize = ctx.data(|d| d.get_temp(sel_id)).unwrap_or(0);
        #[cfg(test)]
        ctx.data_mut(|d| {
            d.remove::<Vec<(usize, String)>>(Id::new("settings-row-titles"));
            d.remove::<Vec<String>>(Id::new("settings-results"));
            d.remove::<Vec<(String, Rect)>>(Id::new("settings-result-rects"));
            d.remove::<String>(Id::new("settings-empty-state"));
            d.remove::<Vec<(String, String)>>(Id::new("status-chips"));
            d.remove::<Vec<String>>(Id::new("settings-row-disabled"));
            d.remove::<Vec<String>>(Id::new("settings-sidebar-active"));
            d.remove::<Vec<(String, Option<String>)>>(Id::new("settings-result-accent"));
            d.remove::<Vec<(usize, String)>>(Id::new("settings-pane-empty"));
            d.remove::<Rect>(Id::new("settings-empty-button"));
            d.remove::<Rect>(Id::new("settings-reset-button"));
        });
        // an unused jump (its row is not on the tab) does not wait for ever
        if let Some(j) = ctx.data(|d| d.get_temp::<SettingsJump>(Id::new("settings-jump"))) {
            if j.t0.is_none() && ctx.cumulative_pass_nr().saturating_sub(j.pass0) > 60 {
                ctx.data_mut(|d| d.remove::<SettingsJump>(Id::new("settings-jump")));
            }
        }
        if ctx.input_mut(|inp| inp.consume_key(CMD, Key::F)) {
            ctx.memory_mut(|m| m.request_focus(search_id));
        }
        let mut opened = false;
        {
            // Up/Down/Enter act on the results of the query as of the last frame
            let shown = if query.trim().is_empty() { vec![] } else { ferrocast_core::settings_index::search(&query) };
            sel = sel.min(shown.len().saturating_sub(1));
            if !shown.is_empty() && ctx.memory(|m| m.has_focus(search_id)) {
                let (up, down, enter) = ctx.input_mut(|inp| {
                    (
                        inp.consume_key(Modifiers::NONE, Key::ArrowUp),
                        inp.consume_key(Modifiers::NONE, Key::ArrowDown),
                        inp.consume_key(Modifiers::NONE, Key::Enter),
                    )
                });
                if up {
                    sel = sel.saturating_sub(1);
                }
                if down {
                    sel = (sel + 1).min(shown.len() - 1);
                }
                if enter {
                    open_settings_result(&ctx, shown[sel], &mut tab, &mut query);
                    ctx.memory_mut(|m| m.surrender_focus(search_id));
                    opened = true;
                }
            }
        }

        // ---- sidebar keyboard: ⌘1…⌘9 anywhere in Settings; ↑/↓ only when the pointer is over the
        // sidebar (and no text field is being edited) or a sidebar item has the keyboard focus
        let item_id = |i: usize| Id::new(("settings-tab-item", i));
        for (i, key) in SETTINGS_NUM_KEYS.iter().enumerate() {
            if ctx.input_mut(|inp| inp.consume_key(CMD, *key)) {
                tab = i;
            }
        }
        let focused_item = (0..SETTINGS_TABS.len()).find(|i| ctx.memory(|m| m.has_focus(item_id(*i))));
        let over_side = ctx.input(|inp| inp.pointer.hover_pos()).is_some_and(|p| side.contains(p));
        if focused_item.is_some() || (over_side && !ctx.egui_wants_keyboard_input()) {
            let (up, down) =
                ctx.input_mut(|inp| (inp.consume_key(Modifiers::NONE, Key::ArrowUp), inp.consume_key(Modifiers::NONE, Key::ArrowDown)));
            let from = focused_item.unwrap_or(tab);
            let to = if up {
                from.saturating_sub(1)
            } else if down {
                (from + 1).min(SETTINGS_TABS.len() - 1)
            } else {
                from
            };
            if up || down {
                tab = to;
                if focused_item.is_some() {
                    ctx.memory_mut(|m| m.request_focus(item_id(to)));
                }
            }
        }

        // ---- sidebar
        #[cfg(test)]
        let mut marks: (Vec<(String, Rect)>, Vec<Rect>) = (vec![], vec![]);
        #[cfg(test)]
        let mut active_names: Vec<String> = vec![];
        ui.scope_builder(UiBuilder::new().max_rect(side), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            // search field: the Settings field look, a magnifier on the left, a clear button on the right
            let sr = search_field(ui, pal, &mut query, side.width(), "Search settings", search_id);
            #[cfg(test)]
            ctx.data_mut(|d| d.insert_temp(Id::new("settings-search-rect"), sr.rect));
            let mg = Rect::from_center_size(Pos2::new(sr.rect.min.x + 16.0, sr.rect.center().y), Vec2::splat(15.0));
            record_text(&ctx, pal, "search-icon", pal.sub_strong, &[pal.field.gamma_multiply(1.6)], 1.0);
            match self.icons.texture(&ctx, &Icon::Glyph("sf:magnifyingglass", [0, 0, 0])) {
                Some(t) => {
                    ui.painter().image(t.id(), mg, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), pal.sub_strong);
                }
                None => {
                    ui.painter().text(mg.center(), Align2::CENTER_CENTER, "⌕", FontId::proportional(15.0), pal.sub_strong);
                }
            }
            if !query.is_empty() {
                let xr = Rect::from_center_size(Pos2::new(sr.rect.max.x - 14.0, sr.rect.center().y), Vec2::splat(28.0));
                let xe = ui.interact(xr, Id::new("settings-search-clear"), Sense::click()).on_hover_text("Clear search");
                record_ctl(&ctx, "search-clear", xe.id, xr, true);
                record_text(&ctx, pal, "search-clear", pal.sub_strong, &[pal.field.gamma_multiply(1.6)], 1.0);
                focus_ring(ui, pal, &xe, xr.shrink(2.0));
                ui.painter().text(
                    xr.center(),
                    Align2::CENTER_CENTER,
                    "×",
                    FontId::proportional(16.0),
                    if xe.hovered() { pal.text } else { pal.sub_strong },
                );
                if xe.clicked() {
                    query.clear();
                    ctx.memory_mut(|m| m.request_focus(search_id));
                }
            }
            ui.add_space(6.0);
            // the tabs scroll when they do not fit (nine tabs need more than the window's height)
            egui::ScrollArea::vertical().id_salt("settings-sidebar-scroll").auto_shrink([false; 2]).show(ui, |ui| {
                for (i, (name, symbol)) in SETTINGS_TABS.iter().enumerate() {
                    if SETTINGS_GROUP_STARTS.contains(&i) {
                        // thin divider between the Everyday / Intelligence / Power groups
                        ui.add_space(8.0);
                        let (dr, _) = ui.allocate_exact_size(Vec2::new(side.width(), 1.0), Sense::hover());
                        ui.painter().hline(dr.x_range(), dr.center().y, Stroke::new(1.0, pal.sep));
                        ui.add_space(8.0);
                        #[cfg(test)]
                        marks.1.push(dr);
                    }
                    let id = item_id(i);
                    ui.ctx().memory_mut(|m| {
                        m.set_focus_lock_filter(id, egui::EventFilter { vertical_arrows: true, ..Default::default() });
                    });
                    let (r, _) = ui.allocate_exact_size(Vec2::new(side.width(), 34.0), Sense::hover());
                    let resp = ui.interact(r, id, Sense::click()).on_hover_text(format!("{name}  ⌘{}", i + 1));
                    record_ctl(&ctx, "sidebar", resp.id, r, true);
                    #[cfg(test)]
                    marks.0.push((name.to_string(), r));
                    // while a search shows its results no tab is the current one
                    let on = i == tab && query.trim().is_empty();
                    #[cfg(test)]
                    if on {
                        active_names.push(name.to_string());
                    }
                    if on {
                        ui.painter().rect_filled(r, 8.0, pal.sel);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 8.0, pal.field);
                    }
                    let gr = Rect::from_center_size(Pos2::new(r.min.x + 20.0, r.center().y), Vec2::splat(20.0));
                    ui.painter().rect_filled(gr, 5.0, if on { pal.accent } else { pal.field });
                    let tint = if on { Color32::WHITE } else { pal.sub_strong };
                    if on {
                        record_text(&ctx, pal, "side-icon-active", tint, &[pal.sel, pal.accent], 1.0);
                        record_text(&ctx, pal, "side-label-active", pal.text, &[pal.sel], 1.0);
                    } else {
                        record_text(&ctx, pal, "side-icon", tint, &[pal.field], 1.0);
                        record_text(&ctx, pal, "side-label", pal.sub_strong, &[], 1.0);
                        record_text(&ctx, pal, "side-label-hover", pal.sub_strong, &[pal.field], 1.0);
                    }
                    match self.icons.texture(&ctx, &Icon::Glyph(symbol, [0, 0, 0])) {
                        Some(t) => {
                            ui.painter().image(t.id(), gr, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);
                        }
                        None => {
                            ui.painter().circle_filled(gr.center(), 2.0, tint);
                        }
                    }
                    ui.painter().text(
                        Pos2::new(r.min.x + 38.0, r.center().y),
                        Align2::LEFT_CENTER,
                        name,
                        FontId::proportional(13.5),
                        if on { pal.text } else { pal.sub_strong },
                    );
                    focus_ring(ui, pal, &resp, r.shrink(1.0));
                    if on {
                        // bring the current tab into view when it changes (and when Settings opens on it)
                        let seen = Id::new("settings-sidebar-seen-tab");
                        if ctx.data(|d| d.get_temp::<usize>(seen)) != Some(i) {
                            ui.scroll_to_rect(r, None);
                            ctx.data_mut(|d| d.insert_temp(seen, i));
                        }
                    }
                    if resp.clicked() {
                        tab = i;
                    }
                }
            });
        });
        #[cfg(test)]
        ctx.data_mut(|d| {
            d.insert_temp(Id::new("settings-sidebar"), marks);
            d.insert_temp(Id::new("settings-sidebar-active"), active_names);
        });
        if tab != tab_before {
            if !opened {
                // choosing a tab (click, Cmd+N, arrows) leaves the search
                query.clear();
            }
            ctx.request_repaint();
        }
        if query != query_before {
            sel = 0;
            if query.trim().is_empty() != query_before.trim().is_empty() {
                // the header's breadcrumb reads the query from the previous frame
                ctx.request_repaint();
            }
        }
        let searching = !query.trim().is_empty();
        let hits = if searching { ferrocast_core::settings_index::search(&query) } else { vec![] };
        sel = sel.min(hits.len().saturating_sub(1));
        let mut clicked_result: Option<&'static ferrocast_core::settings_index::Entry> = None;
        ctx.data_mut(|d| d.insert_temp(tab_id, tab));
        ui.painter().vline(side.max.x + 9.0, pane.y_range(), Stroke::new(1.0, pal.sep));

        // ---- pane
        let acc = pal.accent;
        ui.scope_builder(UiBuilder::new().max_rect(pane), |ui| {
            let mut area = egui::ScrollArea::vertical().id_salt("settings_pane").auto_shrink([false; 2]);
            // dev aid: the screenshot hook scrolls the pane to a group
            if let Some(y) = ui.ctx().data(|d| d.get_temp::<f32>(Id::new("dev-settings-scroll"))) {
                area = area.vertical_scroll_offset(y);
            }
            area.show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(2.0);
                if searching {
                    clicked_result = self.draw_settings_results(ui, pal, &query, &hits, &mut sel);
                } else {
                ui.label(RichText::new(SETTINGS_TABS[tab].0).size(20.0).strong().color(pal.text));
                ui.add_space(14.0);
                match tab {
                    0 => {
                        // the three wheel keys only matter while the drop wheel is on
                        let wheel_off = !s.drop_wheel;
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
                            row_disabled_if(ui, wheel_off, |ui| {
                                row(ui, pal, "Convert wheel key", "Tap during a file drag for conversions.", |ui| {
                                    let opts: Vec<(String, &str)> = ["Shift", "Control", "Option", "Command"].map(|v| (v.to_string(), v)).to_vec();
                                    segmented(ui, pal, &mut s.wheel_convert_key, &opts);
                                });
                            });
                            row_disabled_if(ui, wheel_off, |ui| {
                                row(ui, pal, "Tools wheel key", "Tap during a file drag for compress, crop, redact…", |ui| {
                                    let opts: Vec<(String, &str)> = ["Shift", "Control", "Option", "Command"].map(|v| (v.to_string(), v)).to_vec();
                                    segmented(ui, pal, &mut s.wheel_tools_key, &opts);
                                });
                            });
                            row_disabled_if(ui, wheel_off, |ui| {
                                row(ui, pal, "Actions wheel key", "Tap during a file drag for copy, move, AirDrop…", |ui| {
                                    let opts: Vec<(String, &str)> = ["Shift", "Control", "Option", "Command"].map(|v| (v.to_string(), v)).to_vec();
                                    segmented(ui, pal, &mut s.wheel_actions_key, &opts);
                                });
                            });
                            row(
                                ui,
                                pal,
                                "Enter on Finder selection",
                                "Press Return on selected files in Finder to open the Convert wheel instead of renaming. Needs Accessibility access. Off by default.",
                                |ui| {
                                    toggle(ui, pal, &mut s.enter_on_finder);
                                    #[cfg(test)]
                                    ui.ctx().data_mut(|d| d.insert_temp(Id::new("enter-on-finder-toggle"), ui.min_rect()));
                                },
                            );
                            row(ui, pal, "Menu bar icon", "A small icon to open Ferrocast with the mouse. Needs a restart.", |ui| toggle(ui, pal, &mut s.show_menu_bar_icon));
                        });
                        ui.add_space(14.0);
                        section(ui, pal, "Updates");
                        group(ui, pal, |ui| {
                            row(
                                ui,
                                pal,
                                "Check for updates automatically",
                                "Checks GitHub once a day. Off means Ferrocast never contacts GitHub by itself.",
                                |ui| toggle(ui, pal, &mut s.update_check),
                            );
                            let updates_off = !s.update_check;
                            row_disabled_if(ui, updates_off, |ui| {
                                row(
                                    ui,
                                    pal,
                                    "Install updates",
                                    "Ask first shows the release notes and lets you skip a version. Automatically installs and relaunches.",
                                    |ui| {
                                        use ferrocast_core::models::UpdateInstall as U;
                                        segmented(ui, pal, &mut s.update_install, &[(U::AskFirst, "Ask first"), (U::Automatic, "Automatically")]);
                                    },
                                );
                            });
                            let checked = match s.update_last_check {
                                Some(t) => format!("Checked {}", services::ago(services::now_secs() - t)),
                                None => "Checked never".to_string(),
                            };
                            let desc = match &update_available {
                                Some(r) => format!(
                                    "Version {} · {checked} · {} available",
                                    env!("CARGO_PKG_VERSION"),
                                    r.tag.trim_start_matches('v')
                                ),
                                None => format!("Version {} · {checked}", env!("CARGO_PKG_VERSION")),
                            };
                            row(ui, pal, "Version", &desc, |ui| {
                                if update_available.is_some() && accent_btn(ui, pal, "Install") {
                                    install_now = true;
                                }
                                if small_btn(ui, pal, "Check now") {
                                    check_now = true;
                                }
                            });
                        });
                        match update_channel {
                            ferrocast_core::update::Channel::Brew => {
                                ui.add_space(10.0);
                                note(ui, pal, "Installed with Homebrew: updates run brew upgrade ferrocast.");
                            }
                            ferrocast_core::update::Channel::Dev => {
                                ui.add_space(10.0);
                                note(ui, pal, "Development build: updates are checked but never installed.");
                            }
                            ferrocast_core::update::Channel::Direct => {}
                        }
                        ui.add_space(14.0);
                        section(ui, pal, "Root search");
                        group(ui, pal, |ui| {
                            row(ui, pal, "Sensitivity", "Low matches loosely; High needs every letter to start a word.", |ui| {
                                use ferrocast_core::fuzzy::Sensitivity as S;
                                segmented(ui, pal, &mut s.sensitivity, &[(S::Low, "Low"), (S::Medium, "Medium"), (S::High, "High")]);
                            });
                            row(ui, pal, "Include files", "Show Spotlight file results after apps and commands.", |ui| toggle(ui, pal, &mut s.file_search_in_root));
                            row(ui, pal, "Web search fallback", "Used by “Search the web for …”. {query} is replaced.", |ui| {
                                field(ui, pal, &mut s.fallback_search_url, 470.0, "https://…?q={query}", false);
                            });
                        });
                    }
                    1 => {
                        group(ui, pal, |ui| {
                            row(ui, pal, "Record history", "Keep what you copy so you can paste it later. Password managers are skipped.", |ui| toggle(ui, pal, &mut s.clipboard_enabled));
                            let history_off = !s.clipboard_enabled;
                            row_disabled_if(ui, history_off, |ui| {
                                row(ui, pal, "Keep history for", "Older entries are removed automatically. Pinned entries stay.", |ui| {
                                    field_combo(ui, pal, "retention", s.clipboard_retention.label(), 120.0, |ui| {
                                        for r in Retention::ALL {
                                            ui.selectable_value(&mut s.clipboard_retention, r, r.label());
                                        }
                                    });
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
                        if self.snippets.is_empty() {
                            ui.add_space(14.0);
                            let line = "A snippet is text you reuse: type its keyword anywhere and it expands.";
                            if empty_state(ui, pal, 2, "No snippets yet", line, "New snippet") {
                                new_snippet = true;
                            }
                        }
                    }
                    3 => {
                        use ferrocast_core::models::AiProvider as P;
                        let prov = s.ai_provider;
                        let ready = ai_readiness(prov, has_key, prov == P::ChatGpt && cached_chatgpt_signed(&ctx), prov == P::ClaudeCode && services::claude_code_path().is_some());
                        group(ui, pal, |ui| {
                            row_stacked_chip(ui, pal, "Provider", match prov {
                                P::OpenAiCompatible => "Any server speaking the OpenAI API: Ollama on this Mac, LM Studio, OpenAI, OpenRouter…",
                                P::Anthropic => "Claude through Anthropic's API, billed to your API key. A ChatGPT or Claude subscription does not include API access.",
                                P::ClaudeCode => "Runs the official Claude Code command-line tool, signed in with your own Claude subscription. Slower to start (a few seconds) but no API key.",
                                P::ChatGpt => "Signs in with your ChatGPT account the way OpenAI's Codex CLI does and uses your ChatGPT plan. No API key.",
                            }, Some((ready.0, ready.1)), |ui| {
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
                                    row_chip(ui, pal, "Server", "Base URL ending in /v1.", url_problem(&s.ai_base_url).map(|m| (StatusKind::Warn, m)), |ui| {
                                        field(ui, pal, &mut s.ai_base_url, 470.0, "http://localhost:11434/v1", false);
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
                                        field(ui, pal, &mut key_scratch, 220.0, if has_key { "••••••••" } else { "sk-…" }, true);
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
                                        field(ui, pal, &mut key_scratch, 220.0, if has_key { "••••••••" } else { "sk-ant-…" }, true);
                                    });
                                });
                            }
                            P::ChatGpt => {
                                section(ui, pal, "ChatGPT account");
                                let signed = cached_chatgpt_signed(&ctx);
                                group(ui, pal, |ui| {
                                    row(ui, pal, "Account", if signed { "Signed in. The session is stored in the macOS Keychain and refreshed automatically." } else { "Sign in opens your browser and listens on port 1455 for the redirect. If that port is taken (Codex, Pi), it switches to a one-time code you type on openai.com." }, |ui| {
                                        if signed {
                                            if small_btn(ui, pal, "Sign out") {
                                                chatgpt_out = true;
                                            }
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
                                    row(ui, pal, "Command-line tool", &path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not found. Install from claude.ai/code, then run `claude` once in Terminal to sign in.".into()), |_ui| {});
                                    row(ui, pal, "Model", "Claude Code's default, one of its aliases, or Custom… for a full model id.", |ui| {
                                        if model_picker(ui, pal, "claudecode", &mut s.claude_code_model, "Claude Code default", models_state.as_ref()) {
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
                    TRANSLATE_TAB => {
                        use ferrocast_core::models::{AiProvider as P, TranslateOutput as T};
                        section(ui, pal, "Wheel");
                        group(ui, pal, |ui| {
                            let wheel_off = !s.drop_wheel;
                            row_disabled_if(ui, wheel_off, |ui| {
                                row(ui, pal, "Translate wheel key", "Tap during a text drag for the Translate wheel. Not Shift: it extends the selection.", |ui| {
                                    let opts: Vec<(String, &str)> = ["Control", "Option", "Command"].map(|v| (v.to_string(), v)).to_vec();
                                    segmented(ui, pal, &mut s.wheel_translate_key, &opts);
                                });
                            });
                            let id = Id::new("translate_langs_buf");
                            let typed: String = ctx.data(|d| d.get_temp(id)).unwrap_or_else(|| s.translate_languages.join(", "));
                            let lang_note = ferrocast_core::translate::language_note(&typed);
                            let lang_kind = if lang_note.warn { StatusKind::Warn } else { StatusKind::Neutral };
                            row_chip(ui, pal, "Languages", "Drag selected text and tap the translate key. Comma-separated, up to 12.", Some((lang_kind, &lang_note.text)), |ui| {
                                let mut buf = typed.clone();
                                if field(ui, pal, &mut buf, 470.0, "English, Spanish, French…", false).changed() {
                                    s.translate_languages = ferrocast_core::translate::parse_list(&buf);
                                }
                                ui.ctx().data_mut(|d| d.insert_temp(id, buf));
                            });
                        });
                        ui.add_space(14.0);
                        section(ui, pal, "Result");
                        group(ui, pal, |ui| {
                            row_stacked_chip(
                                ui,
                                pal,
                                "After translating",
                                "Auto replaces the selection where the app allows it and shows a window otherwise.",
                                None,
                                |ui| {
                                    segmented(
                                        ui,
                                        pal,
                                        &mut s.translate_output,
                                        &[(T::Auto, "Auto"), (T::Show, "Window"), (T::Replace, "Paste ⌘V"), (T::Copy, "Copy only")],
                                    );
                                },
                            );
                            card_note(ui, pal, PASTE_NOTE);
                            let auto = s.translate_output == T::Auto;
                            #[cfg(test)]
                            ui.ctx().data_mut(|d| {
                                if auto {
                                    d.insert_temp(Id::new("auto-js-note"), true);
                                } else {
                                    d.remove::<bool>(Id::new("auto-js-note"));
                                }
                            });
                            if auto {
                                card_note(ui, pal, AUTO_JS_NOTE);
                            }
                            row(ui, pal, "Translating effect", "Show a glow on the text while it is being translated.", |ui| toggle(ui, pal, &mut s.translate_effect));
                        });
                        ui.add_space(14.0);
                        section(ui, pal, "Engine");
                        group(ui, pal, |ui| {
                            let apple = s.translate_apple && apple_ok;
                            let desc = match s.translate_provider {
                                _ if apple => "Translations run on this Mac with Apple Translation, without sending the text anywhere.".to_string(),
                                _ if s.translate_apple => "Apple Translation needs macOS 26, so the chat provider translates here.".to_string(),
                                None => "Translating dragged text uses the chat provider and its model.".to_string(),
                                Some(p) => format!("Translations use {} with the model and sign-in set for it.", ferrocast_core::translate::provider_name(p)),
                            };
                            // readiness of the engine that will translate (the chat's when "Same as chat")
                            let eff = s.translate_provider.unwrap_or(s.ai_provider);
                            let eff_key = if eff == s.ai_provider { has_key } else { cached_has_key(&ctx, eff) };
                            let eff_ready = ai_readiness(
                                eff,
                                eff_key,
                                eff == P::ChatGpt && cached_chatgpt_signed(&ctx),
                                eff == P::ClaudeCode && services::claude_code_path().is_some(),
                            );
                            let before = (apple, s.translate_provider);
                            let chip = if apple { (StatusKind::Ok, "On this Mac") } else { (eff_ready.0, eff_ready.1) };
                            row_stacked_chip(ui, pal, "Provider", &desc, Some(chip), |ui| {
                                // (Apple engine, AI provider): Apple is its own choice and leaves the provider as it was
                                let mut choice = (apple, if apple { None } else { s.translate_provider });
                                let mut options = vec![((false, None), "Same as chat")];
                                if apple_ok {
                                    options.push(((true, None), "Apple"));
                                }
                                options.extend([
                                    ((false, Some(P::OpenAiCompatible)), "OpenAI API"),
                                    ((false, Some(P::ChatGpt)), "ChatGPT"),
                                    ((false, Some(P::Anthropic)), "Anthropic"),
                                    ((false, Some(P::ClaudeCode)), "Claude Code"),
                                ]);
                                // six cells with the Apple engine: tighter than the usual 22 px padding so the bar fits the card
                                let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
                                let current = options.iter().position(|(v, _)| *v == choice);
                                if let (Some(i), _) = segmented_bar(ui, pal, current, &labels, 56.0, 10.0) {
                                    choice = options[i].0;
                                }
                                if choice.0 {
                                    s.translate_apple = true;
                                } else if choice != (apple, if apple { None } else { s.translate_provider }) {
                                    s.translate_apple = false;
                                    s.translate_provider = choice.1;
                                }
                            });
                            if (s.translate_apple && apple_ok, s.translate_provider) != before {
                                // a model name of one provider is never sent to another
                                s.translate_model.clear();
                                ui.ctx().data_mut(|d| d.remove::<bool>(Id::new(("model_custom", "translate"))));
                            }
                            if let Some(p) = s.translate_provider.filter(|_| !apple) {
                                let listed: Option<Option<Result<Vec<String>, String>>> = match p {
                                    P::ChatGpt => Some(Some(Ok(services::CHATGPT_MODELS.iter().map(|m| m.to_string()).collect()))),
                                    P::ClaudeCode => Some(Some(Ok(services::CLAUDE_CODE_MODELS.iter().map(|m| m.to_string()).collect()))),
                                    P::OpenAiCompatible | P::Anthropic => translate_models_state.clone(),
                                };
                                let hint = match p {
                                    P::OpenAiCompatible => ferrocast_core::ai::DEFAULT_MODEL,
                                    P::Anthropic => ferrocast_core::ai::DEFAULT_ANTHROPIC_MODEL,
                                    P::ChatGpt => ferrocast_core::chatgpt::DEFAULT_MODEL,
                                    P::ClaudeCode => "Claude Code default",
                                };
                                row(ui, pal, "Model", "Provider default uses the model set for it in the AI tab. Or pick one, or Custom… to type a name.", |ui| {
                                    if model_picker_with(ui, pal, "translate", &mut s.translate_model, hint, "Provider default", listed.as_ref()) {
                                        fetch_translate_models = true;
                                    }
                                });
                            }
                        });
                        if s.translate_apple && apple_ok {
                            ui.add_space(14.0);
                            section(ui, pal, "Apple languages");
                            // Apple decides: with some macOS versions a language needs a pack first, with others
                            // its model is already on the Mac and every supported language reports as ready
                            let needs_download = ferrocast_core::translate::languages(&s.translate_languages)
                                .iter()
                                .any(|l| apple_states.get(l) == Some(&ferrocast_core::translate::AppleLang::NeedsDownload));
                            group(ui, pal, |ui| {
                                for lang in ferrocast_core::translate::languages(&s.translate_languages) {
                                    let state = apple_states.get(&lang).copied().unwrap_or(ferrocast_core::translate::AppleLang::Checking);
                                    let kind = match state {
                                        ferrocast_core::translate::AppleLang::Installed => StatusKind::Ok,
                                        ferrocast_core::translate::AppleLang::NeedsDownload => StatusKind::Warn,
                                        ferrocast_core::translate::AppleLang::Unsupported | ferrocast_core::translate::AppleLang::Unavailable => StatusKind::Error,
                                        ferrocast_core::translate::AppleLang::Checking => StatusKind::Neutral,
                                    };
                                    // one line per language: a status, not a setting, so it is not a searchable row
                                    let w = ui.available_width();
                                    let frame = egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 9)).show(ui, |ui| {
                                        ui.set_width(w - 28.0);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&lang).size(13.5).color(pal.text));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| status_chip(ui, pal, kind, state.chip()));
                                        });
                                    });
                                    form_sep(ui, pal, frame.response.rect);
                                }
                                if needs_download {
                                    row(ui, pal, "Download languages", "Apple downloads each language once in System Settings → General → Language & Region → Translation Languages. After that, translating works offline.", |ui| {
                                        if small_btn(ui, pal, "Check again") {
                                            recheck_apple = true;
                                        }
                                        if small_btn(ui, pal, "Open Language settings") {
                                            open_language_settings = true;
                                        }
                                    });
                                }
                            });
                            ui.add_space(10.0);
                            if !needs_download {
                                note(ui, pal, "Apple translates on this Mac with its own model: no language needs downloading right now. If Apple ever asks for one, its status says so and a download button appears here.");
                                ui.add_space(6.0);
                            }
                            note(ui, pal, "Each language is checked as a pair with your Mac's system language (with English for the system language itself). Translating from another language needs that one downloaded too.");
                        } else if s.translate_provider.is_none() && !s.translate_apple {
                            ui.add_space(10.0);
                            note(ui, pal, &format!("Uses the chat provider and model: {}", ferrocast_core::translate::engine_label(&s)));
                        }
                        ui.add_space(10.0);
                        note(ui, pal, "Credentials and server address come from the AI tab; this tab only picks which provider and model translate.");
                    }
                    NOTES_TAB => {
                        group(ui, pal, |ui| {
                            row(ui, pal, "Show Notes Toggle Button in Menu Bar", "Adds a button to the menu bar that shows or hides the Notes window.", |ui| {
                                toggle(ui, pal, &mut s.notes_menu_bar_button)
                            });
                            row(ui, pal, "Escape Key Loses Focus", "Esc moves focus away from the Notes window instead of closing it.", |ui| {
                                toggle(ui, pal, &mut s.notes_escape_loses_focus)
                            });
                            row(ui, pal, "Zoom", "Text size in Notes. Also set with ⌘+, ⌘- and ⌘0.", |ui| {
                                // right to left: the drop-down is the rightmost control
                                let shown = notes_zoom_label(s.notes_zoom);
                                #[cfg(test)]
                                ctx.data_mut(|d| {
                                    d.insert_temp(Id::new("notes-zoom-shown"), shown.clone());
                                    d.insert_temp(Id::new("notes-zoom-items"), Vec::<(String, Rect)>::new());
                                });
                                field_combo(ui, pal, "notes_zoom", &shown, 84.0, |ui| {
                                    for z in notes_zoom_steps() {
                                        let label = notes_zoom_label(z);
                                        let r = ui.selectable_label((s.notes_zoom - z).abs() < 1e-6, &label);
                                        #[cfg(test)]
                                        ctx.data_mut(|d| {
                                            d.get_temp_mut_or_default::<Vec<(String, Rect)>>(Id::new("notes-zoom-items")).push((label.clone(), r.rect))
                                        });
                                        if r.clicked() {
                                            s.notes_zoom = z;
                                        }
                                    }
                                });
                                if small_btn(ui, pal, "Reset") {
                                    s.notes_zoom = 1.0;
                                }
                            });
                        });
                    }
                    6 => {
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
                                field(ui, pal, &mut scratch, 220.0, "~/scripts", false);
                            });
                        });
                        ui.add_space(10.0);
                        note(ui, pal, "Supported modes: fullOutput, compact, silent and inline. Scripts marked needsConfirmation ask before running.");
                        if self.scripts.is_empty() && s.script_directories.is_empty() {
                            ui.add_space(14.0);
                            let line = "Scripts with Raycast metadata become commands you can run from the launcher.";
                            if empty_state(ui, pal, 6, "No script commands yet", line, "Create script") {
                                create_script = true;
                            }
                        }
                    }
                    INTEGRATIONS_TAB => {
                        section(ui, pal, "Notion");
                        group(ui, pal, |ui| {
                            let desc = if has_notion {
                                "A token is stored in the Keychain. Enter a new one to replace it, or save empty to remove it."
                            } else {
                                "From notion.so/my-integrations. Stored in the macOS Keychain, never in a file."
                            };
                            row(ui, pal, "Notion token", desc, |ui| {
                                if small_btn(ui, pal, "Save") {
                                    notion_save = true;
                                }
                                let r = field(ui, pal, &mut ntoken, 220.0, if has_notion { "••••••••" } else { "ntn_…" }, true);
                                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                    notion_save = true;
                                }
                            });
                            row(ui, pal, "Test connection", "Asks Notion who this token belongs to.", |ui| {
                                if small_btn(ui, pal, "Test") {
                                    notion_test = true;
                                }
                            });
                            row_disabled_if(ui, !has_notion, |ui| {
                                let shown = match &notion_target {
                                    Some(t) => format!("{} ({})", t.title, if t.kind == "page" { "page" } else { "database" }),
                                    None => "Not set".to_string(),
                                };
                                row(ui, pal, "Capture target", "Where Add to Notion writes. Search below to choose.", |ui| {
                                    if notion_target.is_some() && small_btn(ui, pal, "Clear") {
                                        notion_clear = true;
                                    }
                                    ui.add(egui::Label::new(RichText::new(shown).size(12.5).color(pal.sub_strong)).truncate());
                                });
                                row(ui, pal, "Find a page or database", "Only items shared with the integration show up.", |ui| {
                                    if small_btn(ui, pal, if notion_busy { "Searching…" } else { "Search" }) {
                                        notion_search = true;
                                    }
                                    let r = field(ui, pal, &mut nquery, 180.0, "Search Notion…", false);
                                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                        notion_search = true;
                                    }
                                });
                                for (i, (title, kind)) in notion_rows.iter().enumerate() {
                                    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                                    record_ctl(ui.ctx(), "notion_result", resp.id, r, ui.is_enabled());
                                    if resp.hovered() {
                                        ui.painter().rect_filled(r.shrink2(Vec2::new(6.0, 0.0)), 6.0, pal.sub.gamma_multiply(0.15));
                                    }
                                    let inner = r.shrink2(Vec2::new(14.0, 0.0));
                                    ui.painter().text(Pos2::new(inner.max.x, r.center().y), Align2::RIGHT_CENTER, *kind, FontId::proportional(11.5), pal.sub);
                                    let clip = Rect::from_min_max(inner.min, Pos2::new(inner.max.x - 70.0, inner.max.y));
                                    ui.painter().with_clip_rect(clip).text(Pos2::new(inner.min.x, r.center().y), Align2::LEFT_CENTER, title, FontId::proportional(13.0), pal.text);
                                    if resp.clicked() {
                                        notion_pick = Some(i);
                                    }
                                }
                            });
                            row(ui, pal, "Setup guide", "Opens notion.so/my-integrations to create an integration.", |ui| {
                                if small_btn(ui, pal, "Open notion.so/my-integrations") {
                                    notion_open = true;
                                }
                            });
                        });
                        ui.add_space(10.0);
                        note(ui, pal, "Create an internal integration at notion.so/my-integrations, paste its token here, then share pages with it: ••• → Connections.");
                        ui.add_space(18.0);
                        section(ui, pal, "Obsidian");
                        group(ui, pal, |ui| {
                            let shown = match (&obs_vault, &obs_vaults) {
                                (Some(v), _) => v.name.clone(),
                                (None, None) => "Looking for vaults…".to_string(),
                                (None, Some(l)) if l.is_empty() => "No vaults found".to_string(),
                                (None, Some(_)) => "Choose a vault".to_string(),
                            };
                            row(ui, pal, "Vault", "The vault Search Obsidian, New Obsidian Note and Daily Note use.", |ui| {
                                if obs_vault.is_some() && small_btn(ui, pal, "Clear") {
                                    obs_clear = true;
                                }
                                field_combo(ui, pal, "obsidian_vault", &shown, 200.0, |ui| match &obs_vaults {
                                    None => {
                                        ui.label(RichText::new("Looking for vaults…").size(12.5).color(pal.sub));
                                    }
                                    Some(l) if l.is_empty() => {
                                        ui.label(RichText::new("No vaults found").size(12.5).color(pal.sub));
                                    }
                                    Some(l) => {
                                        for (i, v) in l.iter().enumerate() {
                                            let open = obs_open.as_deref() == Some(v.path.as_path());
                                            let label = if open { format!("{} · open", v.name) } else { v.name.clone() };
                                            let on = obs_vault.as_ref().is_some_and(|c| c.path == v.path);
                                            if ui.selectable_label(on, label).clicked() {
                                                obs_pick = Some(i);
                                            }
                                        }
                                    }
                                });
                            });
                            row(ui, pal, "Vault folder", "Not in the list? Pick the vault's folder yourself.", |ui| {
                                if small_btn(ui, pal, if obs_picking { "Choosing…" } else { "Choose Folder…" }) && !obs_picking {
                                    obs_choose = true;
                                }
                            });
                            let chip = match obs_installed {
                                None => None,
                                Some(true) => Some((StatusKind::Ok, "Installed")),
                                Some(false) => Some((StatusKind::Warn, "Not installed")),
                            };
                            row_chip(ui, pal, "Obsidian app", obsidian_status(obs_installed), chip, |_| {});
                        });
                        ui.add_space(10.0);
                        note(ui, pal, "Reading a vault happens on this Mac and nothing is uploaded. Creating notes and the daily note go through Obsidian.");
                    }
                    _ => {
                        section(ui, pal, "Permissions");
                        group(ui, pal, |ui| {
                            row(ui, pal, "Accessibility", "Needed for window management, pasting and snippet expansion.", |ui| {
                                if trusted {
                                    status_chip(ui, pal, StatusKind::Ok, "Granted");
                                } else {
                                    if accent_btn(ui, pal, "Grant…") {
                                        req_acc = true;
                                    }
                                    status_chip(ui, pal, StatusKind::Warn, "Not granted");
                                }
                            });
                        });
                        ui.add_space(14.0);
                        section(ui, pal, "Units");
                        group(ui, pal, |ui| {
                            row(ui, pal, "1 rem", "Used by the calculator for rem and em conversions.", |ui| {
                                field_drag(ui, pal, &mut s.rem_px, 8.0..=64.0, " px");
                            });
                            row(ui, pal, "Window gap", "Space left between windows and screen edges by window commands.", |ui| {
                                field_drag(ui, pal, &mut s.window_gap, 0.0..=64.0, " pt");
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
                if RESET_FIELDS.iter().any(|(t, _)| *t == tab) {
                    ui.add_space(14.0);
                    let icon = self.icons.texture(&ctx, &Icon::Glyph("sf:arrow.counterclockwise", [0, 0, 0])).map(|t| t.id());
                    // right-aligned at the end of the tab content
                    let r = ui
                        .allocate_ui_with_layout(Vec2::new(ui.available_width(), FIELD_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            text_btn(ui, pal, "Reset this tab", icon).on_hover_text("Restore this tab's settings to their defaults")
                        })
                        .inner;
                    #[cfg(test)]
                    ctx.data_mut(|d| d.insert_temp(Id::new("settings-reset-button"), r.rect));
                    if r.clicked() {
                        reset_req = Some(tab);
                    }
                }
                }
                ui.add_space(16.0);
            });
        });
        let _ = acc;
        if obs_pick.is_some() || obs_choose || obs_clear {
            // choosing a vault commits on its own, like the Notion target
            self.scratch = scratch;
            self.key_scratch = key_scratch;
            self.notion.token = ntoken;
            self.notion.query = nquery;
            if let Some(i) = obs_pick {
                self.obsidian_pick(i);
            } else if obs_choose {
                self.obsidian_choose_folder();
            } else {
                self.obsidian_clear();
            }
            return;
        }
        if notion_pick.is_some() || notion_clear {
            // choosing or clearing the target commits on its own: restore the scratch fields first
            self.scratch = scratch;
            self.key_scratch = key_scratch;
            self.notion.token = ntoken;
            self.notion.query = nquery;
            match notion_pick {
                Some(i) => self.notion_pick(i),
                None => self.notion_clear_target(),
            }
            return;
        }
        if let Some(t) = reset_req {
            // a reset replaces whatever else was edited in this frame
            self.scratch = scratch;
            self.key_scratch = key_scratch;
            self.notion.token = ntoken;
            self.notion.query = nquery;
            self.reset_settings_tab(t);
            return;
        }
        if new_snippet {
            self.scratch = scratch;
            self.key_scratch = key_scratch;
            self.notion.token = ntoken;
            self.notion.query = nquery;
            self.new_snippet(String::new());
            return;
        }
        if create_script {
            self.scratch = scratch;
            self.key_scratch = key_scratch;
            self.notion.token = ntoken;
            self.notion.query = nquery;
            self.push(View::CreateScript { title: String::new(), mode: 0 });
            return;
        }
        if let Some(en) = clicked_result {
            open_settings_result(&ctx, en, &mut tab, &mut query);
            ctx.memory_mut(|m| m.surrender_focus(search_id));
            ctx.data_mut(|d| d.insert_temp(tab_id, tab));
            ctx.request_repaint();
        }
        ctx.data_mut(|d| {
            d.insert_temp(query_id, query.clone());
            d.insert_temp(sel_id, sel);
        });

        if add_dir && !scratch.trim().is_empty() {
            s.script_directories.push(scratch.trim().to_string());
            scratch.clear();
        }
        self.scratch = scratch;
        if save_key {
            let ok = services::set_ai_key_for(s.ai_provider, &key_scratch);
            let now_has = !key_scratch.trim().is_empty();
            key_scratch.clear();
            ctx.data_mut(|d| {
                d.insert_temp(Id::new("ai_has_key"), now_has);
                d.remove::<bool>(has_key_cache_id(s.ai_provider));
            });
            self.toast(if !ok {
                "Could not update Keychain"
            } else if now_has {
                "AI key saved to Keychain"
            } else {
                "AI key removed"
            });
        }
        self.key_scratch = key_scratch;
        self.notion.query = nquery;
        if notion_save {
            let typed = std::mem::take(&mut ntoken);
            self.notion_save_token(&typed);
        }
        self.notion.token = ntoken;
        if notion_test {
            self.notion_test();
        }
        if notion_search && has_notion {
            self.notion_search();
        }
        if notion_open {
            let _ = std::process::Command::new("/usr/bin/open").arg("https://www.notion.so/my-integrations").spawn();
        }
        let need_models = tab == 3 && !self.ai_models.contains_key(&models_key);
        if fetch_models || need_models {
            self.ai_models.insert(models_key.clone(), None);
            services::fetch_models(&s, self.bus.clone());
        }
        if open_language_settings {
            services::open_language_settings();
        }
        if recheck_apple {
            self.apple_langs.clear();
        }
        if tab == TRANSLATE_TAB && s.translate_apple && apple_ok {
            let missing: Vec<String> = ferrocast_core::translate::languages(&s.translate_languages)
                .into_iter()
                .filter(|l| !self.apple_langs.contains_key(l))
                .collect();
            if !missing.is_empty() {
                for l in &missing {
                    self.apple_langs.insert(l.clone(), ferrocast_core::translate::AppleLang::Checking);
                }
                services::apple_language_status(missing, self.bus.clone());
            }
        }
        if tab == TRANSLATE_TAB {
            if let Some(ts) = fetched_translate_settings(&s) {
                let k = services::models_key(&ts);
                if fetch_translate_models || !self.ai_models.contains_key(&k) {
                    self.ai_models.insert(k, None);
                    services::fetch_models(&ts, self.bus.clone());
                }
            }
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
        if check_now {
            self.check_updates();
        }
        if install_now {
            if let Some(release) = self.update_available.clone() {
                self.push(View::Update { release });
                return;
            }
        }
        if rec {
            self.push(View::Hotkey { id: "__launcher".into(), name: "Ferrocast launcher".into(), err: String::new() });
            return;
        }
        if s.ai_provider != self.settings.ai_provider {
            ctx.data_mut(|d| d.remove::<bool>(Id::new("ai_has_key")));
        }
        self.commit_settings(s);
    }
}

// ---- settings widgets: a tone-stepped card of rows, each with a title, a description and its control.

/// Drag-to-reorder for a run of equal cells, shared by the Choose Order list (vertical) and the
/// collage picture strip (horizontal). `cell(ui, i, lifted)` draws cell `i`; `lifted` is true for
/// the copy that follows the pointer, whose original place is left as a dimmed hole. While a drag
/// is over the cells an insertion line shows where it would land. Returns `(from, to)` when a
/// release changed the order (`to` already accounts for the removal); a release where the cell
/// was, or outside the cells, or an Escape during the drag, changes nothing. `overlay` draws
/// widgets (buttons) on top of a resting cell, after the cell's own drag/click area so they win
/// the pointer. A plain click on a cell is reported in `clicked`.
#[derive(Default)]
struct CellDrag {
    moved: Option<(usize, usize)>,
    clicked: Option<usize>,
}

#[allow(clippy::too_many_arguments)]
fn drag_reorder_cells(
    ui: &mut Ui,
    name: &'static str,
    count: usize,
    horizontal: bool,
    pal: &Palette,
    mut cell: impl FnMut(&mut Ui, usize, bool),
    mut overlay: impl FnMut(&mut Ui, usize, Rect),
) -> CellDrag {
    use ferrocast_core::reorder::{drop_index, slot_for_pointer};
    let mut out = CellDrag::default();
    let ctx = ui.ctx().clone();
    let key = Id::new(("dnd-reorder", name));
    let prev_id = Id::new(("dnd-prev", name));
    // egui ends the payload (not the pointer drag) when Escape arrives, and the source would set
    // it again next frame: a payload that vanished while the button is still down is a cancel
    let payload_now = |ctx: &Context| egui::DragAndDrop::payload::<(Id, usize)>(ctx).filter(|p| p.0 == key).map(|p| p.1);
    let prev: bool = ctx.data(|d| d.get_temp(prev_id)).unwrap_or(false);
    if prev && payload_now(&ctx).is_none() && ctx.input(|i| i.pointer.primary_down()) {
        ctx.stop_dragging();
    }
    let mut rects: Vec<Rect> = Vec::with_capacity(count);
    for i in 0..count {
        let id = key.with(i);
        let lifted = ctx.is_being_dragged(id);
        let r = if lifted {
            // the same as Ui::dnd_drag_source: the cell is painted on a tooltip layer that follows
            // the pointer, while the cell's own place stays reserved
            egui::DragAndDrop::set_payload(&ctx, (key, i));
            let layer = egui::LayerId::new(egui::Order::Tooltip, id);
            let resp = ui.scope_builder(UiBuilder::new().layer_id(layer), |ui| cell(ui, i, true)).response;
            if let Some(p) = ctx.pointer_interact_pos() {
                ctx.transform_layer_shapes(layer, egui::emath::TSTransform::from_translation(p - resp.rect.center()));
            }
            ui.painter().rect_filled(resp.rect, 8.0, pal.bg.gamma_multiply(0.6));
            ui.painter().rect_stroke(resp.rect, 8.0, Stroke::new(1.0, pal.sub.gamma_multiply(0.5)), StrokeKind::Inside);
            resp.rect
        } else {
            let r = ui.scope(|ui| cell(ui, i, false)).response.rect;
            // click and drag on one widget (dnd_drag_source only senses drag, which would hide clicks)
            let resp = ui.interact(r, id, egui::Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::Grab);
            resp.dnd_set_drag_payload((key, i));
            if resp.clicked() {
                out.clicked = Some(i);
            }
            overlay(ui, i, r);
            r
        };
        rects.push(r);
    }
    ctx.data_mut(|d| d.insert_temp(Id::new(("dnd-rects", name)), rects.clone()));
    let from = payload_now(&ctx);
    ctx.data_mut(|d| d.insert_temp(prev_id, from.is_some()));
    let (Some(from), true) = (from, count > 0) else { return out };
    let bounds = rects.iter().fold(Rect::NOTHING, |a, r| a.union(*r)).intersect(ui.clip_rect());
    let Some(pointer) = ctx.input(|i| i.pointer.latest_pos()).filter(|p| bounds.contains(*p)) else { return out };
    let extent = |r: &Rect| if horizontal { (r.min.x, r.max.x) } else { (r.min.y, r.max.y) };
    let spans: Vec<(f32, f32)> = rects.iter().map(extent).collect();
    let slot = slot_for_pointer(&spans, if horizontal { pointer.x } else { pointer.y });
    let to = drop_index(from, slot, count);
    if to == from {
        return out;
    }
    if ctx.input(|i| i.pointer.any_released()) {
        out.moved = Some((from, to));
        return out;
    }
    let at = match slot {
        0 => extent(&rects[0]).0,
        k if k >= count => extent(&rects[count - 1]).1,
        k => (extent(&rects[k - 1]).1 + extent(&rects[k]).0) / 2.0,
    };
    let (a, b) = if horizontal {
        (Pos2::new(at, bounds.min.y), Pos2::new(at, bounds.max.y))
    } else {
        (Pos2::new(bounds.min.x, at), Pos2::new(bounds.max.x, at))
    };
    ui.painter().line_segment([a, b], Stroke::new(3.0, pal.accent));
    ui.painter().circle_filled(a, 3.5, pal.accent);
    ui.painter().circle_filled(b, 3.5, pal.accent);
    out
}

/// A search result being opened: the row `title` of the current tab is scrolled into view for a
/// few frames and flashes for one second from the first frame it is drawn.
#[derive(Clone)]
struct SettingsJump {
    title: &'static str,
    /// time the flash started (set when the row is first drawn)
    t0: Option<f64>,
    frames: u8,
    /// pass the jump was requested in, to drop a jump whose row never shows
    pass0: u64,
}

/// Open a search result: switch to its tab, clear the query, and arm the scroll and flash.
fn open_settings_result(ctx: &Context, en: &'static ferrocast_core::settings_index::Entry, tab: &mut usize, query: &mut String) {
    *tab = en.tab;
    query.clear();
    let pass0 = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(Id::new("settings-jump"), SettingsJump { title: en.title, t0: None, frames: 0, pass0 }));
    #[cfg(test)]
    ctx.data_mut(|d| d.insert_temp(Id::new("settings-flash"), en.title.to_string()));
    ctx.request_repaint();
}

/// Called by every settings row after it is drawn: when it is the row of a pending search result,
/// scroll it to the middle of the pane and tint it with the accent, fading out over one second.
fn settings_row_jump(ui: &Ui, pal: &Palette, title: &str, rect: Rect) {
    let ctx = ui.ctx();
    let id = Id::new("settings-jump");
    let Some(mut j) = ctx.data(|d| d.get_temp::<SettingsJump>(id)) else { return };
    if j.title != title {
        return;
    }
    let now = ctx.input(|i| i.time);
    let t0 = *j.t0.get_or_insert(now);
    if j.frames < 4 {
        // the rows settle over the first frames, so ask again for a few
        ui.scroll_to_rect(rect, Some(egui::Align::Center));
        j.frames += 1;
    }
    let age = (now - t0) as f32;
    if age < 1.0 {
        let a = 0.25 * (1.0 - age);
        ui.painter().rect_filled(rect.shrink2(Vec2::new(3.0, 1.0)), 6.0, pal.accent.gamma_multiply(a));
        ctx.request_repaint();
        ctx.data_mut(|d| d.insert_temp(id, j));
    } else {
        ctx.data_mut(|d| d.remove::<SettingsJump>(id));
    }
}

/// Settings tabs: (name, SF Symbol). Everyday = 0..3, Intelligence = 3..5, Power = 5..8.
const SETTINGS_TABS: [(&str, &str); 9] = [
    ("General", "sf:gearshape"),
    ("Clipboard", "sf:doc.on.clipboard"),
    ("Snippets", "sf:curlybraces"),
    ("AI", "sf:sparkles"),
    ("Translate", "sf:character.bubble"),
    ("Notes", "sf:note.text"),
    ("Scripts", "sf:terminal"),
    ("Integrations", "sf:link"),
    ("Advanced", "sf:slider.horizontal.3"),
];
/// Indices of the first tab of the second and third group: a divider is drawn before each.
const SETTINGS_GROUP_STARTS: [usize; 2] = [3, 5];
const SETTINGS_NUM_KEYS: [Key; 9] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
/// Index of the Integrations tab in `draw_settings` (after Notes and Scripts).
pub(crate) const INTEGRATIONS_TAB: usize = 7;

/// Width of Search Obsidian's list when the preview pane is shown (the pane takes the rest of the 750 pt).
const OBSIDIAN_LIST_W: f32 = 310.0;
/// Blocks drawn per frame; a note longer than this ends with a hint to open it in Obsidian.
const OBSIDIAN_MAX_BLOCKS: usize = 400;

/// Paint preview blocks: headings larger, bullets, ordered items, task boxes, quotes with a bar, code on a
/// dark block in the monospace font, and rules as lines.
fn draw_blocks(ui: &mut Ui, pal: &Palette, blocks: &[ferrocast_core::obsidian::Block]) {
    use ferrocast_core::obsidian::Block;
    let text = |t: &str, size: f32, color: Color32| egui::Label::new(RichText::new(t).size(size).color(color)).wrap().selectable(false);
    let indent = |ui: &mut Ui, level: u8| ui.add_space(level.min(8) as f32 * 16.0);
    // a fixed-width gutter (bullet, number, checkbox) with the item text wrapping next to it
    let gutter = |ui: &mut Ui| ui.allocate_exact_size(Vec2::new(20.0, 18.0), Sense::hover()).0;
    for b in blocks.iter().take(OBSIDIAN_MAX_BLOCKS) {
        match b {
            Block::Heading(level, t) => {
                let size = [22.0, 19.0, 17.0, 15.5, 14.5, 14.0][(*level as usize).clamp(1, 6) - 1];
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new(t).size(size).strong().color(pal.text)).wrap().selectable(false));
            }
            Block::Paragraph(t) => {
                ui.add(text(t, 13.5, pal.text));
            }
            Block::Bullet(level, t) => {
                ui.horizontal_top(|ui| {
                    indent(ui, *level);
                    let g = gutter(ui);
                    ui.painter().circle_filled(Pos2::new(g.center().x, g.min.y + 9.0), 2.2, pal.sub_strong);
                    ui.add(text(t, 13.5, pal.text));
                });
            }
            Block::Ordered(level, n, t) => {
                ui.horizontal_top(|ui| {
                    indent(ui, *level);
                    let g = gutter(ui);
                    ui.painter().text(
                        Pos2::new(g.max.x - 4.0, g.min.y + 8.0),
                        Align2::RIGHT_CENTER,
                        format!("{n}."),
                        FontId::proportional(13.0),
                        pal.sub_strong,
                    );
                    ui.add(text(t, 13.5, pal.text));
                });
            }
            Block::Task(level, done, t) => {
                ui.horizontal_top(|ui| {
                    indent(ui, *level);
                    let g = gutter(ui);
                    let bx = Rect::from_center_size(Pos2::new(g.center().x - 1.0, g.min.y + 9.0), Vec2::splat(13.0));
                    let p = ui.painter();
                    if *done {
                        p.rect_filled(bx, 3.0, pal.accent);
                        let c = bx.center();
                        p.line_segment([c + Vec2::new(-3.5, 0.5), c + Vec2::new(-1.0, 3.0)], Stroke::new(1.8, Color32::WHITE));
                        p.line_segment([c + Vec2::new(-1.0, 3.0), c + Vec2::new(3.8, -3.2)], Stroke::new(1.8, Color32::WHITE));
                    } else {
                        p.rect_stroke(bx, 3.0, Stroke::new(1.3, pal.sub_strong), StrokeKind::Inside);
                    }
                    ui.add(text(t, 13.5, if *done { pal.sub } else { pal.text }));
                });
            }
            Block::Quote(t) => {
                let r = egui::Frame::NONE.inner_margin(egui::Margin { left: 12, right: 0, top: 1, bottom: 1 }).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(t).size(13.5).italics().color(pal.sub_strong)).wrap().selectable(false));
                });
                let rr = r.response.rect;
                ui.painter().rect_filled(Rect::from_min_max(rr.min, Pos2::new(rr.min.x + 3.0, rr.max.y)), 1.5, pal.accent);
            }
            Block::Code(_, t) => {
                egui::Frame::NONE
                    .fill(Color32::from_rgb(12, 12, 15))
                    .stroke(Stroke::new(1.0, pal.sep))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(
                            egui::Label::new(
                                RichText::new(if t.is_empty() { " " } else { t.as_str() })
                                    .monospace()
                                    .size(12.0)
                                    .color(Color32::from_gray(225)),
                            )
                            .wrap()
                            .selectable(false),
                        );
                    });
            }
            Block::Rule => {
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), Sense::hover());
                ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, pal.sep));
            }
        }
    }
    if blocks.len() > OBSIDIAN_MAX_BLOCKS {
        ui.add(text("Open the note in Obsidian to read the rest.", 12.0, pal.sub));
    }
}

fn section(ui: &mut Ui, pal: &Palette, title: &str) {
    record_text(ui.ctx(), pal, "section", pal.sub_strong, &[], ui.opacity());
    ui.label(RichText::new(title).size(12.5).color(pal.sub_strong));
    ui.add_space(6.0);
}

/// Shown under the "After translating" bar.
pub(crate) const PASTE_NOTE: &str = "Paste ⌘V only presses Cmd-V, which does nothing on a page you cannot edit.";

/// Shown under "After translating" while Auto is selected.
pub(crate) const AUTO_JS_NOTE: &str = "To replace text in web pages, turn on \u{201c}Allow JavaScript from Apple Events\u{201d} in your browser (Safari: Develop > Developer Settings; Chrome: View > Developer), and allow Ferrocast to control it when macOS asks.";

/// A grey note inside a card, under a row, inset like the row text.
fn card_note(ui: &mut Ui, pal: &Palette, text: &str) {
    egui::Frame::new().inner_margin(egui::Margin { left: 14, right: 14, top: 0, bottom: 10 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        record_text(ui.ctx(), pal, "card-note", pal.sub_strong, &[pal.field], ui.opacity());
        ui.label(RichText::new(text).size(12.0).color(pal.sub_strong));
    });
}

fn note(ui: &mut Ui, pal: &Palette, text: &str) {
    record_text(ui.ctx(), pal, "note", pal.sub_strong, &[], ui.opacity());
    ui.label(RichText::new(text).size(12.0).color(pal.sub_strong));
}

fn group(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui)) {
    let idx = record_card_start(ui.ctx());
    let r = group_rect(ui, pal, add);
    record_card_end(ui.ctx(), idx, r);
}

/// Radius of every Settings card.
const CARD_RADIUS: u8 = 12;

/// `group`, returning the rect of the card it drew. A row's separator is drawn by the row below
/// it, so the card forgets the pending line when it starts and when it ends: the last row of a
/// card never has a line under it.
fn group_rect(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui)) -> Rect {
    let fill = pal.field;
    ui.ctx().data_mut(|d| d.remove::<Rect>(Id::new("form-sep-pending")));
    let r = egui::Frame::new()
        .fill(fill)
        .corner_radius(CARD_RADIUS)
        .inner_margin(egui::Margin::symmetric(0, 2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui);
        })
        .response
        .rect;
    ui.ctx().data_mut(|d| d.remove::<Rect>(Id::new("form-sep-pending")));
    r
}

fn row(ui: &mut Ui, pal: &Palette, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    row_chip(ui, pal, title, desc, None, control);
}

/// `row` with a status chip after the title.
fn row_chip(ui: &mut Ui, pal: &Palette, title: &str, desc: &str, chip: Option<(StatusKind, &str)>, control: impl FnOnce(&mut Ui)) {
    let (r, c) = row_flow(ui, pal, RowLook::PANE, false, title, chip, desc, control);
    form_sep(ui, pal, r);
    record_row(ui.ctx(), title, r, c);
}

/// How a settings-style row is laid out. `SETTINGS` is the look of the Settings pane.
#[derive(Clone, Copy)]
struct RowLook {
    /// vertical padding above and below the text
    pad_y: i8,
    /// width of the title column; `None` = the Settings rule
    text_w: Option<f32>,
    /// draw the grey description line
    desc: bool,
}

impl RowLook {
    const SETTINGS: RowLook = RowLook { pad_y: 11, text_w: None, desc: true };
    /// The Settings pane: trimmed padding (a one-line row is about 48 pt).
    const PANE: RowLook = RowLook { pad_y: 8, text_w: None, desc: true };
}

/// Longest line of a Settings description: about 62 characters at 12 pt.
const TEXT_MAX_W: f32 = 372.0;
/// A control wider than this share of the row moves under the text.
/// A row stacks its control under the text only when less than this much width is left for the text.
const MIN_TEXT_W: f32 = 170.0;
/// Height of a stacked control line (fields are 28 pt).
const CONTROL_H: f32 = 28.0;

/// The title line of a row: the title, and a status chip right after it when there is one.
fn row_title(ui: &mut Ui, pal: &Palette, title: &str, chip: Option<(StatusKind, &str)>) {
    record_text(ui.ctx(), pal, if ui.is_enabled() { "title" } else { "title-disabled" }, pal.text, &[pal.field], ui.opacity());
    match chip {
        None => {
            ui.label(RichText::new(title).size(13.5).color(pal.text));
        }
        Some((kind, text)) => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(RichText::new(title).size(13.5).color(pal.text));
                status_chip(ui, pal, kind, text);
            });
        }
    }
}

/// The description line of a Settings row.
fn row_desc(ui: &mut Ui, pal: &Palette, desc: &str) {
    record_text(ui.ctx(), pal, if ui.is_enabled() { "desc" } else { "desc-disabled" }, pal.sub_strong, &[pal.field], ui.opacity());
    ui.label(RichText::new(desc).size(12.0).color(pal.sub_strong));
}

/// One adaptive Settings row. The text column takes what the control leaves (up to
/// `TEXT_MAX_W`); a control that leaves less than `MIN_TEXT_W` for the text goes under it instead
/// (`force_stack` always does). The control's width is learnt from the previous frame (a control
/// cannot be measured before it is drawn), so the first frame of a new row lays it out beside the
/// text and repaints once. Returns (the row's rect, the rect the control used). No separator is
/// drawn here.
#[allow(clippy::too_many_arguments)]
fn row_flow(
    ui: &mut Ui,
    pal: &Palette,
    look: RowLook,
    force_stack: bool,
    title: &str,
    chip: Option<(StatusKind, &str)>,
    desc: &str,
    control: impl FnOnce(&mut Ui),
) -> (Rect, Rect) {
    let w = ui.available_width();
    let inner_w = w - 28.0;
    let key = Id::new(("row-control-w", ui.next_auto_id(), title));
    let known: Option<f32> = ui.ctx().data(|d| d.get_temp(key));
    // also stack when the description would wrap to more than three lines beside the control
    // (about 6 pt per character at 12 pt)
    let stacked = force_stack
        || known.is_some_and(|cw| {
            let text_w = inner_w - cw - 16.0;
            text_w < MIN_TEXT_W || (desc.chars().count() as f32 * 6.0 / text_w).ceil() > 3.0
        });
    let mut used = Rect::NOTHING;
    let inner = egui::Frame::new().inner_margin(egui::Margin::symmetric(14, look.pad_y)).show(ui, |ui| {
        ui.set_width(inner_w);
        ui.spacing_mut().item_spacing.y = 2.0;
        if stacked {
            row_title(ui, pal, title, chip);
            if look.desc {
                row_desc(ui, pal, desc);
            }
            ui.add_space(6.0);
            // the control keeps the right-to-left order of a side-by-side row, packed to the left
            let cw = known.map_or(inner_w, |k| (k + 1.0).min(inner_w));
            ui.allocate_ui_with_layout(Vec2::new(cw, CONTROL_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                control(ui);
                used = ui.min_rect();
            });
        } else {
            // an unknown control gets nearly the whole row, so it is measured at its natural width
            let text_w = look.text_w.unwrap_or_else(|| match known {
                Some(cw) => (inner_w - cw - 16.0).clamp(150.0, TEXT_MAX_W),
                None => 60.0,
            });
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(text_w);
                    ui.set_max_width(text_w);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    row_title(ui, pal, title, chip);
                    if look.desc {
                        row_desc(ui, pal, desc);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    control(ui);
                    used = ui.min_rect();
                });
            });
        }
    });
    #[cfg(test)]
    if !ui.is_enabled() {
        ui.ctx().data_mut(|d| d.get_temp_mut_or_default::<Vec<String>>(Id::new("settings-row-disabled")).push(title.to_string()));
    }
    // a text field asks for a fixed width that the first side-by-side frame may have clamped: learn the
    // width it asked for, so the stacked layout gives it the room it wants
    let wanted: f32 = ui.ctx().data_mut(|d| d.remove_temp::<f32>(Id::new("field-wanted-w"))).unwrap_or(0.0);
    let learnt = used.width().max(wanted.min(inner_w));
    if known.is_none_or(|k| (k - learnt).abs() > 0.5) {
        ui.ctx().data_mut(|d| d.insert_temp(key, learnt));
        ui.ctx().request_repaint();
    }
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        let tab: usize = d.get_temp(Id::new("settings_tab")).unwrap_or(0);
        d.get_temp_mut_or_default::<Vec<(usize, String)>>(Id::new("settings-row-titles")).push((tab, title.to_string()));
    });
    settings_row_jump(ui, pal, title, inner.response.rect);
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        let v = d.get_temp_mut_or_default::<Vec<(String, bool)>>(Id::new("row-stacked"));
        v.retain(|(t, _)| t != title);
        v.push((title.to_string(), stacked));
    });
    (inner.response.rect, used)
}

/// The fixed two-column row of the form windows' cards (they place their own hairlines).
fn row_body(ui: &mut Ui, pal: &Palette, look: RowLook, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) -> (Rect, Rect) {
    let w = ui.available_width();
    let text_w = look.text_w.unwrap_or_else(|| (w * 0.46).min(w - 340.0).max(150.0));
    let mut used = Rect::NOTHING;
    let inner = egui::Frame::new().inner_margin(egui::Margin::symmetric(14, look.pad_y)).show(ui, |ui| {
        ui.set_width(w - 28.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(text_w);
                ui.set_max_width(text_w);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).size(13.5).color(pal.text));
                if look.desc {
                    ui.label(RichText::new(desc).size(11.5).color(pal.sub));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                control(ui);
                used = ui.min_rect();
            });
        });
    });
    (inner.response.rect, used)
}

/// Model dropdown: Default, every model the provider lists, and Custom… (a free text field).
/// Returns true when the list should be (re)fetched.
/// Index of the Translate tab in `draw_settings`.
const TRANSLATE_TAB: usize = 4;
/// Index of the Notes tab.
const NOTES_TAB: usize = 5;

/// The zoom levels of the Notes tab: 50 %, 60 % … 300 %, the steps of the editor's ⌘+ and ⌘-.
fn notes_zoom_steps() -> Vec<f64> {
    (5..=30).map(|n| f64::from(n) / 10.0).collect()
}

/// "150%" for 1.5.
fn notes_zoom_label(zoom: f64) -> String {
    format!("{}%", (zoom * 100.0).round() as i64)
}

/// For a translation provider whose model list is fetched (OpenAI-compatible, Anthropic): the
/// settings that list ITS models, i.e. the chat configuration with that provider selected.
fn fetched_translate_settings(s: &Settings) -> Option<Settings> {
    use ferrocast_core::models::AiProvider as P;
    match s.translate_provider {
        Some(p @ (P::OpenAiCompatible | P::Anthropic)) => Some(Settings { ai_provider: p, ..s.clone() }),
        _ => None,
    }
}

fn model_picker(
    ui: &mut Ui,
    pal: &Palette,
    id: &str,
    value: &mut String,
    default_label: &str,
    state: Option<&Option<Result<Vec<String>, String>>>,
) -> bool {
    model_picker_with(ui, pal, id, value, default_label, &format!("Default ({default_label})"), state)
}

/// `model_picker` with its own text for the "empty value" entry (`default_text`); `default_label`
/// stays the hint of the custom field.
fn model_picker_with(
    ui: &mut Ui,
    pal: &Palette,
    id: &str,
    value: &mut String,
    default_label: &str,
    default_text: &str,
    state: Option<&Option<Result<Vec<String>, String>>>,
) -> bool {
    #[cfg(test)]
    ui.ctx().data_mut(|d| d.insert_temp(Id::new("model-items"), Vec::<(String, Rect)>::new()));
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
        field(ui, pal, value, 170.0, default_label, false);
    }
    let shown = match state {
        None | Some(None) => "Loading…".to_string(),
        _ => {
            if custom {
                "Custom".to_string()
            } else if value.is_empty() {
                default_text.to_string()
            } else {
                value.clone()
            }
        }
    };
    let combo = field_combo(ui, pal, ("model_combo", id), &shown, if custom { 110.0 } else { 220.0 }, |ui| {
        ui.set_min_width(240.0);
        let r = ui.selectable_label(!custom && value.is_empty(), default_text);
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.get_temp_mut_or_default::<Vec<(String, Rect)>>(Id::new("model-items")).push((default_text.to_string(), r.rect))
        });
        if r.clicked() {
            value.clear();
            custom = false;
        }
        for m in models {
            let r = ui.selectable_label(!custom && value == m, m);
            #[cfg(test)]
            ui.ctx().data_mut(|d| d.get_temp_mut_or_default::<Vec<(String, Rect)>>(Id::new("model-items")).push((m.clone(), r.rect)));
            if r.clicked() {
                *value = m.clone();
                custom = false;
            }
        }
        if let Some(Some(Err(e))) = state {
            ui.label(RichText::new(format!("List unavailable: {e}")).size(11.5).color(pal.sub));
        }
        let r = ui.selectable_label(custom, "Custom…");
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.get_temp_mut_or_default::<Vec<(String, Rect)>>(Id::new("model-items")).push(("Custom…".to_string(), r.rect)));
        if r.clicked() {
            custom = true;
        }
    });
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        let v = d.get_temp_mut_or_default::<Vec<(String, Rect)>>(Id::new("model-combos"));
        v.retain(|(i, _)| i != id);
        v.push((id.to_string(), combo.rect));
    });
    #[cfg(not(test))]
    let _ = combo;
    ui.ctx().data_mut(|d| d.insert_temp(custom_id, custom));
    refresh
}

/// A row whose control always sits on its own line under the text, with a status chip after the title.
fn row_stacked_chip(ui: &mut Ui, pal: &Palette, title: &str, desc: &str, chip: Option<(StatusKind, &str)>, control: impl FnOnce(&mut Ui)) {
    let (r, c) = row_flow(ui, pal, RowLook::PANE, true, title, chip, desc, control);
    form_sep(ui, pal, r);
    record_row(ui.ctx(), title, r, c);
}

fn row_stacked_impl(ui: &mut Ui, pal: &Palette, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) -> (Rect, Rect) {
    let w = ui.available_width();
    let mut used = Rect::NOTHING;
    let inner = egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 11)).show(ui, |ui| {
        ui.set_width(w - 28.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(RichText::new(title).size(13.5).color(pal.text));
        ui.label(RichText::new(desc).size(11.5).color(pal.sub));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            control(ui);
            used = ui.min_rect();
        });
    });
    (inner.response.rect, used)
}

// ---- form windows: the Settings card and row look, shared with the tool windows

/// Widest card a form window draws; the card is centred in the window.
const FORM_W: f32 = 560.0;

/// The centred column a form window draws its cards in, inside the content `area`.
fn form_rect(area: Rect) -> Rect {
    let w = FORM_W.min(area.width() - 32.0);
    Rect::from_min_max(Pos2::new(area.center().x - w / 2.0, area.min.y + 4.0), Pos2::new(area.center().x + w / 2.0, area.max.y - 30.0))
}

/// A rounded card with the Settings look; rows go inside it (`form_row`).
fn card(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui)) {
    let idx = record_card_start(ui.ctx());
    ui.ctx().data_mut(|d| d.remove::<Rect>(Id::new("form-sep-pending")));
    let r = group_rect(ui, pal, add);
    ui.ctx().data_mut(|d| d.remove::<Rect>(Id::new("form-sep-pending")));
    record_card_end(ui.ctx(), idx, r);
}

/// A Settings-style row inside a card: title and one short grey description, control on the right.
fn form_row(ui: &mut Ui, pal: &Palette, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    let (r, c) = row_body(ui, pal, RowLook::SETTINGS, title, desc, control);
    form_sep(ui, pal, r);
    record_row(ui.ctx(), title, r, c);
}

/// A tighter row for dense side panels and lists: title only, the title column `text_w` wide.
fn form_row_compact(ui: &mut Ui, pal: &Palette, title: &str, text_w: f32, control: impl FnOnce(&mut Ui)) {
    let look = RowLook { pad_y: 6, text_w: Some(text_w), desc: false };
    let (r, c) = row_body(ui, pal, look, title, "", control);
    form_sep(ui, pal, r);
    record_row(ui.ctx(), title, r, c);
}

/// A row whose control sits on its own line under the title (wide controls in narrow cards).
fn form_row_stacked(ui: &mut Ui, pal: &Palette, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    let (r, c) = row_stacked_impl(ui, pal, title, desc, control);
    form_sep(ui, pal, r);
    record_row(ui.ctx(), title, r, c);
}

/// The hairline between two rows of a form card. A row only learns that it is not the last one when
/// the next row starts, so each line is drawn by the row below it and the last row gets none (it
/// would sit against the card's bottom edge). `card` forgets the pending line when it ends.
fn form_sep(ui: &mut Ui, pal: &Palette, r: Rect) {
    let id = Id::new("form-sep-pending");
    if let Some(prev) = ui.ctx().data(|d| d.get_temp::<Rect>(id)) {
        ui.painter().hline((prev.min.x + 14.0)..=(prev.max.x - 14.0), prev.max.y, Stroke::new(1.0, pal.sep));
        record_sep(ui.ctx(), prev.max.y);
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, r));
}

/// What `card` and `form_row` drew in the latest pass: tests read it.
#[cfg(test)]
#[derive(Clone, Debug)]
struct FormCard {
    rect: Rect,
    /// (title, row rect, rect the control used)
    rows: Vec<(String, Rect, Rect)>,
    /// y of every separator line drawn inside the card
    seps: Vec<f32>,
}

#[cfg(test)]
impl Default for FormCard {
    fn default() -> Self {
        FormCard { rect: Rect::NOTHING, rows: vec![], seps: vec![] }
    }
}

#[cfg(test)]
fn record_card_start(ctx: &egui::Context) -> usize {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        if d.get_temp::<u64>(Id::new("form-cards-pass")) != Some(pass) {
            d.insert_temp(Id::new("form-cards-pass"), pass);
            d.insert_temp(Id::new("form-cards"), Vec::<FormCard>::new());
        }
        let list = d.get_temp_mut_or_default::<Vec<FormCard>>(Id::new("form-cards"));
        list.push(FormCard::default());
        list.len() - 1
    })
}

#[cfg(test)]
fn record_card_end(ctx: &egui::Context, idx: usize, rect: Rect) {
    ctx.data_mut(|d| {
        if let Some(c) = d.get_temp_mut_or_default::<Vec<FormCard>>(Id::new("form-cards")).get_mut(idx) {
            c.rect = rect;
        }
    });
}

#[cfg(test)]
fn record_row(ctx: &egui::Context, title: &str, row: Rect, control: Rect) {
    ctx.data_mut(|d| {
        if let Some(c) = d.get_temp_mut_or_default::<Vec<FormCard>>(Id::new("form-cards")).last_mut() {
            c.rows.push((title.to_string(), row, control));
        }
    });
}

#[cfg(test)]
fn record_sep(ctx: &egui::Context, y: f32) {
    ctx.data_mut(|d| {
        if let Some(c) = d.get_temp_mut_or_default::<Vec<FormCard>>(Id::new("form-cards")).last_mut() {
            c.seps.push(y);
        }
    });
}

#[cfg(not(test))]
fn record_sep(_: &egui::Context, _: f32) {}

#[cfg(not(test))]
fn record_card_start(_: &egui::Context) -> usize {
    0
}
#[cfg(not(test))]
fn record_card_end(_: &egui::Context, _: usize, _: Rect) {}
#[cfg(not(test))]
fn record_row(_: &egui::Context, _: &str, _: Rect, _: Rect) {}

// ---- field helpers: one look for every Settings text field, combo and number box

/// Minimum height of every field.
const FIELD_H: f32 = 28.0;

/// The Settings field look, applied to the widgets of `ui` (call it inside a scope): the card
/// field fill, a 1 px border that brightens on hover and turns accent on focus, 8 pt corners.
fn field_style(ui: &mut Ui, pal: &Palette) {
    let fill = pal.field.gamma_multiply(1.6);
    let radius = egui::CornerRadius::same(8);
    ui.spacing_mut().interact_size.y = FIELD_H;
    ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
    let v = ui.visuals_mut();
    v.text_edit_bg_color = Some(fill);
    v.selection.stroke = Stroke::new(1.0, pal.accent);
    for (w, f, line) in [
        (&mut v.widgets.inactive, fill, pal.border),
        (&mut v.widgets.hovered, pal.field.gamma_multiply(2.4), pal.border.gamma_multiply(2.2)),
        (&mut v.widgets.active, pal.field.gamma_multiply(2.4), pal.accent),
        (&mut v.widgets.open, pal.field.gamma_multiply(2.4), pal.border.gamma_multiply(2.2)),
    ] {
        w.weak_bg_fill = f;
        w.bg_fill = f;
        w.bg_stroke = Stroke::new(1.0, line);
        w.corner_radius = radius;
        w.expansion = 0.0;
        w.fg_stroke.color = pal.text;
    }
}

/// Tests read the text roles Settings drew: (role, text colour, fills it sits on from the window
/// background up, opacity). A no-op outside tests.
fn record_text(ctx: &egui::Context, pal: &Palette, role: &str, fg: Color32, over: &[Color32], alpha: f32) {
    #[cfg(test)]
    ctx.data_mut(|d| {
        let mut layers = vec![pal.bg];
        layers.extend_from_slice(over);
        d.get_temp_mut_or_default::<Vec<(String, Color32, Vec<Color32>, f32)>>(Id::new("text-roles")).push((
            role.to_string(),
            fg,
            layers,
            alpha,
        ))
    });
    #[cfg(not(test))]
    let _ = (ctx, pal, role, fg, over, alpha);
}

/// Tests read every interactive rect Settings drew: (kind, id, rect, enabled). A no-op outside tests.
fn record_ctl(ctx: &egui::Context, kind: &str, id: Id, r: Rect, enabled: bool) {
    #[cfg(test)]
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")).push((kind.to_string(), id, r, enabled))
    });
    #[cfg(not(test))]
    let _ = (ctx, kind, id, r, enabled);
}

/// The keyboard focus ring of every Settings control: 2 px accent at about 70 %, 6 pt radius,
/// drawn just outside `rect` while `resp` has the focus.
fn focus_ring(ui: &Ui, pal: &Palette, resp: &egui::Response, rect: Rect) {
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(1.0), 6.0, Stroke::new(2.0, pal.accent.gamma_multiply(0.7)), egui::StrokeKind::Outside);
        #[cfg(test)]
        ui.ctx().data_mut(|d| d.get_temp_mut_or_default::<Vec<Id>>(Id::new("focus-rings")).push(resp.id));
    }
}

/// Opacity of a dimmed (disabled) Settings row.
const DISABLED_OPACITY: f32 = 0.72;

/// Tests read every field the Settings code drew: (kind, rect).
fn record_field(ctx: &egui::Context, kind: &str, r: Rect) {
    #[cfg(test)]
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(String, Rect)>>(Id::new("field-count")).push((kind.to_string(), r)));
    #[cfg(not(test))]
    let _ = (ctx, kind, r);
}

/// The Settings search box: the field look with room for a magnifier on the left and a clear
/// button on the right (the caller paints both). Not counted in `field-count`.
fn search_field(ui: &mut Ui, pal: &Palette, text: &mut String, width: f32, hint: &str, id: Id) -> egui::Response {
    record_text(ui.ctx(), pal, "hint-sidebar", pal.sub_strong, &[pal.field.gamma_multiply(1.6)], 1.0);
    let r = ui
        .scope(|ui| {
            field_style(ui, pal);
            ui.add(
                TextEdit::singleline(text)
                    .id(id)
                    .desired_width(width)
                    .font(FontId::proportional(12.5))
                    .hint_text(RichText::new(hint).color(pal.sub_strong))
                    .margin(egui::Margin { left: 28, right: 24, top: 5, bottom: 5 })
                    .min_size(Vec2::new(0.0, FIELD_H))
                    .vertical_align(egui::Align::Center),
            )
        })
        .inner;
    record_ctl(ui.ctx(), "search", r.id, r.rect, true);
    focus_ring(ui, pal, &r, r.rect);
    r
}

/// A single-line text field in the Settings look; `hint` shows in the sub colour while empty.
fn field(ui: &mut Ui, pal: &Palette, text: &mut String, width: f32, hint: &str, password: bool) -> egui::Response {
    let fill = pal.field.gamma_multiply(1.6);
    record_text(ui.ctx(), pal, "hint-card", pal.sub_strong, &[pal.field, fill], ui.opacity());
    record_text(ui.ctx(), pal, "field-text", pal.text, &[pal.field, fill], ui.opacity());
    let r = ui
        .scope(|ui| {
            field_style(ui, pal);
            ui.add(
                TextEdit::singleline(text)
                    .desired_width(width)
                    .font(FontId::proportional(12.5))
                    .hint_text(RichText::new(hint).color(pal.sub_strong))
                    .password(password)
                    .margin(egui::Margin::symmetric(8, 5))
                    .min_size(Vec2::new(0.0, FIELD_H))
                    .vertical_align(egui::Align::Center),
            )
        })
        .inner;
    record_field(ui.ctx(), "text", r.rect);
    record_ctl(ui.ctx(), "field", r.id, r.rect, ui.is_enabled());
    focus_ring(ui, pal, &r, r.rect);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new("field-wanted-w"), width));
    r
}

/// A drop-down in the Settings look; `add` draws its entries.
fn field_combo(
    ui: &mut Ui,
    pal: &Palette,
    id: impl std::hash::Hash + std::fmt::Debug,
    shown: &str,
    width: f32,
    add: impl FnOnce(&mut Ui),
) -> egui::Response {
    let r = ui
        .scope(|ui| {
            field_style(ui, pal);
            egui::ComboBox::from_id_salt(id).selected_text(RichText::new(shown).size(12.5)).width(width).show_ui(ui, add).response
        })
        .inner;
    record_field(ui.ctx(), "combo", r.rect);
    record_ctl(ui.ctx(), "combo", r.id, r.rect, ui.is_enabled());
    focus_ring(ui, pal, &r, r.rect);
    r
}

/// A number box (drag or type) in the Settings look.
fn field_drag(ui: &mut Ui, pal: &Palette, value: &mut f64, range: std::ops::RangeInclusive<f64>, suffix: &str) {
    let r = ui
        .scope(|ui| {
            field_style(ui, pal);
            ui.spacing_mut().interact_size.x = 76.0;
            ui.add(egui::DragValue::new(value).range(range).suffix(suffix))
        })
        .inner;
    record_field(ui.ctx(), "drag", r.rect);
    record_ctl(ui.ctx(), "drag", r.id, r.rect, ui.is_enabled());
    focus_ring(ui, pal, &r, r.rect);
}

/// A pill switch that slides in 120 ms.
fn toggle(ui: &mut Ui, pal: &Palette, on: &mut bool) {
    // the switch is 40x24, its hit rect 28 pt tall
    let (hit, resp) = ui.allocate_exact_size(Vec2::new(40.0, 28.0), Sense::click());
    let r = Rect::from_center_size(hit.center(), Vec2::new(40.0, 24.0));
    record_ctl(ui.ctx(), "toggle", resp.id, hit, ui.is_enabled());
    focus_ring(ui, pal, &resp, r);
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

/// A colour swatch: the colour with a hairline edge, so black on a dark card and white on a light
/// one still read as a dot.
fn swatch_dot(p: &egui::Painter, at: Pos2, fill: Color32, pal: &Palette) {
    p.add(egui::epaint::CircleShape { center: at, radius: 8.0, fill, stroke: Stroke::new(1.0, pal.border) });
}

/// `toggle`, returning whether it was flipped this frame.
fn toggle_changed(ui: &mut Ui, pal: &Palette, on: &mut bool) -> bool {
    let before = *on;
    toggle(ui, pal, on);
    *on != before
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

fn small_btn(ui: &mut Ui, pal: &Palette, label: &str) -> bool {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.5), pal.text);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 20.0, 28.0), Sense::click());
    record_ctl(ui.ctx(), "small_btn", resp.id, r, ui.is_enabled());
    focus_ring(ui, pal, &resp, r);
    record_text(ui.ctx(), pal, "button", pal.text, &[pal.field, pal.field.gamma_multiply(1.6)], ui.opacity());
    let p = ui.painter();
    p.rect_filled(r, 7.0, if resp.hovered() { pal.sel } else { pal.field.gamma_multiply(1.6) });
    p.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, pal.text);
    resp.clicked()
}

fn accent_btn(ui: &mut Ui, pal: &Palette, label: &str) -> bool {
    accent_btn_r(ui, pal, label).0
}

/// `accent_btn`, also returning the button's rect.
fn accent_btn_r(ui: &mut Ui, pal: &Palette, label: &str) -> (bool, Rect) {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.5), Color32::WHITE);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 24.0, 28.0), Sense::click());
    record_ctl(ui.ctx(), "accent_btn", resp.id, r, ui.is_enabled());
    focus_ring(ui, pal, &resp, r);
    record_text(ui.ctx(), pal, "accent-button", Color32::WHITE, &[pal.accent], ui.opacity());
    let p = ui.painter();
    p.rect_filled(r, 7.0, if resp.hovered() { pal.accent.gamma_multiply(1.15) } else { pal.accent });
    p.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
    (resp.clicked(), r)
}

fn pill(ui: &mut Ui, pal: &Palette, label: &str, on: bool) -> bool {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.0), if on { Color32::WHITE } else { pal.text });
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 18.0, 28.0), Sense::click());
    record_ctl(ui.ctx(), "pill", resp.id, r, ui.is_enabled());
    focus_ring(ui, pal, &resp, r);
    let p = ui.painter();
    p.rect_filled(
        r,
        14.0,
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
    let idx = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
    if let (Some(i), _) = segmented_bar(ui, pal, Some(idx), &labels, 56.0, 22.0) {
        *value = options[i].0.clone();
    }
}

/// `segmented` over a plain index (`0..labels.len()`), for windows that keep a mode as a `usize`.
fn seg_index(ui: &mut Ui, pal: &Palette, value: &mut usize, labels: &[&str]) {
    if let (Some(i), _) = segmented_bar(ui, pal, Some((*value).min(labels.len() - 1)), labels, 56.0, 22.0) {
        *value = i;
    }
}

/// The index-based core of `segmented`, for values that are not a plain enum (an index, a bool, an
/// `Option`, a float preset). `sel` is the chosen option, or `None` when the current value matches
/// none of them (no segment is filled then). Returns the option clicked this frame and the rect of
/// every cell. `min_w` and `pad` set the cell width (widest label + `pad`, at least `min_w`);
/// Settings uses 56 and 22, narrow panels can ask for less.
fn segmented_bar(ui: &mut Ui, pal: &Palette, sel: Option<usize>, labels: &[&str], min_w: f32, pad: f32) -> (Option<usize>, Vec<Rect>) {
    let seg_w = labels
        .iter()
        .map(|l| ui.painter().layout_no_wrap(l.to_string(), FontId::proportional(12.0), pal.text).size().x + pad)
        .fold(min_w, f32::max);
    let (r, base) = ui.allocate_exact_size(Vec2::new(seg_w * labels.len() as f32 + 4.0, 28.0), Sense::hover());
    let id = base.id;
    let p = ui.painter().clone();
    p.rect_filled(r, 8.0, pal.field.gamma_multiply(1.3));
    if let Some(idx) = sel {
        let x = ui.ctx().animate_value_with_time(id.with("seg"), idx as f32, 0.14);
        let sr = Rect::from_min_size(Pos2::new(r.min.x + 2.0 + x * seg_w, r.min.y + 2.0), Vec2::new(seg_w, 24.0));
        p.rect_filled(sr, 6.0, pal.sel.gamma_multiply(1.8));
    }
    let mut clicked = None;
    let mut cells = Vec::with_capacity(labels.len());
    for (i, label) in labels.iter().enumerate() {
        let cell = Rect::from_min_size(Pos2::new(r.min.x + 2.0 + i as f32 * seg_w, r.min.y), Vec2::new(seg_w, 28.0));
        let resp = ui.interact(cell, id.with(("cell", i)), Sense::click());
        record_ctl(ui.ctx(), "segment", resp.id, cell, ui.is_enabled());
        focus_ring(ui, pal, &resp, cell.shrink(2.0));
        record_text(
            ui.ctx(),
            pal,
            match (sel == Some(i), ui.is_enabled()) {
                (true, true) => "segment-active",
                (false, true) => "segment",
                (true, false) => "segment-active-disabled",
                (false, false) => "segment-disabled",
            },
            if sel == Some(i) { pal.text } else { pal.sub_strong },
            &[pal.field, pal.field.gamma_multiply(1.3), pal.sel.gamma_multiply(1.8)][..if sel == Some(i) { 3 } else { 2 }],
            ui.opacity(),
        );
        if resp.clicked() {
            clicked = Some(i);
        }
        p.text(
            cell.center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional(12.0),
            if sel == Some(i) { pal.text } else { pal.sub_strong },
        );
        cells.push(cell);
    }
    #[cfg(test)]
    {
        // tests read what each window drew: one entry per bar, its container and its cells
        let key = Id::new("segmented-count");
        ui.ctx().data_mut(|d| {
            let list = d.get_temp_mut_or_default::<Vec<(Rect, Vec<Rect>)>>(key);
            list.retain(|(c, _)| c.min != r.min);
            list.push((r, cells.clone()));
        });
    }
    (clicked, cells)
}

/// Tests read the rect of every control of the Annotate tool strip (cleared by the test).
fn record_strip(ctx: &egui::Context, r: Rect) {
    #[cfg(test)]
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Rect>>(Id::new("annot-strip")).push(r));
    #[cfg(not(test))]
    let _ = (ctx, r);
}

/// The grey one-line note under a tool window (information only; keys live in the footer).
fn note_line(ui: &Ui, at: Pos2, text: impl AsRef<str>, pal: &Palette) {
    record_note(ui.ctx(), text.as_ref());
    ui.painter().text(at, Align2::CENTER_CENTER, text.as_ref(), FontId::proportional(11.5), pal.sub);
}

/// Tests read every note a window drew (cleared by the test before rendering).
fn record_note(ctx: &egui::Context, text: &str) {
    #[cfg(test)]
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<String>>(Id::new("body-notes")).push(text.to_string()));
    #[cfg(not(test))]
    let _ = (ctx, text);
}

fn keycap(ui: &mut Ui, pal: &Palette, label: &str) {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.0), pal.text);
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 16.0, 24.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 6.0, pal.field.gamma_multiply(1.6));
    p.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, pal.text);
}

// ---- SD4: status chips, validation and per-tab reset

/// How a status chip reads: green, amber, red or grey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusKind {
    Ok,
    Warn,
    Error,
    Neutral,
}

impl StatusKind {
    fn name(self) -> &'static str {
        match self {
            StatusKind::Ok => "ok",
            StatusKind::Warn => "warn",
            StatusKind::Error => "error",
            StatusKind::Neutral => "neutral",
        }
    }
}

/// Readiness of an AI provider as one status chip.
fn ai_readiness(
    provider: ferrocast_core::models::AiProvider,
    has_key: bool,
    signed_in: bool,
    claude_found: bool,
) -> (StatusKind, &'static str) {
    use ferrocast_core::models::AiProvider as P;
    match provider {
        // a local server (Ollama, LM Studio) needs no key, so its absence is not a warning
        P::OpenAiCompatible if has_key => (StatusKind::Ok, "Key stored"),
        P::OpenAiCompatible => (StatusKind::Neutral, "No key set"),
        P::Anthropic if has_key => (StatusKind::Ok, "Key stored"),
        P::Anthropic => (StatusKind::Warn, "Not configured"),
        P::ChatGpt if signed_in => (StatusKind::Ok, "Signed in"),
        P::ChatGpt => (StatusKind::Warn, "Not signed in"),
        P::ClaudeCode if claude_found => (StatusKind::Ok, "Claude Code found"),
        P::ClaudeCode => (StatusKind::Error, "Claude Code not found"),
    }
}

/// Can Apple Translation run here (macOS 26+ and the helper in the bundle)? Asked once per session
/// and cached in egui temp data, like the Keychain lookups.
fn cached_apple_available(ctx: &Context, check: fn() -> bool) -> bool {
    let id = Id::new("apple_translate_available");
    match ctx.data(|d| d.get_temp::<bool>(id)) {
        Some(v) => v,
        None => {
            let v = check();
            ctx.data_mut(|d| d.insert_temp(id, v));
            v
        }
    }
}

/// Is the ChatGPT session in the Keychain? Read once, then cached in egui temp data (the AI tab
/// and the Translate tab both ask on every frame; sign-in and sign-out update the cache).
fn cached_chatgpt_signed(ctx: &Context) -> bool {
    let id = Id::new("chatgpt_signed");
    match ctx.data(|d| d.get_temp::<bool>(id)) {
        Some(v) => v,
        None => {
            let v = services::chatgpt_credentials().is_some();
            ctx.data_mut(|d| d.insert_temp(id, v));
            v
        }
    }
}

/// Cache key of "is an API key stored for provider `p`" (see `cached_has_key`).
fn has_key_cache_id(p: ferrocast_core::models::AiProvider) -> Id {
    Id::new(("ai_has_key_for", format!("{p:?}")))
}

/// Is a key stored for `p`? One Keychain read per provider, then cached in egui temp data.
fn cached_has_key(ctx: &Context, p: ferrocast_core::models::AiProvider) -> bool {
    let id = has_key_cache_id(p);
    match ctx.data(|d| d.get_temp::<bool>(id)) {
        Some(v) => v,
        None => {
            let v = services::ai_key_for(p).is_some();
            ctx.data_mut(|d| d.insert_temp(id, v));
            v
        }
    }
}

/// Why a server address cannot work, or `None` when it has the shape of an http(s) URL.
fn url_problem(url: &str) -> Option<&'static str> {
    let u = url.trim().to_ascii_lowercase();
    let host = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://"));
    match host {
        Some(h) if !h.trim_start_matches('/').is_empty() => None,
        _ => Some("Should start with http:// or https://"),
    }
}

/// Colour of a status chip: readable on the card fill in both themes.
fn status_color(pal: &Palette, kind: StatusKind) -> Color32 {
    let dark = pal.text.r() > 128;
    match (kind, dark) {
        (StatusKind::Ok, true) => Color32::from_rgb(89, 212, 153),
        (StatusKind::Ok, false) => Color32::from_rgb(10, 96, 56),
        (StatusKind::Warn, true) => Color32::from_rgb(240, 184, 76),
        (StatusKind::Warn, false) => Color32::from_rgb(122, 74, 0),
        (StatusKind::Error, true) => Color32::from_rgb(255, 140, 140),
        (StatusKind::Error, false) => Color32::from_rgb(164, 26, 26),
        (StatusKind::Neutral, _) => pal.sub_strong,
    }
}

/// A small status pill: a dot and a short sentence-case text, tinted by `kind`.
fn status_chip(ui: &mut Ui, pal: &Palette, kind: StatusKind, text: &str) {
    let c = status_color(pal, kind);
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(11.5), c);
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 26.0, 20.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 10.0, c.gamma_multiply(0.16));
    p.circle_filled(Pos2::new(r.min.x + 10.0, r.center().y), 3.0, c);
    p.galley(Pos2::new(r.min.x + 18.0, r.center().y - g.size().y / 2.0), g, c);
    // a chip inside a dimmed (disabled) row is held to the disabled floor, like the row's other text
    let role = if ui.opacity() < 1.0 { format!("chip-{}-disabled", kind.name()) } else { format!("chip-{}", kind.name()) };
    record_text(ui.ctx(), pal, &role, c, &[pal.field, c.gamma_multiply(0.16)], ui.opacity());
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<(String, String)>>(Id::new("status-chips")).push((kind.name().to_string(), text.to_string()))
    });
}

/// A row whose controls are dimmed and inert while `off` (its parent switch is off). The row stays
/// in place so the setting is still discoverable; `row_flow` records disabled titles for tests.
fn row_disabled_if(ui: &mut Ui, off: bool, add: impl FnOnce(&mut Ui)) {
    ui.add_enabled_ui(!off, |ui| {
        if off {
            ui.set_opacity(DISABLED_OPACITY);
        }
        add(ui);
    });
}

/// A quiet text button: `sub_strong` text with an optional 12 pt icon; on hover (or keyboard
/// focus) the text turns accent and is underlined. 28 pt tall, so it is as easy to hit as a field.
fn text_btn(ui: &mut Ui, pal: &Palette, label: &str, icon: Option<egui::TextureId>) -> egui::Response {
    let font = FontId::proportional(12.0);
    let g = ui.painter().layout_no_wrap(label.to_string(), font.clone(), pal.sub_strong);
    let icon_w = if icon.is_some() { 17.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + icon_w + 16.0, FIELD_H), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    record_ctl(ui.ctx(), "text_btn", resp.id, r, ui.is_enabled());
    record_text(ui.ctx(), pal, "text-btn", pal.sub_strong, &[], ui.opacity());
    record_text(ui.ctx(), pal, "text-btn-hover", pal.accent_text, &[], ui.opacity());
    let col = if resp.hovered() || resp.has_focus() { pal.accent_text } else { pal.sub_strong };
    let x = r.min.x + 8.0;
    if let Some(tex) = icon {
        let ir = Rect::from_center_size(Pos2::new(x + 6.0, r.center().y), Vec2::splat(12.0));
        ui.painter().image(tex, ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), col);
    }
    let at = Pos2::new(x + icon_w, r.center().y - g.size().y / 2.0);
    ui.painter().text(at, Align2::LEFT_TOP, label, font, col);
    if resp.hovered() {
        ui.painter().hline((at.x)..=(at.x + g.size().x), at.y + g.size().y, Stroke::new(1.0, col));
    }
    focus_ring(ui, pal, &resp, r);
    resp
}

/// A composed empty state inside the Settings pane: a title, one line about what the thing is and
/// one primary button. Returns true when the button was clicked.
fn empty_state(ui: &mut Ui, pal: &Palette, tab: usize, title: &str, line: &str, button: &str) -> bool {
    let mut clicked = false;
    egui::Frame::new().fill(pal.field).corner_radius(CARD_RADIUS).inner_margin(egui::Margin::symmetric(20, 22)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(RichText::new(title).size(14.5).color(pal.text));
            ui.label(RichText::new(line).size(12.0).color(pal.sub));
            ui.add_space(10.0);
            let (c, r) = accent_btn_r(ui, pal, button);
            clicked = c;
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(Id::new("settings-empty-button"), r);
                d.get_temp_mut_or_default::<Vec<(usize, String)>>(Id::new("settings-pane-empty")).push((tab, title.to_string()));
            });
            #[cfg(not(test))]
            let _ = (r, tab);
        });
    });
    clicked
}

/// Toast shown after "Reset this tab"; the footer turns its "Undo" into a link.
const RESET_TOAST: &str = "Tab reset · Undo";
/// How long Undo stays available.
const UNDO_SECS: u64 = 8;

/// The settings each tab's "Reset this tab" restores, by their stored (serde) names. Only tabs
/// that hold no user data have an entry; commands, hotkeys per command, scripts and snippets stay.
const RESET_FIELDS: [(usize, &[&str]); 4] = [
    (
        0,
        &[
            "launcher_hotkey",
            "theme",
            "compact_mode",
            "launch_at_login",
            "drop_wheel",
            "wheel_convert_key",
            "wheel_tools_key",
            "wheel_actions_key",
            "enter_on_finder",
            "show_menu_bar_icon",
            "update_check",
            "update_install",
            "sensitivity",
            "file_search_in_root",
            "fallback_search_url",
        ],
    ),
    (1, &["clipboard_enabled", "clipboard_retention"]),
    (
        4,
        &[
            "wheel_translate_key",
            "translate_languages",
            "translate_output",
            "translate_effect",
            "translate_provider",
            "translate_model",
            "translate_apple",
        ],
    ),
    (8, &["rem_px", "window_gap"]),
];

/// `s` with the fields of `tab` (see `RESET_FIELDS`) copied from `from`; a tab without an entry,
/// or a value that cannot round-trip, leaves `s` as it is. Works on the stored (serde) form, so
/// the table's names are the single source of truth.
fn with_tab_fields_from(s: &Settings, from: &Settings, tab: usize) -> Settings {
    use serde_json::Value;
    let Some((_, fields)) = RESET_FIELDS.iter().find(|(t, _)| *t == tab) else { return s.clone() };
    let (Ok(Value::Object(mut cur)), Ok(Value::Object(src))) = (serde_json::to_value(s), serde_json::to_value(from)) else {
        return s.clone();
    };
    for f in *fields {
        match src.get(*f) {
            Some(v) => {
                cur.insert((*f).to_string(), v.clone());
            }
            None => {
                cur.remove(*f);
            }
        }
    }
    serde_json::from_value(Value::Object(cur)).unwrap_or_else(|_| s.clone())
}

/// `s` with the fields of `tab` set to their defaults.
fn reset_tab(s: &Settings, tab: usize) -> Settings {
    with_tab_fields_from(s, &Settings::default(), tab)
}

impl App {
    /// Apply an edited copy of the settings: persist it and run what a changed value needs (login
    /// item, Finder tap, theme, hotkey, script folders).
    pub(crate) fn commit_settings(&mut self, mut s: Settings) {
        if s == self.settings {
            return;
        }
        if s.launch_at_login != self.settings.launch_at_login {
            match services::set_launch_at_login(s.launch_at_login) {
                Ok(()) => self.toast(if s.launch_at_login { "Will launch at login" } else { "Login item removed" }),
                Err(e) => {
                    s.launch_at_login = self.settings.launch_at_login;
                    self.toast(format!("Could not change login item: {e}"));
                }
            }
        }
        let finder_enter_changed = s.enter_on_finder != self.settings.enter_on_finder;
        let enter_on_finder = s.enter_on_finder;
        let theme_changed = s.theme != self.settings.theme;
        let hotkey_changed = s.launcher_hotkey != self.settings.launcher_hotkey;
        let dirs_changed = s.script_directories != self.settings.script_directories;
        let start_poller = s.update_check && self.update_poller.is_none();
        let notes_changed = s.notes_zoom != self.settings.notes_zoom
            || s.notes_escape_loses_focus != self.settings.notes_escape_loses_focus
            || s.notes_menu_bar_button != self.settings.notes_menu_bar_button;
        self.settings = s;
        self.save_settings();
        if notes_changed {
            self.apply_notes_settings();
        }
        if start_poller {
            // the poller only exists once the check was on: turning it on later starts it now
            self.update_poller = services::start_update_poller(self.shared.clone(), self.bus.clone());
        }
        if finder_enter_changed {
            // on: ask for Accessibility once and create the tap; off: remove it. No restart.
            self.set_finder_enter(enter_on_finder, true);
        }
        if theme_changed {
            self.set_theme();
        }
        if hotkey_changed {
            self.apply_hotkeys();
        }
        if dirs_changed {
            self.reload_scripts();
        }
    }

    /// "Reset this tab": restore the tab's settings to their defaults and offer Undo for 8 seconds.
    /// Returns false (and changes nothing) when the tab already has its defaults.
    fn reset_settings_tab(&mut self, tab: usize) -> bool {
        let new = reset_tab(&self.settings, tab);
        if new == self.settings {
            self.toast("Already at the defaults");
            return false;
        }
        self.settings_undo = Some((tab, self.settings.clone(), Instant::now()));
        self.commit_settings(new);
        // the Languages field keeps what was typed in egui temp data: show the restored value
        self.ctx.data_mut(|d| d.remove::<String>(Id::new("translate_langs_buf")));
        self.toast(RESET_TOAST);
        self.ctx.request_repaint_after(Duration::from_secs(UNDO_SECS));
        true
    }

    /// Undo the last reset: only that tab's fields go back; later changes elsewhere stay.
    fn undo_settings_reset(&mut self) -> bool {
        let Some((tab, old, _)) = self.settings_undo.take() else { return false };
        let back = with_tab_fields_from(&self.settings, &old, tab);
        self.commit_settings(back);
        self.ctx.data_mut(|d| d.remove::<String>(Id::new("translate_langs_buf")));
        self.toast("Previous settings restored");
        true
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
