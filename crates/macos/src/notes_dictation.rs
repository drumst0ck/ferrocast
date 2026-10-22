//! Dictation for Notes: speech recognition into the editor.
//!
//! An `AVAudioEngine` input-node tap feeds an `SFSpeechAudioBufferRecognitionRequest`; partial and
//! final transcripts are delivered on the main thread as the full transcript so far. Recognition
//! runs on the device when the recognizer supports it. All state lives on the main thread.
//!
//! Needs `NSMicrophoneUsageDescription` and `NSSpeechRecognitionUsageDescription` in the app's
//! Info.plist (see `scripts/bundle.sh`); without them macOS kills the process on first access.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2::{class, msg_send, AnyThread, MainThreadMarker};
use objc2_avf_audio::{AVAudioEngine, AVAudioPCMBuffer, AVAudioTime};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSError, NSLocale, NSRunLoop, NSString, NSURL};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognitionTask,
    SFSpeechRecognizer, SFSpeechURLRecognitionRequest,
};

// `AVCaptureDevice` (microphone permission) lives in AVFoundation, which the bindings above do not link.
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

// Main-queue hop without an extra dependency: libdispatch is part of libSystem.
extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

type MainJob = Box<dyn FnOnce() + Send>;

extern "C" fn run_main_job(context: *mut c_void) {
    // SAFETY: `context` is the `Box<MainJob>` leaked by `run_on_main`, consumed exactly once here.
    let job: Box<MainJob> = unsafe { Box::from_raw(context.cast()) };
    job();
}

/// Run `job` on the main thread (asynchronously, from any thread).
fn run_on_main(job: MainJob) {
    let context = Box::into_raw(Box::new(job)).cast::<c_void>();
    // SAFETY: the main queue is a process-lifetime global; `run_main_job` frees `context`.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, context, run_main_job) };
}

/// Combined microphone and speech-recognition permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    NotDetermined,
    Denied,
    Restricted,
    Authorized,
}

impl Status {
    /// `Denied` or `Restricted` from either permission wins; `Authorized` needs both.
    fn combine(a: Status, b: Status) -> Status {
        use Status::*;
        match (a, b) {
            (Denied, _) | (_, Denied) => Denied,
            (Restricted, _) | (_, Restricted) => Restricted,
            (Authorized, Authorized) => Authorized,
            _ => NotDetermined,
        }
    }
}

fn speech_status() -> Status {
    // SFSpeechRecognizerAuthorizationStatus: 0 notDetermined, 1 denied, 2 restricted, 3 authorized.
    match unsafe { SFSpeechRecognizer::authorizationStatus() }.0 {
        3 => Status::Authorized,
        2 => Status::Restricted,
        1 => Status::Denied,
        _ => Status::NotDetermined,
    }
}

fn audio_media_type() -> Retained<NSString> {
    NSString::from_str("soun") // AVMediaTypeAudio
}

fn mic_status() -> Status {
    let media = audio_media_type();
    // AVAuthorizationStatus: 0 notDetermined, 1 restricted, 2 denied, 3 authorized.
    let raw: isize = unsafe { msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: &*media] };
    match raw {
        3 => Status::Authorized,
        2 => Status::Denied,
        1 => Status::Restricted,
        _ => Status::NotDetermined,
    }
}

/// Current permission state. Never prompts.
pub fn status() -> Status {
    Status::combine(mic_status(), speech_status())
}

type DoneCb = Rc<dyn Fn(Status)>;

thread_local! {
    static ASK_DONE: RefCell<Option<DoneCb>> = const { RefCell::new(None) };
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

fn finish_ask() {
    let done = ASK_DONE.with(|d| d.borrow_mut().take());
    if let Some(done) = done {
        done(status());
    }
}

/// Ask for speech-recognition access, then microphone access, and call `done` on the main thread
/// with the combined status. This SHOWS SYSTEM PERMISSION PROMPTS the first time. Call it on the
/// main thread.
pub fn request_access(done: Box<dyn Fn(Status)>) {
    ASK_DONE.with(|d| *d.borrow_mut() = Some(Rc::from(done)));
    // The completion handlers run on background threads: they only hop back to the main thread.
    let speech = RcBlock::new(|auth: objc2_speech::SFSpeechRecognizerAuthorizationStatus| {
        if auth.0 != 3 {
            run_on_main(Box::new(finish_ask));
            return;
        }
        let mic = RcBlock::new(|_granted: Bool| run_on_main(Box::new(finish_ask)));
        let media = audio_media_type();
        let _: () = unsafe { msg_send![class!(AVCaptureDevice), requestAccessForMediaType: &*media, completionHandler: &*mic] };
    });
    unsafe { SFSpeechRecognizer::requestAuthorization(&speech) };
}

/// A recognizer for the system locale, falling back to en-US when that locale is unsupported.
fn recognizer() -> Option<Retained<SFSpeechRecognizer>> {
    unsafe {
        let system = NSLocale::currentLocale();
        if let Some(r) = SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &system) {
            return Some(r);
        }
        let fallback = NSLocale::initWithLocaleIdentifier(NSLocale::alloc(), &NSString::from_str("en-US"));
        SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &fallback)
    }
}

