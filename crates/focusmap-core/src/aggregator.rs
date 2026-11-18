//! Turns raw timing events into one-minute focus samples.
//!
//! The aggregator is a plain state machine with an injected clock, so the whole
//! thing is testable without hooks or threads. It only ever holds the current
//! minute in memory: no unbounded buffers, no replay log.

use std::collections::HashMap;

use crate::apps;
use crate::capture::CaptureEvent;
use crate::model::{local_day_and_mod, FocusLevel, MinuteSample};
use crate::scoring::{focus_score, MinuteMetrics, ScoreConfig};

/// Apps kept per minute. Beyond this the least used entry is dropped; a minute
/// with nine different foreground apps is already fragmented.
const MAX_APPS_PER_MINUTE: usize = 8;

#[derive(Default)]
struct Bucket {
    minute: i64,
    start_ts: i64,
    keys: u32,
    backspaces: u32,
    clicks: u32,
    mouse_px: u32,
    mouse_events: u32,
    switches: u32,
    active_mask: u64,
    pauses: u32,
    max_pause_secs: u32,
    app_hits: HashMap<String, u32>,
    manual: bool,
}

impl Bucket {
    fn new(minute: i64, start_ts: i64, manual: bool) -> Self {
        Self {
            minute,
            start_ts,
            manual,
            ..Default::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.active_mask == 0 && self.app_hits.is_empty()
    }

    fn mark_active(&mut self, ts: i64) {
        let sec = (ts.rem_euclid(60_000) / 1_000) as u32;
        self.active_mask |= 1u64 << sec.min(59);
    }

    fn note_app(&mut self, app: &str) {
        if app.is_empty() {
            return;
        }
        let entry = self.app_hits.entry(app.to_string()).or_default();
        *entry += 1;
        if self.app_hits.len() > MAX_APPS_PER_MINUTE {
            if let Some(weakest) = self
                .app_hits
                .iter()
                .min_by_key(|(_, n)| **n)
                .map(|(k, _)| k.clone())
            {
                if weakest != app {
                    self.app_hits.remove(&weakest);
                }
            }
        }
    }

    fn top_app(&self) -> String {
        self.app_hits
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    fn metrics(&self) -> MinuteMetrics {
        let app = self.top_app();
        let category = apps::categorize(&app);
        MinuteMetrics {
            keys: self.keys,
            backspaces: self.backspaces,
            clicks: self.clicks,
            mouse_px: self.mouse_px,
            mouse_events: self.mouse_events,
            switches: self.switches,
            active_secs: self.active_mask.count_ones(),
            pauses: self.pauses,
            max_pause_secs: self.max_pause_secs,
            category,
            manual: self.manual,
        }
    }
}

pub struct Aggregator {
    cfg: ScoreConfig,
    idle_timeout_secs: u32,
    bucket: Bucket,
    last_ts: i64,
    current_app: String,
    last_app: String,
    manual: bool,
}

impl Aggregator {
    pub fn new(cfg: ScoreConfig, idle_timeout_secs: u32) -> Self {
        Self {
            cfg,
            idle_timeout_secs: idle_timeout_secs.max(60),
            // Starts empty: the first event (or tick) opens the real bucket, so
            // tests and clock changes cannot inject samples into a stale minute.
            bucket: Bucket::default(),
            last_ts: 0,
            current_app: String::new(),
            last_app: String::new(),
            manual: false,
        }
    }

    pub fn config(&self) -> &ScoreConfig {
        &self.cfg
    }

    /// Apply new settings without restarting capture.
    pub fn set_config(&mut self, cfg: ScoreConfig, idle_timeout_secs: u32) {
        self.cfg = cfg;
        self.idle_timeout_secs = idle_timeout_secs.max(60);
    }

    pub fn manual(&self) -> bool {
        self.manual
    }

    pub fn set_manual(&mut self, on: bool, now_ms: i64) {
        if on == self.manual {
            return;
        }
        self.manual = on;
        self.bucket = Bucket::new(
            now_ms.div_euclid(60_000),
            now_ms,
            on,
        );
        if on {
            self.bucket.note_app("Manual focus");
        }
    }

    pub fn current_app(&self) -> &str {
        &self.current_app
    }

    /// Feed one event. Returns any minutes that were closed by it.
    pub fn observe(&mut self, ev: &CaptureEvent) -> Vec<MinuteSample> {
        let (ts, minute) = match ev.ts() {
            0 => return Vec::new(),
            ts => (ts, ts.div_euclid(60_000)),
        };
        // A stale event (older than the open bucket) can happen when the
        // foreground poller and the hooks race. Timing only, so dropping it is
        // safe.
        if minute < self.bucket.minute {
            return Vec::new();
        }

        let mut out = Vec::new();
        if minute > self.bucket.minute {
            if let Some(sample) = self.finish_bucket() {
                out.push(sample);
            }
            self.bucket = Bucket::new(minute, ts, self.manual);
            self.last_ts = 0;
        }

        let gap = if self.last_ts == 0 { 0 } else { ts - self.last_ts };
        if gap >= 3_000 {
            // Long pauses are recorded as breaks inside the minute. A gap past
            // the idle timeout is a different working session, not a pause.
            if gap <= self.idle_timeout_secs as i64 * 1_000 {
                self.bucket.pauses += 1;
                self.bucket.max_pause_secs = self
                    .bucket
                    .max_pause_secs
                    .max((gap / 1_000).min(60) as u32);
            } else {
                self.last_ts = 0;
            }
        }
        if self.last_ts == 0 {
            self.last_ts = ts;
        } else {
            self.last_ts = self.last_ts.max(ts);
        }

        match ev {
            CaptureEvent::Key { backspace, .. } => {
                self.bucket.keys += 1;
                if *backspace {
                    self.bucket.backspaces += 1;
                }
                self.bucket.mark_active(ts);
                self.bucket.note_app(&self.current_app);
            }
            CaptureEvent::Click { .. } => {
                self.bucket.clicks += 1;
                self.bucket.mark_active(ts);
                self.bucket.note_app(&self.current_app);
            }
            CaptureEvent::Move { px, events, .. } => {
                self.bucket.mouse_px = self.bucket.mouse_px.saturating_add(*px);
                self.bucket.mouse_events = self.bucket.mouse_events.saturating_add(*events);
                self.bucket.mark_active(ts);
                self.bucket.note_app(&self.current_app);
            }
            CaptureEvent::App { name, .. } => {
                self.current_app = name.clone();
                if !self.last_app.is_empty() && !name.is_empty() && *name != self.last_app {
                    self.bucket.switches += 1;
                    self.bucket.note_app(name);
                }
                if !name.is_empty() {
                    self.last_app = name.clone();
                }
            }
            CaptureEvent::Idle { .. } => {}
        }
        out
    }

    /// Periodic call from the tracker loop. Closes finished minutes and keeps
    /// manual focus sessions alive through quiet reading.
    pub fn tick(&mut self, now_ms: i64) -> Vec<MinuteSample> {
        let now_minute = now_ms.div_euclid(60_000);
        let mut out = Vec::new();
        if now_minute <= self.bucket.minute {
            return out;
        }
        let bucket_ended_at = (self.bucket.minute + 1) * 60_000;
        // Give late events a moment to land before closing the minute.
        if now_ms < bucket_ended_at + 1_500 {
            return out;
        }
        if let Some(sample) = self.finish_bucket() {
            out.push(sample);
        }
        self.bucket = Bucket::new(now_minute, now_minute * 60_000, self.manual);
        if self.manual {
            self.bucket.note_app("Manual focus");
        }
        self.last_ts = 0;
        out
    }

    /// Close whatever is open (pause, shutdown, settings change).
    pub fn flush(&mut self) -> Option<MinuteSample> {
        if self.bucket.is_empty() && !self.bucket.manual {
            return None;
        }
        self.finish_bucket()
    }

    /// Live score for the part of the current minute that has happened so far.
    pub fn partial_score(&self, _now_ms: i64) -> u8 {
        if self.bucket.is_empty() && !self.bucket.manual {
            return 0;
        }
        focus_score(&self.bucket.metrics(), &self.cfg)
    }

    fn finish_bucket(&mut self) -> Option<MinuteSample> {
        let minute = self.bucket.minute;
        let start_ts = self.bucket.start_ts;
        let manual = self.bucket.manual;
        let bucket = std::mem::replace(&mut self.bucket, Bucket::new(minute, start_ts, manual));
        if bucket.is_empty() && !bucket.manual {
            return None;
        }
        let metrics = bucket.metrics();
        let score = focus_score(&metrics, &self.cfg);
        let (day, minute_of_day) = local_day_and_mod(bucket.start_ts);
        let app = bucket.top_app();
        let category = apps::categorize(&app);
        Some(MinuteSample {
            minute: bucket.minute,
            day,
            minute_of_day,
            score,
            level: FocusLevel::from_score(
                score,
                self.cfg.deep_threshold,
                self.cfg.medium_threshold,
            ),
            keys: metrics.keys,
            backspaces: metrics.backspaces,
            clicks: metrics.clicks,
            mouse_px: metrics.mouse_px,
            switches: metrics.switches,
            active_secs: metrics.active_secs.min(60) as u8,
            pauses: metrics.pauses.min(255) as u8,
            app,
            category,
            manual: bucket.manual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(minute_base: i64, sec: i64) -> i64 {
        minute_base * 60_000 + sec * 1_000
    }

    fn key_events(agg: &mut Aggregator, minute: i64, count: u32, app: &str) -> Vec<MinuteSample> {
        let mut out = Vec::new();
        out.extend(agg.observe(&CaptureEvent::App {
            ts: ts(minute, 0),
            name: app.to_string(),
        }));
        for i in 0..count {
            out.extend(agg.observe(&CaptureEvent::Key {
                ts: ts(minute, (i as i64 * 55) / count.max(1) as i64),
                backspace: i % 20 == 0,
            }));
        }
        out
    }

    #[test]
    fn steady_minute_is_scored_once() {
        let minute = 29_000_000;
        let mut agg = Aggregator::new(ScoreConfig::default(), 600);
        let closed = key_events(&mut agg, minute, 110, "Code.exe");
        assert!(closed.is_empty(), "minute is still open");
        let closed = agg.tick(ts(minute + 1, 5));
        assert_eq!(closed.len(), 1);
        let s = &closed[0];
        assert_eq!(s.keys, 110);
        assert_eq!(s.switches, 0);
        assert!(s.active_secs > 40, "active_secs {}", s.active_secs);
        assert_eq!(s.level, FocusLevel::Deep);
        assert_eq!(s.app, "Code.exe");
        assert_eq!(s.category, apps::AppCategory::Editor);
    }

    #[test]
    fn window_switches_are_counted() {
        let minute = 29_100_000;
        let mut agg = Aggregator::new(ScoreConfig::default(), 600);
        agg.observe(&CaptureEvent::App {
            ts: ts(minute, 0),
            name: "Code.exe".into(),
        });
        for i in 0..6 {
            agg.observe(&CaptureEvent::Key {
                ts: ts(minute, 1 + i),
                backspace: false,
            });
            agg.observe(&CaptureEvent::App {
                ts: ts(minute, 10 + i),
                name: if i % 2 == 0 { "chrome" } else { "Code.exe" }.into(),
            });
        }
        let closed = agg.tick(ts(minute + 1, 5));
        assert_eq!(closed[0].switches, 6);
    }

    #[test]
    fn gaps_become_pauses_but_idle_resets() {
        let minute = 29_200_000;
        let mut agg = Aggregator::new(ScoreConfig::default(), 120);
        let mut closed = agg.observe(&CaptureEvent::Key {
            ts: ts(minute, 0),
            backspace: false,
        });
        closed.extend(agg.observe(&CaptureEvent::Key {
            ts: ts(minute, 8),
            backspace: false,
        }));
        closed.extend(agg.observe(&CaptureEvent::Key {
            ts: ts(minute, 20),
            backspace: false,
        }));
        // 5 minutes of nothing: past the idle timeout, so it is not a pause.
        closed.extend(agg.observe(&CaptureEvent::Key {
            ts: ts(minute + 5, 30),
            backspace: false,
        }));
        closed.extend(agg.tick(ts(minute + 6, 5)));
        assert_eq!(closed[0].pauses, 2);
        assert_eq!(closed[1].pauses, 0);
    }

    #[test]
    fn quiet_minutes_are_never_written() {
        let minute = 29_300_000;
        let mut agg = Aggregator::new(ScoreConfig::default(), 600);
        agg.observe(&CaptureEvent::Key {
            ts: ts(minute, 3),
            backspace: false,
        });
        let closed = agg.tick(ts(minute + 1, 5));
        assert_eq!(closed.len(), 1);
        let closed = agg.tick(ts(minute + 10, 0));
        assert!(closed.is_empty(), "no input means no row, not a zero row");
    }

    #[test]
    fn manual_sessions_keep_producing_minutes() {
        let minute = 29_400_000;
        let mut agg = Aggregator::new(ScoreConfig::default(), 600);
        agg.set_manual(true, ts(minute, 2));
        let closed = agg.tick(ts(minute + 1, 5));
        assert_eq!(closed.len(), 1);
        assert!(closed[0].manual);
        assert!(closed[0].score >= 70);
        assert_eq!(closed[0].app, "Manual focus");
    }

    #[test]
    fn flush_closes_the_open_minute() {
        let minute = 29_500_000;
        let mut agg = Aggregator::new(ScoreConfig::default(), 600);
        key_events(&mut agg, minute, 10, "Code.exe");
        let sample = agg.flush().expect("open minute is flushed on pause");
        assert_eq!(sample.keys, 10);
        assert!(agg.flush().is_none(), "nothing left to flush");
    }
}
