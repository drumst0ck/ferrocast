//! Installed application discovery and control.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: Option<String>,
    /// Alternate names (bundle name, executable, display name) for searching.
    pub keywords: Vec<String>,
}

fn search_roots() -> Vec<PathBuf> {
    let mut v = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
        PathBuf::from("/Applications/Utilities"),
        PathBuf::from("/System/Library/CoreServices/Applications"),
        PathBuf::from("/System/Library/CoreServices/Finder.app/Contents/Applications"),
    ];
    if let Some(h) = dirs::home_dir() {
        v.push(h.join("Applications"));
    }
    v
}

/// Display names of an app in the given preferred languages (e.g. "es-ES"), read from the bundle's
/// own localisation files, as Finder and Spotlight show them ("Captura de Pantalla" for
/// Screenshot.app). Never reads anything outside the bundle.
pub(crate) fn localized_names(app: &Path, languages: &[String]) -> Vec<String> {
    let res = app.join("Contents/Resources");
    let mut out: Vec<String> = Vec::new();
    let mut push = |dict: &dyn Fn(&str) -> Option<String>| {
        for key in ["CFBundleDisplayName", "CFBundleName"] {
            if let Some(n) = dict(key).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
    };
    // One locale tag as written on disk: "es-ES" and "es_ES" are the same, "es" matches "es_419".
    let base_of = |tag: &str| tag.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    let wanted: Vec<String> = languages.iter().map(|l| base_of(l)).filter(|b| !b.is_empty()).collect();
    if wanted.is_empty() {
        return out;
    }
    let loctable = plist::Value::from_file(res.join("InfoPlist.loctable")).ok();
    for want in &wanted {
        if let Some(dict) = loctable.as_ref().and_then(|v| v.as_dictionary()) {
            // exact language first ("es"), then regional variants ("es_419")
            let mut keys: Vec<&String> = dict.keys().filter(|k| base_of(k) == *want).collect();
            keys.sort_by_key(|k| (k.contains(['-', '_']), k.len()));
            for k in keys {
                if let Some(d) = dict.get(k).and_then(|v| v.as_dictionary()) {
                    push(&|key| d.get(key).and_then(|v| v.as_string()).map(String::from));
                }
            }
        }
        let Ok(rd) = std::fs::read_dir(&res) else { continue };
        let mut dirs: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "lproj"))
            .filter(|p| p.file_stem().is_some_and(|n| base_of(&n.to_string_lossy()) == *want))
            .collect();
        dirs.sort();
        for d in dirs {
            if let Some(strings) = read_strings_file(&d.join("InfoPlist.strings")) {
                push(&|key| strings.get(key).cloned());
            }
        }
    }
    out
}

/// `InfoPlist.strings` as key -> value: an XML/binary property list, or the classic
/// `"key" = "value";` text (UTF-8, or UTF-16 with a byte-order mark).
fn read_strings_file(path: &Path) -> Option<std::collections::HashMap<String, String>> {
    let bytes = std::fs::read(path).ok()?;
    if let Ok(v) = plist::Value::from_reader(std::io::Cursor::new(&bytes)) {
        let d = v.as_dictionary()?;
        return Some(d.iter().filter_map(|(k, v)| Some((k.clone(), v.as_string()?.to_string()))).collect());
    }
    let text = match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => {
            String::from_utf16(&rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()).ok()?
        }
        [0xFE, 0xFF, rest @ ..] => {
            String::from_utf16(&rest.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>()).ok()?
        }
        _ => String::from_utf8(bytes).ok()?,
    };
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        // "key" = "value";  (no escapes beyond \" are expected in application names)
        let Some(rest) = line.strip_prefix('"') else { continue };
        let Some((key, rest)) = rest.split_once("\" =") else { continue };
        let rest = rest.trim().trim_end_matches(';').trim();
        if let Some(val) = rest.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
            map.insert(key.to_string(), val.replace("\\\"", "\""));
        }
    }
    Some(map)
}

/// The user's preferred languages, most preferred first (e.g. ["es-ES", "en-US"]).
fn preferred_languages() -> Vec<String> {
    objc2_foundation::NSLocale::preferredLanguages().iter().map(|s| s.to_string()).collect()
}

