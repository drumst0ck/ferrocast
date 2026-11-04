//! Moving an application to the Trash from the launcher: what may be moved, what the confirmation
//! says and how the app index changes afterwards. Pure, so every rule is tested without a Trash.

use ferrocast_core::i18n::trf;
use ferrocast_macos::apps::AppInfo;
use std::path::Path;

/// Bundle identifier of Ferrocast itself.
pub(crate) const OWN_BUNDLE_ID: &str = "dev.ferrocast.launcher";

/// Why an app is not offered to the Trash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The path cannot be resolved on disk any more.
    Gone,
    NotAnApp,
    System,
    AppleProtected,
    Ferrocast,
}

/// Folders macOS owns: nothing in them is ever offered.
/// Lowercase: APFS volumes are case-insensitive, so `/system` is `/System`.
const SYSTEM_PREFIXES: &[&str] = &["/system", "/library/apple", "/usr", "/bin", "/sbin", "/private/var/db"];

/// Apple apps that are not part of macOS: the user installed them, so the user may remove them.
const OPTIONAL_APPLE_IDS: &[&str] = &[
    "com.apple.dt.",
    "com.apple.iWork.",
    "com.apple.garageband",
    "com.apple.iMovieApp",
    "com.apple.logic",
    "com.apple.FinalCut",
    "com.apple.motionapp",
    "com.apple.Compressor",
    "com.apple.mainstage",
    "com.apple.configurator",
    "com.apple.Playgrounds",
    "com.apple.TestFlight",
];

