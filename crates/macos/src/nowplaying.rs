//! Feed of the notch player: runs the bundled `ungive/mediaremote-adapter` through the system perl
//! (`/usr/bin/perl <script> <framework> stream`), off the main thread, and turns its JSON lines into
//! `Event`s. The framework is only passed to perl as an argument; it is never linked into Ferrocast.
//!
//! * The child is killed when the [`Poller`] is dropped (setting off, app quit).
//! * If it exits it is restarted with backoff; after repeated failures it is given up on and one
//!   `Event::Unavailable` is sent (`ferrocast_core::nowplaying_feed::Supervisor`).
//! * Nothing else is touched: no AppleScript, no permission prompt, no network, no temp files.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as Proc, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ferrocast_core::nowplaying::{Command, Event};
use ferrocast_core::nowplaying_feed::{self as feed, Next, Supervisor, Tracker};

/// Variable that points to a directory holding `mediaremote-adapter.pl` and
/// `MediaRemoteAdapter.framework` (a dev override).
pub const DIR_ENV: &str = "FERROCAST_MEDIAREMOTE_DIR";
const SCRIPT: &str = "mediaremote-adapter.pl";
const FRAMEWORK: &str = "MediaRemoteAdapter.framework";

/// Where the helper lives.
#[derive(Clone, Debug, PartialEq)]
pub struct Paths {
    pub script: PathBuf,
    pub framework: PathBuf,
}

/// Find the helper. With `dev` (debug builds) first the `env` directory, then, after the bundle, the build tree. In order: `env` directory; the app bundle (`Contents/Resources/` for the script,
/// `Contents/Frameworks/` for the framework, next to `exe` in `Contents/MacOS/`); a development
/// build, `<target>/mediaremote-adapter/` made by `scripts/bundle.sh`, found by walking up from `exe`.
/// `None` when it is nowhere: the feature is then unavailable (never a crash).
pub fn resolve(exe: &Path, env: Option<&Path>, dev: bool, exists: impl Fn(&Path) -> bool) -> Option<Paths> {
    let pair = |script_dir: &Path, fw_dir: &Path| {
        let p = Paths { script: script_dir.join(SCRIPT), framework: fw_dir.join(FRAMEWORK) };
        (exists(&p.script) && exists(&p.framework)).then_some(p)
    };
    // the override and the build-tree copy are for development only: a release build loads the helper
    // that is sealed inside its own bundle and nothing else
    if let Some(d) = env.filter(|_| dev) {
        if let Some(p) = pair(d, d) {
            return Some(p);
        }
    }
    let dir = exe.parent()?;
    if let Some(contents) = dir.parent() {
        if let Some(p) = pair(&contents.join("Resources"), &contents.join("Frameworks")) {
            return Some(p);
        }
    }
    if !dev {
        return None;
    }
    dir.ancestors().take(4).find_map(|a| {
        let d = a.join("mediaremote-adapter");
        pair(&d, &d)
    })
}

/// The helper for this process.
pub fn locate() -> Option<Paths> {
    let exe = std::env::current_exe().ok()?;
    let env = std::env::var_os(DIR_ENV).map(PathBuf::from);
    resolve(&exe, env.as_deref(), cfg!(debug_assertions), |p| p.exists())
}

fn epoch_micros() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_micros() as f64)
}

/// A window as CGWindowList reports it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WinRect {
    pub layer: i64,
    pub pid: i32,
    /// x, y, w, h in top-left display points.
    pub rect: [f64; 4],
}

/// True when another app has a normal window over the whole `display` (x, y, w, h): macOS full
/// screen, a game, a presentation. A zoomed window leaves the menu bar showing and does not count.
pub fn covers_display(wins: &[WinRect], display: [f64; 4], own_pid: i32) -> bool {
    const SLACK: f64 = 1.5;
    wins.iter().any(|w| {
        w.layer == 0
            && w.pid != own_pid
            && w.rect[0] <= display[0] + SLACK
            && w.rect[1] <= display[1] + SLACK
            && w.rect[0] + w.rect[2] >= display[0] + display[2] - SLACK
            && w.rect[1] + w.rect[3] >= display[1] + display[3] - SLACK
    })
}

