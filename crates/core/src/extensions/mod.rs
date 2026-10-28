//! Raycast extensions support: finding Node, reading installed manifests, and the
//! JSON protocol and view model shared with the `extensions-runtime` sidecar. No I/O beyond what each
//! function documents; the app owns the process supervision.

pub mod hostcalls;
pub mod manifest;
pub mod node;
pub mod protocol;
pub mod sidecar;
pub mod storage;
pub mod view;

/// The bundled JavaScript runtime (built from `extensions-runtime/`, committed as dist/runtime.js).
pub const RUNTIME_JS: &str = include_str!("../../../../extensions-runtime/dist/runtime.js");

/// Write the embedded runtime to `<data_dir>/runtime/extensions-runtime.js` (only when its content
/// differs) and return the path.
pub fn write_runtime(data_dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let dir = data_dir.join("runtime");
    std::fs::create_dir_all(&dir)?;
    let p = dir.join("extensions-runtime.js");
    if std::fs::read_to_string(&p).ok().as_deref() != Some(RUNTIME_JS) {
        std::fs::write(&p, RUNTIME_JS)?;
    }
    Ok(p)
}

/// Refuse `open` requests that could smuggle options into /usr/bin/open (a target or application
/// starting with `-`) or that are empty.
pub fn validate_open(target: &str, application: Option<&str>) -> Result<(), String> {
    let bad = |s: &str| s.trim().is_empty() || s.trim_start().starts_with('-');
    if bad(target) {
        return Err("open: invalid target".into());
    }
    if application.is_some_and(bad) {
        return Err("open: invalid application".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_rejects_option_like_and_empty_arguments() {
        assert!(validate_open("https://example.test", None).is_ok());
        assert!(validate_open("/Users/me/file.txt", Some("Safari")).is_ok());
        assert!(validate_open("-a", None).is_err());
        assert!(validate_open("  --args", None).is_err());
        assert!(validate_open("", None).is_err());
        assert!(validate_open("https://x", Some("-n")).is_err());
        assert!(validate_open("https://x", Some("")).is_err());
    }
}
