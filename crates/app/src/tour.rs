//! First-run tour: a seven-slide deck drawn straight onto the 750×475 window.
//!
//! One claim per slide, large type, whole fields of colour, real screenshots and keycaps for
//! shortcuts. The deck state lives in [`Tour`] (kept in `App`); the drawing is a set of small
//! painter helpers on [`Scene`] so every slide reads as a layout, not as widgets.

use super::*;
use egui::epaint::{CornerRadius, RectShape};
use egui::{Color32, FontFamily, TextureHandle, TextureId, TextureOptions};
use std::cell::RefCell;

pub(super) const SLIDES: usize = 7;
pub(super) const PERMS_SLIDE: usize = 5;
const LAST: usize = SLIDES - 1;

/// Time the old slide takes to leave and the new one to arrive.
const TRANSITION: f64 = 0.38;
/// Staggered reveal of the elements of the slide that just arrived.
const REVEAL_DELAY: f64 = 0.10;
const REVEAL_STEP: f64 = 0.07;
const REVEAL_TIME: f64 = 0.45;
/// How often the permission status is read while its slide is on screen.
const POLL_EVERY: f64 = 0.5;

const GREEN: Color32 = Color32::from_rgb(89, 212, 153);
const AMBER: Color32 = Color32::from_rgb(255, 184, 77);

/// What the permissions slide knows. Both values come from silent, read-only checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Perms {
    pub accessibility: bool,
    pub input_monitoring: bool,
}

impl Perms {
    fn poll() -> Self {
        Perms { accessibility: windows::accessibility_trusted(false), input_monitoring: windows::input_monitoring_granted() }
    }
}

/// Opens the tour on the first visible launch, once. `--hidden` neither opens it nor uses it up.
pub(super) fn first_run(settings: &mut Settings, hidden: bool) -> bool {
    if settings.onboarded || hidden {
        return false;
    }
    settings.onboarded = true;
    true
}

pub(super) struct Tour {
    pub slide: usize,
    from: usize,
    /// Egui time of the last slide change; stamped by the first frame that draws it.
    changed: Option<f64>,
    /// Test hook: pretend these permission values were read.
    pub injected: Option<Perms>,
    perms: Perms,
    polled: f64,
    reduce: Option<bool>,
    shots: [Option<TextureHandle>; 3],
}

impl Tour {
    pub(super) fn new() -> Self {
        Tour {
            slide: 0,
            from: 0,
            changed: None,
            injected: None,
            perms: Perms { accessibility: false, input_monitoring: false },
            polled: f64::NEG_INFINITY,
            reduce: None,
            shots: [None, None, None],
        }
    }

    /// Back to slide 0 with the entrance animation pending.
    pub(super) fn reset(&mut self) {
        self.slide = 0;
        self.from = 0;
        self.changed = None;
        self.polled = f64::NEG_INFINITY;
    }

    pub(super) fn go(&mut self, to: usize) -> bool {
        if to >= SLIDES || to == self.slide {
            return false;
        }
        self.from = self.slide;
        self.slide = to;
        self.changed = None;
        true
    }

    pub(super) fn next(&mut self) -> bool {
        self.slide < LAST && self.go(self.slide + 1)
    }

    pub(super) fn prev(&mut self) -> bool {
        self.slide > 0 && self.go(self.slide - 1)
    }

    /// Footer chips: the primary action first, Skip last.
    pub(super) fn hints(&self) -> Vec<(&'static str, &'static str)> {
        vec![(if self.slide == LAST { "Start" } else { "Next" }, "↵"), ("Navigate", "←/→"), ("Skip", "esc")]
    }

    fn reduce_motion(&mut self) -> bool {
        *self.reduce.get_or_insert_with(|| match std::env::var("FERROCAST_REDUCE_MOTION").as_deref() {
            Ok("1") => true,
            Ok("0") => false,
            _ => system::reduce_motion(),
        })
    }
}

