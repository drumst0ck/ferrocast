//! Editor panels (T27): the pure mapping from panel values to EDL edits, the undo/redo + autosave
//! wiring of the editor state, and the right-hand panel drawing (Zoom / Cursor / Style / Clips).
//!
//! Every change of the project goes through [`Editor::apply`]: the edit runs on a copy, is checked
//! with `timeline::validate` + `Project::validate` (an edit that adds a problem is rejected and its
//! message shown inline), then recorded in `timeline::History` so Undo/Redo cover every EDL field.
//! The worker gets the edited project for the next frame; `project.json` is written by the
//! [`AutosaveClock`] one idle second after the last user change.

use crate::editor_ctl::{Editor, WorkerCmd};
use crate::ui::Palette;
use egui::{Rect, Ui, UiBuilder};
use ferrocast_core::capture::project::{Project, Style, Trim, Zoom};
use ferrocast_core::capture::timeline::{self, EditKind, History, Timeline};
use ferrocast_core::capture::zoom::{self, ZoomOpts, MAX_SCALE};
use ferrocast_core::capture::MIN_SEGMENT;
use std::time::Instant;

/// Undo steps kept.
pub const UNDO_DEPTH: usize = 200;
/// Edits of one kind closer than this (seconds) are one undo step (slider drags).
pub const COALESCE_WINDOW: f64 = 1.0;
/// `project.json` is written this long after the last change.
pub const AUTOSAVE_IDLE: f64 = 1.0;
/// Shortest zoom the panel creates or leaves behind.
pub const MIN_ZOOM: f64 = 0.2;
/// Length of a zoom added at the playhead.
pub const NEW_ZOOM_LEN: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Zoom,
    Cursor,
    Style,
    Clips,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Zoom, Tab::Cursor, Tab::Style, Tab::Clips];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Zoom => "Zoom",
            Tab::Cursor => "Cursor",
            Tab::Style => "Style",
            Tab::Clips => "Clips",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Autosave clock (pure)

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStatus {
    Saved,
    Saving,
    Failed(String),
}

/// Debounce of `project.json` writes on a clock the caller supplies (seconds).
#[derive(Debug, Clone)]
pub struct AutosaveClock {
    idle: f64,
    dirty: bool,
    /// Last user change, or last failed attempt (the retry waits one more idle period).
    mark: f64,
    error: Option<String>,
}

impl AutosaveClock {
    pub fn new(idle: f64) -> Self {
        Self { idle, dirty: false, mark: 0.0, error: None }
    }

    /// The user changed something at `now`.
    pub fn changed(&mut self, now: f64) {
        self.dirty = true;
        self.mark = now;
        self.error = None;
    }

    /// A write is owed (changes not on disk).
    pub fn pending(&self) -> bool {
        self.dirty
    }

    /// Time to write: dirty and quiet for the idle period.
    pub fn due(&self, now: f64) -> bool {
        self.dirty && now - self.mark >= self.idle
    }

    /// Seconds until a write is due (None when nothing is owed).
    pub fn wait(&self, now: f64) -> Option<f64> {
        self.dirty.then(|| (self.mark + self.idle - now).max(0.0))
    }

    /// The write succeeded.
    pub fn written(&mut self) {
        self.dirty = false;
        self.error = None;
    }

    /// The write failed: still dirty, retried after another idle period, the text is kept for the footer.
    pub fn failed(&mut self, now: f64, message: String) {
        self.dirty = true;
        self.mark = now;
        self.error = Some(message);
    }

