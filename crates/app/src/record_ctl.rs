//! Recording lifecycle (S13): the pure state machine T19 drives.
//!
//! `idle → countdown → recording ⇄ paused → stopping → idle`. Nothing here touches the helper,
//! AppKit or the tray: every transition returns an [`Effect`] the caller performs (show a numeral,
//! send `record.start`/`pause`/`resume`/`stop`, toast a warning), and helper events are fed in
//! through [`RecordCtl::on_event`]. The tray title and the Stop item derive from the state.
//! The AppKit pieces live in `ferrocast_macos::hud`, whose pure helpers (`Countdown`,
//! `disk_level`, `low_disk_message`, `tray_title`) are reused here.
// T19 is the first caller of most of this; until it lands only the tray and the stop item use it.
#![allow(dead_code)]

use ferrocast_core::capture::protocol::{RecordEvent, RecordState};
use ferrocast_macos::hud::{self, Countdown, CountdownStep, DiskLevel};

/// Where a recording is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Idle,
    /// Showing the `3…2…1` numerals; nothing is recorded yet.
    Countdown(Countdown),
    Recording,
    Paused,
    /// `record.stop` was sent (user, tray, or the disk guard); waiting for the helper to finish.
    Stopping,
}

/// What the caller must do after a transition.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    None,
    ShowCountdown(u32),
    /// The countdown was cancelled before recording started: hide the numerals.
    CancelCountdown,
    /// Hide the numerals and send `record.start` (with `hud::excluded_window_ids()`).
    StartRecording,
    SendPause,
    SendResume,
    SendStop,
    /// Show this warning text (`Low disk space: N GB left`).
    LowDisk(String),
    /// The recording is over: hide the HUD. `error` is set when the helper failed.
    Ended {
        error: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct RecordCtl {
    phase: Phase,
    elapsed: f64,
    dropped: u64,
    low_disk: Option<String>,
    stopped_for_disk: bool,
}

impl RecordCtl {
    pub fn new() -> Self {
        Self { phase: Phase::Idle, elapsed: 0.0, dropped: 0, low_disk: None, stopped_for_disk: false }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Seconds recorded so far, from the helper's `tick` events.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The `Low disk space: N GB left` text currently showing, if any.
    pub fn low_disk_warning(&self) -> Option<&str> {
        self.low_disk.as_deref()
    }

    /// True when the last recording was stopped by the < 200 MB guard (kept until the next `begin`).
    pub fn stopped_for_low_disk(&self) -> bool {
        self.stopped_for_disk
    }

    pub fn is_idle(&self) -> bool {
        self.phase == Phase::Idle
    }

    /// Start a recording: counts `countdown_secs` first (0 starts at once). Ignored unless idle.
    pub fn begin(&mut self, countdown_secs: u32) -> Effect {
        if !self.is_idle() {
            return Effect::None;
        }
        self.elapsed = 0.0;
        self.dropped = 0;
        self.low_disk = None;
        self.stopped_for_disk = false;
        let countdown = Countdown::new(countdown_secs);
        match countdown.current() {
            Some(n) => {
                self.phase = Phase::Countdown(countdown);
                Effect::ShowCountdown(n)
            }
            None => {
                self.phase = Phase::Recording;
                Effect::StartRecording
            }
        }
    }

    /// One second of countdown passed.
    pub fn tick_countdown(&mut self) -> Effect {
        let Phase::Countdown(mut c) = self.phase else { return Effect::None };
        match c.tick() {
            CountdownStep::Show(n) => {
                self.phase = Phase::Countdown(c);
                Effect::ShowCountdown(n)
            }
            CountdownStep::Finished => {
                self.phase = Phase::Recording;
                Effect::StartRecording
            }
        }
    }

    /// Stop (HUD, tray or shortcut). During the countdown it cancels; otherwise it asks the
    /// helper to finish the file. Repeated requests send nothing.
    pub fn request_stop(&mut self) -> Effect {
        match self.phase {
            Phase::Countdown(_) => {
                self.phase = Phase::Idle;
                Effect::CancelCountdown
            }
            Phase::Recording | Phase::Paused => {
                self.phase = Phase::Stopping;
                Effect::SendStop
            }
            Phase::Idle | Phase::Stopping => Effect::None,
        }
    }

    pub fn toggle_pause(&mut self) -> Effect {
        match self.phase {
            Phase::Recording => {
                self.phase = Phase::Paused;
                Effect::SendPause
            }
            Phase::Paused => {
                self.phase = Phase::Recording;
                Effect::SendResume
            }
            _ => Effect::None,
        }
    }

    fn running(&self) -> bool {
        matches!(self.phase, Phase::Recording | Phase::Paused | Phase::Stopping)
    }

    /// Feed one helper event.
    pub fn on_event(&mut self, ev: &RecordEvent) -> Effect {
        if !self.running() {
            return Effect::None;
        }
        match ev {
            RecordEvent::Tick { elapsed, dropped, .. } => {
                self.elapsed = *elapsed;
                self.dropped = *dropped;
                Effect::None
            }
            RecordEvent::State { state, message } => match state {
                RecordState::Recording if self.phase == Phase::Paused => {
                    self.phase = Phase::Recording;
                    Effect::None
                }
                RecordState::Paused if self.phase == Phase::Recording => {
                    self.phase = Phase::Paused;
                    Effect::None
                }
                RecordState::Stopped => self.on_stopped(),
                RecordState::Failed => self.finish(Some(message.clone().unwrap_or_else(|| "The recording failed".into()))),
                _ => Effect::None,
            },
            RecordEvent::Warn { code, free_bytes: Some(free), .. } if code == "low-disk" => match hud::disk_level(*free) {
                DiskLevel::Ok => {
                    self.low_disk = None;
                    Effect::None
                }
                DiskLevel::Low => match hud::low_disk_message(*free) {
                    Some(text) if self.low_disk.as_deref() != Some(text.as_str()) => {
                        self.low_disk = Some(text.clone());
                        Effect::LowDisk(text)
                    }
                    _ => Effect::None,
                },
                DiskLevel::Critical => {
                    self.low_disk = hud::low_disk_message(*free);
                    self.stopped_for_disk = true;
                    self.request_stop()
                }
            },
            RecordEvent::Warn { .. } => Effect::None,
        }
    }

    /// The helper finished the file (`record.stop` replied or a `stopped` state arrived).
    pub fn on_stopped(&mut self) -> Effect {
        self.finish(None)
    }

    fn finish(&mut self, error: Option<String>) -> Effect {
        if self.is_idle() {
            return Effect::None;
        }
        self.phase = Phase::Idle;
        self.elapsed = 0.0;
        self.dropped = 0;
        self.low_disk = None;
        Effect::Ended { error }
    }

    /// Menu-bar title (`●  mm:ss`) while a recording exists; `None` otherwise.
    pub fn tray_title(&self) -> Option<String> {
        self.running().then(|| hud::tray_title(self.elapsed))
    }

    /// Whether the tray menu offers `Stop Recording`.
    pub fn shows_stop_item(&self) -> bool {
        matches!(self.phase, Phase::Recording | Phase::Paused)
    }
}

impl Default for RecordCtl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(elapsed: f64, dropped: u64) -> RecordEvent {
        RecordEvent::Tick { elapsed, frames: 0, dropped }
    }
    fn state(s: RecordState) -> RecordEvent {
        RecordEvent::State { state: s, message: None }
    }
    fn low_disk(free: u64) -> RecordEvent {
        RecordEvent::Warn { code: "low-disk".into(), message: None, free_bytes: Some(free) }
    }
    fn recording() -> RecordCtl {
        let mut c = RecordCtl::new();
        assert_eq!(c.begin(0), Effect::StartRecording);
        c
    }

    #[test]
    fn starts_idle_with_no_tray_state() {
        let c = RecordCtl::new();
        assert!(c.is_idle());
        assert_eq!(c.tray_title(), None);
        assert!(!c.shows_stop_item());
        assert_eq!(c.elapsed(), 0.0);
    }

    #[test]
    fn countdown_runs_3_2_1_then_starts_recording() {
        let mut c = RecordCtl::new();
        assert_eq!(c.begin(3), Effect::ShowCountdown(3));
        assert!(matches!(c.phase(), Phase::Countdown(_)));
        assert_eq!(c.tray_title(), None, "nothing is recording during the countdown");
        assert_eq!(c.tick_countdown(), Effect::ShowCountdown(2));
        assert_eq!(c.tick_countdown(), Effect::ShowCountdown(1));
        assert_eq!(c.tick_countdown(), Effect::StartRecording);
        assert_eq!(c.phase(), Phase::Recording);
        assert_eq!(c.tick_countdown(), Effect::None, "ticks outside a countdown do nothing");
    }

    #[test]
    fn zero_countdown_starts_at_once() {
        let mut c = RecordCtl::new();
        assert_eq!(c.begin(0), Effect::StartRecording);
        assert_eq!(c.phase(), Phase::Recording);
    }

    #[test]
    fn begin_is_ignored_unless_idle() {
        let mut c = recording();
        assert_eq!(c.begin(3), Effect::None);
        assert_eq!(c.phase(), Phase::Recording);
    }

    #[test]
    fn stop_during_countdown_cancels_it() {
        let mut c = RecordCtl::new();
        c.begin(3);
        assert_eq!(c.request_stop(), Effect::CancelCountdown);
        assert!(c.is_idle());
    }

    #[test]
    fn pause_and_resume_toggle() {
        let mut c = recording();
        assert_eq!(c.toggle_pause(), Effect::SendPause);
        assert_eq!(c.phase(), Phase::Paused);
        assert_eq!(c.toggle_pause(), Effect::SendResume);
        assert_eq!(c.phase(), Phase::Recording);
        let mut idle = RecordCtl::new();
        assert_eq!(idle.toggle_pause(), Effect::None);
    }

    #[test]
    fn stop_goes_through_stopping_and_back_to_idle() {
        let mut c = recording();
        assert_eq!(c.request_stop(), Effect::SendStop);
        assert_eq!(c.phase(), Phase::Stopping);
        assert_eq!(c.request_stop(), Effect::None, "a second Stop is not sent twice");
        assert_eq!(c.toggle_pause(), Effect::None, "no pause while stopping");
        assert_eq!(c.on_stopped(), Effect::Ended { error: None });
        assert!(c.is_idle());
    }

    #[test]
    fn stop_from_paused_works() {
        let mut c = recording();
        c.toggle_pause();
        assert_eq!(c.request_stop(), Effect::SendStop);
    }

    #[test]
    fn ticks_update_elapsed_and_dropped_only_while_a_recording_runs() {
        let mut idle = RecordCtl::new();
        idle.on_event(&tick(5.0, 2));
        assert_eq!((idle.elapsed(), idle.dropped()), (0.0, 0));
        let mut c = recording();
        assert_eq!(c.on_event(&tick(12.4, 3)), Effect::None);
        assert_eq!((c.elapsed(), c.dropped()), (12.4, 3));
        c.toggle_pause();
        c.on_event(&tick(12.4, 3));
        assert_eq!(c.elapsed(), 12.4);
    }

    #[test]
    fn tray_title_follows_elapsed_while_recording_or_paused() {
        let mut c = recording();
        c.on_event(&tick(12.0, 0));
        assert_eq!(c.tray_title().as_deref(), Some("\u{25CF}  00:12"));
        assert!(c.shows_stop_item());
        c.toggle_pause();
        assert_eq!(c.tray_title().as_deref(), Some("\u{25CF}  00:12"));
        assert!(c.shows_stop_item());
        c.request_stop();
        assert!(c.tray_title().is_some(), "title stays until the file is finished");
        assert!(!c.shows_stop_item(), "Stop Recording disappears once stopping");
        c.on_stopped();
        assert_eq!(c.tray_title(), None);
    }

    #[test]
    fn helper_state_events_confirm_pause_and_resume() {
        let mut c = recording();
        c.on_event(&state(RecordState::Paused));
        assert_eq!(c.phase(), Phase::Paused);
        c.on_event(&state(RecordState::Recording));
        assert_eq!(c.phase(), Phase::Recording);
    }

    #[test]
    fn helper_stopped_or_failed_ends_the_recording() {
        let mut c = recording();
        assert_eq!(c.on_event(&state(RecordState::Stopped)), Effect::Ended { error: None });
        assert!(c.is_idle());
        let mut c = recording();
        let failed = RecordEvent::State { state: RecordState::Failed, message: Some("writer failed".into()) };
        assert_eq!(c.on_event(&failed), Effect::Ended { error: Some("writer failed".into()) });
        assert!(c.is_idle());
    }

    #[test]
    fn low_disk_warns_under_2_gb_once_and_keeps_recording() {
        let mut c = recording();
        assert_eq!(c.on_event(&low_disk(1_500_000_000)), Effect::LowDisk("Low disk space: 1.5 GB left".into()));
        assert_eq!(c.phase(), Phase::Recording);
        assert_eq!(c.low_disk_warning(), Some("Low disk space: 1.5 GB left"));
        assert_eq!(c.on_event(&low_disk(1_500_000_000)), Effect::None, "same text is not repeated");
        assert_eq!(c.on_event(&low_disk(1_400_000_000)), Effect::LowDisk("Low disk space: 1.4 GB left".into()));
    }

    #[test]
    fn low_disk_above_2_gb_is_not_a_warning() {
        let mut c = recording();
        assert_eq!(c.on_event(&low_disk(2_000_000_000)), Effect::None);
        assert_eq!(c.low_disk_warning(), None);
    }

    #[test]
    fn under_200_mb_stops_cleanly() {
        let mut c = recording();
        assert_eq!(c.on_event(&low_disk(199_999_999)), Effect::SendStop);
        assert_eq!(c.phase(), Phase::Stopping);
        assert!(c.stopped_for_low_disk());
        assert_eq!(c.on_event(&low_disk(100_000_000)), Effect::None, "already stopping");
        c.on_stopped();
        assert!(c.stopped_for_low_disk(), "the caller can still tell why it stopped");
        c.begin(0);
        assert!(!c.stopped_for_low_disk(), "a new recording starts clean");
        assert_eq!(c.low_disk_warning(), None);
    }

    #[test]
    fn other_warnings_are_ignored() {
        let mut c = recording();
        let w = RecordEvent::Warn { code: "dropped-frames".into(), message: Some("3 frames dropped".into()), free_bytes: None };
        assert_eq!(c.on_event(&w), Effect::None);
        assert_eq!(c.phase(), Phase::Recording);
    }

    #[test]
    fn ended_resets_the_counters_for_the_next_recording() {
        let mut c = recording();
        c.on_event(&tick(30.0, 4));
        c.on_stopped();
        assert_eq!((c.elapsed(), c.dropped()), (0.0, 0));
        c.begin(0);
        assert_eq!(c.tray_title().as_deref(), Some("\u{25CF}  00:00"));
    }
}
