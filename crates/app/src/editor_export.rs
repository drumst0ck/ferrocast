//! MP4 and GIF export from the studio editor (T28 + T29, S16): the sheet state, the pure helpers behind it
//! (default destination, size and duration summary, progress text), the export thread and the sheet
//! drawing. Planning and the per-frame maths live in `ferrocast_core::capture::export`; the helper
//! renders and encodes; this module only drives them and shows progress.
//!
//! `Export…` (⌘E) first writes `project.json` (the helper renders the SAVED project: the auto zooms
//! that only exist in memory and any edit still inside the autosave second are written first), then
//! runs `export_project` on a thread. Cancel sets a flag the thread forwards to the helper, which
//! removes its `.partial`. Closing the editor while an export runs cancels it.

use crate::editor_ctl::{Editor, EditorMsg};
use crate::ui::Palette;
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Ui, Vec2};
#[cfg(test)]
use ferrocast_core::capture::events::Events;
use ferrocast_core::capture::export::{self, Codec, ExportOptions, Format, Quality};
use ferrocast_core::capture::project::Project;
use ferrocast_core::capture::protocol::ExportDone;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Output resolution choices of the sheet. `P1080` / `P4k` set the LONGER side (1920 / 3840), so a
/// portrait aspect stays inside the 3840 x 2160 budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Res {
    Source,
    P1080,
    P4k,
    /// GIF widths (S16: at most 960 px).
    W480,
    W720,
    W960,
}

impl Res {
    /// The MP4 choices.
    pub const ALL: [Res; 3] = [Res::Source, Res::P1080, Res::P4k];
    /// The GIF choices.
    pub const GIF: [Res; 3] = [Res::W480, Res::W720, Res::W960];
    /// GIF frame rates offered (the limit is [`export::GIF_MAX_FPS`]).
    pub const GIF_FPS: [u32; 2] = [10, 15];

    pub fn label(self) -> &'static str {
        match self {
            Res::Source => "Source",
            Res::P1080 => "1080p",
            Res::P4k => "4K",
            Res::W480 => "480 px",
            Res::W720 => "720 px",
            Res::W960 => "960 px",
        }
    }

    fn gif_width(self) -> Option<u32> {
        match self {
            Res::W480 => Some(480),
            Res::W720 => Some(720),
            Res::W960 => Some(960),
            _ => None,
        }
    }

    fn long_edge(self) -> Option<u32> {
        match self {
            Res::Source => None,
            Res::P1080 => Some(1920),
            Res::P4k => Some(3840),
            Res::W480 | Res::W720 | Res::W960 => None,
        }
    }
}

/// A running export as the UI sees it.
pub struct ExportRun {
    pub cancel: Arc<AtomicBool>,
    pub done: u32,
    pub total: u32,
    pub eta: Option<f64>,
    pub cancelling: bool,
}

impl Drop for ExportRun {
    /// The editor session ends (view closed): the export must not outlive it.
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// What the export thread tells the app (inside `EditorMsg::Export`).
#[derive(Debug)]
pub enum ExportMsg {
    Progress {
        done: u32,
        total: u32,
        eta: Option<f64>,
    },
    Done(ExportDone),
    Failed(String),
    Cancelled,
    /// The destination picked in the save dialog.
    Dest(PathBuf),
}

/// What the app does after a message.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    None,
    /// Toast `Exported <name>` and reveal the file in Finder.
    Finished(PathBuf),
}

/// What the sheet asks the app for (it owns the helper path, the log and the message bus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetAction {
    None,
    Start,
    Choose,
}

pub struct ExportSheet {
    pub open: bool,
    pub format: Format,
    pub codec: Codec,
    /// MP4 resolution and frame rate; the GIF ones are kept apart so switching back and forth
    /// remembers both.
    pub res: Res,
    pub fps: u32,
    pub gif_res: Res,
    pub gif_fps: u32,
    pub quality: Quality,
    pub dest: Option<PathBuf>,
    pub run: Option<ExportRun>,
    /// Why the last export did not produce a file (the helper's own message).
    pub message: Option<String>,
}

impl Default for ExportSheet {
    fn default() -> Self {
        Self {
            open: false,
            format: Format::Mp4,
            codec: Codec::H264,
            res: Res::P1080,
            fps: 30,
            gif_res: Res::W720,
            gif_fps: 15,
            quality: Quality::Standard,
            dest: None,
            run: None,
            message: None,
        }
    }
}