/// The player's full-screen rule. Without an auto-hidden menu bar it is [`covers_display`]. With one
/// (System Settings → Control Center → "Automatically hide and show the menu bar", `_HIHideMenuBar`)
/// a zoomed window also fills the whole display, so a covering window only counts when no other
/// normal window of any app is on screen next to it, as in a native full-screen Space. The menu bar
/// itself, revealed at the top edge, is a layer 25 window and never counts.
pub fn is_fullscreen(wins: &[WinRect], display: [f64; 4], own_pid: i32, menu_bar_auto_hidden: bool) -> bool {
    if !covers_display(wins, display, own_pid) {
        return false;
    }
    if !menu_bar_auto_hidden {
        return true;
    }
    // helper windows (tool tips, badges) are small; real windows are not
    let big = wins.iter().filter(|w| w.layer == 0 && w.pid != own_pid && w.rect[2] >= 200.0 && w.rect[3] >= 200.0).count();
    big == 1
}

/// True when macOS hides the menu bar until the pointer reaches the top edge.
fn menu_bar_auto_hidden() -> bool {
    use objc2_foundation::{NSString, NSUserDefaults};
    NSUserDefaults::standardUserDefaults().boolForKey(&NSString::from_str("_HIHideMenuBar"))
}

/// Whether a full-screen window covers the main display right now (CGWindowList: no permission).
fn fullscreen_now() -> bool {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::window::*;
    let Some(arr) = copy_window_info(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID) else {
        return false;
    };
    let num = |d: &CFDictionary<CFString, CFType>, k: &str| {
        d.find(CFString::new(k)).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_f64())
    };
    let mut wins = vec![];
    for item in arr.iter() {
        let dict: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(*item as _) };
        let (Some(layer), Some(pid)) = (num(&dict, "kCGWindowLayer"), num(&dict, "kCGWindowOwnerPID")) else { continue };
        let Some(b) = dict.find(CFString::new("kCGWindowBounds")) else { continue };
        let b: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(b.as_CFTypeRef() as _) };
        let (Some(x), Some(y), Some(w), Some(h)) = (num(&b, "X"), num(&b, "Y"), num(&b, "Width"), num(&b, "Height")) else { continue };
        wins.push(WinRect { layer: layer as i64, pid: pid as i32, rect: [x, y, w, h] });
    }
    let d = core_graphics::display::CGDisplay::main().bounds();
    is_fullscreen(&wins, [d.origin.x, d.origin.y, d.size.width, d.size.height], std::process::id() as i32, menu_bar_auto_hidden())
}

/// Result of reading one line.
#[derive(Debug, PartialEq)]
enum LineRead {
    Eof,
    Line,
    /// Longer than the cap: it was consumed to its newline and not kept.
    TooLong,
}

/// Read one line (without its `\n`) into `buf`, keeping at most `max` bytes: a longer line is
/// skipped to its end instead of being buffered, so a runaway helper cannot grow our memory.
fn read_bounded_line(r: &mut impl BufRead, max: usize, buf: &mut Vec<u8>) -> std::io::Result<LineRead> {
    buf.clear();
    let (mut any, mut too_long) = (false, false);
    loop {
        let chunk = r.fill_buf()?;
        if chunk.is_empty() {
            return Ok(if !any {
                LineRead::Eof
            } else if too_long {
                LineRead::TooLong
            } else {
                LineRead::Line
            });
        }
        any = true;
        let (take, done) = match chunk.iter().position(|b| *b == b'\n') {
            Some(i) => (i, true),
            None => (chunk.len(), false),
        };
        if !too_long {
            if buf.len() + take > max {
                too_long = true;
                buf.clear();
            } else {
                buf.extend_from_slice(&chunk[..take]);
            }
        }
        r.consume(take + usize::from(done));
        if done {
            return Ok(if too_long { LineRead::TooLong } else { LineRead::Line });
        }
    }
}