    pub fn status(&self) -> SaveStatus {
        match (&self.error, self.dirty) {
            (Some(e), _) => SaveStatus::Failed(e.clone()),
            (None, true) => SaveStatus::Saving,
            (None, false) => SaveStatus::Saved,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Edits: panel value -> project change (clamped; validation is the caller's)

pub enum ZoomField {
    Start(f64),
    End(f64),
    Scale(f64),
    /// `None` follows the cursor.
    Focus(Option<[f64; 2]>),
    EaseIn(f64),
    EaseOut(f64),
}

fn finite(v: f64) -> Result<f64, String> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err("That is not a number.".into())
    }
}

/// A manual zoom at `at`, [`NEW_ZOOM_LEN`] long, cut short by the next zoom or the end.
pub fn new_zoom(p: &Project, at: f64) -> Result<Zoom, String> {
    let at = finite(at)?.clamp(0.0, p.duration.max(0.0));
    if p.zooms.iter().any(|z| z.start <= at && at < z.end) {
        return Err("There is already a zoom at the playhead.".into());
    }
    let next = p.zooms.iter().filter(|z| z.start > at).map(|z| z.start).fold(f64::INFINITY, f64::min);
    let end = (at + NEW_ZOOM_LEN).min(p.duration).min(next);
    if end - at < MIN_ZOOM {
        return Err("There is no room for a zoom at the playhead.".into());
    }
    let id = (1..).map(|n| format!("m{n}")).find(|id| p.zooms.iter().all(|z| z.id != *id)).unwrap_or_default();
    let zoom = Zoom { id, start: at, end, mode: "manual".into(), scale: 2.0, focus: Some([0.5, 0.5]), ease_in: 0.6, ease_out: 0.7 };
    Ok(zoom)
}

pub fn add_zoom(p: &mut Project, at: f64) -> Result<(), String> {
    let z = new_zoom(p, at)?;
    p.zooms.push(z);
    p.zooms.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(())
}

/// Edit one field of zoom `id` (clamped). Editing makes the zoom manual, so `Auto zoom` keeps it.
pub fn set_zoom(p: &mut Project, id: &str, field: ZoomField) -> Result<(), String> {
    let dur = p.duration.max(0.0);
    let z = p.zooms.iter_mut().find(|z| z.id == id).ok_or("That zoom no longer exists.")?;
    let before = z.clone();
    match field {
        ZoomField::Start(v) => z.start = finite(v)?.clamp(0.0, (z.end - MIN_ZOOM).max(0.0)),
        ZoomField::End(v) => z.end = finite(v)?.clamp(z.start + MIN_ZOOM, dur.max(z.start + MIN_ZOOM)),
        ZoomField::Scale(v) => z.scale = finite(v)?.clamp(1.0, MAX_SCALE),
        ZoomField::Focus(f) => {
            z.focus = match f {
                Some([x, y]) => Some([finite(x)?.clamp(0.0, 1.0), finite(y)?.clamp(0.0, 1.0)]),
                None => None,
            }
        }
        ZoomField::EaseIn(v) => z.ease_in = finite(v)?.clamp(0.0, 3.0),
        ZoomField::EaseOut(v) => z.ease_out = finite(v)?.clamp(0.0, 3.0),
    }
    if *z != before {
        z.mode = "manual".into();
    }
    Ok(())
}

pub fn delete_zoom(p: &mut Project, id: &str) -> Result<(), String> {
    let n = p.zooms.len();
    p.zooms.retain(|z| z.id != id);
    if p.zooms.len() == n {
        return Err("That zoom no longer exists.".into());
    }
    Ok(())
}

/// `Auto zoom`: the automatic zooms are replaced by `fresh`; manual ones stay and win overlaps.
pub fn replace_auto(p: &mut Project, fresh: Vec<Zoom>) {
    let mut zooms: Vec<Zoom> = p.zooms.iter().filter(|z| z.mode != "auto").cloned().collect();
    let manual = zooms.clone();
    zooms.extend(fresh.into_iter().filter(|a| manual.iter().all(|m| !(a.start < m.end && m.start < a.end))));
    zooms.sort_by(|a, b| a.start.total_cmp(&b.start));
    p.zooms = zooms;
}

/// Cursor styles `compose` understands: the sprite, or no cursor (`none` and `hidden` are the same).
pub const CURSOR_STYLES: [(&str, &str); 3] = [("arrow", "Default"), ("none", "None"), ("hidden", "Hidden")];

pub enum CursorField {
    Style(String),
    Size(f64),
    Smoothing(f64),
    ClickRing(bool),
    MotionBlur(f64),
}

pub fn set_cursor(p: &mut Project, field: CursorField) -> Result<(), String> {
    let c = &mut p.cursor;
    match field {
        CursorField::Style(s) => {
            if !CURSOR_STYLES.iter().any(|(id, _)| *id == s) {
                return Err(format!("`{s}` is not a cursor style."));
            }
            c.style = s;
        }
        CursorField::Size(v) => c.size = finite(v)?.clamp(0.5, 4.0),
        CursorField::Smoothing(v) => c.smoothing = finite(v)?.clamp(0.0, 1.0),
        CursorField::ClickRing(on) => c.click_effect = if on { "ring" } else { "none" }.into(),
        CursorField::MotionBlur(v) => c.motion_blur = finite(v)?.clamp(0.0, 2.0),
    }
    Ok(())
}

/// Output aspects `render.frame` knows (anything else follows the recording).
pub const ASPECTS: [(&str, &str); 5] = [("16:9", "16:9"), ("4:3", "4:3"), ("1:1", "1:1"), ("9:16", "9:16"), ("source", "Source")];

/// Bundled wallpapers (the same gradients `Render.swift` generates). `Project::validate` only accepts
/// `#rrggbb` stops, so a wallpaper is stored as the gradient it is rather than as kind `wallpaper`.
pub const WALLPAPERS: [(&str, &[&str], f64); 8] = [
    ("Aurora", &["#0f2027", "#2c5364", "#42e695"], 135.0),
    ("Dusk", &["#2b1055", "#d53369", "#ff9a5a"], 160.0),
    ("Ocean", &["#0f4c81", "#1fa2ff", "#a6ffcb"], 120.0),
    ("Graphite", &["#141414", "#3a3a3c"], 135.0),
    ("Sand", &["#f5e6c8", "#d9a066"], 150.0),
    ("Lilac", &["#e0c3fc", "#8ec5fc"], 135.0),
    ("Forest", &["#0b3d2e", "#2e8b57", "#b6e388"], 145.0),
    ("Rose", &["#ffdde1", "#ee9ca7"], 135.0),
];

pub enum StyleField {
    Aspect(String),
    Padding(f64),
    Radius(f64),
    ShadowBlur(f64),
    ShadowOpacity(f64),
    ShadowDy(f64),
    /// `color`, `gradient` or `blur`.
    BgKind(String),
    /// Stop `i` as `#rrggbb`.
    BgStop(usize, String),
    BgAngle(f64),
    Wallpaper(usize),
}

fn hex_ok(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn set_style(p: &mut Project, field: StyleField) -> Result<(), String> {
    let s = &mut p.style;
    match field {
        StyleField::Aspect(a) => {
            if !ASPECTS.iter().any(|(id, _)| *id == a) {
                return Err(format!("`{a}` is not an aspect ratio."));
            }
            s.aspect = a;
        }
        StyleField::Padding(v) => s.padding = finite(v)?.clamp(0.0, 0.45),
        StyleField::Radius(v) => s.radius = finite(v)?.clamp(0.0, 64.0),
        StyleField::ShadowBlur(v) => s.shadow.blur = finite(v)?.clamp(0.0, 120.0),
        StyleField::ShadowOpacity(v) => s.shadow.opacity = finite(v)?.clamp(0.0, 1.0),
        StyleField::ShadowDy(v) => s.shadow.dy = finite(v)?.clamp(0.0, 80.0),
        StyleField::BgKind(k) => {
            if !matches!(k.as_str(), "color" | "gradient" | "blur") {
                return Err(format!("`{k}` is not a background kind."));
            }
            let stops = &mut s.background.stops;
            let want = if k == "gradient" { 2 } else { 1 };
            let defaults = ["#1f1c2c", "#928dab"];
            while stops.len() < want {
                stops.push(defaults[stops.len().min(1)].into());
            }
            s.background.kind = k;
        }
        StyleField::BgStop(i, hex) => {
            if !hex_ok(&hex) {
                return Err(format!("`{hex}` is not a #rrggbb colour."));
            }
            let slot = s.background.stops.get_mut(i).ok_or("That colour does not exist.")?;
            *slot = hex.to_ascii_lowercase();
        }
        StyleField::BgAngle(v) => s.background.angle = finite(v)?.clamp(0.0, 360.0),
        StyleField::Wallpaper(i) => {
            let (_, stops, angle) = WALLPAPERS.get(i).ok_or("That wallpaper does not exist.")?;
            s.background.kind = "gradient".into();
            s.background.stops = stops.iter().map(|c| (*c).to_string()).collect();
            s.background.angle = *angle;
        }
    }
    Ok(())
}

// ---- Clips

/// The kept range (a trim, or the whole recording) holding source time `t`.
pub fn segment_at(p: &Project, t: f64) -> Option<(f64, f64)> {
    let ranges = kept(p);
    ranges.iter().copied().find(|&(s, e)| t >= s && t < e).or_else(|| ranges.iter().copied().find(|&(_, e)| t == e))
}

fn kept(p: &Project) -> Vec<(f64, f64)> {
    if p.trims.is_empty() {
        return if p.duration > 0.0 { vec![(0.0, p.duration)] } else { vec![] };
    }
    let mut r: Vec<(f64, f64)> = p.trims.iter().map(|t| (t.start, t.end)).collect();
    r.sort_by(|a, b| a.0.total_cmp(&b.0));
    r
}

/// Trim in: cut everything before the playhead.
pub fn trim_in(p: &mut Project, t: f64) -> Result<(), String> {
    if timeline::remove_range(p, 0.0, finite(t)?) {
        Ok(())
    } else {
        Err("There is nothing to trim before the playhead.".into())
    }
}

/// Trim out: cut everything after the playhead.
pub fn trim_out(p: &mut Project, t: f64) -> Result<(), String> {
    let end = p.duration;
    if timeline::remove_range(p, finite(t)?, end) {
        Ok(())
    } else {
        Err("There is nothing to trim after the playhead.".into())
    }
}

/// Split the segment under the playhead in two (each at least `MIN_SEGMENT` long).
pub fn split_at(p: &mut Project, t: f64) -> Result<(), String> {
    let t = finite(t)?;
    let ranges = kept(p);
    let Some(i) = ranges.iter().position(|&(s, e)| t - s >= MIN_SEGMENT && e - t >= MIN_SEGMENT) else {
        return Err("Move the playhead inside a segment, away from its ends, to split it.".into());
    };
    let (s, e) = ranges[i];
    let mut next: Vec<Trim> = ranges.iter().map(|&(start, end)| Trim { start, end }).collect();
    next.splice(i..=i, [Trim { start: s, end: t }, Trim { start: t, end: e }]);
    p.trims = next;
    Ok(())
}

/// Delete the segment under the playhead.
pub fn delete_segment(p: &mut Project, t: f64) -> Result<(), String> {
    let (s, e) = segment_at(p, finite(t)?).ok_or("The playhead is not inside a segment.")?;
    if timeline::remove_range(p, s, e) {
        Ok(())
    } else {
        Err("The only segment cannot be deleted.".into())
    }
}

/// Play the segment under the playhead at `factor` (`SPEED_MIN..=SPEED_MAX`; 1 clears it).
pub fn set_segment_speed(p: &mut Project, t: f64, factor: f64) -> Result<(), String> {
    let factor = finite(factor)?;
    if !(ferrocast_core::videoplan::SPEED_MIN..=timeline::SPEED_MAX).contains(&factor) {
        return Err(format!("Speed must be between {}× and {}×.", ferrocast_core::videoplan::SPEED_MIN, timeline::SPEED_MAX));
    }
    let (s, e) = segment_at(p, finite(t)?).ok_or("The playhead is not inside a segment.")?;
    timeline::set_speed(p, s, e, factor);
    Ok(())
}

// ---- Focus picking

fn aspect_ratio(spec: &str, source: f64) -> f64 {
    let parts: Vec<f64> = spec.split([':', '/']).filter_map(|x| x.parse().ok()).collect();
    match parts[..] {
        [w, h] if w > 0.0 && h > 0.0 => w / h,
        _ => source,
    }
}

/// A click inside the preview image (`0..=1` both axes) -> the zoom focus in the normalised video, or
/// `None` when the click is on the padding. `video_aspect` is width/height of the recording;
/// `camera` is the `[x, y, w, h]` crop the frame on screen shows (a zoomed frame).
pub fn preview_to_focus(click: [f64; 2], style: &Style, video_aspect: f64, camera: Option<[f64; 4]>) -> Option<[f64; 2]> {
    let src = if video_aspect.is_finite() && video_aspect > 0.0 { video_aspect } else { 16.0 / 9.0 };
    let (w, h) = (1.0, 1.0 / aspect_ratio(&style.aspect, src));
    let pad = style.padding.clamp(0.0, 0.45) * w;
    let s = ((w - 2.0 * pad) / src).min((h - 2.0 * pad) / 1.0);
    let (fw, fh) = (src * s, s);
    let (fx, fy) = ((w - fw) / 2.0, (h - fh) / 2.0);
    let (u, v) = ((click[0] * w - fx) / fw, (click[1] * h - fy) / fh);
    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return None;
    }
    let [cx, cy, cw, ch] = camera.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    Some([(cx + u * cw).clamp(0.0, 1.0), (cy + v * ch).clamp(0.0, 1.0)])
}

// ---------------------------------------------------------------------------------------------
// Editor state: history, autosave

/// What the panels add to the editor state.
pub struct Studio {
    pub history: History,
    pub autosave: AutosaveClock,
    pub tab: Tab,
    /// Inline message of the last rejected edit.
    pub edit_error: Option<String>,
    pub selected_zoom: Option<String>,
    /// The next click on the preview sets the focus of the selected zoom.
    pub picking: bool,
    /// Speed factor offered by the Clips tab.
    pub speed: f64,
    started: Instant,
}

impl Studio {
    pub fn new() -> Self {
        Self {
            history: History::new(UNDO_DEPTH, COALESCE_WINDOW),
            autosave: AutosaveClock::new(AUTOSAVE_IDLE),
            tab: Tab::default(),
            edit_error: None,
            selected_zoom: None,
            picking: false,
            speed: 2.0,
            started: Instant::now(),
        }
    }
}

impl Default for Studio {
    fn default() -> Self {
        Self::new()
    }
}

/// Problems of `p` as the editor sees them: both validators.
fn problems(p: &Project) -> Vec<String> {
    let mut v = timeline::validate(p);
    for m in p.validate(|_| true) {
        if !v.contains(&m) {
            v.push(m);
        }
    }
    v
}

impl Editor {
    /// Seconds on the editor's own wall clock (the autosave debounce and undo coalescing use it).
    pub fn now(&self) -> f64 {
        self.st.started.elapsed().as_secs_f64()
    }

    /// Run an edit: on a copy, validated, then recorded in the history. Returns whether the project
    /// changed. A rejected edit leaves the project alone and sets `st.edit_error`.
    pub fn apply(&mut self, kind: EditKind, now: f64, f: impl FnOnce(&mut Project) -> Result<(), String>) -> bool {
        let Some(project) = self.project.as_mut() else { return false };
        let mut cand = project.clone();
        if let Err(e) = f(&mut cand) {
            self.st.edit_error = Some(e);
            return false;
        }
        if cand == *project {
            self.st.edit_error = None;
            return false;
        }
        let before = problems(project);
        let new: Vec<String> = problems(&cand).into_iter().filter(|m| !before.contains(m)).collect();
        if !new.is_empty() {
            self.st.edit_error = Some(new.join(" "));
            return false;
        }
        self.st.history.edit(project, kind, now, |p| *p = cand);
        self.st.edit_error = None;
        self.after_change(now);
        true
    }

    /// Width / height of the recorded video (16:9 when `events.json` is missing).
    pub fn video_aspect(&self) -> f64 {
        use ferrocast_core::capture::events::CaptureSpace;
        let Some(e) = self.events.as_ref() else { return 16.0 / 9.0 };
        let [w, h] = CaptureSpace::new(&e.screen).size();
        if w > 0.0 && h > 0.0 {
            w / h
        } else {
            16.0 / 9.0
        }
    }

    /// ⌘Z.
    pub fn undo(&mut self, now: f64) -> bool {
        let done = self.project.as_mut().is_some_and(|p| self.st.history.undo(p));
        if done {
            self.after_change(now);
        }
        done
    }

    /// ⇧⌘Z.
    pub fn redo(&mut self, now: f64) -> bool {
        let done = self.project.as_mut().is_some_and(|p| self.st.history.redo(p));
        if done {
            self.after_change(now);
        }
        done
    }

    /// The project changed: rebuild the timeline, keep the playhead on a kept time, give the worker
    /// the new project, make the next frame render again and start the autosave countdown.
    fn after_change(&mut self, now: f64) {
        let Some(p) = self.project.as_ref() else { return };
        let tl = Timeline::new(p);
        self.t = crate::editor_ctl::snap_kept(&tl, self.t.clamp(0.0, p.duration.max(0.0)), 1);
        self.timeline = Some(tl);
        self.play = None;
        self.st.edit_error = None;
        let cmd = WorkerCmd::Project(Box::new(p.clone()));
        if self.worker.as_ref().is_none_or(|w| w.send(cmd).is_err()) {
            self.error = Some("The preview renderer stopped.".into());
        }
        self.queue.invalidate();
        self.st.autosave.changed(now);
        self.dirty = true;
    }

    /// Write `project.json` (atomically) now; a failure keeps the project dirty and is shown in the footer.
    fn save_now(&mut self, now: f64) {
        let Some(p) = self.project.as_ref() else { return };
        match p.save(&self.dir) {
            Ok(()) => {
                self.st.autosave.written();
                self.dirty = false;
            }
            Err(e) => self.st.autosave.failed(now, format!("Could not save project.json: {e}")),
        }
    }

    /// Once per UI frame: write when the last change is one idle second old.
    pub fn autosave_tick(&mut self, now: f64) {
        if self.st.autosave.due(now) {
            self.save_now(now);
        }
    }

    /// Esc, closing, hiding, switching view: write what is owed right now.
    pub fn flush(&mut self, now: f64) {
        if self.st.autosave.pending() {
            self.save_now(now);
        }
    }

    /// `Saved`, `Saving…` or the error text.
    pub fn save_label(&self) -> String {
        match self.st.autosave.status() {
            SaveStatus::Saved => "Saved".into(),
            SaveStatus::Saving => "Saving…".into(),
            SaveStatus::Failed(e) => e,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Panels (drawing)

/// Width of the right-hand panel.
pub const PANEL_W: f32 = 252.0;

pub fn hex_to_rgb(hex: &str) -> Option<[u8; 3]> {
    if !hex_ok(hex) {
        return None;
    }
    let v = u32::from_str_radix(&hex[1..], 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn rgb_to_hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// `Auto 0:02.00–0:04.00 · 2.0×`.
pub fn zoom_label(z: &Zoom) -> String {
    use crate::editor_ctl::clock;
    let kind = if z.mode == "auto" { "Auto" } else { "Manual" };
    format!("{kind}  {}–{}  ·  {:.1}×", clock(z.start), clock(z.end), z.scale)
}

/// A labelled slider; returns the new value (in the unit of `v`) when it changed. `scale` shows the
/// value multiplied (0.06 as 6 %).
fn row_slider(ui: &mut Ui, label: &str, v: f64, range: std::ops::RangeInclusive<f64>, scale: f64, suffix: &str) -> Option<f64> {
    let mut shown = v * scale;
    let (lo, hi) = (range.start() * scale, range.end() * scale);
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 18.0], egui::Label::new(label));
        ui.spacing_mut().slider_width = 96.0;
        let r = ui.add(egui::Slider::new(&mut shown, lo..=hi).suffix(suffix).max_decimals(2));
        r.changed().then(|| shown / scale)
    })
    .inner
}

fn note(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).size(11.5).color(pal.sub));
}

/// The right-hand panel: tabs and the active tab's controls. Every change goes through [`Editor::apply`].
pub fn draw_panel(ui: &mut Ui, rect: Rect, pal: &Palette, ed: &mut Editor, auto: &ZoomOpts, fps: f64, now: f64) {
    ui.painter().rect_filled(rect, 8.0, pal.field);
    ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(8.0)), |ui| {
        ui.horizontal(|ui| {
            for t in Tab::ALL {
                if ui.selectable_label(ed.st.tab == t, t.label()).clicked() {
                    ed.st.tab = t;
                    ed.st.picking = false;
                }
            }
        });
        ui.separator();
        let Some(proj) = ed.project.clone() else { return };
        // the reason an edit was refused stays in view above the controls
        if let Some(e) = ed.st.edit_error.clone() {
            ui.label(egui::RichText::new(e).size(12.0).color(pal.danger));
        }
        let tab = ed.st.tab;
        egui::ScrollArea::vertical().id_salt("editor-panel").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            match tab {
                Tab::Zoom => zoom_tab(ui, pal, ed, &proj, auto, now),
                Tab::Cursor => cursor_tab(ui, pal, ed, &proj, now),
                Tab::Style => style_tab(ui, pal, ed, &proj, now),
                Tab::Clips => clips_tab(ui, pal, ed, &proj, fps, now),
            }
        });
    });
}