impl ExportSheet {
    pub fn running(&self) -> bool {
        self.run.is_some()
    }

    /// Apply a thread message; `Finished` carries the file to reveal.
    pub fn apply(&mut self, msg: ExportMsg) -> Outcome {
        match msg {
            ExportMsg::Progress { done, total, eta } => {
                if let Some(r) = self.run.as_mut() {
                    (r.done, r.total, r.eta) = (done, total, eta);
                }
                Outcome::None
            }
            ExportMsg::Done(d) => {
                self.run = None;
                self.open = false;
                self.message = None;
                Outcome::Finished(PathBuf::from(d.path))
            }
            ExportMsg::Failed(m) => {
                self.run = None;
                self.message = Some(m);
                Outcome::None
            }
            ExportMsg::Cancelled => {
                self.run = None;
                self.message = None;
                Outcome::None
            }
            ExportMsg::Dest(p) => {
                self.dest = Some(p);
                Outcome::None
            }
        }
    }

    /// Ask the running export to stop (the thread forwards `cancel` to the helper).
    pub fn cancel(&mut self) {
        if let Some(r) = self.run.as_mut() {
            r.cancel.store(true, Ordering::Relaxed);
            r.cancelling = true;
        }
    }

    /// Show the sheet with the default destination for `dir` (kept when one was already chosen).
    pub fn show(&mut self, dir: &Path, project: &Project) {
        self.open = true;
        self.message = None;
        if self.dest.is_none() {
            self.dest = Some(default_dest(dir, &project.name, &|p| p.exists()));
        }
    }

    /// Switch the container; the destination's extension follows (never onto an existing file).
    pub fn set_format(&mut self, format: Format) {
        if self.format == format {
            return;
        }
        self.format = format;
        if let Some(d) = self.dest.take() {
            self.dest = Some(with_extension(&d, ext_of(format), &|p| p.exists()));
        }
    }

    /// The options the sheet stands for, for a project whose video is `source` pixels.
    pub fn options(&self, project: &Project, source: [u32; 2]) -> ExportOptions {
        if self.format == Format::Gif {
            return ExportOptions {
                format: Format::Gif,
                codec: self.codec,
                width: self.gif_res.gif_width().unwrap_or(export::GIF_MAX_WIDTH),
                fps: self.gif_fps.min(export::GIF_MAX_FPS),
                quality: self.quality,
            };
        }
        let aspect = export::aspect_ratio(&project.style.aspect, source);
        ExportOptions {
            format: Format::Mp4,
            codec: self.codec,
            width: export::width_for(self.res.long_edge(), aspect, source),
            fps: self.fps,
            quality: self.quality,
        }
    }
}

/// File extension of a container.
pub fn ext_of(format: Format) -> &'static str {
    match format {
        Format::Mp4 => "mp4",
        Format::Gif => "gif",
    }
}

/// `dest` with its extension replaced by `ext`, numbered (`name 2.gif`…) when that file exists.
pub fn with_extension(dest: &Path, ext: &str, exists: &dyn Fn(&Path) -> bool) -> PathBuf {
    let parent = dest.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = dest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Recording".into());
    parent.join(crate::notes_ctl::unique_file_name(&format!("{stem}.{ext}"), &|n| exists(&parent.join(n))))
}

/// `<project name>.mp4` next to the project folder (in the recordings folder), `name 2.mp4`… when taken.
pub fn default_dest(dir: &Path, name: &str, exists: &dyn Fn(&Path) -> bool) -> PathBuf {
    let stem = dir.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty());
    let base: String = if name.trim().is_empty() { stem.unwrap_or_else(|| "Recording".into()) } else { name.trim().to_string() };
    let clean: String = base.chars().map(|c| if matches!(c, '/' | ':' | '\\' | '\0') { '-' } else { c }).collect();
    let parent = dir
        .parent()
        .map(Path::to_path_buf)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| dirs::home_dir().map(|h| h.join("Movies")).unwrap_or_default());
    let file = crate::notes_ctl::unique_file_name(&format!("{clean}.mp4"), &|n| exists(&parent.join(n)));
    parent.join(file)
}

