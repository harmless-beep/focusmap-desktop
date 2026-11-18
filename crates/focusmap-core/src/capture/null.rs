//! A capture source that never produces events.
//!
//! Used on Linux and macOS until their hooks land, and in tests. The app stays
//! fully usable: manual focus sessions still get recorded.

use std::sync::mpsc::SyncSender;

use anyhow::Result;

use super::{CaptureEvent, InputSource};

pub struct NullSource;

impl InputSource for NullSource {
    fn start(&mut self, _sink: SyncSender<CaptureEvent>) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn label(&self) -> &'static str {
        "manual only"
    }
}