pub(super) fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// `⌥ Space` → ["⌥", "Space"]; `⇧⌘K` → ["⇧", "⌘", "K"].
fn hotkey_keys(label: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut word = String::new();
    for ch in label.chars() {
        if "⌥⇧⌃⌘".contains(ch) {
            if !word.is_empty() {
                keys.push(std::mem::take(&mut word));
            }
            keys.push(ch.to_string());
        } else if ch == ' ' {
            if !word.is_empty() {
                keys.push(std::mem::take(&mut word));
            }
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        keys.push(word);
    }
    keys
}

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Normal,
    /// On a red field: white ink, dark keys.
    OnAccent,
}

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Ok,
    Warn,
    Neutral,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    Go(usize),
    Finish,
    GrantAccessibility,
    GrantInputMonitoring,
    OpenPane(&'static str),
}

const PANE_ACCESSIBILITY: &str = "Privacy_Accessibility";
const PANE_INPUT: &str = "Privacy_ListenEvent";
const PANE_AUTOMATION: &str = "Privacy_Automation";

/// Everything a slide needs to paint itself. Positions are relative to the content rect.
struct Scene<'a> {
    ui: &'a Ui,
    p: egui::Painter,
    pal: Palette,
    c: Rect,
    now: f64,
    /// Start of this slide's arrival.
    t0: f64,
    /// 1.0 for a settled slide; the transition fades it for arriving and leaving slides.
    alpha: f32,
    dx: f32,
    /// No stagger: the leaving slide, Reduce Motion, and the dev screenshots.
    still: bool,
    interactive: bool,
    slide: usize,
    shots: [Option<TextureId>; 3],
    perms: Perms,
    hotkey: Vec<String>,
    actions: &'a RefCell<Vec<Action>>,
}

