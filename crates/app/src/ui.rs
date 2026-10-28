//! Look & feel: palette, layout constants, icon cache and icon painting.

use crate::items::Icon;
use egui::{Align2, Color32, Context, FontId, Pos2, Rect, TextureHandle, TextureOptions, Ui};
use std::collections::HashMap;
use std::path::PathBuf;

pub const WIDTH: f32 = 750.0;
pub const HEIGHT: f32 = 475.0;
pub const COMPACT_HEIGHT: f32 = 58.0;
pub const SEARCH_H: f32 = 58.0;
pub const FOOTER_H: f32 = 38.0;
pub const ROW_H: f32 = 42.0;
pub const RADIUS: f32 = 14.0;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub border: Color32,
    pub text: Color32,
    pub sub: Color32,
    /// Settings-only: `sub` pushed far enough from the card fills to read at 4.5:1 (descriptions, labels, icons).
    pub sub_strong: Color32,
    /// Settings-only: the accent as text colour (4.5:1 on the card and row fills in both themes).
    pub accent_text: Color32,
    pub sel: Color32,
    pub accent: Color32,
    pub field: Color32,
    pub sep: Color32,
    pub danger: Color32,
    /// Hover fill of rows and buttons.
    pub hover: Color32,
    /// Text and glyphs drawn on an accent fill.
    pub on_accent: Color32,
    /// Fill of popovers (the actions panel).
    pub popover: Color32,
    /// Fill of the drop wheel's disc.
    pub disc: Color32,
    /// A custom theme (opaque surface fills) rather than the built-in translucent ones.
    pub custom: bool,
}

impl Palette {
    /// `field` made `k` times stronger (1.3 resting control, 1.6 hover, 2.4 pressed). The built-in
    /// palettes brighten their translucent overlay; a custom theme's opaque surface moves toward
    /// its text colour instead, which works on light and dark bases alike.
    pub fn lift(&self, k: f32) -> Color32 {
        if !self.custom {
            return self.field.gamma_multiply(k);
        }
        let t = ((k - 1.0) * 0.1).clamp(0.0, 0.5);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color32::from_rgb(m(self.field.r(), self.text.r()), m(self.field.g(), self.text.g()), m(self.field.b(), self.text.b()))
    }
}

/// The palette of a custom theme: every value comes from `ferrocast_core::themes::CustomTheme::derive`.
pub fn palette_custom(t: &ferrocast_core::themes::CustomTheme) -> Palette {
    let d = t.derive();
    let c = |v: [u8; 4]| Color32::from_rgba_unmultiplied(v[0], v[1], v[2], v[3]);
    let dark = t.base == ferrocast_core::themes::ThemeBase::Dark;
    Palette {
        bg: c(d.bg),
        border: c(d.border),
        text: c(d.text),
        sub: c(d.sub),
        sub_strong: c(d.sub_strong),
        accent_text: c(d.accent_text),
        sel: c(d.sel),
        accent: c(d.accent),
        field: c(d.field),
        sep: c(d.sep),
        danger: c(d.danger),
        hover: c(d.hover),
        on_accent: c(d.on_accent),
        popover: c([d.field[0], d.field[1], d.field[2], 255]),
        custom: true,
        disc: c([d.bg[0], d.bg[1], d.bg[2], if dark { 222 } else { 226 }]),
    }
}

pub fn palette(dark: bool) -> Palette {
    let mut p = default_palette(dark);
    p.hover = p.lift(1.6);
    p
}