/// The absolute path with `.` and `..` resolved and no trailing slash; `None` when it is relative.
fn normalize(path: &str) -> Option<String> {
    if !path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = vec![];
    for c in path.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            c => parts.push(c),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// May the bundle at `path` be moved to the Trash? `own_bundle` is the `.app` running this launcher.
pub(crate) fn eligibility(path: &str, bundle_id: Option<&str>, own_bundle: Option<&Path>) -> Result<(), Refusal> {
    let norm = normalize(path).ok_or(Refusal::NotAnApp)?;
    let parts: Vec<&str> = norm.split('/').filter(|c| !c.is_empty()).collect();
    let is_app = |c: &str| c.len() > 4 && c.ends_with(".app");
    // the last component is the bundle and no folder above it is another bundle
    if !parts.last().is_some_and(|c| is_app(c)) || parts[..parts.len() - 1].iter().any(|c| c.ends_with(".app")) {
        return Err(Refusal::NotAnApp);
    }
    let lower = norm.to_lowercase();
    if SYSTEM_PREFIXES.iter().any(|p| lower == *p || lower.starts_with(&format!("{p}/"))) {
        return Err(Refusal::System);
    }
    let own = own_bundle.and_then(|p| normalize(&p.to_string_lossy())).map(|p| p.to_lowercase());
    if bundle_id == Some(OWN_BUNDLE_ID) || own.as_deref() == Some(lower.as_str()) {
        return Err(Refusal::Ferrocast);
    }
    if bundle_id.is_some_and(|id| id.starts_with("com.apple.") && !OPTIONAL_APPLE_IDS.iter().any(|o| id.starts_with(o))) {
        return Err(Refusal::AppleProtected);
    }
    Ok(())
}

impl Refusal {
    /// The toast shown instead of a confirmation.
    pub(crate) fn message(self, name: &str) -> String {
        match self {
            Refusal::Gone => trf("“{name}” could not be found, so it cannot be moved to the Trash.", &[("name", name)]),
            Refusal::NotAnApp => trf("“{name}” is not an application bundle, so it cannot be moved to the Trash.", &[("name", name)]),
            Refusal::System => trf("“{name}” is part of macOS and cannot be moved to the Trash.", &[("name", name)]),
            Refusal::AppleProtected => trf("“{name}” is protected by macOS and cannot be moved to the Trash.", &[("name", name)]),
            Refusal::Ferrocast => {
                trf("“{name}” is the app you are using right now, so it cannot be moved to the Trash.", &[("name", name)])
            }
        }
    }
}

/// The text of the confirmation.
pub(crate) fn confirm_text(name: &str) -> String {
    trf("Move “{name}” to the Trash? Only the app is moved; you can put it back from the Trash.", &[("name", name)])
}

/// The name shown to the user for the bundle at `path` when the index does not know it.
pub(crate) fn display_name(path: &str) -> String {
    let file = path.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
    file.strip_suffix(".app").unwrap_or(file).to_string()
}

/// A fresh scan without the bundles trashed in this session (the scan may predate the move).
pub(crate) fn without_trashed(apps: Vec<AppInfo>, trashed: &std::collections::HashSet<std::path::PathBuf>) -> Vec<AppInfo> {
    apps.into_iter().filter(|a| !trashed.contains(&a.path) && !std::fs::canonicalize(&a.path).is_ok_and(|c| trashed.contains(&c))).collect()
}

/// The app index without the bundle at `path`.
pub(crate) fn without_app(apps: Vec<AppInfo>, path: &str) -> Vec<AppInfo> {
    let gone = path.trim_end_matches('/');
    apps.into_iter().filter(|a| a.path.to_string_lossy().trim_end_matches('/') != gone).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ok(p: &str) -> Result<(), Refusal> {
        eligibility(p, None, Some(Path::new("/Applications/Ferrocast.app")))
    }

    #[test]
    fn ordinary_apps_may_be_moved() {
        assert_eq!(ok("/Applications/Foo.app"), Ok(()));
        assert_eq!(ok("/Users/me/Applications/Foo.app/"), Ok(()));
        assert_eq!(eligibility("/Applications/Xcode.app", Some("com.apple.dt.Xcode"), None), Ok(()));
    }

    #[test]
    fn only_app_bundles_qualify() {
        assert_eq!(ok("/Users/me/Documents/readme.txt"), Err(Refusal::NotAnApp));
        assert_eq!(ok("/Applications/Foo.app/Contents/MacOS/foo"), Err(Refusal::NotAnApp));
        assert_eq!(ok("/Applications"), Err(Refusal::NotAnApp));
        assert_eq!(ok(""), Err(Refusal::NotAnApp));
        assert_eq!(ok("Foo.app"), Err(Refusal::NotAnApp), "relative paths never");
    }

    #[test]
    fn system_locations_are_refused() {
        assert_eq!(ok("/System/Applications/Calculator.app"), Err(Refusal::System));
        assert_eq!(ok("/System/Library/CoreServices/Finder.app"), Err(Refusal::System));
        assert_eq!(ok("/System/Volumes/Preboot/Cryptexes/App/System/Applications/Safari.app"), Err(Refusal::System));
        assert_eq!(ok("/Applications/../System/Applications/Mail.app"), Err(Refusal::System), "no dot-dot tricks");
        assert_eq!(ok("/system/applications/calculator.app"), Err(Refusal::System), "APFS ignores case");
        assert_eq!(ok("/SYSTEM/Library/CoreServices/Finder.app"), Err(Refusal::System));
    }

    #[test]
    fn apple_apps_the_system_protects_are_refused_but_optional_ones_are_not() {
        assert_eq!(eligibility("/Applications/Safari.app", Some("com.apple.Safari"), None), Err(Refusal::AppleProtected));
        assert_eq!(eligibility("/Applications/Utilities/Terminal.app", Some("com.apple.Terminal"), None), Err(Refusal::AppleProtected));
        assert_eq!(eligibility("/Applications/Pages.app", Some("com.apple.iWork.Pages"), None), Ok(()));
        assert_eq!(eligibility("/Applications/GarageBand.app", Some("com.apple.garageband10"), None), Ok(()));
    }

    #[test]
    fn ferrocast_itself_is_refused_by_path_or_bundle_id() {
        let own = Some(Path::new("/Applications/Ferrocast.app"));
        assert_eq!(eligibility("/Applications/Ferrocast.app", None, own), Err(Refusal::Ferrocast));
        assert_eq!(eligibility("/Applications/Ferrocast.app/", None, own), Err(Refusal::Ferrocast));
        assert_eq!(eligibility("/applications/ferrocast.app", None, own), Err(Refusal::Ferrocast), "case-insensitive");
        assert_eq!(eligibility("/Users/me/Old/Ferrocast.app", Some(OWN_BUNDLE_ID), None), Err(Refusal::Ferrocast));
    }

    #[test]
    fn texts_name_the_app() {
        let c = confirm_text("Foo");
        assert!(c.contains("Foo") && c.contains("Trash"), "{c}");
        for r in [Refusal::Gone, Refusal::NotAnApp, Refusal::System, Refusal::AppleProtected, Refusal::Ferrocast] {
            assert!(r.message("Foo").contains("Foo"), "{r:?}");
        }
        assert_eq!(display_name("/Applications/Foo Bar.app"), "Foo Bar");
    }

    #[test]
    fn a_rescan_drops_what_was_trashed() {
        let app =
            |n: &str| AppInfo { name: n.into(), path: PathBuf::from(format!("/Applications/{n}.app")), bundle_id: None, keywords: vec![] };
        let gone: std::collections::HashSet<PathBuf> = [PathBuf::from("/Applications/B.app")].into();
        let left = without_trashed(vec![app("A"), app("B")], &gone);
        assert_eq!(left.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["A"]);
        assert_eq!(without_trashed(vec![app("A")], &Default::default()).len(), 1);
    }

    #[test]
    fn the_index_loses_only_that_bundle() {
        let app =
            |n: &str| AppInfo { name: n.into(), path: PathBuf::from(format!("/Applications/{n}.app")), bundle_id: None, keywords: vec![] };
        let left = without_app(vec![app("A"), app("B"), app("C")], "/Applications/B.app/");
        assert_eq!(left.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["A", "C"]);
        assert_eq!(without_app(vec![app("A")], "/Applications/Z.app").len(), 1);
    }
}
