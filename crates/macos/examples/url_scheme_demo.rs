//! Check for the `ferrocast://` URL scheme:
//! `cargo run -p ferrocast-macos --example url_scheme_demo --locked -- --check`
//!
//! Builds a throwaway `.app` (this executable plus an Info.plist declaring the `ferrocast` scheme
//! under a unique bundle identifier) in a temp directory, registers it with LaunchServices, starts
//! it with `open -W` and a launch URL (cold launch: the URL is delivered before `install` runs and
//! must be buffered, then delivered exactly once), then sends URLs from this process with `open -a <demo bundle path> <url>` so the
//! real Ferrocast install, if any, is never involved. The temp bundle is always unregistered and
//! deleted at the end, and the check verifies that LaunchServices no longer knows it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ferrocast_macos::url_scheme::{self, Route};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEventMask};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode};

const BUNDLE_ID: &str = "dev.ferrocast.url-scheme-demo";
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
const UUID: &str = "3f2b8c1e-9d4a-4e7b-8a61-0c5d2f9e7b14";

const COLD_UUID: &str = "c01d0000-0000-4000-8000-000000000001";
static SECOND_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Runs inside the temporary bundle, in cold-launch order: `init` first, the launch URL arrives
/// while the event loop is pumped and NO callback exists yet, and only then `install` is called.
/// Every delivered route is recorded; the demo exits on the `new` sentinel.
fn run_inside_bundle(skip_init: bool) -> ! {
    let mtm = MainThreadMarker::new().expect("main thread");
    if !skip_init {
        url_scheme::init();
    }
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
    println!("LAUNCHED");
    // Pump events for a while without any callback: the launch URL is delivered (and buffered) here.
    let until = NSDate::dateWithTimeIntervalSinceNow(2.5);
    while let Some(ev) =
        unsafe { app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(&until), NSDefaultRunLoopMode, true) }
    {
        app.sendEvent(&ev);
    }
    println!("INSTALLING");
    // The first closure receives the buffered launch route; the second replaces it and must not
    // see it again.
    url_scheme::install(Box::new(|route| {
        if SECOND_INSTALLED.load(Ordering::SeqCst) {
            println!("REPLACED-CLOSURE-CALLED");
        }
        println!("ROUTE {route:?}");
    }));
    SECOND_INSTALLED.store(true, Ordering::SeqCst);
    url_scheme::install(Box::new(|route| {
        println!("ROUTE {route:?}");
        if route == Route::NewNote {
            println!("DONE");
            std::process::exit(0);
        }
    }));
    println!("READY");
    // Watchdog: the check must never leave the demo running.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(40));
        println!("TIMEOUT");
        std::process::exit(1);
    });
    // Apple Events are dispatched by the application event loop, like in the real app.
    app.run();
    std::process::exit(0);
}

fn lsregister(args: &[&str]) -> String {
    match Command::new(LSREGISTER).args(args).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(e) => format!("lsregister failed: {e}"),
    }
}

/// Unregisters and deletes the temporary bundle on drop, whatever happened in between.
struct Cleanup {
    dir: PathBuf,
    app: PathBuf,
    registered: bool,
}

impl Cleanup {
    fn run(&mut self) -> usize {
        if self.registered {
            lsregister(&["-u", &self.app.to_string_lossy()]);
            self.registered = false;
        }
        // Only ever delete the temp directory this process created.
        if self.dir.starts_with(std::env::temp_dir()) && self.dir.exists() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
        let dump = lsregister(&["-dump"]);
        dump.lines().filter(|l| l.contains("url-scheme-demo")).count()
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if self.registered || self.dir.exists() {
            self.run();
        }
    }
}

fn open_url(app: &Path, url: &str) -> bool {
    // `-a <bundle path>` targets the demo explicitly (the bundle-id lookup `-b` does not index
    // bundles under the per-user temp directory), never the global default for the scheme.
    Command::new("open").arg("-a").arg(app).arg(url).status().map(|s| s.success()).unwrap_or(false)
}

