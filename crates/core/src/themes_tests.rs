use super::*;

fn sample() -> CustomTheme {
    CustomTheme {
        id: "t-ocean".into(),
        name: "Ocean".into(),
        base: ThemeBase::Dark,
        accent: "#38BDF8".into(),
        background: "#0B1220".into(),
        surface: "#16213A".into(),
        text: "#E6EEFF".into(),
        secondary_text: "#9FB0D0".into(),
        separator: "#26324D".into(),
    }
}

#[test]
fn hex_parses_with_or_without_hash_and_short_form() {
    assert_eq!(parse_hex("#FF8000"), Some([255, 128, 0]));
    assert_eq!(parse_hex("ff8000"), Some([255, 128, 0]));
    assert_eq!(parse_hex("#f80"), Some([255, 136, 0]));
    assert_eq!(parse_hex(" #0a0B0c "), Some([10, 11, 12]));
    assert_eq!(parse_hex("#GG0000"), None);
    assert_eq!(parse_hex("#12345"), None);
    assert_eq!(parse_hex(""), None);
    assert_eq!(to_hex([10, 11, 12]), "#0A0B0C");
}

#[test]
fn contrast_ratio_matches_wcag_reference_values() {
    let r = contrast_ratio([0, 0, 0], [255, 255, 255]);
    assert!((r - 21.0).abs() < 1e-9, "{r}");
    assert!((contrast_ratio([255, 255, 255], [255, 255, 255]) - 1.0).abs() < 1e-9);
    // #777777 on white is the classic 4.48:1
    let g = contrast_ratio([0x77, 0x77, 0x77], [255, 255, 255]);
    assert!((g - 4.48).abs() < 0.01, "{g}");
    assert_eq!(contrast_ratio([1, 2, 3], [200, 100, 50]), contrast_ratio([200, 100, 50], [1, 2, 3]));
}

#[test]
fn validation_accepts_a_good_theme_and_rejects_bad_name_or_colour() {
    assert_eq!(sample().validate(), Ok(()));
    let mut t = sample();
    t.name = "   ".into();
    assert_eq!(t.validate(), Err(ThemeError::Name));
    t.name = "x".repeat(61);
    assert_eq!(t.validate(), Err(ThemeError::Name));
    let mut t = sample();
    t.accent = "red".into();
    assert!(matches!(t.validate(), Err(ThemeError::Color { field: "accent", .. })));
    let mut t = sample();
    t.separator = "".into();
    assert!(matches!(t.validate(), Err(ThemeError::Color { field: "separator", .. })));
}

#[test]
fn normalized_uppercases_expands_and_trims() {
    let mut t = sample();
    t.accent = " f80 ".into();
    t.text = "e6eeff".into();
    t.name = "  Ocean  ".into();
    let n = t.normalized();
    assert_eq!(n.accent, "#FF8800");
    assert_eq!(n.text, "#E6EEFF");
    assert_eq!(n.name, "Ocean");
}

#[test]
fn warnings_flag_pairs_below_4_5_and_stay_quiet_otherwise() {
    assert!(sample().warnings().is_empty());
    let mut t = sample();
    t.text = "#1A2540".into(); // dark text on a dark background
    let w = t.warnings();
    assert!(w.iter().any(|w| w.pair == Pair::TextOnBackground && w.ratio < MIN_CONTRAST));
    assert!(w.iter().any(|w| w.pair == Pair::TextOnSurface));
    let mut t = sample();
    t.secondary_text = "#16213B".into();
    let w = t.warnings();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].pair, Pair::SecondaryOnBackground);
}

#[test]
fn builtin_themes_are_valid_and_readable() {
    for b in [ThemeBase::Dark, ThemeBase::Light] {
        let t = builtin(b);
        assert_eq!(t.validate(), Ok(()));
        assert_eq!(t.base, b);
        assert!(t.warnings().is_empty(), "{b:?}: {:?}", t.warnings());
    }
    assert_eq!(builtin(ThemeBase::Dark).background, "#18181B");
    assert_eq!(builtin(ThemeBase::Light).background, "#FAFAFB");
}

#[test]
fn derived_colours_come_from_the_base_and_keep_text_readable() {
    let d = sample().derive();
    assert_eq!(&d.accent[..3], &[0x38, 0xBD, 0xF8]);
    assert_eq!(&d.bg[..3], &[0x0B, 0x12, 0x20]);
    assert_eq!(d.bg[3], 248, "dark windows keep the default translucency");
    let light = CustomTheme { base: ThemeBase::Light, ..sample() }.derive();
    assert_eq!(light.bg[3], 250);
    // selected is stronger than hover, hover is distinct from the plain surface
    let lum = |c: [u8; 4]| contrast_ratio([c[0], c[1], c[2]], [0, 0, 0]);
    assert!(lum(d.sel) > lum(d.hover) && lum(d.hover) > lum(d.field));
    // sub_strong and accent_text reach 4.5:1 on background and surface
    let bg = [0x0B, 0x12, 0x20];
    let su = [0x16, 0x21, 0x3A];
    for c in [d.sub_strong, d.accent_text] {
        let c = [c[0], c[1], c[2]];
        assert!(contrast_ratio(c, bg) >= MIN_CONTRAST && contrast_ratio(c, su) >= MIN_CONTRAST);
    }
    // text on the accent button is black or white, whichever reads better
    let mut yellow = sample();
    yellow.accent = "#FFE600".into();
    assert_eq!(&yellow.derive().on_accent[..3], &[0, 0, 0]);
    assert_eq!(&sample().derive().on_accent[..3], &[0, 0, 0], "sky blue is light enough for black");
    let mut navy = sample();
    navy.accent = "#1D2B8F".into();
    assert_eq!(&navy.derive().on_accent[..3], &[255, 255, 255]);
}

