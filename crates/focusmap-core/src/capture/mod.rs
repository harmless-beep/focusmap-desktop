//! Input capture.
//!
//! The capture layer's contract with the rest of the app is one sentence: it
//! only ever emits *timing and counts*. There is no field anywhere in
//! [`CaptureEvent`] that can carry a character, a key name or window text.
//!
//! Each platform implements [`InputSource`]. Windows is the reference
//! implementation; other platforms fall back to [`NullSource`] until their hooks
//! land, which keeps manual focus sessions and the whole UI usable everywhere.

use std::sync::mpsc::SyncSender;

use anyhow::Result;

pub mod null;

#[cfg(windows)]
pub mod windows;

#[derive(Clone, Debug)]
pub enum CaptureEvent {
    /// A key went down. `backspace` is a category, not content: the key value is
    /// compared and thrown away inside the hook.
    Key { ts: i64, backspace: bool },
    Click { ts: i64 },
    /// Aggregated pointer movement: total distance and number of samples.
    Move { ts: i64, px: u32, events: u32 },
    /// Foreground process changed (executable name only, never a title).
    App { ts: i64, name: String },
    /// System-wide idle time from the OS.
    Idle { ts: i64, secs: u32 },
}

impl CaptureEvent {
    pub fn ts(&self) -> i64 {
        match self {
            CaptureEvent::Key { ts, .. }
            | CaptureEvent::Click { ts }
            | CaptureEvent::Move { ts, .. }
            | CaptureEvent::App { ts, .. }
            | CaptureEvent::Idle { ts, .. } => *ts,
        }
    }
}

pub trait InputSource: Send {
    /// Start emitting events into `sink`. Returns once the capture is running.
    fn start(&mut self, sink: SyncSender<CaptureEvent>) -> Result<()>;
    fn stop(&mut self);
    fn label(&self) -> &'static str;
}

/// The best available source for this platform.
pub fn default_source() -> Box<dyn InputSource> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsSource::default())
    }
    #[cfg(not(windows))]
    {
        Box::new(null::NullSource)
    }
}

#[cfg(windows)]
pub use windows::{is_paused, set_paused};

#[cfg(not(windows))]
pub fn set_paused(_paused: bool) {}

#[cfg(not(windows))]
pub fn is_paused() -> bool {
    false
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