impl Scene<'_> {
    fn reveal(&self, i: usize) -> (f32, f32) {
        if self.still {
            return (1.0, 0.0);
        }
        let t = ((self.now - self.t0 - REVEAL_DELAY - REVEAL_STEP * i as f64) / REVEAL_TIME) as f32;
        let e = ease_out_cubic(t);
        (e, 14.0 * (1.0 - e))
    }

    /// Absolute point of a content-relative one, with the element's reveal offset applied.
    fn at(&self, x: f32, y: f32, i: usize) -> Pos2 {
        Pos2::new(self.c.min.x + x + self.dx, self.c.min.y + y + self.reveal(i).1)
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32, i: usize) -> Rect {
        Rect::from_min_size(self.at(x, y, i), Vec2::new(w, h))
    }

    fn ink(&self, col: Color32, i: usize) -> Color32 {
        col.gamma_multiply(self.alpha * self.reveal(i).0)
    }

    fn txt(&self, i: usize, x: f32, y: f32, s: &str, size: f32, col: Color32) -> Rect {
        self.p.text(self.at(x, y, i), Align2::LEFT_TOP, s, FontId::proportional(size), self.ink(col, i))
    }

    /// Headline: no bold face is loaded, so the strokes are thickened with a second pass.
    fn headline(&self, i: usize, x: f32, y: f32, s: &str, size: f32, col: Color32) -> Rect {
        let pos = self.at(x, y, i);
        let font = FontId::proportional(size);
        let ink = self.ink(col, i);
        let line_h = size * 1.08;
        let mut bottom = pos.y;
        let mut right = pos.x;
        for (n, line) in s.split('\n').enumerate() {
            let y = pos.y + n as f32 * line_h;
            let r = self.p.text(Pos2::new(pos.x, y), Align2::LEFT_TOP, line, font.clone(), ink);
            self.p.text(Pos2::new(pos.x + size * 0.012, y), Align2::LEFT_TOP, line, font.clone(), ink);
            bottom = r.max.y;
            right = right.max(r.max.x);
        }
        Rect::from_min_max(pos, Pos2::new(right, bottom))
    }

    /// Wrapped paragraph; returns its rect.
    #[allow(clippy::too_many_arguments)]
    fn para(&self, i: usize, x: f32, y: f32, w: f32, s: &str, size: f32, col: Color32) -> Rect {
        let pos = self.at(x, y, i);
        let ink = self.ink(col, i);
        let g = self.p.layout(s.to_string(), FontId::proportional(size), ink, w);
        let r = Rect::from_min_size(pos, g.size());
        self.p.galley(pos, g, ink);
        r
    }

    fn text_w(&self, s: &str, size: f32) -> f32 {
        self.p.layout_no_wrap(s.to_string(), FontId::proportional(size), Color32::WHITE).size().x
    }

    /// A field of colour. Corners that touch the window's top corners follow its rounding.
    fn field(&self, x0: f32, y0: f32, x1: f32, y1: f32, col: Color32) {
        let r = Rect::from_min_max(
            Pos2::new(self.c.min.x + x0 + self.dx, self.c.min.y + y0),
            Pos2::new(self.c.min.x + x1 + self.dx, self.c.min.y + y1),
        );
        let top = y0 <= 0.0;
        let rad = ui::RADIUS as u8;
        let cr = CornerRadius {
            nw: if top && x0 <= 0.0 { rad } else { 0 },
            ne: if top && x1 >= self.c.width() { rad } else { 0 },
            sw: 0,
            se: 0,
        };
        self.p.rect_filled(r, cr, col.gamma_multiply(self.alpha));
    }

    fn keycap(&self, i: usize, x: f32, y: f32, label: &str, h: f32, tone: Tone) -> Rect {
        let size = h * 0.42;
        let tw = self.text_w(label, size);
        let w = (tw + h * 0.62).max(h);
        let r = self.rect(x, y, w, h, i);
        let rad = h * 0.2;
        let dark = self.pal.bg.r() < 128;
        let (face, edge, ink) = match tone {
            Tone::Normal if dark => (Color32::from_gray(64), Color32::from_gray(10), self.pal.text),
            Tone::Normal => (Color32::WHITE, Color32::from_gray(190), self.pal.text),
            Tone::OnAccent => (Color32::from_rgb(30, 22, 24), Color32::from_rgb(116, 30, 40), Color32::WHITE),
        };
        let a = self.alpha * self.reveal(i).0;
        let lip = (h * 0.07).max(2.0);
        // the lower edge shows as a lip under the face
        self.p.rect_filled(Rect::from_min_max(r.min + Vec2::new(0.0, h * 0.4), r.max + Vec2::new(0.0, lip)), rad, edge.gamma_multiply(a));
        self.p.rect_filled(r, rad, face.gamma_multiply(a));
        self.p.text(r.center() - Vec2::new(0.0, h * 0.02), Align2::CENTER_CENTER, label, FontId::proportional(size), ink.gamma_multiply(a));
        r
    }

    /// A row of keycaps starting at (x, y); returns the right edge.
    fn keys(&self, i: usize, x: f32, y: f32, labels: &[String], h: f32, tone: Tone) -> f32 {
        let mut cx = x;
        for l in labels {
            let r = self.keycap(i, cx, y, l, h, tone);
            cx = r.max.x - self.c.min.x - self.dx + h * 0.22;
        }
        cx
    }

    fn image(&self, i: usize, which: usize, r: Rect) {
        let Some(id) = self.shots[which] else { return };
        let a = self.alpha * self.reveal(i).0;
        // a thin edge so the screenshot's dark frame does not dissolve into the field
        self.p.add(
            RectShape::filled(r, 11.0, Color32::WHITE.gamma_multiply(a))
                .with_texture(id, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))),
        );
        self.p.rect_stroke(r, 11.0, Stroke::new(1.0, self.pal.border.gamma_multiply(a)), StrokeKind::Outside);
    }

    fn pill(&self, i: usize, x: f32, y: f32, label: &str, size: f32) -> f32 {
        let w = self.text_w(label, size) + 20.0;
        let r = self.rect(x, y, w, size + 12.0, i);
        let a = self.alpha * self.reveal(i).0;
        self.p.rect_filled(r, r.height() / 2.0, self.pal.field.gamma_multiply(2.0 * a));
        self.p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(size), self.pal.text.gamma_multiply(a));
        w
    }

    fn status(&self, i: usize, x: f32, y: f32, label: &str, kind: Status) -> f32 {
        let col = match kind {
            Status::Ok => GREEN,
            Status::Warn => AMBER,
            Status::Neutral => self.pal.sub_strong,
        };
        let pos = self.at(x, y, i);
        self.p.circle_filled(pos + Vec2::new(4.0, 8.0), 3.5, self.ink(col, i));
        let r = self.p.text(pos + Vec2::new(14.0, 0.0), Align2::LEFT_TOP, label, FontId::proportional(12.5), self.ink(col, i));
        r.max.x - pos.x
    }

    /// A button; `primary` is the red one. Returns its width (relative layout) and pushes `action` when clicked.
    #[allow(clippy::too_many_arguments)]
    fn button(&self, i: usize, x: f32, y: f32, label: &str, primary: bool, on_accent: bool, action: Action) -> f32 {
        let size = 12.5;
        let w = self.text_w(label, size) + 24.0;
        let r = self.rect(x, y, w, 30.0, i);
        let a = self.alpha * self.reveal(i).0;
        let (hover, down) = if self.interactive && a > 0.99 {
            let resp = self
                .ui
                .interact(r, Id::new(("tour-btn", self.slide, i, label)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                self.actions.borrow_mut().push(action);
            }
            (resp.hovered(), resp.is_pointer_button_down_on())
        } else {
            (false, false)
        };
        let (fill, ink) = match (primary, on_accent) {
            (true, true) => (Color32::WHITE, self.pal.accent),
            (true, false) => (self.pal.accent, Color32::WHITE),
            (false, true) => (Color32::from_black_alpha(70), Color32::WHITE),
            (false, false) => (self.pal.field.gamma_multiply(2.0), self.pal.text),
        };
        let fill = if down {
            fill.gamma_multiply(0.85)
        } else if hover {
            fill.gamma_multiply(1.18)
        } else {
            fill
        };
        self.p.rect_filled(r, 8.0, fill.gamma_multiply(a));
        self.p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(size), ink.gamma_multiply(a));
        w
    }

    fn hairline(&self, i: usize, x: f32, y: f32, w: f32) {
        let r = self.rect(x, y, w, 1.0, i);
        self.p.hline(r.x_range(), r.min.y, Stroke::new(1.0, self.pal.sep.gamma_multiply(self.alpha * self.reveal(i).0)));
    }
}

