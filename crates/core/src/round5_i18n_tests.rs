//! Bugs round 5, i18n: error texts built in the core reach the user through a toast or a card,
//! which show them as they are. They must already be in the interface language.

use crate::i18n::{tr, with_language, Lang};

/// `produce()` in English and as a control shows it in German: it must differ from the English one.
fn shown_in_german(label: &str, produce: impl Fn() -> String) -> String {
    let en = with_language(Lang::En, &produce);
    let de = with_language(Lang::De, || {
        let s = produce();
        // what a control does with it: a text already in German stays, an English one is looked up
        tr(&s).into_owned()
    });
    assert_ne!(de, en, "{label}: still English ({en:?})");
    de
}

#[test]
fn theme_errors_follow_the_language() {
    use crate::themes::ThemeError;
    for e in [
        ThemeError::Name,
        ThemeError::Color { field: "accent", value: "red".into() },
        ThemeError::Format,
        ThemeError::Version(9),
        ThemeError::Parse("expected value".into()),
    ] {
        shown_in_german(&format!("{e:?}"), || e.to_string());
    }
    let de = with_language(Lang::De, || ThemeError::Version(9).to_string());
    assert!(de.contains('9'), "{de}");
}

#[test]
fn snippet_import_errors_follow_the_language() {
    use crate::import::parse_snippets;
    shown_in_german("not json", || parse_snippets("{").unwrap_err());
    shown_in_german("not an array", || parse_snippets("{}").unwrap_err());
    shown_in_german("empty", || parse_snippets("[]").unwrap_err());
}

#[test]
fn export_plan_errors_follow_the_language() {
    use crate::capture::export::PlanError;
    for e in [
        PlanError::NothingToExport,
        PlanError::InvalidTimeline("overlap".into()),
        PlanError::BadFps(99),
        PlanError::GifTooLong { frames: 900, max_seconds: 30.0 },
    ] {
        shown_in_german(&format!("{e:?}"), || e.to_string());
    }
}

#[test]
fn loupe_and_cut_errors_follow_the_language() {
    use crate::capture::project::Project;
    use crate::capture::{transcript::apply_cuts, zoom::add_loupe};
    let mut p = Project::new("t", "2026-01-01T00:00:00Z");
    p.duration = 10.0;
    assert!(add_loupe(&mut p, 1.0).is_ok());
    shown_in_german("loupe twice", || add_loupe(&mut p.clone(), 1.0).unwrap_err());
    shown_in_german("loupe at the end", || add_loupe(&mut p.clone(), 10.0).unwrap_err());
    shown_in_german("cut everything", || apply_cuts(&p, &[(0.0, 10.0)]).unwrap_err());
}

#[test]
fn the_sync_folder_error_follows_the_language() {
    let de = shown_in_german("sync folder", || crate::sync_io::Unavailable::Folder("read-only".into()).message());
    assert!(de.contains("read-only"), "{de}");
}