fn default_palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: Color32::from_rgba_unmultiplied(24, 24, 27, 248),
            border: Color32::from_white_alpha(28),
            text: Color32::from_gray(238),
            sub: Color32::from_gray(140),
            sub_strong: Color32::from_gray(166),
            accent_text: Color32::from_rgb(255, 112, 122),
            sel: Color32::from_white_alpha(24),
            accent: Color32::from_rgb(240, 83, 95),
            field: Color32::from_white_alpha(14),
            sep: Color32::from_white_alpha(20),
            danger: Color32::from_rgb(240, 83, 95),
            hover: Color32::TRANSPARENT,
            on_accent: Color32::WHITE,
            popover: Color32::from_rgb(34, 34, 38),
            disc: Color32::from_rgba_unmultiplied(24, 24, 27, 222),
            custom: false,
        }
    } else {
        Palette {
            bg: Color32::from_rgba_unmultiplied(250, 250, 251, 250),
            border: Color32::from_black_alpha(36),
            text: Color32::from_gray(30),
            sub: Color32::from_gray(120),
            sub_strong: Color32::from_gray(88),
            accent_text: Color32::from_rgb(190, 28, 52),
            sel: Color32::from_black_alpha(18),
            accent: Color32::from_rgb(232, 54, 75),
            field: Color32::from_black_alpha(10),
            sep: Color32::from_black_alpha(22),
            danger: Color32::from_rgb(200, 40, 40),
            hover: Color32::TRANSPARENT,
            on_accent: Color32::WHITE,
            popover: Color32::from_rgb(246, 246, 248),
            disc: Color32::from_rgba_unmultiplied(250, 250, 251, 226),
            custom: false,
        }
    }
}

pub struct IconCache {
    pub map: HashMap<String, Option<TextureHandle>>,
    /// When this frame started painting; synchronous icon work stops after `FRAME_BUDGET`.
    pub frame_start: std::time::Instant,
    pub images_dir: PathBuf,
    /// PNG bytes fetched off the main thread, waiting to become textures.
    pub prefetched: HashMap<String, Vec<u8>>,
    /// Keys already handed to the prefetch thread.
    pub inflight: std::collections::HashSet<String>,
    /// Where worker threads send decoded clipboard images; `None` loads them on the calling thread.
    bus: Option<crate::services::Bus>,
    /// Clipboard image keys, least recently used first.
    clip_lru: std::collections::VecDeque<String>,
}

/// Longest side, in pixels, of a clipboard image texture (the detail pane shows it at most this big).
pub const CLIP_IMAGE_PX: u32 = 720;
/// Clipboard image textures kept at once; the least recently used one is dropped.
pub const CLIP_TEXTURES: usize = 32;