// ---- the slides

fn slide_meet(s: &Scene) {
    let pal = &s.pal;
    s.field(400.0, 0.0, 750.0, s.c.height(), pal.accent);
    // the hotkey, as big keycaps on the red field
    let keys = &s.hotkey;
    let h = 86.0;
    let widths: Vec<f32> = keys.iter().map(|k| (s.text_w(k, h * 0.42) + h * 0.62).max(h)).collect();
    let total: f32 = widths.iter().sum::<f32>() + 16.0 * (keys.len().saturating_sub(1)) as f32;
    let mut x = 400.0 + (350.0 - total) / 2.0;
    for (k, w) in keys.iter().zip(&widths) {
        s.keycap(3, x, 150.0, k, h, Tone::OnAccent);
        x += w + 16.0;
    }
    s.txt(4, 400.0 + (350.0 - s.text_w("open it from anywhere", 15.0)) / 2.0, 272.0, "open it from anywhere", 15.0, Color32::WHITE);
    // the promise, on the dark side
    s.headline(1, 48.0, 122.0, "Meet\nFerrocast.", 58.0, pal.text);
    s.para(2, 48.0, 270.0, 310.0, "Search, calculate, paste, convert and ask without leaving the keyboard.", 15.5, pal.sub_strong);
}

fn slide_type(s: &Scene) {
    let pal = &s.pal;
    s.headline(0, 48.0, 44.0, "Type.\nIt answers.", 46.0, pal.text);
    s.para(1, 48.0, 168.0, 236.0, "Apps, files and commands, ranked by what you actually use. Math, percentages, units, currencies and time zones answer right in the search box.", 14.0, pal.sub_strong);
    s.field(318.0, 0.0, 750.0, s.c.height(), pal.field);
    let img = s.rect(344.0, 40.0, 372.0, 235.0, 2);
    s.image(2, 0, img);
    let mut x = 344.0;
    for (n, q) in ["18% of 240", "100 usd in eur", "5pm ldn in sf"].iter().enumerate() {
        let w = s.p.layout_no_wrap(q.to_string(), FontId::new(12.5, FontFamily::Monospace), Color32::WHITE).size().x + 24.0;
        let r = s.rect(x, 296.0, w, 30.0, 3 + n);
        let a = s.alpha * s.reveal(3 + n).0;
        s.p.rect_filled(r, 8.0, pal.bg.gamma_multiply(a));
        s.p.rect_stroke(r, 8.0, Stroke::new(1.0, pal.border.gamma_multiply(a)), StrokeKind::Inside);
        s.p.text(r.center(), Align2::CENTER_CENTER, q, FontId::new(12.5, FontFamily::Monospace), pal.text.gamma_multiply(a));
        x += w + 8.0;
    }
    s.txt(6, 344.0, 340.0, "Try these in the search box.", 12.5, pal.sub_strong);
}

