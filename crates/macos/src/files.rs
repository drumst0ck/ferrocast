//! File search (Spotlight via `mdfind`) and file icons.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Search files by name using Spotlight. Returns at most `limit` paths.
pub fn search(query: &str, limit: usize) -> Vec<PathBuf> {
    let q = query.trim();
    if q.len() < 2 {
        return vec![];
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let Ok(mut child) =
        Command::new("/usr/bin/mdfind").arg("-onlyin").arg(&home).arg("-name").arg(q).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
    else {
        return vec![];
    };
    let mut out = vec![];
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            // skip noise: hidden folders and Library caches
            if line.contains("/.") || line.contains("/Library/Caches") || line.contains("/node_modules/") {
                continue;
            }
            out.push(PathBuf::from(line));
            if out.len() >= limit * 4 {
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    // Prefer names that start with the query, then shorter paths.
    let ql = q.to_lowercase();
    out.sort_by_key(|p| {
        let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        (!name.starts_with(&ql), name != ql, p.components().count())
    });
    out.truncate(limit);
    out
}

/// Render the Finder icon of a path (app bundle, file, folder) as PNG bytes
/// at `size`×`size` pixels.
pub fn icon_png(path: &str, size: f64) -> Option<Vec<u8>> {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
    // Resolve symlinks (e.g. /Applications/Safari.app -> cryptex) so Finder does not draw the alias badge.
    let real = std::fs::canonicalize(path).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| path.to_string());
    let img = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&real));
    let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(size, size));
    let cg = unsafe { img.CGImageForProposedRect_context_hints(&mut rect, None, None) }?;
    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cg);
    let props = NSDictionary::new();
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) }?;
    Some(png.to_vec())
}
