//! The tracker: capture thread -> aggregator -> SQLite, plus the live state the
//! UI reads.
//!
//! Threads are deliberately few:
//! 1. capture hooks + poller (inside the Windows source),
//! 2. this tracker loop (aggregation + one small write per minute),
//! 3. the egui UI thread.
//!
//! The tracker wakes 5 times a second when idle, writes once a minute, and keeps
//! no unbounded buffers, which is how the app stays inside its CPU/memory budget.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

use anyhow::Result;

use crate::aggregator::Aggregator;
use crate::capture::{self, CaptureEvent, InputSource};
use crate::model::{local_day_and_mod, sessions_from, today, DaySummary, MinuteSample, Session};
use crate::scoring::ScoreConfig;
use crate::settings::Settings;
use crate::storage::Db;

#[derive(Clone, Debug)]
pub struct LiveState {
    pub paused: bool,
    pub manual: bool,
    pub tracking: bool,
    pub source: String,
    pub today: String,
    pub samples: Vec<MinuteSample>,
    pub summary: DaySummary,
    pub sessions: Vec<Session>,
    pub live_score: u8,
    pub current_app: String,
    pub idle_secs: u32,
    pub last_event_ms: i64,
    pub last_error: Option<String>,
}

impl LiveState {
    fn empty(today: String, source: &str) -> Self {
        Self {
            paused: false,
            manual: false,
            tracking: true,
            source: source.to_string(),
            summary: DaySummary {
                day: today.clone(),
                ..Default::default()
            },
            today,
            samples: Vec::new(),
            sessions: Vec::new(),
            live_score: 0,
            current_app: String::new(),
            idle_secs: 0,
            last_event_ms: 0,
            last_error: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NudgeKind {
    Goal,
    Break,
    Burnout,
}

#[derive(Clone, Debug)]
pub struct Nudge {
    pub kind: NudgeKind,
    pub title: String,
    pub body: String,
}

#[derive(Clone)]
pub struct RuntimeCfg {
    pub score: ScoreConfig,
    pub idle_timeout: u32,
    pub goal: u32,
    pub break_minutes: u32,
    pub rev: u64,
}

pub struct Engine {
    pub state: Arc<RwLock<LiveState>>,
    paused: Arc<AtomicBool>,
    manual: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    nudges: Arc<Mutex<Vec<Nudge>>>,
    db: Arc<Db>,
    cfg: Arc<RwLock<RuntimeCfg>>,
    join: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn start(settings: &Settings, db: Arc<Db>) -> Result<Self> {
        let today = today();
        let samples = db.samples(&today).unwrap_or_default();
        let summary = db.day(&today).ok().flatten().unwrap_or(DaySummary {
            day: today.clone(),
            ..Default::default()
        });
        let sessions = db.sessions(&today).unwrap_or_default();

        let source = capture::default_source();
        let source_label = source.label().to_string();
        let mut state = LiveState::empty(today, &source_label);
        state.samples = samples;
        state.sessions = sessions;
        state.live_score = summary.score;
        state.summary = summary;
        let state = Arc::new(RwLock::new(state));

        let paused = Arc::new(AtomicBool::new(false));
        let manual = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let nudges = Arc::new(Mutex::new(Vec::new()));
        let cfg = Arc::new(RwLock::new(RuntimeCfg {
            score: settings.score.clone(),
            idle_timeout: settings.idle_timeout_secs,
            goal: settings.goal_minutes,
            break_minutes: settings.break_reminder_minutes,
            rev: 0,
        }));

        let join = {
            let state = state.clone();
            let paused = paused.clone();
            let manual = manual.clone();
            let stop = stop.clone();
            let nudges = nudges.clone();
            let db = db.clone();
            let cfg = cfg.clone();
            std::thread::spawn(move || {
                tracker_loop(source, state, paused, manual, stop, nudges, db, cfg);
            })
        };

        Ok(Self {
            state,
            paused,
            manual,
            stop,
            nudges,
            db,
            cfg,
            join: Some(join),
        })
    }

    pub fn db(&self) -> Arc<Db> {
        self.db.clone()
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
        capture::set_paused(paused);
        if let Ok(mut s) = self.state.write() {
            s.paused = paused;
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_manual(&self, on: bool) {
        self.manual.store(on, Ordering::Relaxed);
        if let Ok(mut s) = self.state.write() {
            s.manual = on;
        }
    }

    pub fn is_manual(&self) -> bool {
        self.manual.load(Ordering::Relaxed)
    }

    /// Apply settings that affect tracking without restarting the engine.
    pub fn apply_settings(&self, settings: &Settings) {
        if let Ok(mut cfg) = self.cfg.write() {
            cfg.score = settings.score.clone();
            cfg.idle_timeout = settings.idle_timeout_secs;
            cfg.goal = settings.goal_minutes;
            cfg.break_minutes = settings.break_reminder_minutes;
            cfg.rev = cfg.rev.wrapping_add(1);
        }
    }

    pub fn snapshot(&self) -> LiveState {
        self.state
            .read()
            .map(|s| s.clone())
            .unwrap_or_else(|_| LiveState::empty(today(), "unavailable"))
    }

    pub fn drain_nudges(&self) -> Vec<Nudge> {
        self.nudges
            .lock()
            .map(|mut n| std::mem::take(&mut *n))
            .unwrap_or_default()
    }

    /// Drop the in-memory day and re-read it from disk. Used after a wipe so the
    /// timeline and statistics cannot keep showing rows that no longer exist.
    pub fn reload(&self) {
        let day = today();
        let samples = self.db.samples(&day).unwrap_or_default();
        let summary = self.db.day(&day).ok().flatten().unwrap_or(DaySummary {
            day: day.clone(),
            ..Default::default()
        });
        let tags = self.db.tags(&day).unwrap_or_default();
        let sessions = sessions_from(&samples, &tags);
        if let Ok(mut s) = self.state.write() {
            let paused = s.paused;
            let manual = s.manual;
            let tracking = s.tracking;
            let source = s.source.clone();
            *s = LiveState::empty(day, &source);
            s.samples = samples;
            s.summary = summary;
            s.sessions = sessions;
            s.paused = paused;
            s.manual = manual;
            s.tracking = tracking;
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        capture::set_paused(false);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

#[allow(clippy::too_many_arguments)]
fn tracker_loop(
    mut source: Box<dyn InputSource>,
    state: Arc<RwLock<LiveState>>,
    paused: Arc<AtomicBool>,
    manual: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    nudges: Arc<Mutex<Vec<Nudge>>>,
    db: Arc<Db>,
    cfg: Arc<RwLock<RuntimeCfg>>,
) {
    let (tx, rx) = sync_channel::<CaptureEvent>(1024);
    if let Err(err) = source.start(tx) {
        if let Ok(mut s) = state.write() {
            s.tracking = false;
            s.last_error = Some(format!("{err}"));
        }
    }

    let initial = cfg.read().map(|c| c.clone()).unwrap_or(RuntimeCfg {
        score: ScoreConfig::default(),
        idle_timeout: 600,
        goal: 120,
        break_minutes: 90,
        rev: 0,
    });
    let mut agg = Aggregator::new(initial.score, initial.idle_timeout);
    let mut applied_rev = initial.rev;
    let mut goal = initial.goal;
    let mut break_minutes = initial.break_minutes;

    let mut day = today();
    let mut paused_now = false;
    let mut last_idle = 0u32;
    let mut deep_streak = 0u32;
    let mut last_minute = 0i64;
    let mut goal_notified = false;
    let mut break_notified = false;
    let mut burnout_checked = false;

    while !stop.load(Ordering::Relaxed) {
        // 1. Settings changed?
        if let Ok(c) = cfg.read() {
            if c.rev != applied_rev {
                goal = c.goal;
                break_minutes = c.break_minutes;
                agg.set_config(c.score.clone(), c.idle_timeout);
                applied_rev = c.rev;
            }
        }

        // 2. Drain events (bounded work per wake-up).
        match rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(event) => {
                let mut batch = vec![event];
                while batch.len() < 256 {
                    match rx.try_recv() {
                        Ok(ev) => batch.push(ev),
                        Err(_) => break,
                    }
                }
                if paused.load(Ordering::Relaxed) {
                    continue;
                }
                for ev in &batch {
                    if let CaptureEvent::Idle { secs, .. } = ev {
                        last_idle = *secs;
                    }
                    if let CaptureEvent::App { .. } = ev {
                        // current app is pulled from the aggregator below
                    }
                    for sample in agg.observe(ev) {
                        record(&db, &state, &sample, &nudges, goal, &mut goal_notified);
                        track_streak(
                            &sample,
                            &mut deep_streak,
                            &mut last_minute,
                            &mut break_notified,
                            break_minutes,
                            &nudges,
                        );
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // 3. Pause / resume: flush the open minute so nothing is lost.
        let is_paused = paused.load(Ordering::Relaxed);
        if is_paused != paused_now {
            if is_paused {
                if let Some(sample) = agg.flush() {
                    record(&db, &state, &sample, &nudges, goal, &mut goal_notified);
                }
            }
            paused_now = is_paused;
        }

        // 4. Manual focus follows the UI.
        if !is_paused {
            agg.set_manual(manual.load(Ordering::Relaxed), capture::now_ms());
        }

        // 5. Close finished minutes.
        for sample in agg.tick(capture::now_ms()) {
            record(&db, &state, &sample, &nudges, goal, &mut goal_notified);
            track_streak(
                &sample,
                &mut deep_streak,
                &mut last_minute,
                &mut break_notified,
                break_minutes,
                &nudges,
            );
        }

        // 6. Refresh live fields for the tray and the ring.
        let (today_now, _) = local_day_and_mod(capture::now_ms());
        if today_now != day {
            day = today_now;
            goal_notified = false;
            deep_streak = 0;
            last_minute = 0;
            break_notified = false;
            let mut fresh = LiveState::empty(day.clone(), &source.label().to_string());
            fresh.samples = db.samples(&day).unwrap_or_default();
            fresh.summary = db.day(&day).ok().flatten().unwrap_or(DaySummary {
                day: day.clone(),
                ..Default::default()
            });
            fresh.sessions = sessions_from(&fresh.samples, &Default::default());
            if let Ok(mut s) = state.write() {
                let manual = s.manual;
                *s = fresh;
                s.manual = manual;
            }
        }
        if !burnout_checked {
            burnout_checked = true;
            if let Ok(days) = db.days_between(&crate::model::shift_day(&day, -30), &day) {
                if let Some(message) = crate::stats::burnout(&days, &day) {
                    if let Ok(mut list) = nudges.lock() {
                        list.push(Nudge {
                            kind: NudgeKind::Burnout,
                            title: "Ease off a little".into(),
                            body: message,
                        });
                    }
                }
            }
        }

        if let Ok(mut s) = state.write() {
            s.live_score = if is_paused || !s.tracking {
                s.summary.score
            } else {
                let partial = agg.partial_score(capture::now_ms());
                if partial > 0 {
                    partial
                } else {
                    s.summary.score
                }
            };
            if !is_paused {
                let app = agg.current_app().to_string();
                if !app.is_empty() {
                    s.current_app = app;
                }
            }
            s.idle_secs = last_idle;
            s.paused = is_paused;
        }
    }

    if let Some(sample) = agg.flush() {
        record(&db, &state, &sample, &nudges, goal, &mut goal_notified);
    }
    source.stop();
}

fn track_streak(
    sample: &MinuteSample,
    deep_streak: &mut u32,
    last_minute: &mut i64,
    break_notified: &mut bool,
    break_minutes: u32,
    nudges: &Arc<Mutex<Vec<Nudge>>>,
) {
    let gap = sample.minute - *last_minute;
    if *last_minute != 0 && gap > 5 {
        *deep_streak = 0;
        *break_notified = false;
    }
    if sample.level.is_deep() {
        *deep_streak += 1;
        if break_minutes >= 10 && *deep_streak == break_minutes && !*break_notified {
            *break_notified = true;
            if let Ok(mut list) = nudges.lock() {
                list.push(Nudge {
                    kind: NudgeKind::Break,
                    title: "Time to stand up".into(),
                    body: format!(
                        "You have been in flow for {break_minutes} minutes. Two minutes away from the screen protects the next block."
                    ),
                });
            }
        }
    } else {
        *deep_streak = 0;
        *break_notified = false;
    }
    *last_minute = sample.minute;
}

fn record(
    db: &Db,
    state: &Arc<RwLock<LiveState>>,
    sample: &MinuteSample,
    nudges: &Arc<Mutex<Vec<Nudge>>>,
    goal: u32,
    goal_notified: &mut bool,
) {
    let summary = match db.record_minute(sample) {
        Ok(summary) => summary,
        Err(err) => {
            if let Ok(mut s) = state.write() {
                s.last_error = Some(format!("Could not save the last minute: {err}"));
            }
            return;
        }
    };
    let mut notify_goal = false;
    if let Ok(mut s) = state.write() {
        if sample.day == s.today {
            s.samples.retain(|m| m.minute != sample.minute);
            s.samples.push(sample.clone());
            s.samples.sort_by_key(|m| m.minute);
            let tags = db.tags(&s.today).unwrap_or_default();
            s.sessions = sessions_from(&s.samples, &tags);
        }
        s.summary = summary;
        if s.summary.deep_minutes >= goal && goal > 0 && !*goal_notified {
            *goal_notified = true;
            notify_goal = true;
        }
    }
    if notify_goal {
        if let Ok(mut list) = nudges.lock() {
            list.push(Nudge {
                kind: NudgeKind::Goal,
                title: "Goal reached".into(),
                body: "You hit your deep-work goal for today. That is the streak protected.".into(),
            });
        }
    }
}
