//! Derived insight: streaks, records, trends, best hours, distraction report.
//!
//! Everything here is a pure function over rows the database already returned.
//! That keeps the queries dumb and the logic testable.

use std::collections::HashMap;

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::apps::AppCategory;
use crate::model::{shift_day, DaySummary, Session};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HourStat {
    pub hour: u8,
    pub avg_score: f32,
    pub minutes: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TagStat {
    pub tag: String,
    pub minutes: u32,
    pub sessions: u32,
    pub avg_score: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppStat {
    pub app: String,
    pub category: AppCategory,
    pub minutes: u32,
    pub switches: u32,
    pub avg_score: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Records {
    pub best_day: Option<(String, u8)>,
    pub longest_session_minutes: u32,
    pub best_streak: u32,
    pub total_days: u32,
    pub total_deep_minutes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Trend {
    pub this_week: Option<f32>,
    pub last_week: Option<f32>,
}

impl Trend {
    /// Percent change, positive is better.
    pub fn change(&self) -> Option<f32> {
        match (self.this_week, self.last_week) {
            (Some(now), Some(before)) if before > 0.0 && now > 0.0 => {
                Some((now - before) / before * 100.0)
            }
            _ => None,
        }
    }
}

/// Consecutive days that hit the deep-work goal.
///
/// Today gets a grace period: the streak only breaks once today is over, which
/// is how fitness apps behave and stops the number flickering all morning.
pub fn streak(days: &[DaySummary], today: &str, goal_minutes: u32) -> u32 {
    let by_day: HashMap<&str, &DaySummary> = days.iter().map(|d| (d.day.as_str(), d)).collect();
    let goal = goal_minutes.max(1);
    let meets = |day: &str| {
        by_day
            .get(day)
            .map(|d| d.deep_minutes >= goal)
            .unwrap_or(false)
    };
    let mut cursor = if meets(today) {
        today.to_string()
    } else {
        shift_day(today, -1)
    };
    let mut count = 0;
    while meets(&cursor) {
        count += 1;
        cursor = shift_day(&cursor, -1);
    }
    count
}

pub fn best_streak(days: &[DaySummary], goal_minutes: u32) -> u32 {
    let goal = goal_minutes.max(1);
    let mut sorted: Vec<&DaySummary> = days.iter().collect();
    sorted.sort_by(|a, b| a.day.cmp(&b.day));
    let mut best = 0;
    let mut run = 0;
    let mut prev: Option<&str> = None;
    for day in sorted {
        let hit = day.deep_minutes >= goal;
        let consecutive = prev
            .map(|p| shift_day(p, 1) == day.day)
            .unwrap_or(false);
        if hit {
            run = if consecutive { run + 1 } else { 1 };
        } else {
            run = 0;
        }
        best = best.max(run);
        prev = Some(&day.day);
    }
    best
}

pub fn trend(days: &[DaySummary], today: &str) -> Trend {
    let week = |end: &str| {
        let mut scores = Vec::new();
        for i in 0..7 {
            let key = shift_day(end, -i);
            if let Some(d) = days.iter().find(|d| d.day == key) {
                if d.tracked_minutes > 0 {
                    scores.push(d.score as f32);
                }
            }
        }
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().sum::<f32>() / scores.len() as f32)
        }
    };
    let this_end = shift_day(today, -1);
    let last_end = shift_day(today, -8);
    Trend {
        this_week: week(&this_end),
        last_week: week(&last_end),
    }
}

pub fn records(days: &[DaySummary], streak_now: u32, goal: u32) -> Records {
    let best_day = days
        .iter()
        .filter(|d| d.tracked_minutes >= 10)
        .max_by_key(|d| d.score)
        .map(|d| (d.day.clone(), d.score));
    Records {
        best_day,
        longest_session_minutes: days
            .iter()
            .map(|d| d.longest_session_minutes)
            .max()
            .unwrap_or(0),
        best_streak: best_streak(days, goal).max(streak_now),
        total_days: days.iter().filter(|d| d.tracked_minutes > 0).count() as u32,
        total_deep_minutes: days.iter().map(|d| d.deep_minutes as u64).sum(),
    }
}

pub fn best_hour(hours: &[HourStat]) -> Option<u8> {
    hours
        .iter()
        .filter(|h| h.minutes >= 5)
        .max_by(|a, b| a.avg_score.total_cmp(&b.avg_score))
        .map(|h| h.hour)
}

pub fn best_day(days: &[DaySummary]) -> Option<(String, u8)> {
    days.iter()
        .filter(|d| d.tracked_minutes >= 20)
        .max_by_key(|d| d.score)
        .map(|d| (d.day.clone(), d.score))
}

/// Deep work grouped by the tag a session was given. Untagged sessions are
/// left out: there is nothing to compare there.
pub fn tag_totals(sessions: &[Session]) -> Vec<TagStat> {
    let mut groups: HashMap<&str, TagStat> = HashMap::new();
    for session in sessions.iter() {
        let Some(raw) = session.tag.as_deref() else {
            continue;
        };
        let tag = raw.trim();
        if tag.is_empty() {
            continue;
        }
        let row = groups.entry(tag).or_insert_with(|| TagStat {
            tag: tag.to_string(),
            ..Default::default()
        });
        row.minutes += session.minutes;
        row.sessions += 1;
        row.avg_score += session.avg_score as f32;
    }
    let mut out: Vec<TagStat> = groups.into_values().collect();
    for row in &mut out {
        row.avg_score /= row.sessions as f32;
    }
    out.sort_by(|a, b| b.minutes.cmp(&a.minutes));
    out
}

/// Weekday with the best average score. Sparse days are ignored so one lucky
/// afternoon does not crown a weekday.
pub fn best_weekday(days: &[DaySummary]) -> Option<(&'static str, f32)> {
    const NAMES: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut sums = [0.0f32; 7];
    let mut counts = [0u32; 7];
    for d in days.iter().filter(|d| d.tracked_minutes >= 20) {
        let Ok(date) = chrono::NaiveDate::parse_from_str(&d.day, "%Y-%m-%d") else {
            continue;
        };
        let i = date.weekday().num_days_from_monday() as usize;
        sums[i] += d.score as f32;
        counts[i] += 1;
    }
    let mut best: Option<(usize, f32)> = None;
    for i in 0..7 {
        if counts[i] == 0 {
            continue;
        }
        let avg = sums[i] / counts[i] as f32;
        if best.is_none_or(|(_, value)| avg > value) {
            best = Some((i, avg));
        }
    }
    best.map(|(i, avg)| (NAMES[i], avg))
}

/// Top hours, best first. Used for "your best windows tomorrow".
pub fn predicted_hours(hours: &[HourStat], count: usize) -> Vec<HourStat> {
    let mut usable: Vec<HourStat> = hours
        .iter()
        .filter(|h| h.minutes >= 10)
        .cloned()
        .collect();
    usable.sort_by(|a, b| b.avg_score.total_cmp(&a.avg_score));
    usable.truncate(count);
    usable
}

/// A gentle warning when the last few days are clearly worse than the previous
/// few. Deliberately conservative: this must never nag a healthy week.
pub fn burnout(days: &[DaySummary], today: &str) -> Option<String> {
    let avg = |end: &str, span: i64| -> Option<f32> {
        let mut values = Vec::new();
        for i in 0..span {
            let key = shift_day(end, -i);
            if let Some(d) = days.iter().find(|d| d.day == key) {
                if d.tracked_minutes >= 20 {
                    values.push(d.score as f32);
                }
            }
        }
        if values.len() >= 3 {
            Some(values.iter().sum::<f32>() / values.len() as f32)
        } else {
            None
        }
    };
    let recent = avg(&shift_day(today, -1), 5)?;
    let before = avg(&shift_day(today, -6), 5)?;
    if before >= 5.0 && recent < before * 0.8 && recent < 60.0 {
        Some(format!(
            "Your focus has dropped {}% over the last five days. A lighter day and an early night may do more than another push.",
            ((before - recent) / before * 100.0).round() as i32
        ))
    } else {
        None
    }
}

/// One plain-language tip, chosen from whichever signal is strongest.
pub fn tip(
    hours: &[HourStat],
    apps: &[AppStat],
    days: &[DaySummary],
    today: &str,
    goal: u32,
) -> String {
    if days.iter().all(|d| d.tracked_minutes == 0) {
        return "Your map fills itself as you work. Keep this running while you do normal work and come back tomorrow.".into();
    }
    if let Some(change) = trend(days, today).change() {
        if change >= 10.0 {
            return format!(
                "You are up {}% versus last week. That is a real improvement, not noise.",
                change.round() as i32
            );
        }
    }
    if let Some(app) = apps
        .iter()
        .filter(|a| matches!(a.category, AppCategory::Chat))
        .max_by_key(|a| a.switches)
    {
        if app.switches >= 10 {
            return format!(
                "{} is behind most of your context switches. Try checking it in one block after a session instead of in between.",
                app.app
            );
        }
    }
    if let Some(hour) = best_hour(hours) {
        return format!(
            "Protect {:02}:00-{:02}:00 tomorrow. It is your strongest hour and the one most worth defending.",
            hour,
            hour + 1
        );
    }
    if let Some(day) = days.iter().filter(|d| d.tracked_minutes > 0).last() {
        if day.deep_minutes < goal / 2 {
            return "Short, unbroken stretches beat long, interrupted ones. Try one 25 minute block before checking anything else.".into();
        }
    }
    "Steady beats intense. Your data rewards rhythm, not long hours.".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::day_range;

    fn day(key: &str, score: u8, deep: u32) -> DaySummary {
        DaySummary {
            day: key.into(),
            score,
            deep_minutes: deep,
            focus_minutes: deep,
            tracked_minutes: 60,
            longest_session_minutes: 30,
            switches: 4,
            top_app: "Code.exe".into(),
        }
    }

    #[test]
    fn streak_counts_backwards_with_today_grace() {
        let today = "2026-11-15";
        let days: Vec<DaySummary> = day_range(today, 5)
            .into_iter()
            .map(|d| day(&d, 80, 120))
            .collect();
        assert_eq!(streak(&days, today, 120), 5);
        // Today is still in progress: the streak holds from yesterday.
        let mut partial = days.clone();
        partial.last_mut().unwrap().deep_minutes = 10;
        assert_eq!(streak(&partial, today, 120), 4);
        // Missing yesterday breaks it.
        let broken: Vec<DaySummary> = days
            .iter()
            .filter(|d| d.day != shift_day(today, -1))
            .cloned()
            .collect();
        assert_eq!(streak(&broken, today, 120), 1);
    }

    #[test]
    fn best_streak_ignores_the_gap() {
        let days = vec![
            day("2026-11-01", 70, 100),
            day("2026-11-02", 70, 100),
            day("2026-11-03", 70, 100),
            day("2026-11-06", 70, 100),
            day("2026-11-07", 70, 100),
        ];
        assert_eq!(best_streak(&days, 60), 3);
    }

    #[test]
    fn trend_compares_two_weeks() {
        let today = "2026-11-15";
        let mut days = Vec::new();
        for i in 0..7 {
            let key = shift_day(today, -(i + 1));
            days.push(day(&key, 80, 120));
        }
        for i in 0..7 {
            let key = shift_day(today, -(i + 8));
            days.push(day(&key, 50, 60));
        }
        let t = trend(&days, today);
        let change = t.change().unwrap();
        assert!((change - 60.0).abs() < 1.0, "change was {change}");
    }

    #[test]
    fn tag_totals_group_tagged_deep_sessions() {
        let session = |tag: Option<&str>, minutes, score| Session {
            day: "2026-11-16".into(),
            start_minute: 0,
            start_mod: 540,
            end_mod: 600,
            minutes,
            avg_score: score,
            top_app: "Code.exe".into(),
            tag: tag.map(str::to_string),
        };
        let rows = tag_totals(&[
            session(Some("Project A"), 30, 80),
            session(Some("Project A"), 20, 60),
            session(Some("Project B"), 10, 90),
            session(None, 40, 70),
        ]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].tag, "Project A");
        assert_eq!(rows[0].minutes, 50);
        assert_eq!(rows[0].sessions, 2);
        assert_eq!(rows[0].avg_score, 70.0);
    }

    #[test]
    fn best_weekday_uses_the_average_of_all_that_day() {
        // 2026-11-16 is a Monday, 17 a Tuesday.
        let days = vec![
            day("2026-11-16", 80, 100),
            day("2026-11-17", 60, 100),
            day("2026-11-23", 90, 100),
        ];
        assert_eq!(best_weekday(&days), Some(("Mon", 85.0)));
        assert_eq!(best_weekday(&days[1..2]), Some(("Tue", 60.0)));
    }

    #[test]
    fn burnout_needs_a_clear_drop() {
        let today = "2026-11-15";
        let mut days = Vec::new();
        for i in 0..5 {
            days.push(day(&shift_day(today, -(i + 1)), 40, 30));
            days.push(day(&shift_day(today, -(i + 6)), 80, 120));
        }
        assert!(burnout(&days, today).is_some());
        for d in days.iter_mut() {
            d.score = 75;
        }
        assert!(burnout(&days, today).is_none());
    }

    #[test]
    fn best_hour_needs_enough_data() {
        let hours = vec![
            HourStat { hour: 9, avg_score: 40.0, minutes: 60 },
            HourStat { hour: 10, avg_score: 90.0, minutes: 60 },
            HourStat { hour: 11, avg_score: 99.0, minutes: 2 },
        ];
        assert_eq!(best_hour(&hours), Some(10));
        assert_eq!(predicted_hours(&hours, 3).len(), 2);
    }

    #[test]
    fn records_pick_the_best_day() {
        let days = vec![
            day("2026-11-10", 70, 100),
            day("2026-11-11", 91, 140),
            day("2026-11-12", 60, 20),
        ];
        let r = records(&days, 0, 120);
        assert_eq!(r.best_day.unwrap().1, 91);
        assert_eq!(r.longest_session_minutes, 30);
        assert_eq!(r.total_days, 3);
    }
}