fn read_info(app: &Path) -> Option<AppInfo> {
    read_info_with(app, &preferred_languages())
}

fn read_info_with(app: &Path, languages: &[String]) -> Option<AppInfo> {
    let plist_path = app.join("Contents/Info.plist");
    let fallback = app.file_stem()?.to_string_lossy().to_string();
    let mut info = AppInfo { name: fallback.clone(), path: app.to_path_buf(), bundle_id: None, keywords: vec![] };
    if let Ok(v) = plist::Value::from_file(&plist_path) {
        if let Some(d) = v.as_dictionary() {
            let get = |k: &str| d.get(k).and_then(|v| v.as_string()).map(String::from);
            info.bundle_id = get("CFBundleIdentifier");
            // Background-only / agent helpers are not launchable apps for users.
            if d.get("LSBackgroundOnly").and_then(|v| v.as_boolean()).unwrap_or(false) {
                return None;
            }
            for k in ["CFBundleDisplayName", "CFBundleName", "CFBundleExecutable"] {
                if let Some(n) = get(k) {
                    if !n.is_empty() && n != info.name && !info.keywords.contains(&n) {
                        info.keywords.push(n);
                    }
                }
            }
        }
    }
    // The name Finder shows in the user's language ("Captura de Pantalla"), so searching in that
    // language finds the app; the displayed name stays the file name.
    for n in localized_names(app, languages) {
        if !n.eq_ignore_ascii_case(&info.name) && !info.keywords.contains(&n) {
            info.keywords.push(n);
        }
    }
    Some(info)
}

fn scan_dir(dir: &Path, depth: usize, out: &mut Vec<AppInfo>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let is_app = p.extension().is_some_and(|x| x == "app");
        if is_app {
            // follow symlinks to real bundles
            let real = std::fs::canonicalize(&p).unwrap_or(p.clone());
            if real.is_dir() {
                if let Some(mut i) = read_info(&real) {
                    i.path = p.clone();
                    out.push(i);
                }
            }
        } else if depth > 0 && p.is_dir() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
            scan_dir(&p, depth - 1, out);
        }
    }
}

