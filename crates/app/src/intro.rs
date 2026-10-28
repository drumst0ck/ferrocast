//! First-launch intro: a neon-red sunrise, the logo rising out of it, then FERROCAST.
//!
//! Plays once, over the tour's first slide, and hands over with a crossfade. The whole look is a
//! pure function of time ([`phase`]); drawing builds the glow from one vertex-coloured mesh per
//! layer (egui has no blur), so a frame costs a few thousand vertices and no texture uploads.
//! Any key or click skips to the hand-off. Reduce Motion starts the clock at the settled frame:
//! no movement, the finished logo and wordmark for about a second, then the same short fade.

use super::*;
use egui::epaint::{Mesh, Vertex};
use egui::{Color32, FontFamily, Painter, TextureOptions};
use tour::ease_out_cubic;

/// The sunrise climbs for this long.
const RISE: f32 = 2.6;
/// The logo fades in and rises between these times.
const LOGO: (f32, f32) = (0.9, 1.9);
/// FERROCAST appears letter by letter from `WORD_AT`.
const WORD_AT: f32 = 1.8;
const LETTER_STEP: f32 = 0.06;
const LETTER_TIME: f32 = 0.5;
/// Hold the finished scene until here, then crossfade into the tour.
pub(super) const HANDOFF_AT: f32 = 3.4;
pub(super) const HANDOFF: f32 = 0.5;
pub(super) const DURATION: f32 = HANDOFF_AT + HANDOFF;
/// Reduce Motion starts here: everything settled, about a second of hold.
pub(super) const REDUCED_START: f32 = 2.8;

const LETTERS: &str = "FERROCAST";
const LOGO_SIZE: f32 = 92.0;
const LOGO_RISE_PX: f32 = 34.0;

/// Everything the drawing needs at one instant; all values are 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Phase {
    /// Sun height: 0 below the edge, 1 settled.
    pub rise: f32,
    /// Logo fade and lift.
    pub logo: f32,
    /// Wordmark progress (letters are staggered inside it).
    pub word: f32,
    /// Opacity of the whole intro (falls to 0 during the hand-off).
    pub alpha: f32,
    pub done: bool,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn span(t: f32, a: f32, b: f32) -> f32 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

pub(super) fn phase(t: f32) -> Phase {
    let letters_end = WORD_AT + LETTER_STEP * (LETTERS.len() - 1) as f32 + LETTER_TIME;
    Phase {
        rise: smooth(span(t, 0.0, RISE)),
        logo: span(t, LOGO.0, LOGO.1),
        word: span(t, WORD_AT, letters_end),
        alpha: 1.0 - smooth(span(t, HANDOFF_AT, DURATION)),
        done: t >= DURATION,
    }
}

pub(super) struct Intro {
    /// Egui time of the first frame; stamped by it.
    start: Option<f64>,
    /// Added to the elapsed time: Reduce Motion, or a skip jumping to the hand-off.
    bump: f64,
    /// Dev aid `FERROCAST_INTRO_T=<s>`: the intro is frozen at this time.
    frozen: Option<f32>,
    /// Test hook: use this as the clock instead of egui's.
    pub clock: Option<f64>,
    /// Timeline position of the last frame.
    last_t: f32,
    /// The sound still has to start (with the first frame).
    sound_pending: bool,
}

impl Intro {
    pub(super) fn new(reduce: bool) -> Self {
        let frozen = std::env::var("FERROCAST_INTRO_T").ok().and_then(|v| v.parse().ok());
        Intro {
            start: None,
            bump: if reduce { REDUCED_START as f64 } else { 0.0 },
            frozen,
            clock: None,
            last_t: 0.0,
            sound_pending: !reduce,
        }
    }

    fn raw(&mut self, now: f64) -> f64 {
        let now = self.clock.unwrap_or(now);
        now - *self.start.get_or_insert(now) + self.bump
    }

    /// Seconds into the timeline at egui time `now`.
    pub(super) fn elapsed(&mut self, now: f64) -> f32 {
        let t = self.frozen.unwrap_or_else(|| self.raw(now) as f32);
        self.last_t = t;
        t
    }

    /// Any key or click: jump to the hand-off.
    pub(super) fn skip(&mut self, now: f64) {
        let raw = self.raw(now);
        if self.frozen.is_none() && raw < HANDOFF_AT as f64 {
            self.bump += HANDOFF_AT as f64 - raw;
            self.last_t = HANDOFF_AT;
        }
    }

    /// Whether to start the sound now: once, on the animated intro only. Reduce Motion starts at
    /// the settled frame, where the sound would be out of step; frozen dev frames and tests
    /// (which inject the clock) stay silent.
    pub(super) fn take_sound(&mut self) -> bool {
        let play = self.sound_pending && self.frozen.is_none() && self.clock.is_none();
        self.sound_pending = false;
        play
    }