fn zoom_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, auto: &ZoomOpts, now: f64) {
    ui.horizontal(|ui| {
        if ui.button("Add at playhead").clicked() {
            let at = ed.t;
            if ed.apply(EditKind::Other, now, |p| add_zoom(p, at)) {
                ed.st.selected_zoom = ed.project.as_ref().and_then(|p| p.zooms.iter().find(|z| z.start == at)).map(|z| z.id.clone());
            }
        }
        if ui.button("Auto zoom").on_hover_text("Replace the automatic zooms; manual ones stay").clicked() {
            auto_zoom(ed, auto, now);
        }
    });
    if proj.zooms.is_empty() {
        note(ui, pal, "No zooms. Add one at the playhead or press Auto zoom.");
    }
    for z in &proj.zooms {
        let open = ed.st.selected_zoom.as_deref() == Some(z.id.as_str());
        if ui.selectable_label(open, zoom_label(z)).clicked() {
            ed.st.selected_zoom = (!open).then(|| z.id.clone());
            ed.st.picking = false;
        }
        if open {
            zoom_fields(ui, pal, ed, z, proj.duration, now);
        }
    }
}

/// `Auto zoom`: run the generator again with the settings and replace the automatic zooms.
fn auto_zoom(ed: &mut Editor, auto: &ZoomOpts, now: f64) {
    let Some(events) = ed.events.as_ref() else {
        ed.st.edit_error = Some("This recording has no cursor events, so there is nothing to zoom on.".into());
        return;
    };
    let fresh = zoom::auto_zooms(events, auto);
    if !ed.apply(EditKind::Other, now, |p| {
        replace_auto(p, fresh);
        Ok(())
    }) && ed.st.edit_error.is_none()
    {
        ed.st.edit_error = Some("Auto zoom found nothing new.".into());
    }
}