/// `Audio: mic + system` or `no audio`, for projects with a camera, microphone or system source
/// (a plain screen recording says nothing). A muted track does not count as audible.
pub fn audio_label(project: &Project) -> Option<String> {
    use ferrocast_core::capture::project::AudioTrack;
    let s = &project.sources;
    if s.camera.is_none() && s.mic.is_none() && s.system.is_none() {
        return None;
    }
    let audible = |has: bool, t| has && project.audio.linear_gain(t) > 0.0;
    let parts: Vec<&str> =
        [(audible(s.mic.is_some(), AudioTrack::Mic), "mic"), (audible(s.system.is_some(), AudioTrack::System), "system")]
            .into_iter()
            .filter_map(|(on, name)| on.then_some(name))
            .collect();
    Some(if parts.is_empty() { "no audio".into() } else { format!("Audio: {}", parts.join(" + ")) })
}

/// `1920×1080 · 2:31 · 4530 frames` (+ ` · Audio: mic + system` / ` · no audio` with extra sources) or the reason the options cannot be exported.
pub fn summary(project: &Project, source: [u32; 2], opts: &ExportOptions) -> Result<String, String> {
    let plan = export::export_plan(project, source, opts).map_err(|e| e.to_string())?;
    let audio = audio_label(project).map(|a| format!(" · {a}")).unwrap_or_default();
    Ok(format!(
        "{}×{} · {} · {} frames{audio}",
        plan.size[0],
        plan.size[1],
        crate::editor_ctl::clock(plan.duration).split('.').next().unwrap_or(""),
        plan.frames
    ))
}

/// `42 % · 18 s left` (no estimate before the first progress).
pub fn progress_text(done: u32, total: u32, eta: Option<f64>) -> String {
    let pct = if total == 0 { 0 } else { (u64::from(done) * 100 / u64::from(total)) as u32 };
    match eta {
        Some(e) if done > 0 && e.is_finite() => format!("{pct} % · {} left", eta_text(e)),
        _ => format!("{pct} %"),
    }
}

fn eta_text(secs: f64) -> String {
    let s = secs.max(0.0).ceil() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
    } else if s >= 60 {
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        format!("{s} s")
    }
}

impl Editor {
    /// Write what the helper must read: `project.json` when the project was edited, auto-zoomed in
    /// memory only, or has an autosave owed. A project that is already on disk is left alone.
    pub fn save_for_export(&mut self) -> Result<(), String> {
        let Some(p) = self.project.as_ref() else { return Err("The project is not loaded yet.".into()) };
        if self.dirty || self.st.autosave.pending() {
            p.save(&self.dir).map_err(|e| format!("Could not save project.json before exporting: {e}"))?;
            self.st.autosave.written();
            self.dirty = false;
        }
        Ok(())
    }

    /// Pixel size of the recorded video (from `events.json`; 1920×1080 when it is missing).
    pub fn source_pixels(&self) -> [u32; 2] {
        self.events.as_ref().map_or([1920, 1080], export::source_pixels)
    }

    /// Save, then export on a thread: `helper` is the capture helper executable, `notify` delivers
    /// the `ExportMsg`s to the app. Errors before the thread starts are returned (and shown).
    pub fn start_export(
        &mut self,
        helper: PathBuf,
        log: impl Fn(&str) + Send + 'static,
        notify: Box<dyn Fn(EditorMsg) + Send>,
    ) -> Result<(), String> {
        if self.export.running() {
            return Err("An export is already running.".into());
        }
        let Some(project) = self.project.clone() else { return Err("The project is not loaded yet.".into()) };
        let dest = self.export.dest.clone().ok_or("Choose where to save the video.")?;
        let ext = ext_of(self.export.format);
        if dest.extension().is_none_or(|e| !e.eq_ignore_ascii_case(ext)) {
            return Err(format!("The file name must end in .{ext}."));
        }
        let opts = self.export.options(&project, self.source_pixels());
        summary(&project, self.source_pixels(), &opts)?;
        self.save_for_export()?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.export.run = Some(ExportRun { cancel: cancel.clone(), done: 0, total: 0, eta: None, cancelling: false });
        self.export.message = None;
        let dir = self.dir.clone();
        std::thread::spawn(move || {
            let send = |msg| notify(EditorMsg::Export { dir: dir.clone(), msg });
            let result = ferrocast_macos::capture::export_project(&helper, log, &dir, &dest, &opts, &cancel, |done, total, eta| {
                send(ExportMsg::Progress { done, total, eta })
            });
            send(match result {
                Ok(d) => ExportMsg::Done(d),
                Err(ferrocast_macos::capture::ExportError::Cancelled) => ExportMsg::Cancelled,
                Err(e) => ExportMsg::Failed(e.to_string()),
            });
        });
        Ok(())
    }
}