/// Everything that exists while dictation is active.
struct Session {
    id: u64,
    engine: Retained<AVAudioEngine>,
    request: Retained<SFSpeechAudioBufferRecognitionRequest>,
    task: Retained<SFSpeechRecognitionTask>,
    _recognizer: Retained<SFSpeechRecognizer>,
    last: String,
    on_partial: Rc<dyn Fn(String)>,
    on_final: Rc<dyn Fn(String)>,
    on_error: Rc<dyn Fn(String)>,
}

impl Session {
    /// End the audio, stop the engine and cancel the task. Safe to call once; consumes the session.
    fn teardown(self) {
        unsafe {
            self.engine.stop();
            self.engine.inputNode().removeTapOnBus(0);
            self.request.endAudio();
            self.task.cancel();
        }
    }
}

/// Error codes of the recognizer that mean "recognition ended by itself" (silence timeout, task
/// time limit, nothing recognized), not a failure.
fn benign_end(domain: &str, code: isize) -> bool {
    matches!((domain, code), ("kAFAssistantErrorDomain", 203 | 216 | 1107 | 1110) | ("kLSRErrorDomain", 301))
}

/// Result of one recognition callback, reduced to plain `Send` data.
struct Update {
    text: Option<String>,
    is_final: bool,
    error: Option<(String, isize, String)>,
}

fn on_update(id: u64, up: Update) {
    // Late callbacks of a stopped or replaced session are ignored.
    enum Next {
        Partial(Rc<dyn Fn(String)>, String),
        Final(Rc<dyn Fn(String)>, String),
        Error(Rc<dyn Fn(String)>, String),
        Nothing,
    }
    let next = SESSION.with(|s| {
        let mut slot = s.borrow_mut();
        let Some(session) = slot.as_mut().filter(|x| x.id == id) else { return Next::Nothing };
        if let Some(t) = &up.text {
            session.last = t.clone();
        }
        let ended_by_error = up.error.is_some();
        if !up.is_final && !ended_by_error {
            return match up.text {
                Some(t) => Next::Partial(session.on_partial.clone(), t),
                None => Next::Nothing,
            };
        }
        // The task is over: release everything before telling the caller, so it can restart.
        let done = slot.take().expect("session present");
        let (on_final, on_error, last) = (done.on_final.clone(), done.on_error.clone(), done.last.clone());
        done.teardown();
        match up.error {
            Some((domain, code, msg)) if !up.is_final && !benign_end(&domain, code) && last.is_empty() => Next::Error(on_error, msg),
            _ => Next::Final(on_final, last),
        }
    });
    match next {
        Next::Partial(cb, t) => cb(t),
        Next::Final(cb, t) => cb(t),
        Next::Error(cb, m) => cb(m),
        Next::Nothing => {}
    }
}

fn update_from(result: *mut SFSpeechRecognitionResult, error: *mut NSError) -> Update {
    // SAFETY: the framework passes valid or null pointers for the duration of the callback.
    let result = unsafe { result.as_ref() };
    let error = unsafe { error.as_ref() };
    Update {
        text: result.map(|r| unsafe { r.bestTranscription().formattedString() }.to_string()),
        is_final: result.is_some_and(|r| unsafe { r.isFinal() }),
        error: error.map(|e| (e.domain().to_string(), e.code(), e.localizedDescription().to_string())),
    }
}