fn drag(ui: &mut Ui, label: &str, v: f64, range: std::ops::RangeInclusive<f64>, step: f64, suffix: &str) -> Option<f64> {
    let mut x = v;
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 18.0], egui::Label::new(label));
        let r = ui.add(egui::DragValue::new(&mut x).range(range).speed(step).suffix(suffix).max_decimals(2));
        r.changed().then_some(x)
    })
    .inner
}

fn zoom_fields(ui: &mut Ui, pal: &Palette, ed: &mut Editor, z: &Zoom, duration: f64, now: f64) {
    let id = z.id.clone();
    let edit = |ed: &mut Editor, kind: EditKind, field: ZoomField| {
        let id = id.clone();
        ed.apply(kind, now, move |p| set_zoom(p, &id, field));
    };
    if let Some(v) = drag(ui, "Start", z.start, 0.0..=duration, 0.05, " s") {
        edit(ed, EditKind::Zoom, ZoomField::Start(v));
    }
    if let Some(v) = drag(ui, "End", z.end, 0.0..=duration, 0.05, " s") {
        edit(ed, EditKind::Zoom, ZoomField::End(v));
    }
    if let Some(v) = drag(ui, "Scale", z.scale, 1.0..=MAX_SCALE, 0.02, "×") {
        edit(ed, EditKind::Zoom, ZoomField::Scale(v));
    }
    let [fx, fy] = z.focus.unwrap_or([0.5, 0.5]);
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 18.0], egui::Label::new("Focus"));
        let (mut x, mut y) = (fx * 100.0, fy * 100.0);
        let a = ui.add(egui::DragValue::new(&mut x).range(0.0..=100.0).suffix(" %").speed(0.5)).changed();
        let b = ui.add(egui::DragValue::new(&mut y).range(0.0..=100.0).suffix(" %").speed(0.5)).changed();
        if a || b {
            edit(ed, EditKind::Zoom, ZoomField::Focus(Some([x / 100.0, y / 100.0])));
        }
    });
    let follow = z.focus.is_none();
    if follow {
        note(ui, pal, "Following the cursor.");
    }
    ui.horizontal(|ui| {
        let pick = ed.st.picking;
        if ui.selectable_label(pick, "Pick on preview").on_hover_text("Click the preview to set the focus").clicked() {
            ed.st.picking = !pick;
        }
        if ui.add_enabled(!follow, egui::Button::new("Follow cursor")).clicked() {
            edit(ed, EditKind::Other, ZoomField::Focus(None));
        }
    });
    if let Some(v) = drag(ui, "Ease in", z.ease_in, 0.0..=3.0, 0.02, " s") {
        edit(ed, EditKind::Zoom, ZoomField::EaseIn(v));
    }
    if let Some(v) = drag(ui, "Ease out", z.ease_out, 0.0..=3.0, 0.02, " s") {
        edit(ed, EditKind::Zoom, ZoomField::EaseOut(v));
    }
    if ui.button("Delete zoom").clicked() {
        let id = z.id.clone();
        if ed.apply(EditKind::Other, now, move |p| delete_zoom(p, &id)) {
            ed.st.selected_zoom = None;
            ed.st.picking = false;
        }
    }
    ui.separator();
}

/// A click on the preview while picking: set the focus of the selected zoom. `click` is `0..=1` in the
/// preview image; `camera` is the crop the frame on screen shows.
pub fn pick_focus(ed: &mut Editor, click: [f64; 2], camera: Option<[f64; 4]>, now: f64) {
    let (Some(id), Some(style)) = (ed.st.selected_zoom.clone(), ed.project.as_ref().map(|p| p.style.clone())) else { return };
    let aspect = ed.video_aspect();
    match preview_to_focus(click, &style, aspect, camera) {
        Some(f) => {
            ed.st.picking = false;
            ed.apply(EditKind::Other, now, move |p| set_zoom(p, &id, ZoomField::Focus(Some(f))));
        }
        None => ed.st.edit_error = Some("Click inside the video, not on the background.".into()),
    }
}

fn cursor_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, now: f64) {
    let c = &proj.cursor;
    ui.horizontal(|ui| {
        for (id, label) in CURSOR_STYLES {
            if ui.selectable_label(c.style == id, label).clicked() {
                ed.apply(EditKind::Other, now, |p| set_cursor(p, CursorField::Style(id.into())));
            }
        }
    });
    if let Some(v) = row_slider(ui, "Size", c.size, 0.5..=4.0, 1.0, "×") {
        ed.apply(EditKind::Cursor, now, |p| set_cursor(p, CursorField::Size(v)));
    }
    if let Some(v) = row_slider(ui, "Smoothing", c.smoothing, 0.0..=1.0, 100.0, " %") {
        ed.apply(EditKind::Cursor, now, |p| set_cursor(p, CursorField::Smoothing(v)));
    }
    let mut ring = c.click_effect == "ring";
    if ui.checkbox(&mut ring, "Click ring").changed() {
        ed.apply(EditKind::Other, now, |p| set_cursor(p, CursorField::ClickRing(ring)));
    }
    if let Some(v) = row_slider(ui, "Motion blur", c.motion_blur, 0.0..=2.0, 100.0, " %") {
        ed.apply(EditKind::Cursor, now, |p| set_cursor(p, CursorField::MotionBlur(v)));
    }
    note(ui, pal, "The cursor is drawn from the recorded events; None and Hidden remove it.");
}

fn style_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, now: f64) {
    let s = &proj.style;
    ui.horizontal_wrapped(|ui| {
        ui.label("Aspect");
        for (id, label) in ASPECTS {
            if ui.selectable_label(s.aspect == id, label).clicked() {
                ed.apply(EditKind::Other, now, |p| set_style(p, StyleField::Aspect(id.into())));
            }
        }
    });
    background_rows(ui, pal, ed, s, now);
    if let Some(v) = row_slider(ui, "Padding", s.padding, 0.0..=0.45, 100.0, " %") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::Padding(v)));
    }
    if let Some(v) = row_slider(ui, "Corners", s.radius, 0.0..=64.0, 1.0, " px") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::Radius(v)));
    }
    if let Some(v) = row_slider(ui, "Shadow", s.shadow.blur, 0.0..=120.0, 1.0, " px") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::ShadowBlur(v)));
    }
    if let Some(v) = row_slider(ui, "Opacity", s.shadow.opacity, 0.0..=1.0, 100.0, " %") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::ShadowOpacity(v)));
    }
    if let Some(v) = row_slider(ui, "Offset", s.shadow.dy, 0.0..=80.0, 1.0, " px") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::ShadowDy(v)));
    }
}

