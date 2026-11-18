//! Demo data for screenshots and first impressions.
//!
//! `--demo` points the database at a temporary folder, seeds 45 days of
//! plausible work, and never writes the user's real settings. The generator is a
//! deterministic LCG, so every launch shows the same map.

use std::path::PathBuf;

use chrono::{Local, NaiveDate, TimeZone};
use focusmap_core::apps::categorize;
use focusmap_core::model::{local_day_and_mod, shift_day, today, FocusLevel, MinuteSample};
use focusmap_core::settings::{Settings, ThemeMode};
use focusmap_core::storage::Db;

pub fn settings() -> Settings {
    Settings {
        theme: ThemeMode::Dark,
        goal_minutes: 120,
        break_reminder_minutes: 90,
        onboarded: true,
        data_dir: demo_dir().display().to_string(),
        ..Default::default()
    }
}

pub fn demo_dir() -> PathBuf {
    std::env::temp_dir().join("FocusMapDemo")
}

/// Seed only when the folder is empty or stale, so relaunching is instant and
/// the map always ends today.
pub fn seed_if_needed(db: &Db, settings: &Settings) {
    let up_to_date = db
        .all_days()
        .map(|days| {
            days.len() >= 40 && days.last().map(|d| d.day == today()).unwrap_or(false)
        })
        .unwrap_or(false);
    if up_to_date {
        return;
    }
    let _ = db.wipe();
    let samples = generate(settings);
    if let Err(err) = db.record_minutes(&samples) {
        eprintln!("demo seed failed: {err}");
        return;
    }
    tag_recent_sessions(db);
}