/// What to run: a program and its arguments (perl and the stream command, or a stand-in in tests).
#[derive(Clone, Debug)]
pub struct Spec {
    pub program: String,
    pub args: Vec<String>,
}

impl Spec {
    fn stream(p: &Paths) -> Spec {
        Spec { program: feed::PERL.into(), args: feed::stream_args(&p.script.to_string_lossy(), &p.framework.to_string_lossy()) }
    }
}

type Slot = Arc<Mutex<Option<Child>>>;

/// Keeps the helper running; dropping it kills the child and ends the threads.
pub struct Poller {
    cancel: Arc<AtomicBool>,
    child: Slot,
    /// Wakes the backoff sleep so a drop does not wait for it.
    wake: Option<Sender<()>>,
}

impl Poller {
    /// A poller with nothing behind it (tests).
    pub fn detached() -> Poller {
        Poller { cancel: Arc::new(AtomicBool::new(false)), child: Arc::new(Mutex::new(None)), wake: None }
    }

    /// Pid of the running child, if any.
    pub fn child_pid(&self) -> Option<u32> {
        self.child.lock().ok()?.as_ref().map(Child::id)
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut g) = self.child.lock() {
            if let Some(c) = g.as_mut() {
                let _ = c.kill();
            }
        }
        if let Some(w) = &self.wake {
            let _ = w.send(());
        }
    }
}

/// Start the feed: the helper stream plus a watcher for full-screen apps. `on_event` runs on those
/// threads. When the helper cannot be found, `Event::Unavailable` is sent at once.
pub fn spawn(on_event: Arc<dyn Fn(Event) + Send + Sync>) -> Poller {
    let Some(paths) = locate() else {
        on_event(Event::Unavailable);
        return Poller::detached();
    };
    let poller = spawn_with(Spec::stream(&paths), on_event.clone(), 1.0);
    let flag = poller.cancel.clone();
    let _ = std::thread::Builder::new().name("ferrocast-nowplaying-fs".into()).spawn(move || {
        let mut last = false;
        while !flag.load(Ordering::SeqCst) {
            let now = fullscreen_now();
            if now != last {
                last = now;
                on_event(Event::Fullscreen(now));
            }
            // 2 s, in slices so a drop is noticed quickly
            for _ in 0..10 {
                if flag.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    });
    poller
}

/// `spawn` for any `spec`; `time_scale` multiplies the backoff delays (tests use a tiny one).
pub fn spawn_with(spec: Spec, on_event: Arc<dyn Fn(Event) + Send + Sync>, time_scale: f64) -> Poller {
    let cancel = Arc::new(AtomicBool::new(false));
    let child: Slot = Arc::new(Mutex::new(None));
    let (wake_tx, wake_rx) = channel::<()>();
    let (flag, slot, on_event_thread) = (cancel.clone(), child.clone(), on_event.clone());
    let spawned = std::thread::Builder::new().name("ferrocast-nowplaying".into()).spawn(move || {
        let mut sup = Supervisor::new();
        while !flag.load(Ordering::SeqCst) {
            let started = Instant::now();
            let got_data = run_once(&spec, &slot, &flag, on_event_thread.as_ref());
            if flag.load(Ordering::SeqCst) {
                return;
            }
            // the picture it drew is stale now
            on_event_thread(Event::Media(None));
            match sup.on_exit(started.elapsed().as_secs_f64(), got_data) {
                Next::GiveUp => {
                    on_event_thread(Event::Unavailable);
                    return;
                }
                Next::Restart(secs) => match wake_rx.recv_timeout(Duration::from_secs_f64((secs * time_scale).max(0.0))) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) => {}
                },
            }
        }
    });
    if spawned.is_err() {
        on_event(Event::Unavailable);
        return Poller::detached();
    }
    Poller { cancel, child, wake: Some(wake_tx) }
}