    /// True while the intro is fully opaque (the tour underneath is not drawn yet).
    pub(super) fn covers(&self) -> bool {
        self.frozen.is_some() || self.start.is_none() || self.last_t < HANDOFF_AT
    }
}

// ---- drawing

const BG: [f32; 3] = [9.0, 7.0, 10.0];

/// Glow colour by intensity 0..=1: deep crimson, the app's neon red, hot pink-red, near-white core.
/// Returns premultiplied RGBA in 0..=255.
fn ramp(i: f32) -> [f32; 4] {
    const STOPS: [(f32, [f32; 3]); 5] = [
        (0.0, [90.0, 6.0, 30.0]),
        (0.18, [170.0, 24.0, 50.0]),
        (0.45, [240.0, 83.0, 95.0]),
        (0.78, [255.0, 126.0, 120.0]),
        (1.0, [255.0, 232.0, 220.0]),
    ];
    let i = i.clamp(0.0, 1.0);
    let k = STOPS.iter().position(|s| s.0 >= i).unwrap_or(STOPS.len() - 1).max(1);
    let (a, b) = (STOPS[k - 1], STOPS[k]);
    let f = smooth((i - a.0) / (b.0 - a.0));
    let m = |n: usize| a.1[n] + (b.1[n] - a.1[n]) * f;
    // opacity rises smoothly from nothing to full: no plateau, no visible edge
    let alpha = smooth(i / 0.55);
    [m(0) * alpha, m(1) * alpha, m(2) * alpha, alpha * 255.0]
}

fn premul(c: [f32; 4], fade: f32) -> Color32 {
    let q = |v: f32| (v * fade).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(q(c[0]), q(c[1]), q(c[2]), q(c[3]))
}