fn generate(settings: &Settings) -> Vec<MinuteSample> {
    let mut rng = Rng(0x5EED_2026_1115);
    let deep_at = settings.score.deep_threshold;
    let medium_at = settings.score.medium_threshold;
    let today_key = today();
    let (_, now_mod) = local_day_and_mod(focusmap_core::capture::now_ms());
    let mut out = Vec::with_capacity(18_000);
    let mut day_cursor = shift_day(&today_key, -44);

    let block_apps = [
        "Code.exe",
        "WindowsTerminal.exe",
        "chrome.exe",
        "Notepad.exe",
        "Slack.exe",
        "Teams.exe",
    ];

    for index in 0..45 {
        let day = day_cursor.clone();
        day_cursor = shift_day(&day_cursor, 1);
        let weekday = focusmap_core::model::weekday_index(&day);
        let weekend = weekday >= 5;
        // Every eleventh day is a rough one; the last week stays strong so the
        // streak and the trend both look like real life.
        let rough = index % 11 == 3 && index < 37;
        let strong = index >= 37;
        let deep_p = if rough {
            0.28
        } else if weekend {
            0.55
        } else if strong {
            0.74
        } else {
            0.63
        };

        let mut blocks: Vec<(u16, u16, usize)> = if weekend {
            vec![
                (10 * 60 + 20, 12 * 60 + 20, 0),
                (16 * 60 + 10, 17 * 60 + 30, 2),
            ]
        } else if rough {
            vec![
                (9 * 60 + 20, 11 * 60 + 10, 2),
                (13 * 60 + 40, 15 * 60 + 10, 4),
                (16 * 60, 17 * 60 + 10, 1),
            ]
        } else {
            vec![
                (8 * 60 + 40 + rng.range(0, 25) as u16, 12 * 60 + rng.range(0, 15) as u16, 0),
                (13 * 60 + 15 + rng.range(0, 20) as u16, 17 * 60 + rng.range(0, 30) as u16, 0),
                (19 * 60 + 40, 20 * 60 + 40 + rng.range(0, 35) as u16, 1),
            ]
        };
        if strong {
            blocks.push((18 * 60 + 20, 19 * 60 + 20, 1));
        }

        let base = day_base_minute(&day);
        let limit = if day == today_key { now_mod } else { 1440 };
        for (start, end, app_idx) in blocks {
            if start >= limit {
                continue;
            }
            let end = end.min(limit);
            let primary = block_apps[app_idx % block_apps.len()];
            let secondary = if rng.f32() < 0.5 {
                "chrome.exe"
            } else {
                "WindowsTerminal.exe"
            };
            for minute_of_day in start..end {
                if rng.f32() < 0.045 {
                    continue;
                }
                let roll = rng.f32();
                let app = if rng.f32() < 0.09 { secondary } else { primary };
                // A demo day should visit every district: docs mid-morning,
                // meetings after lunch, music in the evening.
                let side = if minute_of_day < 12 * 60 {
                    "Notion.exe"
                } else if minute_of_day >= 18 * 60 {
                    "Spotify.exe"
                } else {
                    "Zoom.exe"
                };
                let app = if rng.f32() < 0.07 { side } else { app };
                // Chat afternoons on rough days, and a short video break.
                let app = if rough && (15 * 60..16 * 60).contains(&minute_of_day) {
                    "Slack.exe"
                } else {
                    app
                };
                let (score, keys, backspaces, clicks, px, switches, active, pauses) =
                    if roll < deep_p {
                        let s = deep_at + rng.range(1, (100 - deep_at).max(2) as u32) as u8;
                        (
                            s.min(97),
                            rng.range(80, 185),
                            rng.range(2, 10),
                            rng.range(0, 5),
                            rng.range(280, 1200),
                            rng.range(0, 2),
                            rng.range(53, 60) as u8,
                            rng.range(0, 4) as u8,
                        )
                    } else if roll < deep_p + 0.27 {
                        let s = medium_at
                            + rng.range(0, (deep_at - medium_at).max(2) as u32) as u8;
                        (
                            s.min(deep_at - 1),
                            rng.range(22, 95),
                            rng.range(1, 9),
                            rng.range(2, 11),
                            rng.range(700, 2600),
                            rng.range(1, 4),
                            rng.range(35, 56) as u8,
                            rng.range(2, 8) as u8,
                        )
                    } else {
                        (
                            rng.range(14, medium_at.max(18) as u32) as u8,
                            rng.range(0, 42),
                            rng.range(0, 5),
                            rng.range(2, 17),
                            rng.range(1200, 4600),
                            rng.range(3, 9),
                            rng.range(8, 35) as u8,
                            rng.range(7, 17) as u8,
                        )
                    };
                let category = categorize(app);
                let score = score.min(category.score_cap());
                out.push(MinuteSample {
                    minute: base + minute_of_day as i64,
                    day: day.clone(),
                    minute_of_day,
                    score,
                    level: FocusLevel::from_score(score, deep_at, medium_at),
                    keys,
                    backspaces,
                    clicks,
                    mouse_px: px,
                    switches,
                    active_secs: active,
                    pauses,
                    app: app.to_string(),
                    category,
                    manual: false,
                });
            }
        }
    }
    out
}

fn tag_recent_sessions(db: &Db) {
    let tags = ["Deep work", "Coding", "Writing", "Review"];
    for i in 0..7 {
        let day = shift_day(&today(), -i);
        let sessions = db.sessions(&day).unwrap_or_default();
        for (n, session) in sessions.iter().take(2).enumerate() {
            let _ = db.set_tag(
                &day,
                session.start_minute,
                Some(tags[(i as usize + n) % tags.len()]),
            );
        }
    }
}

fn day_base_minute(day: &str) -> i64 {
    let Ok(date) = NaiveDate::parse_from_str(day, "%Y-%m-%d") else {
        return 0;
    };
    let Some(naive) = date.and_hms_opt(0, 0, 0) else {
        return 0;
    };
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.timestamp() / 60)
        .unwrap_or(0)
}

/// Small deterministic generator. Not cryptographic; it only needs to look
/// human over 45 days.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, low: u32, high: u32) -> u32 {
        if high <= low {
            return low;
        }
        low + (self.next() % (high - low) as u64) as u32
    }
}