/// Run the helper once and read it until it ends. True when it printed at least one valid line.
fn run_once(spec: &Spec, slot: &Slot, cancel: &AtomicBool, on_event: &(dyn Fn(Event) + Send + Sync)) -> bool {
    let mut cmd = Proc::new(&spec.program);
    cmd.args(&spec.args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else { return false };
    let Some(out) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return false;
    };
    {
        let Ok(mut g) = slot.lock() else { return false };
        // a drop that came while spawning must still kill this child
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
        *g = Some(child);
    }
    let mut tracker = Tracker::new();
    let mut got = false;
    let mut last: Option<Option<ferrocast_core::nowplaying::Snapshot>> = None;
    let mut reader = BufReader::new(out);
    let mut buf = Vec::new();
    loop {
        match read_bounded_line(&mut reader, feed::MAX_LINE_BYTES, &mut buf) {
            Ok(LineRead::Eof) | Err(_) => break,
            Ok(LineRead::TooLong) => continue,
            Ok(LineRead::Line) => {}
        }
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        if tracker.feed(&String::from_utf8_lossy(&buf)) {
            got = true;
            let snap = tracker.snapshot(epoch_micros());
            // derive the disc, the halo and the tint here, off the main thread, once per cover
            if let Some(art) = snap.as_ref().and_then(|s| s.artwork.as_ref()) {
                crate::nowplaying_art::ensure(art);
            }
            // a progress-only update would redraw nothing: let the state machine see changes only
            if last.as_ref() != Some(&snap) {
                last = Some(snap.clone());
                on_event(Event::Media(snap));
            }
        }
    }
    if let Ok(mut g) = slot.lock() {
        if let Some(mut c) = g.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
    got
}

/// Send a transport button to whatever the system is playing. Blocks (up to a few seconds), so call
/// it off the main thread. False when the helper is missing, fails or times out.
pub fn send_command(command: Command) -> bool {
    let Some(p) = locate() else { return false };
    send_with(feed::PERL, feed::send_args(&p.script.to_string_lossy(), &p.framework.to_string_lossy(), command), Duration::from_secs(5))
}

/// Seek the player to `secs`. Blocks like [`send_command`] (same 5 s limit): call it off the main thread.
pub fn send_seek(secs: f64) -> bool {
    let Some(p) = locate() else { return false };
    send_with(feed::PERL, feed::seek_args(&p.script.to_string_lossy(), &p.framework.to_string_lossy(), secs), Duration::from_secs(5))
}

/// Bring the app with this bundle id forward (for a browser, the browser). Does nothing when the id
/// is not plausible or the app is not running. Call it off the main thread.
pub fn open_app(bundle_id: &str) -> bool {
    use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration};
    use objc2_foundation::NSString;
    let Some(id) = ferrocast_core::nowplaying_input::open_target(bundle_id) else { return false };
    let id = NSString::from_str(id);
    // only an app that is running (it is the one that plays); never launch anything
    if NSRunningApplication::runningApplicationsWithBundleIdentifier(&id).iter().next().is_none() {
        return false;
    }
    let ws = NSWorkspace::sharedWorkspace();
    let Some(url) = ws.URLForApplicationWithBundleIdentifier(&id) else { return false };
    // opening a running app activates it (a browser comes forward with its windows); the
    // "ignoring other apps" activation option no longer does anything since macOS 14
    let cfg = NSWorkspaceOpenConfiguration::configuration();
    cfg.setActivates(true);
    ws.openApplicationAtURL_configuration_completionHandler(&url, &cfg, None);
    true
}