fn wait_for(log: &Path, needle: &str, secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if std::fs::read_to_string(log).unwrap_or_default().contains(needle) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--bundled") {
        // `--skip-init` reproduces the old behavior (handler registered only at `install`, after
        // the launch URL was already delivered): by hand only, it is expected to lose the route.
        run_inside_bundle(args.iter().any(|a| a == "--skip-init"));
    }
    if !args.iter().any(|a| a == "--check") {
        eprintln!("usage: url_scheme_demo --check");
        std::process::exit(2);
    }

    let dir = std::env::temp_dir().join(format!("url-scheme-demo-{}", std::process::id()));
    let app = dir.join("UrlSchemeDemo.app");
    let mut cleanup = Cleanup { dir: dir.clone(), app: app.clone(), registered: false };
    let mut failures: Vec<String> = Vec::new();
    let mut check = |name: &str, ok: bool, detail: &str| {
        if ok {
            println!("PASS  {name}");
        } else {
            println!("FAIL  {name}  ({detail})");
            failures.push(name.to_string());
        }
    };

    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).expect("create bundle");
    std::fs::copy(std::env::current_exe().expect("exe"), macos.join("url_scheme_demo")).expect("copy exe");
    std::fs::write(
        app.join("Contents/Info.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>url_scheme_demo</string>
  <key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>
  <key>CFBundleName</key><string>UrlSchemeDemo</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSUIElement</key><true/>
  <key>CFBundleURLTypes</key><array><dict>
    <key>CFBundleURLName</key><string>{BUNDLE_ID}</string>
    <key>CFBundleURLSchemes</key><array><string>ferrocast</string></array>
  </dict></array>
</dict></plist>"#
        ),
    )
    .expect("write plist");

    cleanup.registered = true;
    lsregister(&["-f", &app.to_string_lossy()]);

    let log = dir.join("out.log");
    let skip_init = std::env::args().any(|a| a == "--skip-init");
    let mut child = Command::new("open")
        .args(["-W", "-n", "--stdout"])
        .arg(&log)
        .arg("--stderr")
        .arg(&log)
        .arg("-a")
        .arg(&app)
        .arg(format!("ferrocast://notes/{COLD_UUID}"))
        .arg("--args")
        .arg("--bundled")
        .args(skip_init.then_some("--skip-init"))
        .stdin(Stdio::null())
        .spawn()
        .expect("spawn open");

    let ready = wait_for(&log, "READY", 20);
    check("demo app started and installed the callback", ready, "no READY in log");
    if ready {
        // Invalid URLs: `open` may or may not accept them; what matters is that none is delivered.
        open_url(&app, "ferrocast://notes/../x");
        open_url(&app, "ferrocast://evil/abc");
        let sent = open_url(&app, &format!("ferrocast://notes/{UUID}"));
        check("open -a <demo> ferrocast://notes/<uuid> succeeds", sent, "open failed");
        // Sentinel: the demo exits when it sees `new`, so every earlier URL has been handled.
        let sent_new = open_url(&app, "ferrocast://notes/new");
        check("open -a <demo> ferrocast://notes/new succeeds", sent_new, "open failed");
        wait_for(&log, "DONE", 20);
    }
    let _ = child.kill();
    let _ = child.wait();

    let out = std::fs::read_to_string(&log).unwrap_or_default();
    print!("--- demo output ---\n{out}-------------------\n");
    let installing = out.find("INSTALLING").unwrap_or(usize::MAX);
    let cold = out.find(&format!("ROUTE Note({COLD_UUID:?})")).unwrap_or(0);
    check("cold-launch URL was delivered only after install, from the buffer", cold > installing, "delivered before install or never");
    let routes: Vec<&str> = out.lines().filter(|l| l.starts_with("ROUTE ")).collect();
    let expected = [format!("ROUTE Note({COLD_UUID:?})"), format!("ROUTE Note({UUID:?})"), "ROUTE NewNote".to_string()];
    check(
        "delivered exactly [cold Note, Note(<uuid>), NewNote] (cold route once, no replay) (traversal and foreign host not delivered)",
        routes == expected.iter().map(String::as_str).collect::<Vec<_>>(),
        &format!("{routes:?}"),
    );
    check("second install replaced the first closure", !out.contains("REPLACED-CLOSURE-CALLED"), "old closure ran");

    let left = cleanup.run();
    println!("cleanup: unregistered {} and deleted {}", app.display(), dir.display());
    println!("cleanup: lsregister -dump lines mentioning url-scheme-demo: {left}");
    println!("cleanup: temp dir exists: {}", dir.exists());
    check("no LaunchServices entry left for the temporary bundle", left == 0, &format!("{left} lines"));
    check("temp directory deleted", !dir.exists(), "still exists");

    if failures.is_empty() {
        println!("url_scheme_demo: all checks passed");
    } else {
        println!("url_scheme_demo: {} check(s) FAILED: {}", failures.len(), failures.join("; "));
        std::process::exit(1);
    }
}