fn slide_wheel(s: &Scene) {
    let pal = &s.pal;
    s.field(0.0, 0.0, 750.0, s.c.height(), pal.accent.gamma_multiply(0.15));
    let img = s.rect(44.0, 44.0, 372.0, 235.0, 0);
    s.image(0, 1, img);
    let mut x = 44.0;
    for (n, (k, l)) in [("⇧", "Conversions"), ("⌘", "File tools"), ("⌃", "Actions")].iter().enumerate() {
        let r = s.keycap(1 + n, x, 308.0, k, 38.0, Tone::Normal);
        let lx = r.max.x - s.c.min.x - s.dx + 9.0;
        s.txt(1 + n, lx, 317.0, l, 13.0, pal.text);
        x = lx + s.text_w(l, 13.0) + 22.0;
    }
    s.txt(4, 44.0, 358.0, "Hold a file, tap a key, drop it on a verb.", 12.5, pal.sub_strong);
    let rx = 450.0;
    s.headline(1, rx, 40.0, "Drop a file.\nPick a verb.", 36.0, pal.text);
    s.headline(2, rx, 148.0, "275", 60.0, pal.accent_text);
    s.txt(2, rx, 216.0, "conversions", 13.0, pal.sub_strong);
    s.headline(3, rx + 150.0, 148.0, "26", 60.0, pal.accent_text);
    s.txt(3, rx + 150.0, 216.0, "file tools", 13.0, pal.sub_strong);
    s.hairline(4, rx, 254.0, 250.0);
    s.para(4, rx, 268.0, 250.0, "All of it runs on your Mac. Nothing is uploaded, and results land next to the original.", 14.0, pal.text);
}

fn slide_reach(s: &Scene) {
    let pal = &s.pal;
    s.headline(0, 48.0, 34.0, "Everything you reach for twice.", 36.0, pal.text);
    let cells: [(&str, &str, &str); 9] = [
        ("⌘ V", "Clipboard history", "Text, files and images. Password managers are skipped."),
        ("abc →", "Snippets", "Placeholders, and auto-expansion as you type."),
        ("↗", "Quicklinks", "Any URL, with arguments."),
        ("#!", "Script Commands", "Compatible with Raycast metadata."),
        ("40", "Window layouts", "About forty, from halves to other displays."),
        (":)", "Emoji", "Search it, paste it."),
        ("10m", "Timers", "Type timer 10m and go."),
        ("md", "Notes", "Quick notes in markdown."),
        ("⌘ Y", "Quick Look", "Preview the selected file."),
    ];
    let (x0, y0, cw, ch) = (48.0, 92.0, 228.0, 104.0);
    for (n, (key, title, desc)) in cells.iter().enumerate() {
        let (col, row) = ((n % 3) as f32, (n / 3) as f32);
        let (x, y) = (x0 + col * cw, y0 + row * ch);
        let i = 1 + n;
        s.keycap(i, x, y, key, 28.0, Tone::Normal);
        s.txt(i, x, y + 42.0, title, 14.5, pal.text);
        s.para(i, x, y + 62.0, cw - 28.0, desc, 12.0, pal.sub_strong);
    }
}