/// The save dialog (the extension of `default` decides mp4 or gif): `choose file name` through osascript, with the default name and folder passed as
/// arguments (never inside the script text). `None` when cancelled or when it could not run.
pub fn choose_destination(default: &Path) -> Option<PathBuf> {
    let name = default.file_name()?.to_string_lossy().into_owned();
    let folder = default.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let out = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "POSIX path of (choose file name with prompt \"Export video\" default name (item 1 of argv) default location (POSIX file (item 2 of argv)))",
            "-e",
            "end run",
        ])
        .arg(name)
        .arg(folder)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let ext = if default.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif")) { "gif" } else { "mp4" };
    (path.starts_with('/')).then(|| force_ext(PathBuf::from(path), ext))
}

/// A chosen name always ends in `.ext`.
pub fn force_ext(p: PathBuf, ext: &str) -> PathBuf {
    if p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        p
    } else {
        let mut s = p.into_os_string();
        s.push(".");
        s.push(ext);
        PathBuf::from(s)
    }
}

/// The export sheet: options while idle, progress with ETA and Cancel while running.
pub fn draw_sheet(ui: &mut Ui, rect: Rect, pal: &Palette, ed: &mut Editor) -> SheetAction {
    let mut action = SheetAction::None;
    if !ed.export.open {
        return action;
    }
    let Some(project) = ed.project.clone() else { return action };
    let source = ed.source_pixels();
    let ctx = ui.ctx().clone();
    // Esc: cancel a running export, otherwise close the sheet
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        if ed.export.running() {
            ed.export.cancel();
        } else {
            ed.export.open = false;
        }
    }
    let running = ed.export.running();
    let sheet = Rect::from_center_size(rect.center(), Vec2::new(380.0, if running { 150.0 } else { 290.0 }));
    ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(110));
    // the scrim swallows the pointer so nothing below the sheet reacts
    ui.interact(rect, egui::Id::new("export-scrim"), egui::Sense::click_and_drag());
    // `field` is a translucent tint: over the editor it left the sheet see-through, so the text
    // overlapped the preview and the timeline (seen in the real app); paint the opaque base first
    ui.painter().rect_filled(sheet, 10.0, pal.bg);
    ui.painter().rect_filled(sheet, 10.0, pal.field);
    let opts = ed.export.options(&project, source);
    let plan = summary(&project, source, &opts);
    ui.scope_builder(egui::UiBuilder::new().max_rect(sheet.shrink(16.0)), |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(RichText::new("Export video").size(15.0).strong().color(pal.text));
        if let Some((frac, text, cancelling)) = ed.export.run.as_ref().map(|run| {
            let frac = if run.total == 0 { 0.0 } else { run.done as f32 / run.total as f32 };
            let text = if run.cancelling { "Cancelling…".to_string() } else { progress_text(run.done, run.total, run.eta) };
            (frac, text, run.cancelling)
        }) {
            ui.add(egui::ProgressBar::new(frac).desired_width(sheet.width() - 32.0).text(text));
            if let Some(d) = ed.export.dest.as_ref().and_then(|d| d.file_name()) {
                ui.label(RichText::new(d.to_string_lossy()).size(11.5).color(pal.sub));
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(!cancelling, egui::Button::new("Cancel")).clicked() {
                    ed.export.cancel();
                }
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }
        egui::Grid::new("export-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(RichText::new("Format").color(pal.sub));
            let mut format = ed.export.format;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut format, Format::Mp4, "MP4");
                ui.selectable_value(&mut format, Format::Gif, "GIF");
            });
            ed.export.set_format(format);
            ui.end_row();
            if ed.export.format == Format::Gif {
                ui.label(RichText::new("Width").color(pal.sub));
                ui.horizontal(|ui| {
                    for r in Res::GIF {
                        ui.selectable_value(&mut ed.export.gif_res, r, r.label());
                    }
                });
                ui.end_row();
                ui.label(RichText::new("Frame rate").color(pal.sub));
                ui.horizontal(|ui| {
                    for f in Res::GIF_FPS {
                        ui.selectable_value(&mut ed.export.gif_fps, f, format!("{f} fps"));
                    }
                });
                ui.end_row();
                return;
            }
            ui.label(RichText::new("Codec").color(pal.sub));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ed.export.codec, Codec::H264, "H.264");
                ui.selectable_value(&mut ed.export.codec, Codec::Hevc, "HEVC");
            });
            ui.end_row();
            ui.label(RichText::new("Resolution").color(pal.sub));
            ui.horizontal(|ui| {
                for r in Res::ALL {
                    ui.selectable_value(&mut ed.export.res, r, r.label());
                }
            });
            ui.end_row();
            ui.label(RichText::new("Frame rate").color(pal.sub));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ed.export.fps, 30, "30 fps");
                ui.selectable_value(&mut ed.export.fps, 60, "60 fps");
            });
            ui.end_row();
            ui.label(RichText::new("Quality").color(pal.sub));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ed.export.quality, Quality::Standard, "Standard");
                ui.selectable_value(&mut ed.export.quality, Quality::High, "High");
            });
            ui.end_row();
        });
        let dest = ed.export.dest.as_ref().map(|d| d.display().to_string()).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(RichText::new(truncate_middle(&dest, 38)).size(11.5).color(pal.sub));
            if ui.button("Choose…").clicked() {
                action = SheetAction::Choose;
            }
        });
        match &plan {
            Ok(s) => ui.label(RichText::new(s).size(12.0).color(pal.text)),
            Err(e) => ui.label(RichText::new(e).size(12.0).color(pal.danger)),
        };
        if let Some(m) = &ed.export.message {
            ui.label(RichText::new(m).size(12.0).color(pal.danger));
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(plan.is_ok(), egui::Button::new("Export")).clicked() {
                action = SheetAction::Start;
            }
            if ui.button("Close").clicked() {
                ed.export.open = false;
            }
        });
    });
    ui.painter().text(
        Pos2::new(rect.center().x, rect.max.y - 14.0),
        Align2::CENTER_CENTER,
        "⌘E export · esc closes",
        FontId::proportional(11.0),
        pal.sub,
    );
    action
}

