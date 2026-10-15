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
pub const ROW_H: f32 = 46.0;
pub const RADIUS: f32 = 14.0;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub border: Color32,
    pub text: Color32,
    pub sub: Color32,
    pub sel: Color32,
    pub accent: Color32,
    pub field: Color32,
    pub sep: Color32,
    pub danger: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: Color32::from_rgba_unmultiplied(24, 24, 27, 248),
            border: Color32::from_white_alpha(28),
            text: Color32::from_gray(238),
            sub: Color32::from_gray(140),
            sel: Color32::from_white_alpha(24),
            accent: Color32::from_rgb(255, 99, 99),
            field: Color32::from_white_alpha(14),
            sep: Color32::from_white_alpha(20),
            danger: Color32::from_rgb(255, 99, 99),
        }
    } else {
        Palette {
            bg: Color32::from_rgba_unmultiplied(250, 250, 251, 250),
            border: Color32::from_black_alpha(36),
            text: Color32::from_gray(30),
            sub: Color32::from_gray(120),
            sel: Color32::from_black_alpha(18),
            accent: Color32::from_rgb(255, 99, 99),
            field: Color32::from_black_alpha(10),
            sep: Color32::from_black_alpha(22),
            danger: Color32::from_rgb(200, 40, 40),
        }
    }
}

pub struct IconCache {
    pub map: HashMap<String, Option<TextureHandle>>,
    pub budget: usize,
    pub images_dir: PathBuf,
}

impl IconCache {
    pub fn new(images_dir: PathBuf) -> Self {
        IconCache { map: HashMap::new(), budget: 6, images_dir }
    }

    pub fn new_frame(&mut self) {
        self.budget = 6;
    }

    pub fn texture(&mut self, ctx: &Context, icon: &Icon) -> Option<TextureHandle> {
        let key = match icon {
            Icon::App(p) => format!("app:{p}"),
            Icon::ClipImage(n) => format!("clip:{n}"),
            Icon::Emoji(e) => format!("emoji:{e}"),
            _ => return None,
        };
        if let Some(t) = self.map.get(&key) {
            return t.clone();
        }
        if self.budget == 0 {
            ctx.request_repaint();
            return None;
        }
        self.budget -= 1;
        let bytes = match icon {
            Icon::App(p) => ferrocast_macos::files::icon_png(p, 64.0),
            Icon::ClipImage(n) => std::fs::read(self.images_dir.join(n)).ok(),
            Icon::Emoji(e) => ferrocast_macos::files::emoji_png(e, 96.0),
            _ => None,
        };
        let tex = bytes.and_then(|b| crate::services::decode_png(&b)).map(|img| ctx.load_texture(&key, img, TextureOptions::LINEAR));
        self.map.insert(key, tex.clone());
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
            p.rect_filled(rect, 7.0, Color32::from_rgb(c[0], c[1], c[2]));
            p.text(rect.center(), Align2::CENTER_CENTER, *g, FontId::proportional(rect.height() * 0.55), Color32::WHITE);
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
