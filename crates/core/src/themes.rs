//! Custom colour themes: the model, validation, WCAG contrast, derived colours and the
//! `.ferrocast-theme` import/export format. Pure logic; the app turns [`Derived`] into its palette.
//!
//! The editor exposes seven values (accent, background, surface, text, secondary text, separator
//! and the light/dark base); everything else (hover, selected, borders, readable accent text, text
//! on the accent button) is computed here so a theme cannot be half-defined.
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "ferrocast-theme";
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "ferrocast-theme";
pub const MIN_CONTRAST: f64 = 4.5;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ThemeBase {
    Light,
    #[default]
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomTheme {
    /// Stable key of the theme (survives renames); what iCloud sync merges by. Empty only before
    /// [`load_with_ids`] or a constructor assigned one. Never part of a `.ferrocast-theme` file.
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub base: ThemeBase,
    pub accent: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub secondary_text: String,
    pub separator: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ThemeError {
    Name,
    Color { field: &'static str, value: String },
    Format,
    Version(u32),
    Parse(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pair {
    TextOnBackground,
    TextOnSurface,
    SecondaryOnBackground,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContrastWarning {
    pub pair: Pair,
    pub ratio: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Derived {
    pub bg: [u8; 4],
    pub border: [u8; 4],
    pub text: [u8; 4],
    pub sub: [u8; 4],
    pub sub_strong: [u8; 4],
    pub accent_text: [u8; 4],
    pub on_accent: [u8; 4],
    pub sel: [u8; 4],
    pub hover: [u8; 4],
    pub accent: [u8; 4],
    pub field: [u8; 4],
    pub sep: [u8; 4],
    pub danger: [u8; 4],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ThemeFile {
    pub themes: Vec<CustomTheme>,
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeError::Name => write!(f, "The theme needs a name of 1 to 60 characters"),
            ThemeError::Color { field, value } => write!(f, "\"{value}\" is not a colour for {field} (use #RRGGBB)"),
            ThemeError::Format => write!(f, "This is not a Ferrocast theme file"),
            ThemeError::Version(v) => write!(f, "Theme file version {v} is newer than this Ferrocast understands"),
            ThemeError::Parse(e) => write!(f, "Could not read the theme file: {e}"),
        }
    }
}

/// `#RRGGBB`, `RRGGBB`, `#RGB` or `RGB` (any case, surrounding spaces ignored).
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if !h.is_ascii() {
        return None;
    }
    let digit = |c: &str| u8::from_str_radix(c, 16).ok();
    match h.len() {
        6 => Some([digit(&h[0..2])?, digit(&h[2..4])?, digit(&h[4..6])?]),
        3 => {
            let d = |i: usize| digit(&h[i..i + 1]).map(|v| v * 17);
            Some([d(0)?, d(1)?, d(2)?])
        }
        _ => None,
    }
}

/// `#RRGGBB`, upper case.
pub fn to_hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn luminance(c: [u8; 3]) -> f64 {
    let lin = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG 2 contrast ratio, 1.0 (identical) to 21.0 (black on white); symmetric.
pub fn contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn mix(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    let m = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

fn opaque(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

/// `from` moved toward `toward` until it reads at 4.5:1 on every surface in `on` (or it becomes `toward`).
fn readable(from: [u8; 3], toward: [u8; 3], on: &[[u8; 3]]) -> [u8; 3] {
    for step in 0..=20 {
        let c = mix(from, toward, step as f64 / 20.0);
        if on.iter().all(|s| contrast_ratio(c, *s) >= MIN_CONTRAST) {
            return c;
        }
    }
    toward
}

/// The two built-in themes written as editable values (what "Duplicate" copies); they match the default palette.
pub fn builtin(base: ThemeBase) -> CustomTheme {
    let t = |name: &str, accent: &str, background: &str, surface: &str, text: &str, secondary: &str, sep: &str| CustomTheme {
        id: String::new(),
        name: name.into(),
        base,
        accent: accent.into(),
        background: background.into(),
        surface: surface.into(),
        text: text.into(),
        secondary_text: secondary.into(),
        separator: sep.into(),
    };
    match base {
        ThemeBase::Dark => t("Dark", "#F0535F", "#18181B", "#25252A", "#EEEEEE", "#A6A6A6", "#2A2A2D"),
        ThemeBase::Light => t("Light", "#E8364B", "#FAFAFB", "#F0F0F1", "#1E1E1E", "#585858", "#E4E4E5"),
    }
}

/// A new random theme id.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// `themes.json` with an id on every theme. A file written before ids existed gets them assigned and
/// is saved at once, so the ids are the same on every launch (sync keys themes by id; a new id per
/// launch would look like a deletion plus an addition).
pub fn load_with_ids(store: &crate::storage::Store) -> Vec<CustomTheme> {
    let mut themes = store.load::<ThemeFile>("themes.json").themes;
    let mut changed = false;
    for t in themes.iter_mut().filter(|t| t.id.is_empty()) {
        t.id = new_id();
        changed = true;
    }
    if changed {
        if let Err(e) = store.save("themes.json", &ThemeFile { themes: themes.clone() }) {
            eprintln!("ferrocast: could not save ids to themes.json: {e}");
        }
    }
    themes
}

/// `wanted`, or `wanted 2`, `wanted 3`… when a theme with that name (any case) exists.
pub fn unique_name(existing: &[CustomTheme], wanted: &str) -> String {
    let wanted = wanted.trim();
    let taken = |n: &str| existing.iter().any(|t| t.name.trim().eq_ignore_ascii_case(n));
    if !taken(wanted) {
        return wanted.to_string();
    }
    (2..).map(|i| format!("{wanted} {i}")).find(|n| !taken(n)).unwrap_or_default()
}

/// The `.ferrocast-theme` file: `{"format","version","theme"}`.
pub fn export_json(t: &CustomTheme) -> String {
    let shared = CustomTheme { id: String::new(), ..t.clone() };
    let mut theme = serde_json::to_value(&shared).unwrap_or_default();
    if let Some(o) = theme.as_object_mut() {
        o.remove("id");
    }
    let v = serde_json::json!({ "format": FORMAT, "version": FORMAT_VERSION, "theme": theme });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

/// Read a `.ferrocast-theme` file; the theme comes back validated and normalised.
pub fn import_json(s: &str) -> Result<CustomTheme, ThemeError> {
    let v: serde_json::Value = serde_json::from_str(s).map_err(|e| ThemeError::Parse(e.to_string()))?;
    if v.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err(ThemeError::Format);
    }
    let version = v.get("version").and_then(|n| n.as_u64()).ok_or(ThemeError::Format)?;
    if version == 0 || version > FORMAT_VERSION as u64 {
        return Err(ThemeError::Version(version.min(u32::MAX as u64) as u32));
    }
    let theme = v.get("theme").cloned().ok_or(ThemeError::Format)?;
    let t: CustomTheme = serde_json::from_value(theme).map_err(|e| ThemeError::Parse(e.to_string()))?;
    t.validate()?;
    Ok(CustomTheme { id: new_id(), ..t.normalized() })
}

impl CustomTheme {
    fn colors(&self) -> [(&'static str, &str); 6] {
        [
            ("accent", &self.accent),
            ("background", &self.background),
            ("surface", &self.surface),
            ("text", &self.text),
            ("secondary_text", &self.secondary_text),
            ("separator", &self.separator),
        ]
    }

    pub fn validate(&self) -> Result<(), ThemeError> {
        let n = self.name.trim().chars().count();
        if n == 0 || n > 60 {
            return Err(ThemeError::Name);
        }
        for (field, value) in self.colors() {
            if parse_hex(value).is_none() {
                return Err(ThemeError::Color { field, value: value.to_string() });
            }
        }
        Ok(())
    }

    /// Trimmed name and every colour as `#RRGGBB` (an invalid colour is left as it is).
    pub fn normalized(&self) -> CustomTheme {
        let fix = |s: &str| parse_hex(s).map(to_hex).unwrap_or_else(|| s.to_string());
        CustomTheme {
            id: self.id.clone(),
            name: self.name.trim().to_string(),
            base: self.base,
            accent: fix(&self.accent),
            background: fix(&self.background),
            surface: fix(&self.surface),
            text: fix(&self.text),
            secondary_text: fix(&self.secondary_text),
            separator: fix(&self.separator),
        }
    }

    fn rgb(s: &str) -> [u8; 3] {
        parse_hex(s).unwrap_or([128, 128, 128])
    }

    /// Text pairs under 4.5:1 (WCAG AA for normal text).
    pub fn warnings(&self) -> Vec<ContrastWarning> {
        let (bg, su, tx, se) =
            (Self::rgb(&self.background), Self::rgb(&self.surface), Self::rgb(&self.text), Self::rgb(&self.secondary_text));
        [(Pair::TextOnBackground, tx, bg), (Pair::TextOnSurface, tx, su), (Pair::SecondaryOnBackground, se, bg)]
            .into_iter()
            .map(|(pair, a, b)| ContrastWarning { pair, ratio: contrast_ratio(a, b) })
            .filter(|w| w.ratio < MIN_CONTRAST)
            .collect()
    }

    /// Every palette colour (straight RGBA) from the seven edited values and the base.
    pub fn derive(&self) -> Derived {
        let dark = self.base == ThemeBase::Dark;
        let (bg, su, tx, se, ac, sp) = (
            Self::rgb(&self.background),
            Self::rgb(&self.surface),
            Self::rgb(&self.text),
            Self::rgb(&self.secondary_text),
            Self::rgb(&self.accent),
            Self::rgb(&self.separator),
        );
        let (sel_t, hover_t) = if dark { (0.14, 0.07) } else { (0.10, 0.05) };
        let on_accent =
            if contrast_ratio([0, 0, 0], ac) >= contrast_ratio([255, 255, 255], ac) { [0, 0, 0, 255] } else { [255, 255, 255, 255] };
        let alpha = |c: [u8; 3], a: u8| [c[0], c[1], c[2], a];
        Derived {
            bg: alpha(bg, if dark { 248 } else { 250 }),
            border: alpha(tx, if dark { 28 } else { 36 }),
            text: opaque(tx),
            sub: opaque(se),
            sub_strong: opaque(readable(se, tx, &[bg, su])),
            accent_text: opaque(readable(ac, tx, &[bg, su])),
            on_accent,
            sel: opaque(mix(su, tx, sel_t)),
            hover: opaque(mix(su, tx, hover_t)),
            accent: opaque(ac),
            field: opaque(su),
            sep: opaque(sp),
            danger: if dark { [240, 83, 95, 255] } else { [200, 40, 40, 255] },
        }
    }

    /// An editable copy named "<name> copy" (or "copy 2"…), unique among `existing`.
    pub fn duplicate_of(&self, existing: &[CustomTheme]) -> CustomTheme {
        CustomTheme { id: new_id(), name: unique_name(existing, &format!("{} copy", self.name.trim())), ..self.clone() }
    }
}

#[cfg(test)]
#[path = "themes_tests.rs"]
mod tests;