/// `/Users/me/…/Demo.mp4` shortened in the middle to at most `max` characters.
pub fn truncate_middle(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 5 {
        return s.to_string();
    }
    let keep = max - 1;
    let (head, tail) = (keep / 3, keep - keep / 3);
    let h: String = s.chars().take(head).collect();
    let t: String = s.chars().skip(n - tail).collect();
    format!("{h}…{t}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_ctl::Opened;
    use ferrocast_core::capture::events::ScreenInfo;
    use std::sync::mpsc::channel;

    fn project(duration: f64) -> Project {
        let mut p = Project::new("Demo", "2026-01-01T00:00:00Z");
        p.duration = duration;
        p
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fc-export-ui-{}-{tag}", std::process::id())).join("Demo.ferrocast");
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn default_destination_is_next_to_the_project_and_never_overwrites() {
        let dir = Path::new("/Users/me/Movies/Ferrocast Recordings/Take 1.ferrocast");
        assert_eq!(default_dest(dir, "Demo", &|_| false), PathBuf::from("/Users/me/Movies/Ferrocast Recordings/Demo.mp4"));
        let taken = |p: &Path| p.ends_with("Demo.mp4") || p.ends_with("Demo 2.mp4");
        assert_eq!(default_dest(dir, "Demo", &taken), PathBuf::from("/Users/me/Movies/Ferrocast Recordings/Demo 3.mp4"));
        // an unnamed project uses the folder name; slashes and colons never reach the file name
        assert_eq!(default_dest(dir, " ", &|_| false).file_name().unwrap(), "Take 1.mp4");
        assert_eq!(default_dest(dir, "a/b:c", &|_| false).file_name().unwrap(), "a-b-c.mp4");
    }

    #[test]
    fn options_follow_the_sheet_and_the_aspect() {
        let mut s = ExportSheet::default();
        let p = project(10.0);
        let src = [3456, 2234];
        assert_eq!(
            s.options(&p, src),
            ExportOptions { format: Format::Mp4, codec: Codec::H264, width: 1920, fps: 30, quality: Quality::Standard }
        );
        s.res = Res::P4k;
        s.fps = 60;
        s.codec = Codec::Hevc;
        let o = s.options(&p, src);
        assert_eq!((o.width, o.fps, o.codec), (3840, 60, Codec::Hevc));
        assert!(export::export_plan(&p, src, &o).is_ok(), "4K @ 60 passes the core limits");
        s.res = Res::Source;
        assert_eq!(s.options(&p, src).width, 3456);
        // portrait: 1080p is 1080 x 1920, 4K is 2160 x 3840, both inside the limits
        let mut portrait = project(10.0);
        portrait.style.aspect = "9:16".into();
        for (res, w) in [(Res::P1080, 1080), (Res::P4k, 2160)] {
            s.res = res;
            let o = s.options(&portrait, src);
            assert_eq!(o.width, w);
            assert!(export::export_plan(&portrait, src, &o).is_ok());
        }
    }

    #[test]
    fn gif_options_follow_the_gif_limits_and_leave_mp4_alone() {
        let mut s = ExportSheet::default();
        let p = project(10.0);
        let src = [3456, 2234];
        s.set_format(Format::Gif);
        let o = s.options(&p, src);
        assert_eq!((o.format, o.width, o.fps), (Format::Gif, 720, 15), "default 15 fps, 720 px");
        for r in Res::GIF {
            s.gif_res = r;
            for f in Res::GIF_FPS {
                s.gif_fps = f;
                let o = s.options(&p, src);
                assert!(o.width <= export::GIF_MAX_WIDTH && o.fps <= export::GIF_MAX_FPS);
                let plan = export::export_plan(&p, src, &o).unwrap();
                assert_eq!((plan.size[0], plan.fps), (o.width, f), "nothing is silently clamped");
            }
        }
        // the MP4 choices are kept while GIF is selected and come back
        s.res = Res::P4k;
        s.fps = 60;
        s.set_format(Format::Mp4);
        let o = s.options(&p, src);
        assert_eq!((o.format, o.width, o.fps), (Format::Mp4, 3840, 60));
        assert!(!Res::ALL.iter().any(|r| r.gif_width().is_some()) && Res::GIF.iter().all(|r| r.gif_width().is_some()));
    }

    #[test]
    fn switching_format_changes_the_extension_without_overwriting() {
        let mut s = ExportSheet { dest: Some(PathBuf::from("/o/Demo.mp4")), ..Default::default() };
        s.set_format(Format::Gif);
        assert_eq!(s.dest, Some(PathBuf::from("/o/Demo.gif")));
        s.set_format(Format::Gif);
        assert_eq!(s.dest, Some(PathBuf::from("/o/Demo.gif")), "same format: nothing changes");
        s.set_format(Format::Mp4);
        assert_eq!(s.dest, Some(PathBuf::from("/o/Demo.mp4")));
        let taken = |p: &Path| p.ends_with("Demo.gif") || p.ends_with("Demo 2.gif");
        assert_eq!(with_extension(Path::new("/o/Demo.mp4"), "gif", &taken), PathBuf::from("/o/Demo 3.gif"));
        assert_eq!(force_ext(PathBuf::from("/a/b"), "gif"), PathBuf::from("/a/b.gif"));
        assert_eq!(force_ext(PathBuf::from("/a/b.GIF"), "gif"), PathBuf::from("/a/b.GIF"));
    }

    #[test]
    fn summary_shows_size_length_and_frames_or_the_reason() {
        let p = project(151.0);
        let o = ExportSheet::default().options(&p, [1920, 1080]);
        assert_eq!(summary(&p, [1920, 1080], &o).unwrap(), "1920×1080 · 2:31 · 4530 frames");
        let empty = project(0.0);
        assert!(summary(&empty, [1920, 1080], &o).unwrap_err().contains("nothing to export"));
    }

    #[test]
    fn summary_names_the_audio_only_when_there_are_extra_sources() {
        use ferrocast_core::capture::project::{Source, TrackMix};
        let src = |f: &str| Some(Source { file: f.into(), offset: 0.0 });
        let mut p = project(151.0);
        let o = ExportSheet::default().options(&p, [1920, 1080]);
        assert_eq!(audio_label(&p), None);
        assert_eq!(summary(&p, [1920, 1080], &o).unwrap(), "1920×1080 · 2:31 · 4530 frames", "a plain recording is unchanged");
        p.sources.mic = src("mic.m4a");
        p.sources.system = src("system.m4a");
        assert_eq!(summary(&p, [1920, 1080], &o).unwrap(), "1920×1080 · 2:31 · 4530 frames · Audio: mic + system");
        p.audio.system = Some(TrackMix { gain_db: 0.0, mute: true });
        assert_eq!(audio_label(&p).as_deref(), Some("Audio: mic"), "a muted track is not in the file");
        p.audio.mic = Some(TrackMix { gain_db: 0.0, mute: true });
        assert_eq!(audio_label(&p).as_deref(), Some("no audio"));
        let mut cam_only = project(151.0);
        cam_only.sources.camera = src("camera.mov");
        assert_eq!(audio_label(&cam_only).as_deref(), Some("no audio"), "a camera alone has no sound");
        let mut all_muted = project(151.0);
        all_muted.sources.mic = src("mic.m4a");
        all_muted.audio.mute = true;
        assert_eq!(audio_label(&all_muted).as_deref(), Some("no audio"));
    }

    #[test]
    fn progress_text_has_percent_and_eta() {
        assert_eq!(progress_text(0, 100, None), "0 %");
        assert_eq!(progress_text(42, 100, Some(17.2)), "42 % · 18 s left");
        assert_eq!(progress_text(1, 3, Some(125.0)), "33 % · 2:05 left");
        assert_eq!(progress_text(5, 5, Some(0.0)), "100 % · 0 s left");
        assert_eq!(progress_text(0, 0, Some(3.0)), "0 %");
        assert_eq!(eta_text(3725.0), "1:02:05");
    }

    #[test]
    fn messages_drive_the_sheet_state() {
        let mut s = ExportSheet { open: true, ..Default::default() };
        s.run = Some(ExportRun { cancel: Arc::new(AtomicBool::new(false)), done: 0, total: 0, eta: None, cancelling: false });
        assert_eq!(s.apply(ExportMsg::Progress { done: 5, total: 10, eta: Some(2.0) }), Outcome::None);
        assert_eq!((s.run.as_ref().unwrap().done, s.run.as_ref().unwrap().total), (5, 10));
        s.cancel();
        assert!(s.run.as_ref().unwrap().cancelling && s.run.as_ref().unwrap().cancel.load(Ordering::Relaxed));
        assert_eq!(s.apply(ExportMsg::Cancelled), Outcome::None);
        assert!(!s.running() && s.open && s.message.is_none(), "cancelling leaves the sheet open and quiet");
        s.run = Some(ExportRun { cancel: Arc::new(AtomicBool::new(false)), done: 0, total: 0, eta: None, cancelling: false });
        s.apply(ExportMsg::Failed("The disk is full".into()));
        assert_eq!(s.message.as_deref(), Some("The disk is full"));
        assert!(!s.running());
        s.run = Some(ExportRun { cancel: Arc::new(AtomicBool::new(false)), done: 9, total: 10, eta: None, cancelling: false });
        let out = s.apply(ExportMsg::Done(ExportDone { path: "/o/Demo.mp4".into(), bytes: 1, duration: 1.0, frames: 30 }));
        assert_eq!(out, Outcome::Finished(PathBuf::from("/o/Demo.mp4")));
        assert!(!s.open && !s.running());
    }

    #[test]
    fn dropping_a_running_export_cancels_it() {
        let flag = Arc::new(AtomicBool::new(false));
        let run = ExportRun { cancel: flag.clone(), done: 0, total: 0, eta: None, cancelling: false };
        drop(run);
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn chosen_names_always_end_in_mp4() {
        assert_eq!(force_ext(PathBuf::from("/a/b"), "mp4"), PathBuf::from("/a/b.mp4"));
        assert_eq!(force_ext(PathBuf::from("/a/b.MP4"), "mp4"), PathBuf::from("/a/b.MP4"));
        assert_eq!(force_ext(PathBuf::from("/a/b.mov"), "mp4"), PathBuf::from("/a/b.mov.mp4"));
    }

    #[test]
    fn long_paths_are_shortened_in_the_middle() {
        assert_eq!(truncate_middle("short.mp4", 38), "short.mp4");
        let t = truncate_middle("/Users/someone/Movies/Ferrocast Recordings/An example recording.mp4", 30);
        assert_eq!(t.chars().count(), 30);
        assert!(t.contains('…') && t.ends_with("recording.mp4") && t.starts_with("/Users"));
    }

    fn editor(dir: &Path, dirty: bool) -> Editor {
        let (tx, _rx) = channel();
        let mut ed = Editor::new(dir.to_path_buf(), Some(tx));
        let mut p = project(3.0);
        p.zooms = vec![];
        p.save(dir).unwrap();
        let mut edited = p.clone();
        if dirty {
            edited.name = "Edited".into();
        }
        let events =
            Events::new(ScreenInfo { size: [2000, 1000], scale: 2.0, display_frame: [0.0, 0.0, 1000.0, 500.0], capture_rect: None });
        ed.opened(Ok(Opened { project: edited, auto_zoomed: dirty, note: None, events: Some(events) }));
        ed
    }

    #[test]
    fn saving_for_export_writes_only_what_is_owed() {
        let dir = temp("save");
        let mut clean = editor(&dir, false);
        let before = std::fs::metadata(dir.join("project.json")).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        clean.save_for_export().unwrap();
        assert_eq!(
            std::fs::metadata(dir.join("project.json")).unwrap().modified().unwrap(),
            before,
            "an unchanged project is not rewritten"
        );
        let dir2 = temp("save2");
        let mut dirty = editor(&dir2, true);
        assert_eq!(Project::load(&dir2).unwrap().name, "Demo");
        dirty.save_for_export().unwrap();
        assert_eq!(
            Project::load(&dir2).unwrap().name,
            "Edited",
            "the in-memory project (auto zooms) is written before the helper reads it"
        );
        assert!(!dirty.dirty && !dirty.st.autosave.pending());
    }

    #[test]
    fn a_pending_autosave_is_written_by_the_export() {
        let dir = temp("pending");
        let mut ed = editor(&dir, false);
        let now = ed.now();
        assert!(ed.apply(ferrocast_core::capture::timeline::EditKind::Other, now, |p| {
            p.cursor.size = 2.0;
            Ok(())
        }));
        assert!(ed.st.autosave.pending());
        assert_eq!(Project::load(&dir).unwrap().cursor.size, 1.4);
        ed.save_for_export().unwrap();
        assert_eq!(Project::load(&dir).unwrap().cursor.size, 2.0);
    }

    #[test]
    fn starting_refuses_bad_destinations_and_a_second_export() {
        let dir = temp("start");
        let mut ed = editor(&dir, false);
        let notify = || -> Box<dyn Fn(EditorMsg) + Send> { Box::new(|_| {}) };
        assert!(ed.start_export(PathBuf::from("/nonexistent"), |_| {}, notify()).unwrap_err().contains("Choose where"));
        ed.export.dest = Some(PathBuf::from("/tmp/x.mov"));
        assert!(ed.start_export(PathBuf::from("/nonexistent"), |_| {}, notify()).unwrap_err().contains(".mp4"));
        ed.export.format = Format::Gif;
        ed.export.dest = Some(PathBuf::from("/tmp/x.mp4"));
        assert!(ed.start_export(PathBuf::from("/nonexistent"), |_| {}, notify()).unwrap_err().contains(".gif"));
        ed.export.format = Format::Mp4;
        ed.export.dest = Some(dir.with_file_name("x.mp4"));
        ed.export.res = Res::P1080;
        ed.export.run = Some(ExportRun { cancel: Arc::new(AtomicBool::new(false)), done: 0, total: 0, eta: None, cancelling: false });
        assert!(ed.start_export(PathBuf::from("/nonexistent"), |_| {}, notify()).unwrap_err().contains("already"));
    }

    #[test]
    fn a_failing_helper_reports_failure_through_the_channel() {
        let dir = temp("thread");
        std::fs::write(
            dir.join("events.json"),
            r#"{"version":1,"screen":{"size":[2000,1000],"scale":2.0,"display_frame":[0,0,1000,500]}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("screen.mov"), b"x").unwrap();
        let mut ed = editor(&dir, false);
        ed.export.dest = Some(dir.with_file_name("thread.mp4"));
        let (tx, rx) = channel();
        let tx = std::sync::Mutex::new(tx);
        let notify: Box<dyn Fn(EditorMsg) + Send> = Box::new(move |m| {
            let _ = tx.lock().unwrap().send(m);
        });
        ed.start_export(PathBuf::from("/nonexistent/helper"), |_| {}, notify).unwrap();
        assert!(ed.export.running());
        let mut last = None;
        while let Ok(EditorMsg::Export { msg, .. }) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
            let end = matches!(msg, ExportMsg::Failed(_) | ExportMsg::Done(_) | ExportMsg::Cancelled);
            last = Some(msg);
            if end {
                break;
            }
        }
        assert!(matches!(last, Some(ExportMsg::Failed(_))), "{last:?}");
    }
}