/// A radial glow as one mesh: concentric rings (denser near the core, where the colour changes
/// fastest) with a colour per vertex, so the GPU interpolates a smooth gradient.
/// `core` is the weight of the tight hot centre; 0 for wide, flat glows, where a bright centre
/// stretched into a visible streak. As it fades out the glow also cools down the ramp, so the
/// near-white core turns red instead of grey over the darker tour behind it.
fn glow(p: &Painter, center: Pos2, rx: f32, ry: f32, strength: f32, core: f32, fade: f32) {
    if strength <= 0.002 || fade <= 0.0 {
        return;
    }
    let strength = strength * fade.sqrt();
    const RINGS: usize = 56;
    const SEGS: usize = 72;
    let intensity = |s: f32| {
        let q = 1.0 - s;
        strength * ((1.0 - core) * q.powf(2.3) + core * q.powi(9))
    };
    let mut mesh = Mesh::default();
    let uv = egui::epaint::WHITE_UV;
    mesh.vertices.reserve(1 + RINGS * SEGS);
    mesh.indices.reserve(RINGS * SEGS * 6);
    mesh.vertices.push(Vertex { pos: center, uv, color: premul(ramp(intensity(0.0)), fade) });
    for k in 1..=RINGS {
        let s = (k as f32 / RINGS as f32).powf(1.5);
        let color = premul(ramp(if k == RINGS { 0.0 } else { intensity(s) }), fade);
        for j in 0..SEGS {
            let a = j as f32 / SEGS as f32 * std::f32::consts::TAU;
            mesh.vertices.push(Vertex { pos: center + Vec2::new(a.cos() * rx * s, a.sin() * ry * s), uv, color });
        }
    }
    let ring = |k: usize, j: usize| (1 + (k - 1) * SEGS + j % SEGS) as u32;
    for j in 0..SEGS {
        mesh.indices.extend([0, ring(1, j), ring(1, j + 1)]);
    }
    for k in 1..RINGS {
        for j in 0..SEGS {
            let (a, b, c, d) = (ring(k, j), ring(k, j + 1), ring(k + 1, j), ring(k + 1, j + 1));
            mesh.indices.extend([a, c, b, b, c, d]);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

impl App {
    /// First launch only: the intro covers the tour's first slide until it ends.
    pub(super) fn begin_intro(&mut self, reduce: bool) {
        self.intro = Some(Intro::new(reduce));
    }

    /// Keys and clicks skip the intro and are swallowed, so they do not also drive the tour.
    pub(super) fn intro_input(&mut self, ctx: &Context) {
        let Some(intro) = self.intro.as_mut() else { return };
        let now = ctx.input(|i| i.time);
        intro.elapsed(now);
        if intro.take_sound() && !cfg!(test) {
            play_sound();
        }
        if !intro.covers() {
            return;
        }
        let skipping = |e: &egui::Event| {
            matches!(e, egui::Event::Key { pressed: true, .. } | egui::Event::Text(_) | egui::Event::PointerButton { pressed: true, .. })
        };
        if ctx.input(|i| i.events.iter().any(skipping)) {
            intro.skip(now);
            ctx.input_mut(|i| i.events.retain(|e| !skipping(e)));
        }
    }

    pub(super) fn intro_covers(&self) -> bool {
        self.intro.as_ref().is_some_and(|i| i.covers())
    }

    /// Paint the intro over the whole window; ends it (and stops the repaints) when done.
    pub(super) fn intro_paint(&mut self, ui: &Ui) {
        let Some(intro) = self.intro.take() else { return };
        let ctx = ui.ctx().clone();
        let t = intro.last_t;
        let ph = phase(t);
        if ph.done && intro.frozen.is_none() {
            ctx.request_repaint();
            return;
        }
        let full = ui.max_rect();
        let p = ui.painter();
        let a = ph.alpha;
        let bg = Color32::from_rgba_premultiplied((BG[0] * a) as u8, (BG[1] * a) as u8, (BG[2] * a) as u8, (255.0 * a) as u8);
        p.rect_filled(full, ui::RADIUS, bg);

        // sunrise: the sun climbs from below the bottom edge; the haze along the horizon leads it
        let cx = full.center().x;
        let sun = Pos2::new(cx, full.max.y + 120.0 - 205.0 * ph.rise);
        let grow = 0.55 + 0.45 * ph.rise;
        let strength = ph.rise.powf(0.8) * (1.0 + 0.025 * (t * 1.7).sin());
        glow(p, Pos2::new(cx, full.max.y + 30.0), 440.0, 120.0, 0.34 * strength, 0.0, a);
        glow(p, sun, 520.0 * grow, 360.0 * grow, strength * 0.92, 0.16, a);

        // logo: fades in and rises out of the glow, settling a little above the middle
        let e = ease_out_cubic(ph.logo);
        let settled_y = full.center().y - 52.0;
        let logo_c = Pos2::new(cx, settled_y + LOGO_RISE_PX * (1.0 - e));
        if e > 0.0 {
            glow(p, logo_c, 150.0, 150.0, 0.5 * e, 0.16, a);
            let icon = self.app_icon.get_or_insert_with(|| {
                let img = services::decode_png(include_bytes!("../../../resources/icon-1024.png"))
                    .unwrap_or_else(|| egui::ColorImage::filled([1, 1], Color32::TRANSPARENT));
                ctx.load_texture("app-icon", img, TextureOptions::LINEAR)
            });
            let tint = Color32::WHITE.gamma_multiply(e * a);
            p.image(
                icon.id(),
                Rect::from_center_size(logo_c, Vec2::splat(LOGO_SIZE)),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                tint,
            );
        }

        // wordmark: letters arrive left to right while the tracking tightens
        if ph.word > 0.0 {
            let font = FontId::new(44.0, FontFamily::Proportional);
            let chars: Vec<char> = LETTERS.chars().collect();
            let widths: Vec<f32> = chars.iter().map(|c| p.layout_no_wrap(c.to_string(), font.clone(), Color32::WHITE).size().x).collect();
            let tracking = 11.0 + 18.0 * (1.0 - ease_out_cubic(ph.word));
            let width: f32 = widths.iter().sum::<f32>() + tracking * (chars.len() - 1) as f32;
            let y = settled_y + LOGO_SIZE / 2.0 + 52.0;
            glow(p, Pos2::new(cx, y), width * 0.62, 52.0, 0.34 * ph.word, 0.0, a);
            let mut x = cx - width / 2.0;
            for (n, c) in chars.iter().enumerate() {
                let from = WORD_AT + n as f32 * LETTER_STEP;
                let li = ease_out_cubic(span(t, from, from + LETTER_TIME));
                if li > 0.0 {
                    let lift = 9.0 * (1.0 - li);
                    p.text(Pos2::new(x, y + lift), Align2::LEFT_CENTER, *c, font.clone(), Color32::WHITE.gamma_multiply(li * a));
                }
                x += widths[n] + tracking;
            }
        }

        // a quiet way out, once the scene has settled a little
        let hint = if t < HANDOFF_AT { span(t, 1.2, 1.8) * 0.38 } else { 0.0 };
        if hint > 0.0 {
            p.text(
                Pos2::new(full.max.x - 18.0, full.max.y - 16.0),
                Align2::RIGHT_CENTER,
                tr("Skip  esc"),
                FontId::proportional(11.5),
                Color32::WHITE.gamma_multiply(hint),
            );
        }

        if intro.frozen.is_none() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        self.intro = Some(intro);
    }
}

/// The intro sound, made by scripts/make_intro_sound.py and timed to the animation.
const SOUND: &[u8] = include_bytes!("../../../resources/intro.m4a");

/// Play the intro sound with the system player, off the UI thread; it follows the system volume.
fn play_sound() {
    std::thread::spawn(|| {
        let path = std::env::temp_dir().join("ferrocast-intro.m4a");
        if std::fs::write(&path, SOUND).is_ok() {
            let _ = std::process::Command::new("/usr/bin/afplay").arg("-v").arg("0.7").arg(&path).status();
        }
    });
}