/// Start dictation. `on_partial` and `on_final` receive the full transcript so far; `on_error`
/// gets a failure that happens after start. Returns an error string (never panics) when access is
/// not authorized, the recognizer is unavailable, there is no input device or the engine fails.
pub fn start(on_partial: Box<dyn Fn(String)>, on_final: Box<dyn Fn(String)>, on_error: Box<dyn Fn(String)>) -> Result<(), String> {
    MainThreadMarker::new().ok_or("dictation must be started on the main thread")?;
    if is_active() {
        return Err("dictation is already active".into());
    }
    match status() {
        Status::Authorized => {}
        Status::Denied => return Err("microphone or speech recognition access was denied".into()),
        Status::Restricted => return Err("microphone or speech recognition access is restricted".into()),
        Status::NotDetermined => return Err("microphone and speech recognition access has not been granted yet".into()),
    }
    let recognizer = recognizer().ok_or("speech recognition is not supported for this language")?;
    if !unsafe { recognizer.isAvailable() } {
        return Err("speech recognizer is not available right now".into());
    }
    let request = unsafe { SFSpeechAudioBufferRecognitionRequest::init(SFSpeechAudioBufferRecognitionRequest::alloc()) };
    unsafe {
        request.setShouldReportPartialResults(true);
        if recognizer.supportsOnDeviceRecognition() {
            request.setRequiresOnDeviceRecognition(true);
        }
    }

    let engine = unsafe { AVAudioEngine::new() };
    let input = unsafe { engine.inputNode() };
    let format = unsafe { input.outputFormatForBus(0) };
    // Installing a tap with an empty format raises an Objective-C exception: reject it first.
    if unsafe { format.sampleRate() } <= 0.0 || unsafe { format.channelCount() } == 0 {
        return Err("no audio input device is available".into());
    }
    let tap_request = request.clone();
    let tap = RcBlock::new(move |buffer: NonNull<AVAudioPCMBuffer>, _when: NonNull<AVAudioTime>| {
        // Audio thread: appending buffers is the documented way to feed the request.
        unsafe { tap_request.appendAudioPCMBuffer(buffer.as_ref()) };
    });
    unsafe { input.installTapOnBus_bufferSize_format_block(0, 1024, Some(&format), &*tap as *const _ as *mut _) };
    unsafe { engine.prepare() };
    if let Err(e) = unsafe { engine.startAndReturnError() } {
        unsafe { input.removeTapOnBus(0) };
        return Err(format!("audio engine failed to start: {}", e.localizedDescription()));
    }

    let id = NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    let handler = RcBlock::new(move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
        let up = update_from(result, error);
        // Hop to the main thread whichever queue the framework used.
        run_on_main(Box::new(move || on_update(id, up)));
    });
    let task = unsafe { recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler) };
    let session = Session {
        id,
        engine,
        request,
        task,
        _recognizer: recognizer,
        last: String::new(),
        on_partial: Rc::from(on_partial),
        on_final: Rc::from(on_final),
        on_error: Rc::from(on_error),
    };
    SESSION.with(|s| *s.borrow_mut() = Some(session));
    Ok(())
}

/// Stop dictation and release everything. A no-op when idle. No callback is called afterwards.
pub fn stop() {
    let session = SESSION.with(|s| s.borrow_mut().take());
    if let Some(session) = session {
        session.teardown();
    }
}

/// Whether dictation is running.
pub fn is_active() -> bool {
    SESSION.with(|s| s.borrow().is_some())
}

/// Transcribe an audio file with the same recognizer settings as [`start`], blocking the calling
/// (main) thread on its run loop for up to `timeout`. Needs speech-recognition access only. Used by
/// the end-to-end check; not part of the editor flow.
pub fn transcribe_file(path: &Path, timeout: Duration) -> Result<String, String> {
    MainThreadMarker::new().ok_or("transcribe_file must run on the main thread")?;
    if speech_status() != Status::Authorized {
        return Err("speech recognition access is not authorized".into());
    }
    let recognizer = recognizer().ok_or("speech recognition is not supported for this language")?;
    if !unsafe { recognizer.isAvailable() } {
        return Err("speech recognizer is not available right now".into());
    }
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let request = unsafe { SFSpeechURLRecognitionRequest::initWithURL(SFSpeechURLRecognitionRequest::alloc(), &url) };
    unsafe {
        request.setShouldReportPartialResults(false);
        if recognizer.supportsOnDeviceRecognition() {
            request.setRequiresOnDeviceRecognition(true);
        }
    }
    let outcome: Rc<RefCell<Option<Result<String, String>>>> = Rc::new(RefCell::new(None));
    let sink = outcome.clone();
    let handler = RcBlock::new(move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
        let up = update_from(result, error);
        let mut slot = sink.borrow_mut();
        if slot.is_some() {
            return;
        }
        if up.is_final {
            *slot = Some(Ok(up.text.unwrap_or_default()));
        } else if let Some((_, _, msg)) = up.error {
            *slot = Some(Err(msg));
        }
    });
    let request: &SFSpeechRecognitionRequest = &request;
    let task = unsafe { recognizer.recognitionTaskWithRequest_resultHandler(request, &handler) };
    let deadline = Instant::now() + timeout;
    let run_loop = NSRunLoop::currentRunLoop();
    while outcome.borrow().is_none() && Instant::now() < deadline {
        let until = NSDate::dateWithTimeIntervalSinceNow(0.1);
        unsafe { run_loop.runMode_beforeDate(NSDefaultRunLoopMode, &until) };
    }
    unsafe { task.cancel() };
    let result = outcome.borrow_mut().take();
    result.unwrap_or_else(|| Err("timed out waiting for the transcription".into()))
}