#[test]
fn unique_names_and_duplicates_never_collide() {
    let a = sample();
    assert_eq!(unique_name(&[], "Ocean"), "Ocean");
    assert_eq!(unique_name(std::slice::from_ref(&a), "ocean"), "ocean 2");
    let mut b = a.clone();
    b.name = "Ocean 2".into();
    assert_eq!(unique_name(&[a.clone(), b.clone()], "Ocean"), "Ocean 3");
    let d = a.duplicate_of(std::slice::from_ref(&a));
    assert_eq!(d.name, "Ocean copy");
    assert_eq!(d.accent, a.accent);
    let d2 = a.duplicate_of(&[a.clone(), d.clone()]);
    assert_eq!(d2.name, "Ocean copy 2");
    // a built-in has no editable twin until duplicated
    assert_eq!(builtin(ThemeBase::Dark).duplicate_of(&[]).name, "Dark copy");
}

#[test]
fn export_import_round_trips_and_carries_the_format_and_version() {
    let t = sample();
    let json = export_json(&t);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["format"], FORMAT);
    assert_eq!(v["version"], FORMAT_VERSION);
    assert_eq!(v["theme"]["accent"], "#38BDF8");
    let back = import_json(&json).unwrap();
    assert_eq!(CustomTheme { id: t.id.clone(), ..back }, t, "everything but the id round-trips");
}

#[test]
fn import_rejects_wrong_format_newer_version_garbage_and_bad_colours() {
    assert!(matches!(import_json("not json"), Err(ThemeError::Parse(_))));
    assert_eq!(import_json(r#"{"format":"other","version":1,"theme":{}}"#), Err(ThemeError::Format));
    let mut v: serde_json::Value = serde_json::from_str(&export_json(&sample())).unwrap();
    v["version"] = 99.into();
    assert_eq!(import_json(&v.to_string()), Err(ThemeError::Version(99)));
    let mut v: serde_json::Value = serde_json::from_str(&export_json(&sample())).unwrap();
    v["theme"]["text"] = "nope".into();
    assert!(matches!(import_json(&v.to_string()), Err(ThemeError::Color { field: "text", .. })));
    let mut v: serde_json::Value = serde_json::from_str(&export_json(&sample())).unwrap();
    v["theme"].as_object_mut().unwrap().remove("surface");
    assert!(matches!(import_json(&v.to_string()), Err(ThemeError::Parse(_))));
}

#[test]
fn import_normalises_colours_from_hand_written_files() {
    let mut v: serde_json::Value = serde_json::from_str(&export_json(&sample())).unwrap();
    v["theme"]["accent"] = "f80".into();
    assert_eq!(import_json(&v.to_string()).unwrap().accent, "#FF8800");
}

#[test]
fn theme_file_defaults_to_empty() {
    assert!(ThemeFile::default().themes.is_empty());
    let f: ThemeFile = serde_json::from_str("{}").unwrap();
    assert!(f.themes.is_empty());
}

#[test]
fn a_themes_file_without_ids_gets_ids_once_and_keeps_them() {
    let dir = std::env::temp_dir().join(format!("ferrocast-themes-{}", uuid::Uuid::new_v4()));
    let store = crate::storage::Store::new(&dir);
    let old = |name: &str| {
        serde_json::json!({"name": name, "base": "Dark", "accent": "#38BDF8", "background": "#0B1220", "surface": "#16213A",
            "text": "#E6EEFF", "secondary_text": "#9FB0D0", "separator": "#26324D"})
    };
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(store.path("themes.json"), serde_json::json!({"themes": [old("A"), old("B")]}).to_string()).unwrap();
    let first = load_with_ids(&store);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|t| !t.id.is_empty()) && first[0].id != first[1].id);
    let saved: ThemeFile = store.load("themes.json");
    assert_eq!(saved.themes, first, "the ids were saved at once");
    assert_eq!(load_with_ids(&store), first, "a second launch sees the same ids");
    // a file that already has ids is left alone
    let before = std::fs::read(store.path("themes.json")).unwrap();
    load_with_ids(&store);
    assert_eq!(std::fs::read(store.path("themes.json")).unwrap(), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_theme_file_never_carries_an_id_and_import_and_duplicate_make_new_ones() {
    let t = sample();
    let json = export_json(&t);
    assert!(!json.contains("\"id\""), "the shared file has no id: {json}");
    let imported = import_json(&json).unwrap();
    assert!(!imported.id.is_empty() && imported.id != t.id);
    assert_ne!(import_json(&json).unwrap().id, imported.id, "every import is a new theme");
    let copy = t.duplicate_of(std::slice::from_ref(&t));
    assert!(!copy.id.is_empty() && copy.id != t.id);
}
