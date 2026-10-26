//! Single-instance lock and the on-disk error log.

use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

extern "C" {
    fn flock(fd: i32, op: i32) -> i32;
}
const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;

/// Take the exclusive instance lock; `None` means another copy is already running.
/// Keep the returned file alive for the whole process lifetime.
pub fn acquire(dir: &Path) -> Option<std::fs::File> {
    std::fs::create_dir_all(dir).ok()?;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("ferrocast.lock")).ok()?;
    (unsafe { flock(f.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0).then_some(f)
}

/// Like [`acquire`], but waits up to `wait` for the previous copy to exit. macOS's "Quit & Reopen"
/// after a permission grant starts the new copy while the old one is still shutting down.
pub fn acquire_wait(dir: &Path, wait: std::time::Duration) -> Option<std::fs::File> {
    let start = std::time::Instant::now();
    loop {
        if let Some(f) = acquire(dir) {
            return Some(f);
        }
        if start.elapsed() >= wait {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub fn log_path(dir: &Path) -> PathBuf {
    dir.join("ferrocast.log")
}

/// Append a timestamped line; the log is truncated once it passes 512 KiB.
pub fn log(dir: &Path, msg: &str) {
    let path = log_path(dir);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 512 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), msg.replace('\n', " | "));
    }
}

/// The resolved path when there is one, else the raw one.
pub fn pick_exe(raw: PathBuf, resolved: Option<PathBuf>) -> PathBuf {
    resolved.unwrap_or(raw)
}

/// Follow symlinks (e.g. `/Applications/Ferrocast.app` -> the Homebrew Cellar); the raw path when that fails.
pub fn resolve_exe(raw: PathBuf) -> PathBuf {
    let resolved = std::fs::canonicalize(&raw).ok();
    pick_exe(raw, resolved)
}

/// The real path of the running executable: the one place the updater derives channel and bundle from.
pub fn running_exe() -> Option<PathBuf> {
    std::env::current_exe().ok().map(resolve_exe)
}

/// The `.app` directory that contains the running executable (`…/Ferrocast.app/Contents/MacOS/ferrocast`).
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

/// After a self-update: drop the previous bundle and the download folder. Both are best effort.
pub fn cleanup_update_leftovers(store_root: &Path, exe: &Path) {
    if let Some(app) = bundle_of(exe) {
        if let Some(name) = app.file_name() {
            let previous = app.with_file_name(format!("{}.previous", name.to_string_lossy()));
            let _ = std::fs::remove_dir_all(previous);
        }
    }
    let _ = std::fs::remove_dir_all(store_root.join("updates"));
}

/// Send panics to the log (the app has no terminal when launched from Finder).
pub fn install_panic_hook(dir: PathBuf) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log(&dir, &format!("PANIC: {info}"));
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!("ferrocast-inst-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn second_instance_cannot_take_the_lock() {
        let d = tmp();
        let first = acquire(&d).expect("first instance locks");
        assert!(acquire(&d).is_none(), "second instance must be refused");
        drop(first);
        // Other tests fork child processes; until their `exec` the child briefly holds a copy of the
        // descriptor (and so the lock). Wait for release instead of asserting an instant.
        let released = (0..100).any(|_| {
            let ok = acquire(&d).is_some();
            if !ok {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            ok
        });
        assert!(released, "lock is released when the first instance exits");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn log_appends_single_lines_and_rotates() {
        let d = tmp();
        std::fs::create_dir_all(&d).unwrap();
        log(&d, "first\nmultiline");
        log(&d, "second");
        let text = std::fs::read_to_string(log_path(&d)).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("first | multiline"));
        std::fs::write(log_path(&d), vec![b'x'; 600 * 1024]).unwrap();
        log(&d, "after rotation");
        assert_eq!(std::fs::read_to_string(log_path(&d)).unwrap().lines().count(), 1);
        let _ = std::fs::remove_dir_all(d);
    }
}