fn send_with(program: &str, args: Vec<String>, limit: Duration) -> bool {
    let Ok(mut child) = Proc::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
        return false;
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return st.success(),
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::mpsc::Receiver;

    fn sh(script: &str) -> Spec {
        Spec { program: "/bin/sh".into(), args: vec!["-c".into(), script.into()] }
    }

    fn events() -> (Arc<dyn Fn(Event) + Send + Sync>, Receiver<Event>) {
        let (tx, rx) = channel();
        let tx = Mutex::new(tx);
        (Arc::new(move |e| drop(tx.lock().unwrap().send(e))), rx)
    }

    fn alive(pid: u32) -> bool {
        Proc::new("/bin/kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
    }

    #[test]
    fn paths_are_found_in_the_bundle_in_the_env_dir_or_the_dev_target() {
        let have: HashSet<&str> = [
            "/A.app/Contents/Resources/mediaremote-adapter.pl",
            "/A.app/Contents/Frameworks/MediaRemoteAdapter.framework",
            "/t/mediaremote-adapter/mediaremote-adapter.pl",
            "/t/mediaremote-adapter/MediaRemoteAdapter.framework",
            "/e/mediaremote-adapter.pl",
            "/e/MediaRemoteAdapter.framework",
        ]
        .into();
        let ex = |p: &Path| have.contains(p.to_str().unwrap());
        let bundle = resolve(Path::new("/A.app/Contents/MacOS/ferrocast"), None, true, ex).unwrap();
        assert_eq!(bundle.script, Path::new("/A.app/Contents/Resources/mediaremote-adapter.pl"));
        assert_eq!(bundle.framework, Path::new("/A.app/Contents/Frameworks/MediaRemoteAdapter.framework"));
        let dev = resolve(Path::new("/t/debug/ferrocast"), None, true, ex).unwrap();
        assert_eq!(dev.script, Path::new("/t/mediaremote-adapter/mediaremote-adapter.pl"));
        assert!(resolve(Path::new("/t/debug/deps/ferrocast-1a2b"), None, true, ex).is_some(), "test binaries sit one level deeper");
        let env = resolve(Path::new("/A.app/Contents/MacOS/ferrocast"), Some(Path::new("/e")), true, ex).unwrap();
        assert_eq!(env.script, Path::new("/e/mediaremote-adapter.pl"), "the override wins");
        // an env dir with only half of the pair is ignored
        assert_eq!(resolve(Path::new("/A.app/Contents/MacOS/ferrocast"), Some(Path::new("/nope")), true, ex).unwrap(), bundle);
    }

    #[test]
    fn a_release_build_only_loads_the_copy_inside_its_own_bundle() {
        let have: HashSet<&str> = [
            "/A.app/Contents/Resources/mediaremote-adapter.pl",
            "/A.app/Contents/Frameworks/MediaRemoteAdapter.framework",
            "/t/mediaremote-adapter/mediaremote-adapter.pl",
            "/t/mediaremote-adapter/MediaRemoteAdapter.framework",
            "/e/mediaremote-adapter.pl",
            "/e/MediaRemoteAdapter.framework",
        ]
        .into();
        let ex = |p: &Path| have.contains(p.to_str().unwrap());
        let bundle = Path::new("/A.app/Contents/MacOS/ferrocast");
        let got = resolve(bundle, Some(Path::new("/e")), false, ex).unwrap();
        assert_eq!(got.script, Path::new("/A.app/Contents/Resources/mediaremote-adapter.pl"), "the env override is ignored");
        // outside a bundle, a release build finds nothing: no build-tree fallback, no override
        assert_eq!(resolve(Path::new("/t/release/ferrocast"), None, false, ex), None);
        assert_eq!(resolve(Path::new("/t/release/ferrocast"), Some(Path::new("/e")), false, ex), None);
        assert!(resolve(Path::new("/t/release/ferrocast"), Some(Path::new("/e")), true, ex).is_some(), "a debug build may");
    }

    #[test]
    fn lines_are_read_whole_and_an_overlong_one_is_skipped_without_growing_memory() {
        let data = format!("one\n{}\ntwo\r\nlast", "x".repeat(10_000));
        let mut r = std::io::Cursor::new(data.into_bytes());
        let mut buf = Vec::new();
        assert!(matches!(read_bounded_line(&mut r, 100, &mut buf).unwrap(), LineRead::Line));
        assert_eq!(buf, b"one");
        assert!(matches!(read_bounded_line(&mut r, 100, &mut buf).unwrap(), LineRead::TooLong));
        assert!(buf.len() <= 101, "nothing beyond the cap is kept: {}", buf.len());
        assert!(matches!(read_bounded_line(&mut r, 100, &mut buf).unwrap(), LineRead::Line));
        assert_eq!(buf, b"two\r");
        assert!(matches!(read_bounded_line(&mut r, 100, &mut buf).unwrap(), LineRead::Line), "no final newline");
        assert_eq!(buf, b"last");
        assert!(matches!(read_bounded_line(&mut r, 100, &mut buf).unwrap(), LineRead::Eof));
        // a line of exactly the cap is fine
        let mut r = std::io::Cursor::new(format!("{}\n", "y".repeat(100)).into_bytes());
        assert!(matches!(read_bounded_line(&mut r, 100, &mut buf).unwrap(), LineRead::Line));
    }

    #[test]
    fn an_overlong_line_from_the_child_is_dropped_and_the_next_one_still_works() {
        let (cb, rx) = events();
        let line = r#"{"type":"data","diff":false,"payload":{"bundleIdentifier":"a.b","title":"T","playing":false}}"#;
        // 9 MB of junk on one line (above the 8 MB cap), then a good line
        let p = spawn_with(sh(&format!("head -c 9000000 /dev/zero | tr '\\0' x; echo; echo '{line}'; sleep 30")), cb, 1.0);
        match rx.recv_timeout(Duration::from_secs(20)).unwrap() {
            Event::Media(Some(s)) => assert_eq!(s.title, "T"),
            e => panic!("{e:?}"),
        }
        drop(p);
    }

    #[test]
    fn nothing_found_means_unavailable_not_a_crash() {
        assert_eq!(resolve(Path::new("/x/y/ferrocast"), None, true, |_| false), None);
        assert_eq!(resolve(Path::new("ferrocast"), None, true, |_| false), None);
    }

    #[test]
    fn lines_from_the_child_become_events_and_unchanged_ones_are_dropped() {
        let (cb, rx) = events();
        let line = r#"{"type":"data","diff":false,"payload":{"bundleIdentifier":"a.b","title":"T","playing":false}}"#;
        let p = spawn_with(sh(&format!("echo '{line}'; echo garbage; echo '{line}'; sleep 30")), cb, 1.0);
        match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::Media(Some(s)) => assert_eq!((s.title.as_str(), s.bundle_id.as_str()), ("T", "a.b")),
            e => panic!("{e:?}"),
        }
        assert!(rx.recv_timeout(Duration::from_millis(400)).is_err(), "the repeat and the garbage line say nothing new");
        drop(p);
    }

    #[test]
    fn dropping_the_poller_kills_the_child() {
        let (cb, rx) = events();
        let p = spawn_with(sh("echo '{\"type\":\"data\",\"diff\":false,\"payload\":{}}'; exec sleep 60"), cb, 1.0);
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let pid = p.child_pid().expect("running");
        assert!(alive(pid));
        drop(p);
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(pid), "the helper must not outlive the setting");
    }

    #[test]
    fn a_helper_that_keeps_failing_is_restarted_then_given_up_on_once() {
        let (cb, rx) = events();
        let p = spawn_with(sh("exit 3"), cb, 0.001);
        let mut unavailable = 0;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(300)) {
                Ok(Event::Unavailable) => unavailable += 1,
                Ok(_) => {}
                Err(_) => break,
            }
        }
        assert_eq!(unavailable, 1, "one give-up, then silence");
        drop(p);
    }

    #[test]
    fn a_missing_program_counts_as_a_failure_and_gives_up() {
        let (cb, rx) = events();
        let _p = spawn_with(Spec { program: "/nonexistent/perl".into(), args: vec![] }, cb, 0.001);
        let mut got = false;
        while let Ok(e) = rx.recv_timeout(Duration::from_secs(5)) {
            if e == Event::Unavailable {
                got = true;
                break;
            }
        }
        assert!(got);
    }

    /// Needs `scripts/bundle.sh` to have run (it leaves the helper in target/mediaremote-adapter).
    #[test]
    #[ignore = "runs the real adapter; needs target/mediaremote-adapter"]
    fn the_real_adapter_reports_what_plays_or_nothing() {
        let paths = locate().expect("run scripts/bundle.sh first");
        let (cb, rx) = events();
        let p = spawn_with(Spec::stream(&paths), cb, 1.0);
        let e = rx.recv_timeout(Duration::from_secs(15)).expect("the stream printed a line");
        assert!(matches!(e, Event::Media(_)), "{e:?}");
        drop(p);
    }

    #[test]
    fn a_command_that_hangs_is_killed_after_the_limit() {
        let t = Instant::now();
        assert!(!send_with("/bin/sleep", vec!["30".into()], Duration::from_millis(200)));
        assert!(t.elapsed() < Duration::from_secs(5));
        assert!(send_with("/usr/bin/true", vec![], Duration::from_secs(5)));
        assert!(!send_with("/usr/bin/false", vec![], Duration::from_secs(5)));
        assert!(!send_with("/nonexistent/x", vec![], Duration::from_secs(5)));
    }

    const D: [f64; 4] = [0.0, 0.0, 1512.0, 982.0];
    const ME: i32 = 100;

    fn win(layer: i64, pid: i32, rect: [f64; 4]) -> WinRect {
        WinRect { layer, pid, rect }
    }

    #[test]
    fn a_window_over_the_whole_display_is_full_screen() {
        assert!(covers_display(&[win(0, 7, D)], D, ME));
        assert!(covers_display(&[win(0, 7, [-1.0, 0.0, 1514.0, 983.0])], D, ME), "a point of slack");
    }

    #[test]
    fn a_maximised_window_below_the_menu_bar_is_not() {
        assert!(!covers_display(&[win(0, 7, [0.0, 38.0, 1512.0, 944.0])], D, ME));
        assert!(!covers_display(&[win(0, 7, [100.0, 100.0, 800.0, 600.0])], D, ME));
    }

    #[test]
    fn only_normal_windows_of_other_apps_count() {
        assert!(!covers_display(&[win(25, 7, D)], D, ME), "the menu bar / overlays are not layer 0");
        assert!(!covers_display(&[win(0, ME, D)], D, ME), "our own panels never count");
        assert!(!covers_display(&[], D, ME));
    }

    #[test]
    fn a_revealed_auto_hidden_menu_bar_is_not_full_screen() {
        // _HIHideMenuBar: the pointer at the top edge slides the menu bar (layer 25) in over the windows
        let bar = win(25, 50, [0.0, 0.0, 1512.0, 38.0]);
        let zoomed = win(0, 7, [0.0, 38.0, 1512.0, 944.0]);
        let other = win(0, 8, [0.0, 38.0, 1512.0, 944.0]);
        assert!(!is_fullscreen(&[bar, zoomed, other], D, ME, true));
        assert!(!is_fullscreen(&[bar], D, ME, true));
    }

    #[test]
    fn with_a_hidden_menu_bar_a_window_that_fills_the_display_is_only_full_screen_when_alone() {
        // a zoomed window fills the display (no menu bar to leave room for) but others sit behind it
        let fills = win(0, 7, D);
        let behind = win(0, 8, [100.0, 100.0, 800.0, 600.0]);
        assert!(!is_fullscreen(&[fills, behind], D, ME, true));
        assert!(!is_fullscreen(&[fills, win(0, 8, D)], D, ME, true));
        // a native full screen space holds just that window (and small helper windows)
        assert!(is_fullscreen(&[fills], D, ME, true));
        assert!(is_fullscreen(&[fills, win(0, 9, [0.0, 0.0, 40.0, 40.0]), win(25, 50, [0.0, -30.0, 1512.0, 38.0])], D, ME, true));
    }

    #[test]
    fn a_real_full_screen_app_hides_the_player_with_either_menu_bar_setting() {
        let fills = win(0, 7, D);
        assert!(is_fullscreen(&[fills], D, ME, false));
        assert!(is_fullscreen(&[fills, win(0, 8, [100.0, 100.0, 800.0, 600.0])], D, ME, false), "the old rule is unchanged");
        assert!(is_fullscreen(&[fills], D, ME, true));
        assert!(!is_fullscreen(&[win(0, ME, D)], D, ME, true), "never our own panel");
    }

    #[test]
    fn a_full_screen_window_on_another_display_does_not_hide_this_one() {
        assert!(!covers_display(&[win(0, 7, [1512.0, 0.0, 1920.0, 1080.0])], D, ME));
    }
}