impl IconCache {
    /// Load clipboard images on worker threads that answer through `bus`.
    pub fn with_bus(mut self, bus: crate::services::Bus) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Store a texture; clipboard images are tracked by recency and the oldest ones are dropped
    /// (dropping the handle frees the GPU texture), so browsing many pictures cannot grow without bound.
    fn put(&mut self, key: String, tex: Option<TextureHandle>) {
        self.inflight.remove(&key);
        let is_clip = key.starts_with("clip:");
        self.map.insert(key.clone(), tex);
        if is_clip {
            self.clip_lru.retain(|k| *k != key);
            self.clip_lru.push_back(key);
            while self.clip_lru.len() > CLIP_TEXTURES {
                if let Some(old) = self.clip_lru.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }

    pub fn new(images_dir: PathBuf) -> Self {
        IconCache {
            map: HashMap::new(),
            frame_start: std::time::Instant::now(),
            images_dir,
            prefetched: HashMap::new(),
            inflight: Default::default(),
            bus: None,
            clip_lru: Default::default(),
        }
    }

    /// Main-thread time one frame may spend decoding icons; the rest shows a placeholder and
    /// finishes next frame, so a list of emoji (5–30 ms each to render) never stalls the UI.
    pub const FRAME_BUDGET: std::time::Duration = std::time::Duration::from_millis(4);

    pub fn new_frame(&mut self) {
        self.frame_start = std::time::Instant::now();
    }

    pub fn texture(&mut self, ctx: &Context, icon: &Icon) -> Option<TextureHandle> {
        let key = match icon {
            Icon::App(p) => format!("app:{p}"),
            Icon::ClipImage(n) => format!("clip:{n}"),
            Icon::Emoji(e) => format!("emoji:{e}"),
            Icon::Glyph(g, _) => format!("glyph:{g}"),
            _ => return None,
        };
        if let Some(t) = self.map.get(&key) {
            let t = t.clone();
            if key.starts_with("clip:") {
                if let Some(i) = self.clip_lru.iter().position(|k| *k == key) {
                    let k = self.clip_lru.remove(i).unwrap_or_default();
                    self.clip_lru.push_back(k);
                }
            }
            return t;
        }
        // Bytes a background thread already fetched: only the texture upload is left, which is cheap.
        if let Some(b) = self.prefetched.remove(&key) {
            let tex = crate::services::decode_png(&b).map(|img| ctx.load_texture(&key, img, TextureOptions::LINEAR));
            self.put(key, tex.clone());
            return tex;
        }
        // A clipboard picture can be huge: a worker downscales it, the UI shows a placeholder meanwhile.
        if let (Icon::ClipImage(n), Some(bus)) = (icon, &self.bus) {
            if self.inflight.insert(key.clone()) {
                let (path, bus) = (self.images_dir.join(n), bus.clone());
                std::thread::spawn(move || {
                    // an unreadable file sends empty bytes, which settle as "no picture"
                    let bytes = crate::services::display_png(&path, CLIP_IMAGE_PX).unwrap_or_default();
                    bus.send(crate::services::Msg::IconBytes(key, bytes));
                });
            }
            return None;
        }
        if self.frame_start.elapsed() > Self::FRAME_BUDGET {
            ctx.request_repaint();
            return None;
        }
        let t0 = std::time::Instant::now();
        let bytes = match icon {
            Icon::App(p) => ferrocast_macos::files::icon_png(p, 64.0),
            Icon::ClipImage(n) => std::fs::read(self.images_dir.join(n)).ok(),
            Icon::Emoji(e) => ferrocast_macos::files::emoji_png(e, 96.0),
            // "sf:name" asks for an SF Symbol; anything else is drawn as text with system fallback.
            Icon::Glyph(g, _) => match g.strip_prefix("sf:") {
                Some(name) => ferrocast_macos::files::symbol_png(name, 64.0).or_else(|| ferrocast_macos::files::glyph_png("•", 64.0)),
                None => ferrocast_macos::files::glyph_png(g, 64.0),
            },
            _ => None,
        };
        let tex = bytes.and_then(|b| crate::services::decode_png(&b)).map(|img| ctx.load_texture(&key, img, TextureOptions::LINEAR));
        if std::env::var_os("FERROCAST_PROFILE").is_some() {
            eprintln!("icon sync {key} {:?}", t0.elapsed());
        }
        self.put(key, tex.clone());
        tex
    }
}

pub fn paint_icon(cache: &mut IconCache, ui: &Ui, rect: Rect, icon: &Icon, pal: &Palette) {
    let p = ui.painter();
    match icon {
        Icon::App(_) | Icon::ClipImage(_) | Icon::Emoji(_) => match cache.texture(ui.ctx(), icon) {
            Some(t) => {
                let r = if matches!(icon, Icon::ClipImage(_)) { fit(rect, t.size_vec2()) } else { rect };
                p.image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                p.rect_filled(rect, 7.0, pal.field);
            }
        },
        Icon::Glyph(g, c) => {
            // Coloured tile with the symbol rendered by AppKit (full system font fallback), so
            // no glyph ever shows as an empty box; plain ASCII falls back to the UI font.
            p.rect_filled(rect, 7.0, Color32::from_rgb(c[0], c[1], c[2]));
            if g.is_ascii() && !g.starts_with("sf:") {
                p.text(rect.center(), Align2::CENTER_CENTER, *g, FontId::proportional(rect.height() * 0.5), Color32::WHITE);
            } else {
                if let Some(t) = cache.texture(ui.ctx(), icon) {
                    p.image(t.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
            }
        }
        Icon::Color(c) => {
            p.rect_filled(rect, 7.0, Color32::from_rgba_unmultiplied(c.r, c.g, c.b, (c.a * 255.0) as u8));
        }
    }
}

/// Fit `size` inside `rect`, preserving aspect ratio.
pub fn fit(rect: Rect, size: egui::Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return rect;
    }
    let s = (rect.width() / size.x).min(rect.height() / size.y);
    Rect::from_center_size(rect.center(), size * s)
}