fn slide_ai(s: &Scene) {
    let pal = &s.pal;
    s.field(430.0, 0.0, 750.0, s.c.height(), pal.field);
    s.headline(0, 48.0, 44.0, "Ask. Translate.\nPrivately.", 42.0, pal.text);
    s.para(
        1,
        48.0,
        160.0,
        330.0,
        "Ask AI talks to Ollama on your own Mac by default. Point it somewhere else when you want to:",
        14.0,
        pal.sub_strong,
    );
    let (mut x, mut y) = (48.0, 214.0);
    for (n, name) in ["Ollama", "OpenAI-compatible server", "Anthropic API", "Claude Code", "Sign in with ChatGPT"].iter().enumerate() {
        let w = s.text_w(name, 12.5) + 20.0;
        if x + w > 390.0 {
            x = 48.0;
            y += 36.0;
        }
        s.pill(2 + n, x, y, name, 12.5);
        x += w + 8.0;
    }
    let ty = y + 56.0;
    s.keycap(7, 48.0, ty, "⌘", 30.0, Tone::Normal);
    s.para(
        7,
        92.0,
        ty + 1.0,
        290.0,
        "Drag selected text, tap ⌘ and drop it on a language. It translates through the same provider.",
        13.0,
        pal.text,
    );
    let img = s.rect(452.0, 70.0, 270.0, 171.0, 3);
    s.image(3, 2, img);
    s.txt(4, 452.0, 262.0, "Ask AI", 13.0, pal.text);
    s.para(4, 452.0, 282.0, 262.0, "Short answers, in a conversation that lasts while the window is open.", 12.5, pal.sub_strong);
}

fn slide_permissions(s: &Scene) -> Vec<String> {
    let pal = &s.pal;
    s.headline(0, 48.0, 30.0, "Switch these on.", 38.0, pal.text);
    s.txt(1, 48.0, 84.0, "Search, the calculator, Ask AI and the drag wheel need none of them.", 13.5, pal.sub_strong);
    let ax = if s.perms.accessibility { Status::Ok } else { Status::Warn };
    let im = if s.perms.input_monitoring { Status::Ok } else { Status::Warn };
    let label = |ok: bool| if ok { "Granted" } else { "Not granted" };
    let ax_state = label(s.perms.accessibility);
    let im_state = label(s.perms.input_monitoring);
    let rows: [(&str, &str, Status, &str); 3] = [
        (
            "Accessibility",
            "Window layouts, pasting into the app in front and snippet expansion. Without it those three do not work.",
            ax,
            ax_state,
        ),
        ("Input Monitoring", "Lets snippets expand as you type. Everything else works without it.", im, im_state),
        (
            "Automation: Finder",
            "Convert Finder Selection. macOS asks the first time you use it, and it cannot be checked beforehand.",
            Status::Neutral,
            "Asked on first use",
        ),
    ];
    let mut lines = Vec::new();
    for (n, (name, desc, kind, state)) in rows.iter().enumerate() {
        let y = 122.0 + n as f32 * 92.0;
        let i = 2 + n;
        s.hairline(i, 48.0, y - 10.0, 654.0);
        s.txt(i, 48.0, y, name, 17.0, pal.text);
        s.status(i, 48.0, y + 26.0, state, *kind);
        s.para(i, 222.0, y + 2.0, 236.0, desc, 12.5, pal.sub_strong);
        lines.push(format!("{}: {}", name.split(':').next().unwrap_or(name), state));
        // buttons, right-aligned in a 150 wide column
        let bx = 474.0;
        match n {
            0 if !s.perms.accessibility => {
                let w = s.button(i, bx, y, "Grant", true, false, Action::GrantAccessibility);
                s.button(i, bx + w + 8.0, y, "Open System Settings", false, false, Action::OpenPane(PANE_ACCESSIBILITY));
            }
            1 if !s.perms.input_monitoring => {
                let w = s.button(i, bx, y, "Grant", true, false, Action::GrantInputMonitoring);
                s.button(i, bx + w + 8.0, y, "Open System Settings", false, false, Action::OpenPane(PANE_INPUT));
            }
            2 => {
                s.button(i, bx, y, "Open System Settings", false, false, Action::OpenPane(PANE_AUTOMATION));
            }
            _ => {}
        }
    }
    lines
}