/// The text the editor should show for the current partial transcript, i.e. what replaces the
/// previous partial (never appended to it). Surrounding whitespace is dropped; `previous` is not
/// part of the result, so repeated or corrected partials cannot duplicate text.
pub fn insertion(previous: &str, partial: &str) -> String {
    let _ = previous;
    partial.trim().to_string()
}

/// The minimal edit that turns the previous partial into the new one: how many characters to
/// delete from the end of `previous` and what to append afterwards. Both sides are normalized
/// like [`insertion`], so corrections of earlier words delete back to the first changed character.
pub fn delta(previous: &str, partial: &str) -> (usize, String) {
    let (prev, next) = (previous.trim(), partial.trim());
    let common = prev.chars().zip(next.chars()).take_while(|(a, b)| a == b).count();
    let delete = prev.chars().count() - common;
    (delete, next.chars().skip(common).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_replaces_with_the_full_partial() {
        assert_eq!(insertion("", "hello"), "hello");
        assert_eq!(insertion("hello", "hello world"), "hello world");
        assert_eq!(insertion("hello world", "  hello world  "), "hello world");
    }

    #[test]
    fn insertion_empty_input() {
        assert_eq!(insertion("", ""), "");
        assert_eq!(insertion("hello", ""), "");
        assert_eq!(insertion("", "   "), "");
    }

    #[test]
    fn insertion_takes_corrections_of_earlier_words() {
        assert_eq!(insertion("wreck a nice", "recognize speech"), "recognize speech");
    }

    #[test]
    fn delta_growth_only_appends() {
        assert_eq!(delta("", "hello"), (0, "hello".to_string()));
        assert_eq!(delta("hello", "hello world"), (0, " world".to_string()));
        assert_eq!(delta("hello world", "hello world"), (0, String::new()));
    }

    #[test]
    fn delta_correction_deletes_back_to_the_first_difference() {
        assert_eq!(delta("wreck a nice beach", "recognize speech"), (18, "recognize speech".to_string()));
        assert_eq!(delta("hello wold", "hello world"), (2, "rld".to_string()));
    }

    #[test]
    fn delta_empty_and_multibyte() {
        assert_eq!(delta("hello", ""), (5, String::new()));
        assert_eq!(delta("", ""), (0, String::new()));
        assert_eq!(delta("año", "años"), (0, "s".to_string()));
        assert_eq!(delta("café", "cafe"), (1, "e".to_string()));
    }

    #[test]
    fn status_combination() {
        use Status::*;
        assert_eq!(Status::combine(Authorized, Authorized), Authorized);
        assert_eq!(Status::combine(Authorized, NotDetermined), NotDetermined);
        assert_eq!(Status::combine(Authorized, Denied), Denied);
        assert_eq!(Status::combine(Restricted, Authorized), Restricted);
        assert_eq!(Status::combine(Restricted, Denied), Denied);
        assert_eq!(Status::combine(NotDetermined, NotDetermined), NotDetermined);
    }

    #[test]
    fn benign_end_codes() {
        assert!(benign_end("kAFAssistantErrorDomain", 1110));
        assert!(benign_end("kAFAssistantErrorDomain", 203));
        assert!(!benign_end("kAFAssistantErrorDomain", 1));
        assert!(!benign_end("NSURLErrorDomain", 1110));
    }

    #[test]
    fn stop_when_idle_is_a_no_op() {
        stop();
        assert!(!is_active());
    }
}
