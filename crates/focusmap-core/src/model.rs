//! Data model shared by the tracker, the database and the UI.
//!
//! Everything here is deliberately small and `Copy`-ish: the app keeps at most
//! one day of minute samples in memory and everything else stays in SQLite.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::apps::AppCategory;

/// Focus classes used by the timeline colours and the stats queries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusLevel {
    Deep,
    Medium,
    Fragmented,
    Idle,
}

impl FocusLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            FocusLevel::Deep => "deep",
            FocusLevel::Medium => "medium",
            FocusLevel::Fragmented => "fragmented",
            FocusLevel::Idle => "idle",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "deep" => FocusLevel::Deep,
            "medium" => FocusLevel::Medium,
            "fragmented" => FocusLevel::Fragmented,
            _ => FocusLevel::Idle,
        }
    }

    pub fn from_score(score: u8, deep_threshold: u8, medium_threshold: u8) -> Self {
        if score >= deep_threshold {
            FocusLevel::Deep
        } else if score >= medium_threshold {
            FocusLevel::Medium
        } else if score >= 15 {
            FocusLevel::Fragmented
        } else {
            FocusLevel::Idle
        }
    }

    pub fn is_deep(self) -> bool {
        matches!(self, FocusLevel::Deep)
    }
}

/// One scored minute. This is the smallest unit that ever reaches the disk.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinuteSample {
    /// Unix minute (UTC) - the primary key.
    pub minute: i64,
    /// Local day, `YYYY-MM-DD`. Stored so day queries never need a timezone.
    pub day: String,
    /// Local minute of day, 0..=1439.
    pub minute_of_day: u16,
    pub score: u8,
    pub level: FocusLevel,
    pub keys: u32,
    pub backspaces: u32,
    pub clicks: u32,
    pub mouse_px: u32,
    pub switches: u32,
    pub active_secs: u8,
    pub pauses: u8,
    pub app: String,
    pub category: AppCategory,
    pub manual: bool,
}

impl MinuteSample {
    pub fn hour(&self) -> u8 {
        (self.minute_of_day / 60) as u8
    }

    pub fn clock(&self) -> String {
        format!("{:02}:{:02}", self.minute_of_day / 60, self.minute_of_day % 60)
    }
}

/// A stretch of sustained deep focus. Derived from minute samples, so it can
/// never drift out of sync with the data it describes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub day: String,
    pub start_minute: i64,
    pub start_mod: u16,
    pub end_mod: u16,
    pub minutes: u32,
    pub avg_score: u8,
    pub top_app: String,
    pub tag: Option<String>,
}

impl Session {
    pub fn time_range(&self) -> String {
        let end = self.end_mod + 1;
        format!(
            "{:02}:{:02} - {:02}:{:02}",
            self.start_mod / 60,
            self.start_mod % 60,
            end / 60,
            end % 60
        )
    }
}

/// Daily rollup. Written after every minute so a crash can lose at most one
/// minute of tracking, and never a whole day.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DaySummary {
    pub day: String,
    pub score: u8,
    pub deep_minutes: u32,
    pub focus_minutes: u32,
    pub tracked_minutes: u32,
    pub longest_session_minutes: u32,
    pub switches: u32,
    pub top_app: String,
}

/// Group deep minutes into sessions. A gap of up to two minutes is tolerated
/// (a pause to think is still the same session); anything longer starts a new
/// one. Runs shorter than two minutes are dropped as noise.
pub fn sessions_from(samples: &[MinuteSample], tags: &HashMap<i64, String>) -> Vec<Session> {
    let mut deep: Vec<&MinuteSample> = samples.iter().filter(|s| s.level.is_deep()).collect();
    deep.sort_by_key(|s| s.minute);
    let mut out = Vec::new();
    let mut run: Vec<&MinuteSample> = Vec::new();
    for sample in deep {
        let continues = run
            .last()
            .map(|prev| sample.minute - prev.minute <= 3)
            .unwrap_or(true);
        if !continues {
            push_session(&mut out, &run, tags);
            run.clear();
        }
        run.push(sample);
    }
    push_session(&mut out, &run, tags);
    out
}

fn push_session(out: &mut Vec<Session>, run: &[&MinuteSample], tags: &HashMap<i64, String>) {
    if run.len() < 2 {
        return;
    }
    let start = run[0];
    let end = run[run.len() - 1];
    let mut app_counts: HashMap<&str, u32> = HashMap::new();
    let mut score_sum = 0u32;
    for s in run {
        *app_counts.entry(s.app.as_str()).or_default() += 1;
        score_sum += s.score as u32;
    }
    let top_app = app_counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(a, _)| a.to_string())
        .unwrap_or_default();
    out.push(Session {
        day: start.day.clone(),
        start_minute: start.minute,
        start_mod: start.minute_of_day,
        end_mod: end.minute_of_day,
        minutes: run.len() as u32,
        avg_score: (score_sum / run.len() as u32) as u8,
        top_app,
        tag: tags.get(&start.minute).cloned(),
    });
}

/// Local day + minute-of-day for a unix millisecond timestamp.
pub fn local_day_and_mod(ts_ms: i64) -> (String, u16) {
    use chrono::{DateTime, Local, Timelike};
    let dt = DateTime::from_timestamp_millis(ts_ms)
        .unwrap_or_default()
        .with_timezone(&Local);
    let mod_min = (dt.time().num_seconds_from_midnight() / 60) as u16;
    (dt.format("%Y-%m-%d").to_string(), mod_min)
}

/// Today in local time.
pub fn today() -> String {
    local_day_and_mod(chrono::Utc::now().timestamp_millis()).0
}

/// Shift a `YYYY-MM-DD` key by `days` (negative goes back).
pub fn shift_day(day: &str, days: i64) -> String {
    use chrono::NaiveDate;
    NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|d| {
            (d + chrono::Duration::days(days))
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_else(|_| day.to_string())
}

/// Inclusive list of day keys ending at `end`.
pub fn day_range(end: &str, count: u32) -> Vec<String> {
    (0..count)
        .rev()
        .map(|i| shift_day(end, -(i as i64)))
        .collect()
}

/// Weekday index (0 = Monday) for a day key.
pub fn weekday_index(day: &str) -> u32 {
    use chrono::Datelike;
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|d| d.weekday().num_days_from_monday())
        .unwrap_or(0)
}

/// Human date, e.g. `Sat 15 Nov`.
pub fn pretty_day(day: &str) -> String {
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|d| d.format("%a %d %b").to_string())
        .unwrap_or_else(|_| day.to_string())
}

/// Format minutes as `2h 15m`.
pub fn format_minutes(minutes: u32) -> String {
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}