fn slide_done(s: &Scene) {
    let pal = &s.pal;
    s.field(0.0, 0.0, 750.0, s.c.height(), pal.accent);
    s.headline(0, 48.0, 36.0, "You are set.", 56.0, Color32::WHITE);
    let rows: [(Vec<String>, &str); 3] = [
        (s.hotkey.clone(), "Open Ferrocast from anywhere"),
        (vec!["⌘".into(), "K".into()], "Actions on anything"),
        (vec!["esc".into()], "Close, or go back"),
    ];
    for (n, (keys, what)) in rows.iter().enumerate() {
        let y = 126.0 + n as f32 * 58.0;
        let end = s.keys(1 + n, 48.0, y, keys, 40.0, Tone::OnAccent);
        s.txt(1 + n, end.max(210.0) + 18.0, y + 9.0, what, 18.0, Color32::WHITE);
    }
    s.txt(4, 48.0, 308.0, "To see this tour again, search for “Welcome”.", 14.0, Color32::WHITE.gamma_multiply(0.85));
    s.button(5, 48.0, 340.0, "Start using Ferrocast", true, true, Action::Finish);
}

fn on_accent(slide: usize) -> bool {
    slide == LAST
}

// ---- App side

impl App {
    /// The tour became the current view: start from the first slide.
    pub(super) fn tour_opened(&mut self) {
        self.tour.reset();
    }

    /// Enter: next slide, or finish on the last one.
    pub(super) fn tour_enter(&mut self) {
        if !self.tour.next() {
            self.pop();
        }
    }

    pub(super) fn draw_tour(&mut self, ui: &mut Ui, rect: Rect, pal: &Palette) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let slide = self.tour.slide;
        let reduce = self.tour.reduce_motion();
        let t0 = *self.tour.changed.get_or_insert(now);
        // permissions: injected by tests, otherwise polled while the slide is visible
        let showing_perms = slide == PERMS_SLIDE || self.tour.from == PERMS_SLIDE && now - t0 < TRANSITION;
        if let Some(p) = self.tour.injected {
            self.tour.perms = p;
        } else if showing_perms && now - self.tour.polled >= POLL_EVERY {
            self.tour.perms = Perms::poll();
            self.tour.polled = now;
        }
        if showing_perms && self.tour.injected.is_none() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        // textures, decoded once
        const PNGS: [&[u8]; 3] = [
            include_bytes!("../../../resources/tour/home.png"),
            include_bytes!("../../../resources/tour/files-wheel-convert.png"),
            include_bytes!("../../../resources/tour/ai.png"),
        ];
        for (n, bytes) in PNGS.iter().enumerate() {
            let needed = matches!((n, slide, self.tour.from), (0, 1, _) | (0, _, 1) | (1, 2, _) | (1, _, 2) | (2, 4, _) | (2, _, 4));
            if needed && self.tour.shots[n].is_none() {
                if let Some(img) = services::decode_png(bytes) {
                    self.tour.shots[n] = Some(ctx.load_texture(format!("tour-{n}"), img, TextureOptions::LINEAR));
                }
            }
        }
        let shots = [0, 1, 2].map(|n| self.tour.shots[n].as_ref().map(|t| t.id()));
        let hotkey = hotkey_keys(&services::hotkey_label(&self.settings.launcher_hotkey));
        let actions = RefCell::new(Vec::new());
        let painter = ui.painter().with_clip_rect(rect);
        let transition = if reduce { 1.0 } else { ((now - t0) / TRANSITION) as f32 };
        let e = ease_out_cubic(transition);
        let dir = if slide >= self.tour.from { 1.0 } else { -1.0 };
        let scene = |slide: usize, alpha: f32, dx: f32, still: bool, interactive: bool| Scene {
            ui,
            p: painter.clone(),
            pal: *pal,
            c: rect,
            now,
            t0,
            alpha,
            dx,
            still,
            interactive,
            slide,
            shots,
            perms: self.tour.perms,
            hotkey: hotkey.clone(),
            actions: &actions,
        };
        let mut perm_lines = Vec::new();
        let mut draw_slide = |s: &Scene| match s.slide {
            0 => slide_meet(s),
            1 => slide_type(s),
            2 => slide_wheel(s),
            3 => slide_reach(s),
            4 => slide_ai(s),
            PERMS_SLIDE => perm_lines = slide_permissions(s),
            _ => slide_done(s),
        };
        if self.tour.from != slide && e < 1.0 {
            draw_slide(&scene(self.tour.from, 1.0 - e, -dir * 36.0 * e, true, false));
            draw_slide(&scene(slide, e, dir * 36.0 * (1.0 - e), false, false));
        } else {
            draw_slide(&scene(slide, 1.0, 0.0, reduce, true));
        }
        // what the permissions slide printed (read by the tests)
        ctx.data_mut(|d| d.insert_temp(Id::new("tour-perm-status"), perm_lines));