/// List installed apps (deduplicated by bundle id, then path), sorted by name.
pub fn list_apps() -> Vec<AppInfo> {
    let mut out = vec![];
    for r in search_roots() {
        scan_dir(&r, 2, &mut out);
    }
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_paths = std::collections::HashSet::new();
    out.retain(|a| {
        if !seen_paths.insert(a.path.clone()) {
            return false;
        }
        match &a.bundle_id {
            Some(id) => seen_ids.insert(id.clone()),
            None => true,
        }
    });
    // System Settings alternate names
    for a in out.iter_mut() {
        match a.bundle_id.as_deref() {
            Some("com.apple.systempreferences") => a.keywords.extend(["Preferences".into(), "System Preferences".into()]),
            Some("com.apple.iCal") => a.keywords.push("iCal".into()),
            _ => {}
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Open a file/app/URL with the default handler (or a specific app).
pub fn open(target: &str, with_app: Option<&str>) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("/usr/bin/open");
    if let Some(app) = with_app {
        cmd.arg("-a").arg(app);
    }
    let target = if let Some(rest) = target.strip_prefix("~/") {
        dirs::home_dir().unwrap_or_default().join(rest).to_string_lossy().to_string()
    } else {
        target.to_string()
    };
    let st = cmd.arg(&target).status()?;
    anyhow::ensure!(st.success(), "open failed for {target}");
    Ok(())
}

/// Reveal a path in Finder.
pub fn reveal(path: &Path) -> anyhow::Result<()> {
    std::process::Command::new("/usr/bin/open").arg("-R").arg(path).status()?;
    Ok(())
}

/// Existing absolute paths from newline-separated text, in order, without duplicates.
pub fn parse_finder_selection(out: &str) -> Vec<PathBuf> {
    let mut seen = Vec::new();
    for line in out.lines().map(str::trim).filter(|l| l.starts_with('/')) {
        let p = PathBuf::from(line);
        if p.exists() && !seen.contains(&p) {
            seen.push(p);
        }
    }
    seen
}

/// Files and folders selected in Finder right now (needs Automation permission for Finder).
pub fn finder_selection() -> anyhow::Result<Vec<PathBuf>> {
    let out = crate::system::osascript_timeout(
        "tell application \"Finder\"\nset out to \"\"\nrepeat with f in (selection as alias list)\nset out to out & POSIX path of f & linefeed\nend repeat\nreturn out\nend tell",
        15,
    )?;
    Ok(parse_finder_selection(&out))
}

/// Move a path to the Trash through Finder (recoverable).
pub fn trash(path: &Path) -> anyhow::Result<()> {
    let p = path.to_string_lossy().replace('"', "\\\"");
    crate::system::osascript(&format!("tell application \"Finder\" to delete POSIX file \"{p}\""))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn finder_selection_keeps_existing_absolute_paths_only() {
        let dir = std::env::temp_dir().join(format!("fc-fsel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a b.png");
        std::fs::write(&f, b"x").unwrap();
        let out = format!("{}\n\nrelative/x.png\n/nonexistent-fc-file.png\n{}\n{}\n", f.display(), f.display(), dir.display());
        let got = super::parse_finder_selection(&out);
        assert_eq!(got, vec![f.clone(), dir.clone()], "blank, relative, missing and duplicate lines are dropped");
        assert!(super::parse_finder_selection("").is_empty());
        let _ = std::fs::remove_file(&f);
        let _ = std::fs::remove_dir(&dir);
    }

    fn langs(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    /// Real system data: Screenshot.app ships its Spanish name in InfoPlist.loctable. The macOS
    /// process itself reports the English one (a plain process has no Spanish localisation), which
    /// is why Ferrocast has to read the file.
    #[test]
    fn screenshot_app_is_known_by_its_spanish_name_for_a_spanish_user() {
        let app = std::path::Path::new("/System/Applications/Utilities/Screenshot.app");
        if !app.join("Contents/Resources/InfoPlist.loctable").exists() {
            return;
        }
        let es = super::localized_names(app, &langs(&["es-ES"]));
        assert!(es.iter().any(|n| n == "Captura de Pantalla"), "{es:?}");
        let en = super::localized_names(app, &langs(&["en-US"]));
        assert!(!en.iter().any(|n| n == "Captura de Pantalla"), "an English user does not get the Spanish name: {en:?}");
        assert!(super::localized_names(app, &[]).is_empty(), "no preferred language, no names");
    }

    #[test]
    fn read_info_adds_the_localized_name_as_a_search_keyword() {
        let app = std::path::Path::new("/System/Applications/Utilities/Screenshot.app");
        if !app.join("Contents/Resources/InfoPlist.loctable").exists() {
            return;
        }
        let info = super::read_info_with(app, &langs(&["es-ES"])).unwrap();
        assert_eq!(info.name, "Screenshot", "the displayed name stays the file name");
        assert!(info.keywords.iter().any(|k| k == "Captura de Pantalla"), "{:?}", info.keywords);
    }

    #[test]
    fn localized_names_come_from_lproj_strings_in_xml_and_text_form() {
        let base = std::env::temp_dir().join(format!("fc-loc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let res = base.join("Fake.app/Contents/Resources");
        std::fs::create_dir_all(res.join("es.lproj")).unwrap();
        std::fs::create_dir_all(res.join("de.lproj")).unwrap();
        std::fs::create_dir_all(res.join("fr_CA.lproj")).unwrap();
        std::fs::write(
            res.join("es.lproj/InfoPlist.strings"),
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleDisplayName</key><string>Cámara Falsa</string></dict></plist>"#,
        )
        .unwrap();
        std::fs::write(res.join("de.lproj/InfoPlist.strings"), "/* comment */\n\"CFBundleName\" = \"Falsche Kamera\";\n").unwrap();
        std::fs::write(res.join("fr_CA.lproj/InfoPlist.strings"), "\"CFBundleDisplayName\" = \"Fausse caméra\";").unwrap();
        let app = base.join("Fake.app");
        assert_eq!(super::localized_names(&app, &langs(&["es-ES"])), vec!["Cámara Falsa".to_string()]);
        assert_eq!(super::localized_names(&app, &langs(&["de"])), vec!["Falsche Kamera".to_string()]);
        assert_eq!(super::localized_names(&app, &langs(&["fr-FR"])), vec!["Fausse caméra".to_string()], "same base language, other region");
        assert!(super::localized_names(&app, &langs(&["ja-JP"])).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn finds_system_apps() {
        let apps = super::list_apps();
        assert!(apps.iter().any(|a| a.name == "Safari" || a.name == "Calculator"), "{} apps", apps.len());
    }
}

/// Open the AirDrop recipient picker for these files. Call on the main thread.
pub fn airdrop(paths: &[PathBuf]) -> anyhow::Result<()> {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSSharingService, NSSharingServiceNameSendViaAirDrop};
    use objc2_foundation::NSArray;
    let service = NSSharingService::sharingServiceNamed(unsafe { NSSharingServiceNameSendViaAirDrop })
        .ok_or_else(|| anyhow::anyhow!("AirDrop is not available on this Mac"))?;
    let urls: Vec<Retained<AnyObject>> =
        paths.iter().filter_map(|p| crate::clipboard::ns_file_url(p)).map(|u| Retained::into_super(Retained::into_super(u))).collect();
    if urls.is_empty() {
        anyhow::bail!("There is nothing to share");
    }
    let items = NSArray::from_retained_slice(&urls);
    if !unsafe { service.canPerformWithItems(Some(&items)) } {
        anyhow::bail!("AirDrop cannot share these items");
    }
    unsafe { service.performWithItems(&items) };
    Ok(())
}

/// The folder to open in a terminal: the path itself if it is a folder, else its parent.
pub fn terminal_dir(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/"))
    }
}

/// Open a folder (or a file's folder) in iTerm2 if installed, otherwise Terminal.
pub fn open_in_terminal(path: &Path) -> anyhow::Result<&'static str> {
    let dir = terminal_dir(path);
    let app = if Path::new("/Applications/iTerm.app").exists() { "iTerm" } else { "Terminal" };
    let st = std::process::Command::new("/usr/bin/open").args(["-a", app]).arg(&dir).status()?;
    if !st.success() {
        anyhow::bail!("could not open {app}");
    }
    Ok(app)
}

/// Move items into `dest`, never overwriting: a clash gets " 2", " 3"… before the extension.
/// Returns the new paths.
pub fn move_into(paths: &[PathBuf], dest: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = vec![];
    for p in paths {
        let name = p.file_name().ok_or_else(|| anyhow::anyhow!("invalid path"))?;
        let mut target = dest.join(name);
        let (stem, ext) = (
            p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default(),
        );
        let mut n = 2;
        while target.exists() {
            target = dest.join(format!("{stem} {n}{ext}"));
            n += 1;
        }
        if std::fs::rename(p, &target).is_err() {
            // another volume: let mv copy and delete
            let st = std::process::Command::new("/bin/mv").arg(p).arg(&target).status()?;
            if !st.success() {
                anyhow::bail!("could not move {}", p.display());
            }
        }
        out.push(target);
    }
    Ok(out)
}

/// Native folder chooser (blocks).
pub fn choose_folder(prompt: &str) -> Option<PathBuf> {
    let out = crate::system::osascript(&format!("POSIX path of (choose folder with prompt \"{}\")", prompt.replace('"', ""))).ok()?;
    (!out.trim().is_empty()).then(|| PathBuf::from(out.trim()))
}

#[cfg(test)]
mod file_action_tests {
    use super::*;

    #[test]
    fn move_into_renames_on_clash_and_terminal_dir() {
        let base = std::env::temp_dir().join(format!("fc-move-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (src, dst) = (base.join("src"), base.join("dst"));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(src.join("a.txt"), "new").unwrap();
        std::fs::write(dst.join("a.txt"), "old").unwrap();
        let out = move_into(&[src.join("a.txt")], &dst).unwrap();
        assert_eq!(out, vec![dst.join("a 2.txt")]);
        assert!(!src.join("a.txt").exists());
        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "old");
        assert_eq!(std::fs::read_to_string(&out[0]).unwrap(), "new");
        assert_eq!(terminal_dir(&dst.join("a.txt")), dst);
        assert_eq!(terminal_dir(&dst), dst);
    }
}
