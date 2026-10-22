//! Check for Notes dictation:
//! `cargo run -p ferrocast-macos --example notes_dictation_demo --locked -- --check`
//!
//! `--check` never shows a permission prompt and never needs a microphone. It asserts that
//! `status()` returns, that `start` fails cleanly when access is not authorized (and `stop` is
//! then a no-op), and, only when both permissions are already granted, transcribes speech made
//! with `say` end to end. Otherwise that last step prints `SKIP end-to-end: <status>`.
//!
//! `--ask` is for a human: it calls `request_access`, which SHOWS the system microphone and speech
//! recognition prompts, and prints the resulting status. It is never part of `--check`.
//!
//! The process re-runs itself inside a throwaway `.app` bundle whose Info.plist has the usage
//! descriptions (without them the system kills a process that touches the microphone). The bundle
//! is unregistered and deleted at the end, and the check verifies that LaunchServices forgot it.

use std::process::Command;
use std::time::{Duration, Instant};

use ferrocast_macos::notes_dictation::{self as dictation, Status};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop};

const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
const MARKER: &str = "notes-dictation-demo";

fn lsregister(args: &[&str]) -> String {
    match Command::new(LSREGISTER).args(args).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(e) => format!("lsregister failed: {e}"),
    }
}

/// Re-run this check inside a temporary bundle started with `open`, print its output and exit.
fn relaunch_in_bundle(args: &[String]) -> ! {
    let tmp = std::env::temp_dir().join(format!("{MARKER}-{}", std::process::id()));
    let app = tmp.join("NotesDictationDemo.app");
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).expect("create bundle");
    std::fs::copy(std::env::current_exe().expect("exe"), macos.join("notes_dictation_demo")).expect("copy exe");
    std::fs::write(
        app.join("Contents/Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>notes_dictation_demo</string>
  <key>CFBundleIdentifier</key><string>dev.ferrocast.notes-dictation-demo</string>
  <key>CFBundleName</key><string>NotesDictationDemo</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSUIElement</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Ferrocast listens to your microphone only while you dictate into a note.</string>
  <key>NSSpeechRecognitionUsageDescription</key><string>Ferrocast turns your speech into text only while you dictate into a note.</string>
</dict></plist>"#,
    )
    .expect("write plist");
    let log = tmp.join("out.log");
    let status = Command::new("open")
        .args(["-W", "-n", "--stdout"])
        .arg(&log)
        .arg("--stderr")
        .arg(&log)
        .arg(&app)
        .arg("--args")
        .args(&args[1..])
        .arg("--bundled")
        .status();
    let out = std::fs::read_to_string(&log).unwrap_or_default();
    print!("{out}");

    // Cleanup: only ever delete the temp directory this process created.
    lsregister(&["-u", &app.to_string_lossy()]);
    if tmp.starts_with(std::env::temp_dir()) && tmp.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    let left = lsregister(&["-dump"]).lines().filter(|l| l.contains(MARKER)).count();
    println!("cleanup: unregistered {} and deleted {}", app.display(), tmp.display());
    println!("cleanup: lsregister -dump lines mentioning {MARKER}: {left}");
    println!("cleanup: temp dir exists: {}", tmp.exists());
    let clean = left == 0 && !tmp.exists();
    if !clean {
        println!("FAIL  temporary bundle was not fully removed");
    }
    let ok = status.map(|s| s.success()).unwrap_or(false) && out.contains("notes_dictation_demo: all checks passed") && clean;
    std::process::exit(if ok { 0 } else { 1 });
}

fn pump(seconds: f64) {
    let until = NSDate::dateWithTimeIntervalSinceNow(seconds);
    unsafe { NSRunLoop::currentRunLoop().runMode_beforeDate(NSDefaultRunLoopMode, &until) };
}

fn run_ask() {
    println!("--ask: this SHOWS the system speech recognition and microphone permission prompts.");
    println!("status before: {:?}", dictation::status());
    let result: std::rc::Rc<std::cell::Cell<Option<Status>>> = Default::default();
    let sink = result.clone();
    dictation::request_access(Box::new(move |s| sink.set(Some(s))));
    let deadline = Instant::now() + Duration::from_secs(120);
    while result.get().is_none() && Instant::now() < deadline {
        pump(0.1);
    }
    match result.get() {
        Some(s) => println!("status after: {s:?}"),
        None => println!("no answer to the prompts within 120 s; status now: {:?}", dictation::status()),
    }
    println!("notes_dictation_demo: all checks passed");
}

fn run_check() {
    let mut failures: Vec<String> = Vec::new();
    let mut check = |name: &str, ok: bool, detail: &str| {
        if ok {
            println!("PASS  {name}");
        } else {
            println!("FAIL  {name}  ({detail})");
            failures.push(name.to_string());
        }
    };

    let status = dictation::status();
    println!("status: {status:?}");
    check("status() returned", true, "");
    check("idle: is_active() is false", !dictation::is_active(), "active");

    if status != Status::Authorized {
        let r = dictation::start(Box::new(|_| {}), Box::new(|_| {}), Box::new(|_| {}));
        check("start() returns Err when access is not authorized", r.is_err(), "start succeeded");
        println!("start error: {}", r.err().unwrap_or_default());
        check("not active after the failed start", !dictation::is_active(), "active");
        dictation::stop();
        dictation::stop();
        check("stop() when idle is a no-op", !dictation::is_active(), "active");
    } else {
        println!("SKIP start-error check: access is authorized");
    }

    if status == Status::Authorized {
        let tmp = std::env::temp_dir().join(format!("{MARKER}-{}.aiff", std::process::id()));
        let said = Command::new("say").arg("-o").arg(&tmp).arg("hello world this is a dictation test").status();
        check("say generated the audio file", said.map(|s| s.success()).unwrap_or(false) && tmp.exists(), "say failed");
        let text = dictation::transcribe_file(&tmp, Duration::from_secs(60));
        let _ = std::fs::remove_file(&tmp);
        match text {
            Ok(t) => {
                println!("transcript: {t:?}");
                let lower = t.to_lowercase();
                check("transcript contains \"hello\"", lower.contains("hello"), &t);
                check("transcript contains \"dictation\"", lower.contains("dictation"), &t);
            }
            Err(e) => check("end-to-end transcription", false, &e),
        }
    } else {
        println!("SKIP end-to-end: {status:?}");
    }

    if failures.is_empty() {
        println!("notes_dictation_demo: all checks passed");
    } else {
        println!("notes_dictation_demo: {} check(s) FAILED: {}", failures.len(), failures.join("; "));
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let check = args.iter().any(|a| a == "--check");
    let ask = args.iter().any(|a| a == "--ask");
    if check == ask {
        eprintln!("usage: notes_dictation_demo --check | --ask   (--ask shows the system permission prompts)");
        std::process::exit(2);
    }
    if !args.iter().any(|a| a == "--bundled") {
        relaunch_in_bundle(&args);
    }
    if ask {
        run_ask();
    } else {
        run_check();
    }
}