fn background_rows(ui: &mut Ui, pal: &Palette, ed: &mut Editor, s: &Style, now: f64) {
    let bg = &s.background;
    ui.horizontal_wrapped(|ui| {
        ui.label("Background");
        for (id, label) in [("color", "Colour"), ("gradient", "Gradient"), ("blur", "Blur")] {
            if ui.selectable_label(bg.kind == id, label).clicked() {
                ed.apply(EditKind::Other, now, |p| set_style(p, StyleField::BgKind(id.into())));
            }
        }
    });
    let stops = match bg.kind.as_str() {
        "color" => 1,
        "gradient" => bg.stops.len().min(4),
        _ => 0,
    };
    ui.horizontal(|ui| {
        for (i, hex) in bg.stops.iter().take(stops).enumerate() {
            let mut rgb = hex_to_rgb(hex).unwrap_or([0, 0, 0]);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::BgStop(i, rgb_to_hex(rgb))));
            }
        }
    });
    if bg.kind == "gradient" {
        if let Some(v) = row_slider(ui, "Angle", bg.angle, 0.0..=360.0, 1.0, "°") {
            ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::BgAngle(v)));
        }
        ui.horizontal_wrapped(|ui| {
            for (i, (name, _, _)) in WALLPAPERS.iter().enumerate() {
                if ui.small_button(*name).clicked() {
                    ed.apply(EditKind::Other, now, |p| set_style(p, StyleField::Wallpaper(i)));
                }
            }
        });
    } else if bg.kind == "blur" {
        note(ui, pal, "The recording itself, blurred.");
    }
}