        // progress dots, bottom left; clickable
        let light_on_red = on_accent(slide) && e > 0.5 || on_accent(self.tour.from) && e <= 0.5;
        let mut cx = rect.min.x + 48.0;
        let cy = rect.max.y - 20.0;
        let mut dots = Vec::new();
        let mut jump = None;
        for n in 0..SLIDES {
            let active = if n == slide {
                e
            } else if n == self.tour.from && self.tour.from != slide {
                1.0 - e
            } else {
                0.0
            };
            let w = 7.0 + 15.0 * active;
            let hit = Rect::from_min_size(Pos2::new(cx - 4.0, cy - 11.0), Vec2::new(w + 8.0, 22.0));
            let resp = ui.interact(hit, Id::new(("tour-dot", n)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                jump = Some(n);
            }
            let (on, off) =
                if light_on_red { (Color32::WHITE, Color32::from_white_alpha(110)) } else { (pal.accent, pal.sub.gamma_multiply(0.7)) };
            let col = if active > 0.5 {
                on
            } else if resp.hovered() {
                pal.sub_strong
            } else {
                off
            };
            painter.rect_filled(Rect::from_center_size(Pos2::new(cx + w / 2.0, cy), Vec2::new(w, 7.0)), 3.5, col);
            dots.push(hit);
            cx += w + 9.0;
        }
        ctx.data_mut(|d| d.insert_temp(Id::new("tour-dots"), dots));

        if !reduce && now - t0 < REVEAL_DELAY + REVEAL_STEP * 10.0 + REVEAL_TIME + 0.1 {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // act after painting, with no borrows of the tour left
        let mut acts = actions.into_inner();
        acts.extend(jump.map(Action::Go));
        for a in acts {
            match a {
                Action::Go(n) => {
                    self.tour.go(n);
                }
                Action::Finish => self.pop(),
                Action::GrantAccessibility => {
                    windows::accessibility_trusted(true);
                }
                Action::GrantInputMonitoring => {
                    windows::request_input_monitoring();
                }
                Action::OpenPane(pane) => {
                    let _ = std::process::Command::new("/usr/bin/open")
                        .arg(format!("x-apple.systempreferences:com.apple.preference.security?{pane}"))
                        .status();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_runs_from_zero_to_one_and_clamps() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert_eq!(ease_out_cubic(-3.0), 0.0);
        assert_eq!(ease_out_cubic(9.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5, "ease-out is front loaded");
    }

    #[test]
    fn hotkey_label_splits_into_keycaps() {
        assert_eq!(hotkey_keys("⌥ Space"), ["⌥", "Space"]);
        assert_eq!(hotkey_keys("⇧⌘K"), ["⇧", "⌘", "K"]);
        assert_eq!(hotkey_keys("⌃ F3"), ["⌃", "F3"]);
    }
}
