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

fn read_info(app: &Path) -> Option<AppInfo> {
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
    // Localized display name (e.g. System Settings) from InfoPlist.strings is
    // costly; the file stem is already the user-facing name for most apps.
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