fn clips_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, fps: f64, now: f64) {
    use crate::editor_ctl::clock;
    let t = ed.t;
    ui.horizontal_wrapped(|ui| {
        if ui.button("Trim in").on_hover_text("Cut everything before the playhead").clicked() {
            ed.apply(EditKind::Other, now, |p| trim_in(p, t));
        }
        if ui.button("Trim out").on_hover_text("Cut everything after the playhead").clicked() {
            ed.apply(EditKind::Other, now, |p| trim_out(p, t));
        }
        if ui.button("Split").on_hover_text("Split the segment at the playhead").clicked() {
            ed.apply(EditKind::Split, now, |p| split_at(p, t));
        }
        if ui.button("Delete segment").clicked() {
            ed.apply(EditKind::Other, now, |p| delete_segment(p, t));
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Speed");
        for f in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0] {
            if ui.selectable_label((ed.st.speed - f).abs() < 1e-9, format!("{f}×")).clicked() {
                ed.st.speed = f;
            }
        }
    });
    ui.horizontal(|ui| {
        let mut f = ed.st.speed;
        if ui
            .add(egui::DragValue::new(&mut f).range(ferrocast_core::videoplan::SPEED_MIN..=timeline::SPEED_MAX).speed(0.05).suffix("×"))
            .changed()
        {
            ed.st.speed = f;
        }
        if ui.button("Apply to segment").clicked() {
            let f = ed.st.speed;
            ed.apply(EditKind::Other, now, |p| set_segment_speed(p, t, f));
        }
    });
    ui.separator();
    let Some(tl) = ed.timeline.clone() else { return };
    note(ui, pal, "Segments (click to move the playhead)");
    for (i, b) in crate::editor_ctl::segment_bars(&tl).iter().enumerate() {
        let here = ed.t >= b.start && ed.t < b.end;
        let speed = match b.kind {
            crate::editor_ctl::BarKind::Speed(f) => format!("  ·  {f}×"),
            _ => String::new(),
        };
        if ui.selectable_label(here, format!("{}  {}–{}{speed}", i + 1, clock(b.start), clock(b.end))).clicked() {
            ed.seek(b.start, now, fps);
        }
    }
    note(ui, pal, &format!("{} → {} after the edits", clock(proj.duration), clock(tl.output_duration())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_ctl::{Opened, Req};
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{channel, Receiver};

    fn base() -> Project {
        let mut p = Project::new("t", "2026-01-01T00:00:00Z");
        p.duration = 10.0;
        p
    }

    fn zoom(id: &str, start: f64, end: f64, mode: &str) -> Zoom {
        serde_json::from_value(serde_json::json!({"id":id,"start":start,"end":end,"mode":mode,"scale":2.0})).unwrap()
    }

    // ---- AutosaveClock

    #[test]
    fn nothing_is_written_before_one_idle_second_or_when_clean() {
        let mut c = AutosaveClock::new(1.0);
        assert!(!c.due(1000.0) && !c.pending() && c.wait(0.0).is_none(), "a clean project never writes");
        assert_eq!(c.status(), SaveStatus::Saved);
        c.changed(10.0);
        assert_eq!(c.status(), SaveStatus::Saving);
        assert!(!c.due(10.0) && !c.due(10.99), "no write before 1 s of quiet");
        assert!(c.due(11.0));
        assert!((c.wait(10.25).unwrap() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn a_burst_of_changes_is_one_write_after_the_last_one() {
        let mut c = AutosaveClock::new(1.0);
        for t in [0.0, 0.4, 0.8, 1.5, 2.2] {
            c.changed(t);
            assert!(!c.due(t), "never while the user keeps editing");
        }
        assert!(!c.due(3.19) && c.due(3.2), "1 s after the LAST change");
        c.written();
        assert!(!c.due(99.0) && c.status() == SaveStatus::Saved, "one write per burst");
    }

    #[test]
    fn a_failed_write_stays_dirty_shows_its_text_and_retries_later() {
        let mut c = AutosaveClock::new(1.0);
        c.changed(0.0);
        c.failed(1.0, "disk full".into());
        assert_eq!(c.status(), SaveStatus::Failed("disk full".into()));
        assert!(c.pending(), "still owed");
        assert!(!c.due(1.5), "no retry flood: one more idle second");
        assert!(c.due(2.0));
        c.written();
        assert_eq!(c.status(), SaveStatus::Saved);
        c.failed(5.0, "x".into());
        c.changed(6.0);
        assert_eq!(c.status(), SaveStatus::Saving, "a new change clears the old error");
    }

    // ---- zoom edits

    #[test]
    fn a_manual_zoom_fits_the_gap_and_gets_a_fresh_id() {
        let mut p = base();
        p.zooms = vec![zoom("m1", 5.0, 6.0, "manual")];
        add_zoom(&mut p, 4.0).unwrap();
        let z = p.zooms.iter().find(|z| z.id == "m2").unwrap();
        assert_eq!(
            (z.start, z.end, z.mode.as_str(), z.scale, z.focus),
            (4.0, 5.0, "manual", 2.0, Some([0.5, 0.5])),
            "cut short by the next zoom"
        );
        assert_eq!(add_zoom(&mut p, 5.5).unwrap_err(), "There is already a zoom at the playhead.");
        assert!(add_zoom(&mut p, 9.95).unwrap_err().contains("no room"), "a sliver is refused");
        let mut q = base();
        add_zoom(&mut q, 3.0).unwrap();
        assert_eq!((q.zooms[0].start, q.zooms[0].end), (3.0, 5.0), "NEW_ZOOM_LEN");
    }

    #[test]
    fn zoom_fields_are_clamped_and_editing_makes_a_zoom_manual() {
        let mut p = base();
        p.zooms = vec![zoom("a", 2.0, 4.0, "auto")];
        set_zoom(&mut p, "a", ZoomField::Scale(9.0)).unwrap();
        assert_eq!((p.zooms[0].scale, p.zooms[0].mode.as_str()), (6.0, "manual"));
        set_zoom(&mut p, "a", ZoomField::Scale(0.1)).unwrap();
        assert_eq!(p.zooms[0].scale, 1.0);
        set_zoom(&mut p, "a", ZoomField::Start(99.0)).unwrap();
        assert!((p.zooms[0].start - (4.0 - MIN_ZOOM)).abs() < 1e-9, "start stops before end");
        set_zoom(&mut p, "a", ZoomField::End(99.0)).unwrap();
        assert_eq!(p.zooms[0].end, 10.0, "end stops at the recording end");
        set_zoom(&mut p, "a", ZoomField::Focus(Some([-1.0, 3.0]))).unwrap();
        assert_eq!(p.zooms[0].focus, Some([0.0, 1.0]));
        set_zoom(&mut p, "a", ZoomField::Focus(None)).unwrap();
        assert_eq!(p.zooms[0].focus, None, "follow the cursor");
        assert!(set_zoom(&mut p, "a", ZoomField::Scale(f64::NAN)).is_err());
        assert!(set_zoom(&mut p, "zz", ZoomField::Scale(2.0)).is_err());
        // an auto zoom set to what it already is stays auto
        let mut q = base();
        q.zooms = vec![zoom("a", 2.0, 4.0, "auto")];
        set_zoom(&mut q, "a", ZoomField::Scale(2.0)).unwrap();
        assert_eq!(q.zooms[0].mode, "auto");
        delete_zoom(&mut q, "a").unwrap();
        assert!(q.zooms.is_empty() && delete_zoom(&mut q, "a").is_err());
    }

    #[test]
    fn auto_zoom_replaces_auto_zooms_and_keeps_manual_ones() {
        let mut p = base();
        p.zooms = vec![zoom("a1", 1.0, 2.0, "auto"), zoom("m1", 4.0, 6.0, "manual")];
        replace_auto(&mut p, vec![zoom("n1", 0.5, 1.5, "auto"), zoom("n2", 5.0, 7.0, "auto"), zoom("n3", 8.0, 9.0, "auto")]);
        let ids: Vec<&str> = p.zooms.iter().map(|z| z.id.as_str()).collect();
        assert_eq!(ids, ["n1", "m1", "n3"], "old auto gone, manual kept, the overlapping new one dropped");
        assert!(p.validate(|_| true).iter().all(|m| !m.contains("overlap")));
    }

    // ---- cursor and style

    #[test]
    fn cursor_values_are_clamped_and_styles_are_the_ones_compose_knows() {
        let mut p = base();
        set_cursor(&mut p, CursorField::Size(99.0)).unwrap();
        set_cursor(&mut p, CursorField::Smoothing(-1.0)).unwrap();
        set_cursor(&mut p, CursorField::MotionBlur(5.0)).unwrap();
        assert_eq!((p.cursor.size, p.cursor.smoothing, p.cursor.motion_blur), (4.0, 0.0, 2.0));
        set_cursor(&mut p, CursorField::ClickRing(false)).unwrap();
        assert_eq!(p.cursor.click_effect, "none");
        set_cursor(&mut p, CursorField::ClickRing(true)).unwrap();
        assert_eq!(p.cursor.click_effect, "ring");
        for s in ["none", "hidden", "arrow"] {
            set_cursor(&mut p, CursorField::Style(s.into())).unwrap();
            assert_eq!(p.cursor.style, s);
        }
        assert!(set_cursor(&mut p, CursorField::Style("laser".into())).is_err());
    }

    #[test]
    fn style_values_are_clamped_colours_checked_and_wallpapers_are_valid_gradients() {
        let mut p = base();
        set_style(&mut p, StyleField::Padding(0.9)).unwrap();
        set_style(&mut p, StyleField::Radius(-4.0)).unwrap();
        set_style(&mut p, StyleField::ShadowOpacity(7.0)).unwrap();
        assert_eq!((p.style.padding, p.style.radius, p.style.shadow.opacity), (0.45, 0.0, 1.0));
        assert!(set_style(&mut p, StyleField::Aspect("5:4".into())).is_err());
        set_style(&mut p, StyleField::Aspect("9:16".into())).unwrap();
        assert!(set_style(&mut p, StyleField::BgStop(0, "red".into())).is_err(), "only #rrggbb");
        set_style(&mut p, StyleField::BgStop(0, "#AABBCC".into())).unwrap();
        assert_eq!(p.style.background.stops[0], "#aabbcc");
        assert!(set_style(&mut p, StyleField::BgStop(9, "#000000".into())).is_err());
        set_style(&mut p, StyleField::BgKind("color".into())).unwrap();
        set_style(&mut p, StyleField::BgKind("blur".into())).unwrap();
        assert!(set_style(&mut p, StyleField::BgKind("video".into())).is_err());
        for i in 0..WALLPAPERS.len() {
            set_style(&mut p, StyleField::Wallpaper(i)).unwrap();
            assert!(p.validate(|_| true).is_empty(), "wallpaper {i}: {:?}", p.validate(|_| true));
            assert_eq!(p.style.background.kind, "gradient");
        }
        let mut one = base();
        one.style.background.stops = vec!["#112233".into()];
        set_style(&mut one, StyleField::BgKind("gradient".into())).unwrap();
        assert_eq!(one.style.background.stops.len(), 2, "a gradient has two colours");
    }

    // ---- clips

    #[test]
    fn trim_split_delete_and_speed_work_on_the_segment_under_the_playhead() {
        let mut p = base();
        trim_in(&mut p, 2.0).unwrap();
        trim_out(&mut p, 8.0).unwrap();
        assert_eq!(p.trims, vec![Trim { start: 2.0, end: 8.0 }]);
        assert!(trim_in(&mut p, 1.0).is_err(), "nothing before 1 s is left to cut");
        split_at(&mut p, 5.0).unwrap();
        assert_eq!(p.trims, vec![Trim { start: 2.0, end: 5.0 }, Trim { start: 5.0, end: 8.0 }]);
        assert!(split_at(&mut p, 5.0).is_err(), "not at a boundary");
        assert!(split_at(&mut p, 9.0).is_err(), "not in a cut");
        set_segment_speed(&mut p, 6.0, 4.0).unwrap();
        assert_eq!((p.speeds[0].start, p.speeds[0].end, p.speeds[0].factor), (5.0, 8.0, 4.0));
        assert!(set_segment_speed(&mut p, 6.0, 32.0).unwrap_err().contains("between"));
        assert!(set_segment_speed(&mut p, 6.0, 0.1).is_err());
        set_segment_speed(&mut p, 6.0, 1.0).unwrap();
        assert!(p.speeds.is_empty(), "1x clears the speed");
        delete_segment(&mut p, 3.0).unwrap();
        assert_eq!(p.trims, vec![Trim { start: 5.0, end: 8.0 }]);
        assert!(delete_segment(&mut p, 6.0).is_err(), "the only segment stays");
        assert!(delete_segment(&mut p, 1.0).is_err(), "a cut is not a segment");
        assert!(problems(&p).is_empty(), "{:?}", problems(&p));
    }

    // ---- focus picking

    #[test]
    fn a_click_on_the_preview_maps_to_the_video_through_padding_and_camera() {
        let mut st = Style { padding: 0.0, ..Style::default() };
        let c = preview_to_focus([0.25, 0.5], &st, 16.0 / 9.0, None).unwrap();
        assert!((c[0] - 0.25).abs() < 1e-9 && (c[1] - 0.5).abs() < 1e-9, "no padding: identity {c:?}");
        st.padding = 0.1;
        let c = preview_to_focus([0.5, 0.5], &st, 16.0 / 9.0, None).unwrap();
        assert!((c[0] - 0.5).abs() < 1e-9 && (c[1] - 0.5).abs() < 1e-9, "the centre stays the centre");
        assert!(preview_to_focus([0.02, 0.5], &st, 16.0 / 9.0, None).is_none(), "the padding is not the video");
        // the 16:9 canvas has 10 % padding on every side: the height limits, so the video starts at x = 0.1783
        let left = (1.0 - (1.0 / (16.0 / 9.0) - 0.2) * 16.0 / 9.0) / 2.0;
        assert!(preview_to_focus([left - 1e-3, 0.5], &st, 16.0 / 9.0, None).is_none());
        let edge = preview_to_focus([left + 1e-6, 0.5], &st, 16.0 / 9.0, None).unwrap();
        assert!(edge[0] < 0.001, "{edge:?}");
        // zoomed frame: the preview shows the top-left quarter
        st.padding = 0.0;
        let c = preview_to_focus([0.5, 0.5], &st, 16.0 / 9.0, Some([0.0, 0.0, 0.5, 0.5])).unwrap();
        assert!((c[0] - 0.25).abs() < 1e-9 && (c[1] - 0.25).abs() < 1e-9, "{c:?}");
        // a 1:1 canvas around a 16:9 video letterboxes it vertically
        st.aspect = "1:1".into();
        assert!(preview_to_focus([0.5, 0.05], &st, 16.0 / 9.0, None).is_none());
        assert!(preview_to_focus([0.5, 0.5], &st, 16.0 / 9.0, None).is_some());
    }

    // ---- the editor state: history, validation, worker, autosave

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fc-panels-{}-{tag}", std::process::id())).join("Take.ferrocast");
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn editor(dir: &Path, auto_zoomed: bool) -> (Editor, Receiver<WorkerCmd>) {
        let (tx, rx) = channel();
        let mut ed = Editor::new(dir.to_path_buf(), Some(tx));
        let mut p = base();
        p.zooms = vec![zoom("a", 1.0, 2.0, "auto")];
        p.save(dir).unwrap();
        ed.opened(Ok(Opened { project: p, auto_zoomed, note: None, events: None }));
        (ed, rx)
    }

    fn disk(dir: &Path) -> Project {
        Project::load(dir).unwrap()
    }

    fn project_cmds(rx: &Receiver<WorkerCmd>) -> Vec<Project> {
        rx.try_iter().filter_map(|c| if let WorkerCmd::Project(p) = c { Some(*p) } else { None }).collect()
    }

    #[test]
    fn every_kind_of_edit_undoes_and_redoes_to_identical_json() {
        let (mut ed, _rx) = editor(&temp("undo"), false);
        let start = ed.project.as_ref().unwrap().to_json();
        let mut now = 0.0;
        let mut step = |ed: &mut Editor, kind, f: &dyn Fn(&mut Project) -> Result<(), String>| {
            now += 10.0; // far apart: no coalescing
            assert!(ed.apply(kind, now, f), "{:?}", ed.st.edit_error);
        };
        step(&mut ed, EditKind::Zoom, &|p| add_zoom(p, 5.0));
        step(&mut ed, EditKind::Zoom, &|p| set_zoom(p, "a", ZoomField::Scale(3.0)));
        step(&mut ed, EditKind::Cursor, &|p| set_cursor(p, CursorField::Size(2.0)));
        step(&mut ed, EditKind::Style, &|p| set_style(p, StyleField::Padding(0.2)));
        step(&mut ed, EditKind::Trim, &|p| trim_in(p, 0.5));
        step(&mut ed, EditKind::Split, &|p| split_at(p, 4.0));
        step(&mut ed, EditKind::Speed, &|p| set_segment_speed(p, 6.0, 2.0));
        step(&mut ed, EditKind::Zoom, &|p| delete_zoom(p, "a"));
        let end = ed.project.as_ref().unwrap().to_json();
        let mut n = 0;
        while ed.undo(100.0) {
            n += 1;
        }
        assert_eq!(n, 8, "one undo step per edit");
        assert_eq!(ed.project.as_ref().unwrap().to_json(), start, "all the way back");
        assert!(!ed.st.history.can_undo() && ed.st.history.can_redo());
        while ed.redo(101.0) {}
        assert_eq!(ed.project.as_ref().unwrap().to_json(), end, "and forward again");
        assert_eq!(ed.timeline.as_ref().unwrap().pieces().len(), 2, "the timeline follows the project");
    }

    #[test]
    fn dragging_a_slider_is_one_undo_step() {
        let (mut ed, _rx) = editor(&temp("drag"), false);
        let start = ed.project.as_ref().unwrap().to_json();
        for i in 1..=30 {
            assert!(ed.apply(EditKind::Style, 5.0 + f64::from(i) * 0.016, |p| set_style(p, StyleField::Padding(f64::from(i) / 100.0))));
        }
        assert_eq!(ed.st.history.undo_len(), 1, "30 drag edits inside the window merge");
        assert!(ed.undo(6.0));
        assert_eq!(ed.project.as_ref().unwrap().to_json(), start);
        // a pause longer than the window starts a new step
        ed.apply(EditKind::Style, 10.0, |p| set_style(p, StyleField::Padding(0.1)));
        ed.apply(EditKind::Style, 12.0, |p| set_style(p, StyleField::Padding(0.2)));
        assert_eq!(ed.st.history.undo_len(), 2);
    }

    #[test]
    fn an_invalid_edit_is_rejected_with_its_message_and_nothing_changes() {
        let dir = temp("invalid");
        let (mut ed, rx) = editor(&dir, false);
        let json = ed.project.as_ref().unwrap().to_json();
        project_cmds(&rx);
        // a zoom on top of zoom `a` (1..2)
        let ok = ed.apply(EditKind::Zoom, 1.0, |p| {
            p.zooms.push(zoom("b", 1.5, 3.0, "manual"));
            Ok(())
        });
        assert!(!ok);
        assert!(ed.st.edit_error.as_deref().is_some_and(|e| e.contains("overlap")), "{:?}", ed.st.edit_error);
        assert_eq!(ed.project.as_ref().unwrap().to_json(), json, "not applied");
        assert!(!ed.st.history.can_undo() && !ed.st.autosave.pending(), "nothing recorded, nothing to save");
        assert!(project_cmds(&rx).is_empty(), "the worker never saw it");
        // a refused edit function reports its own text
        assert!(!ed.apply(EditKind::Trim, 2.0, |p| trim_in(p, 0.0)));
        assert!(ed.st.edit_error.as_deref().is_some_and(|e| e.contains("nothing to trim")));
        // a later good edit clears the message
        assert!(ed.apply(EditKind::Cursor, 3.0, |p| set_cursor(p, CursorField::Size(2.0))));
        assert!(ed.st.edit_error.is_none());
        // a project that was already invalid does not block unrelated edits
        let (mut bad, _rx) = editor(&temp("invalid2"), false);
        bad.project.as_mut().unwrap().export.fps = 25;
        assert!(bad.apply(EditKind::Style, 1.0, |p| set_style(p, StyleField::Radius(4.0))), "{:?}", bad.st.edit_error);
    }

    #[test]
    fn an_edit_reaches_the_worker_and_the_same_frame_is_rendered_again() {
        let (mut ed, rx) = editor(&temp("render"), false);
        ed.want(960);
        let first = rx.try_iter().find_map(|c| if let WorkerCmd::Frame(r) = c { Some(r) } else { None }).unwrap();
        let fake = |t: f64| crate::editor_ctl::FrameOut { image: egui::ColorImage::example(), t, helper_ms: 1.0, total_ms: 1.0 };
        assert!(ed.frame_back(first.seq, Ok(fake(0.0))).is_some());
        ed.want(960);
        assert!(rx.try_iter().next().is_none(), "the same frame is not asked for twice");
        assert!(ed.apply(EditKind::Style, 1.0, |p| set_style(p, StyleField::Padding(0.3))));
        let cmds: Vec<WorkerCmd> = rx.try_iter().collect();
        assert!(matches!(&cmds[0], WorkerCmd::Project(p) if p.style.padding == 0.3), "the edited project first");
        ed.want(960);
        let again: Vec<Req> = rx.try_iter().filter_map(|c| if let WorkerCmd::Frame(r) = c { Some(r) } else { None }).collect();
        assert_eq!(again.len(), 1, "the frame at the playhead is rendered again after an edit");
        // an edit while a render is in flight: the stale reply is followed by a fresh request
        assert!(ed.apply(EditKind::Style, 3.0, |p| set_style(p, StyleField::Padding(0.4))));
        assert!(rx.try_iter().all(|c| matches!(c, WorkerCmd::Project(_))), "coalesced: nothing new while in flight");
        ed.frame_back(again[0].seq, Ok(fake(0.0)));
        let next: Vec<Req> = rx.try_iter().filter_map(|c| if let WorkerCmd::Frame(r) = c { Some(r) } else { None }).collect();
        assert_eq!(next.len(), 1, "the frame that predates the edit is replaced");
    }

    #[test]
    fn autosave_writes_one_idle_second_after_the_last_change_and_only_then() {
        let dir = temp("autosave");
        let (mut ed, _rx) = editor(&dir, false);
        let before = std::fs::read(dir.join("project.json")).unwrap();
        ed.apply(EditKind::Style, 100.0, |p| set_style(p, StyleField::Radius(30.0)));
        assert_eq!(ed.save_label(), "Saving…");
        ed.autosave_tick(100.5);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), before, "no write before 1 s idle");
        ed.apply(EditKind::Style, 100.9, |p| set_style(p, StyleField::Radius(31.0)));
        ed.autosave_tick(101.5);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), before, "the second change moved the deadline");
        ed.autosave_tick(101.9);
        assert_eq!(disk(&dir).style.radius, 31.0, "written 1 s after the LAST change");
        assert!(disk(&dir).validate(|_| true).is_empty());
        assert_eq!(ed.save_label(), "Saved");
        assert!(!dir.join("project.json.tmp").exists(), "atomic: no temp file left");
        // one write per burst: a sentinel survives later ticks
        std::fs::write(dir.join("project.json"), b"sentinel").unwrap();
        ed.autosave_tick(110.0);
        ed.autosave_tick(120.0);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), b"sentinel");
    }

    #[test]
    fn opening_a_project_with_auto_zoom_writes_nothing_until_the_user_changes_something() {
        let dir = temp("openonly");
        let (mut ed, _rx) = editor(&dir, true);
        base().save(&dir).unwrap(); // on disk: the recording as it was made, without zooms
        let before = std::fs::read(dir.join("project.json")).unwrap();
        assert!(ed.dirty && !ed.project.as_ref().unwrap().zooms.is_empty(), "the auto zooms are in memory");
        for t in [1.0, 5.0, 60.0] {
            ed.autosave_tick(t);
        }
        ed.flush(61.0);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), before, "a project the user only opened is not rewritten");
        assert_eq!(ed.save_label(), "Saved");
        ed.apply(EditKind::Cursor, 70.0, |p| set_cursor(p, CursorField::Size(2.0)));
        ed.autosave_tick(71.0);
        let saved = disk(&dir);
        assert_eq!(saved.cursor.size, 2.0);
        assert_eq!(saved.zooms.len(), 1, "after the first change the auto zooms are persisted too");
        assert!(!ed.dirty);
    }

    #[test]
    fn a_failed_write_keeps_the_project_dirty_with_the_error_and_recovers() {
        let dir = temp("failwrite");
        let (mut ed, _rx) = editor(&dir, false);
        // the folder vanishes (an external drive was unplugged)
        let moved = dir.with_file_name("Gone.ferrocast");
        std::fs::rename(&dir, &moved).unwrap();
        ed.apply(EditKind::Cursor, 10.0, |p| set_cursor(p, CursorField::Size(3.0)));
        ed.autosave_tick(11.0);
        assert!(ed.save_label().starts_with("Could not save project.json"), "{}", ed.save_label());
        assert!(ed.dirty && ed.st.autosave.pending());
        ed.autosave_tick(11.5);
        assert!(ed.save_label().starts_with("Could not save"), "no flood: still the same error");
        std::fs::rename(&moved, &dir).unwrap();
        ed.autosave_tick(12.0);
        assert_eq!(ed.save_label(), "Saved");
        assert_eq!(disk(&dir).cursor.size, 3.0);
    }

    #[test]
    fn flush_writes_at_once_and_is_a_no_op_when_clean() {
        let dir = temp("flush");
        let (mut ed, _rx) = editor(&dir, false);
        ed.apply(EditKind::Style, 1.0, |p| set_style(p, StyleField::Radius(9.0)));
        ed.flush(1.1);
        assert_eq!(disk(&dir).style.radius, 9.0, "Esc does not wait for the second");
        std::fs::write(dir.join("project.json"), b"sentinel").unwrap();
        ed.flush(2.0);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), b"sentinel");
    }

    #[test]
    fn undo_is_saved_too_and_edits_stay_valid_for_the_core_decoder() {
        let dir = temp("undosave");
        let (mut ed, _rx) = editor(&dir, false);
        ed.apply(EditKind::Style, 1.0, |p| set_style(p, StyleField::Radius(9.0)));
        ed.flush(2.0);
        assert!(ed.undo(3.0));
        ed.autosave_tick(4.0);
        assert_eq!(disk(&dir).style.radius, Style::default().radius);
        assert!(!ed.undo(5.0), "nothing more to undo");
    }

    /// End to end against the REAL helper on the synthetic fixture (a copy under `target/capture-spike/t27-e2e`):
    /// a style edit changes the next rendered frame, `project.json` changes on disk only after the debounce,
    /// still decodes and validates, and Undo brings the first frame back exactly.
    /// Run: `cargo test -p ferrocast --locked live_helper_style -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs target/capture-spike/ferrocast-capture and the editor fixture (scripts/capture-smoke.sh --keep)"]
    fn live_helper_style_edit_changes_the_next_frame_and_autosaves() {
        use crate::editor_ctl::{helper_renderer, EditorMsg, FrameOut};
        use std::time::Duration;
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/capture-spike");
        let fixture = root.join("editor-fixture/Take.ferrocast");
        if !root.join("ferrocast-capture").is_file() || !fixture.join("screen.mov").is_file() {
            eprintln!("SKIP: target/capture-spike/ferrocast-capture or the editor fixture is missing");
            return;
        }
        let dir = root.join("t27-e2e/Take.ferrocast");
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["project.json", "events.json", "screen.mov"] {
            std::fs::copy(fixture.join(f), dir.join(f)).unwrap();
        }
        let on_disk = || std::fs::read(dir.join("project.json")).unwrap();
        let original = on_disk();
        let (mtx, mrx) = channel();
        let mtx = std::sync::Mutex::new(mtx);
        let tx = crate::editor_ctl::spawn_worker(
            dir.clone(),
            Some(ZoomOpts::default()),
            helper_renderer(root.join("ferrocast"), |l| eprintln!("helper: {l}")),
            Box::new(move |m| {
                let _ = mtx.lock().unwrap().send(m);
            }),
        );
        let wait = || mrx.recv_timeout(Duration::from_secs(30)).expect("the worker answers");
        let opened = match wait() {
            EditorMsg::Opened { result, .. } => *result.expect("the fixture opens"),
            _ => panic!("Opened comes first"),
        };
        assert!(opened.auto_zoomed && opened.events.is_some());
        let mut ed = Editor::new(dir.clone(), Some(tx));
        ed.opened(Ok(opened));
        let frame = |ed: &mut Editor, t: f64| -> FrameOut {
            ed.seek(t, 0.0, 60.0);
            ed.want(960);
            loop {
                if let EditorMsg::Frame { seq, result, .. } = wait() {
                    let out = result.expect("frame renders");
                    let copy = FrameOut { image: out.image.clone(), ..out };
                    ed.frame_back(seq, Ok(out));
                    return copy;
                }
            }
        };
        let cold = frame(&mut ed, 0.5);
        let differing = |a: &FrameOut, b: &FrameOut| a.image.pixels.iter().zip(&b.image.pixels).filter(|(x, y)| x != y).count();
        eprintln!("frame 1 (before the edit): {:?} px, helper {:.0} ms", cold.image.size, cold.helper_ms);

        // the edit: Dusk wallpaper + 20 % padding (two edits of different kinds: two undo steps)
        let t0 = ed.now();
        assert!(ed.apply(EditKind::Other, t0, |p| set_style(p, StyleField::Wallpaper(1))));
        assert!(ed.apply(EditKind::Style, t0 + 0.1, |p| set_style(p, StyleField::Padding(0.2))));
        let edited = frame(&mut ed, 0.5);
        let n = differing(&cold, &edited);
        eprintln!(
            "frame 2 (after the edit): {:?} px, helper {:.0} ms, {n} of {} pixels differ",
            edited.image.size,
            edited.helper_ms,
            cold.image.pixels.len()
        );
        assert!(n > cold.image.pixels.len() / 10, "the style edit must change the rendered frame: {n} pixels differ");
        assert_eq!(on_disk(), original, "nothing is written yet: the debounce has not elapsed");
        eprintln!("project.json right after the edit: unchanged ({} bytes)", original.len());

        // the debounce: nothing at +0.5 s, written after one idle second
        std::thread::sleep(Duration::from_millis(500));
        ed.autosave_tick(ed.now());
        assert_eq!(on_disk(), original, "still inside the idle second");
        std::thread::sleep(Duration::from_millis(700));
        ed.autosave_tick(ed.now());
        let saved = on_disk();
        assert_ne!(saved, original, "project.json changed after the debounce");
        let p = Project::from_json(&String::from_utf8(saved.clone()).unwrap()).expect("still decodes with the core model");
        let problems = p.validate(|f| dir.join(f).is_file());
        eprintln!(
            "project.json after the debounce: {} -> {} bytes; background {:?} angle {}, padding {}, zooms {}; validate() = {problems:?}; timeline::validate() = {:?}",
            original.len(),
            saved.len(),
            p.style.background.stops,
            p.style.background.angle,
            p.style.padding,
            p.zooms.len(),
            timeline::validate(&p)
        );
        assert!(problems.is_empty() && timeline::validate(&p).is_empty());
        assert_eq!((p.style.background.stops[0].as_str(), p.style.padding), ("#2b1055", 0.2));
        assert!(!p.zooms.is_empty(), "the auto zooms are persisted together with the user's first change");
        assert!(!dir.join("project.json.tmp").exists(), "atomic write left no temp file");
        assert_eq!(ed.save_label(), "Saved");

        // undo both edits: the first frame comes back exactly
        let t1 = ed.now();
        assert!(ed.undo(t1) && ed.undo(t1 + 0.1));
        let back = frame(&mut ed, 0.5);
        eprintln!("frame 3 (after Undo x2): {} of {} pixels differ from frame 1", differing(&cold, &back), cold.image.pixels.len());
        assert_eq!(cold.image.pixels, back.image.pixels, "undo restores the first frame");
        assert!(ed.st.autosave.pending(), "the undo is owed to disk");
        ed.flush(ed.now());
        assert_eq!(Project::load(&dir).unwrap().style, Project::from_json(&String::from_utf8(original).unwrap()).unwrap().style);
        eprintln!("after Undo x2 + flush: project.json style equals the original again");
    }
}
