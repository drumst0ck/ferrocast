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

/// Move a path to the Trash through Finder (recoverable).
pub fn trash(path: &Path) -> anyhow::Result<()> {
    let p = path.to_string_lossy().replace('"', "\\\"");
    crate::system::osascript(&format!("tell application \"Finder\" to delete POSIX file \"{p}\""))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_system_apps() {
        let apps = super::list_apps();
        assert!(apps.iter().any(|a| a.name == "Safari" || a.name == "Calculator"), "{} apps", apps.len());
    }
}
