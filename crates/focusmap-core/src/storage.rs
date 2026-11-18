//! SQLite storage: one small file, incremental writes, bounded history.
//!
//! Design notes:
//! - one row per scored minute, keyed by unix minute, with a local `day` and
//!   `minute_of_day` so stats never need timezone arithmetic in SQL;
//! - the daily rollup is rewritten after every minute, so a crash loses at most
//!   the minute in flight;
//! - rows older than 90 days are compacted away, keeping the daily rollup. The
//!   file stays small forever instead of growing without bound.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{sessions_from, DaySummary, FocusLevel, MinuteSample, Session};
use crate::settings::Settings;
use crate::stats::{AppStat, HourStat};

pub const SCHEMA_VERSION: i32 = 1;
pub const COMPACT_AFTER_DAYS: i64 = 90;

pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct DataCounts {
    pub minutes: u64,
    pub days: u64,
    pub tags: u64,
    pub bytes: u64,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA temp_store=MEMORY;
             PRAGMA foreign_keys=ON;",
        )?;
        conn.busy_timeout(Duration::from_secs(2))?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
        f(&conn)
    }

    /// Store one minute and refresh its daily rollup. Idempotent: re-inserting a
    /// minute (after a crash or a settings change) replaces it.
    pub fn record_minute(&self, s: &MinuteSample) -> Result<DaySummary> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO minute_samples
                   (minute, day, hour, mod, score, level, keys, backspaces, clicks,
                    mouse_px, switches, active_secs, pauses, app, category, manual)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                 ON CONFLICT(minute) DO UPDATE SET
                   day=excluded.day, hour=excluded.hour, mod=excluded.mod,
                   score=excluded.score, level=excluded.level, keys=excluded.keys,
                   backspaces=excluded.backspaces, clicks=excluded.clicks,
                   mouse_px=excluded.mouse_px, switches=excluded.switches,
                   active_secs=excluded.active_secs, pauses=excluded.pauses,
                   app=excluded.app, category=excluded.category, manual=excluded.manual",
                params![
                    s.minute,
                    s.day,
                    s.hour(),
                    s.minute_of_day,
                    s.score,
                    s.level.as_str(),
                    s.keys,
                    s.backspaces,
                    s.clicks,
                    s.mouse_px,
                    s.switches,
                    s.active_secs,
                    s.pauses,
                    s.app,
                    s.category.as_str(),
                    s.manual as i64,
                ],
            )?;
            refresh_day(conn, &s.day)
        })
    }

    /// Bulk insert used by the demo seeder. One transaction, one rollup rebuild
    /// per affected day, so seeding 45 days stays fast instead of doing 20k
    /// single-minute commits.
    pub fn record_minutes(&self, samples: &[MinuteSample]) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO minute_samples
                       (minute, day, hour, mod, score, level, keys, backspaces, clicks,
                        mouse_px, switches, active_secs, pauses, app, category, manual)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                     ON CONFLICT(minute) DO UPDATE SET
                       day=excluded.day, hour=excluded.hour, mod=excluded.mod,
                       score=excluded.score, level=excluded.level, keys=excluded.keys,
                       backspaces=excluded.backspaces, clicks=excluded.clicks,
                       mouse_px=excluded.mouse_px, switches=excluded.switches,
                       active_secs=excluded.active_secs, pauses=excluded.pauses,
                       app=excluded.app, category=excluded.category, manual=excluded.manual",
                )?;
                for s in samples {
                    stmt.execute(params![
                        s.minute,
                        s.day,
                        s.hour(),
                        s.minute_of_day,
                        s.score,
                        s.level.as_str(),
                        s.keys,
                        s.backspaces,
                        s.clicks,
                        s.mouse_px,
                        s.switches,
                        s.active_secs,
                        s.pauses,
                        s.app,
                        s.category.as_str(),
                        s.manual as i64,
                    ])?;
                }
            }
            let mut days: Vec<&str> = samples.iter().map(|s| s.day.as_str()).collect();
            days.sort_unstable();
            days.dedup();
            for day in days {
                refresh_day(&tx, day)?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn refresh_day(&self, day: &str) -> Result<DaySummary> {
        self.with(|conn| refresh_day(conn, day))
    }

    pub fn day(&self, day: &str) -> Result<Option<DaySummary>> {
        self.with(|conn| {
            conn.query_row(
                "SELECT day, score, deep_minutes, focus_minutes, tracked_minutes,
                        longest_session_minutes, switches, top_app
                 FROM days WHERE day = ?1",
                params![day],
                row_to_day,
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub fn days_between(&self, from: &str, to: &str) -> Result<Vec<DaySummary>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT day, score, deep_minutes, focus_minutes, tracked_minutes,
                        longest_session_minutes, switches, top_app
                 FROM days WHERE day >= ?1 AND day <= ?2 ORDER BY day",
            )?;
            let rows = stmt.query_map(params![from, to], row_to_day)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
        })
    }

    pub fn all_days(&self) -> Result<Vec<DaySummary>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT day, score, deep_minutes, focus_minutes, tracked_minutes,
                        longest_session_minutes, switches, top_app
                 FROM days ORDER BY day",
            )?;
            let rows = stmt.query_map([], row_to_day)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
        })
    }

    pub fn samples(&self, day: &str) -> Result<Vec<MinuteSample>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT minute, day, mod, score, level, keys, backspaces, clicks, mouse_px,
                        switches, active_secs, pauses, app, category, manual
                 FROM minute_samples WHERE day = ?1 ORDER BY minute",
            )?;
            let rows = stmt.query_map(params![day], |row| {
                Ok(MinuteSample {
                    minute: row.get(0)?,
                    day: row.get(1)?,
                    minute_of_day: row.get(2)?,
                    score: row.get(3)?,
                    level: FocusLevel::from_str(&row.get::<_, String>(4)?),
                    keys: row.get(5)?,
                    backspaces: row.get(6)?,
                    clicks: row.get(7)?,
                    mouse_px: row.get(8)?,
                    switches: row.get(9)?,
                    active_secs: row.get(10)?,
                    pauses: row.get(11)?,
                    app: row.get(12)?,
                    category: crate::apps::AppCategory::from_str(&row.get::<_, String>(13)?),
                    manual: row.get::<_, i64>(14)? != 0,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
        })
    }

    pub fn sessions(&self, day: &str) -> Result<Vec<Session>> {
        let samples = self.samples(day)?;
        let tags = self.tags(day)?;
        Ok(sessions_from(&samples, &tags))
    }

    /// Tagged sessions over a window, for the tag comparison insight. Reuses
    /// the day-by-day reads so the tags stay attached to their own sessions.
    pub fn sessions_since(&self, from_day: &str) -> Result<Vec<Session>> {
        let days: Vec<String> = self
            .all_days()?
            .into_iter()
            .filter(|d| d.day.as_str() >= from_day)
            .map(|d| d.day)
            .collect();
        let mut out = Vec::new();
        for day in days {
            out.extend(self.sessions(&day)?);
        }
        Ok(out)
    }

    pub fn tags(&self, day: &str) -> Result<HashMap<i64, String>> {
        self.with(|conn| {
            let mut stmt = conn
                .prepare("SELECT start_minute, tag FROM session_tags WHERE day = ?1")?;
            let rows = stmt.query_map(params![day], |row| Ok((row.get(0)?, row.get(1)?)))?;
            let mut map = HashMap::new();
            for row in rows {
                let (start, tag): (i64, String) = row?;
                map.insert(start, tag);
            }
            Ok(map)
        })
    }

    pub fn set_tag(&self, day: &str, start_minute: i64, tag: Option<&str>) -> Result<()> {
        self.with(|conn| {
            match tag {
                Some(tag) if !tag.trim().is_empty() => conn.execute(
                    "INSERT INTO session_tags (day, start_minute, tag) VALUES (?1, ?2, ?3)
                     ON CONFLICT(day, start_minute) DO UPDATE SET tag = excluded.tag",
                    params![day, start_minute, tag.trim()],
                )?,
                _ => conn.execute(
                    "DELETE FROM session_tags WHERE day = ?1 AND start_minute = ?2",
                    params![day, start_minute],
                )?,
            };
            Ok(())
        })
    }

    pub fn hour_stats(&self, from_day: &str) -> Result<Vec<HourStat>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT hour, AVG(score), COUNT(*) FROM minute_samples
                 WHERE day >= ?1 AND score > 0 GROUP BY hour ORDER BY hour",
            )?;
            let rows = stmt.query_map(params![from_day], |row| {
                Ok(HourStat {
                    hour: row.get(0)?,
                    avg_score: row.get(1)?,
                    minutes: row.get(2)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
        })
    }

    pub fn app_stats(&self, from_day: &str) -> Result<Vec<AppStat>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT app, category, COUNT(*), SUM(switches), AVG(score)
                 FROM minute_samples WHERE day >= ?1 AND score > 0
                 GROUP BY app ORDER BY COUNT(*) DESC LIMIT 8",
            )?;
            let rows = stmt.query_map(params![from_day], |row| {
                Ok(AppStat {
                    app: row.get(0)?,
                    category: crate::apps::AppCategory::from_str(&row.get::<_, String>(1)?),
                    minutes: row.get(2)?,
                    switches: row.get(3)?,
                    avg_score: row.get(4)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
        })
    }

    pub fn counts(&self) -> Result<DataCounts> {
        self.with(|conn| {
            let minutes: i64 =
                conn.query_row("SELECT COUNT(*) FROM minute_samples", [], |r| r.get(0))?;
            let days: i64 = conn.query_row("SELECT COUNT(*) FROM days", [], |r| r.get(0))?;
            let tags: i64 = conn.query_row("SELECT COUNT(*) FROM session_tags", [], |r| r.get(0))?;
            Ok(DataCounts {
                minutes: minutes as u64,
                days: days as u64,
                tags: tags as u64,
                bytes: 0,
            })
        })
        .map(|mut c| {
            c.bytes = self.size_bytes();
            c
        })
    }

    pub fn size_bytes(&self) -> u64 {
        let main = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        let wal = std::fs::metadata(self.path.with_extension("db-wal"))
            .map(|m| m.len())
            .unwrap_or(0);
        main + wal
    }

    /// Drop minute-level rows older than the cutoff, keeping the daily rollup.
    /// This is what keeps the file at a few megabytes after years of use.
    pub fn compact(&self, today: &str) -> Result<u64> {
        let cutoff = crate::model::shift_day(today, -COMPACT_AFTER_DAYS);
        let removed = self.with(|conn| {
            let removed = conn.execute(
                "DELETE FROM minute_samples WHERE day < ?1",
                params![cutoff],
            )?;
            conn.execute("DELETE FROM session_tags WHERE day < ?1", params![cutoff])?;
            Ok(removed as u64)
        })?;
        if removed > 5_000 {
            let _ = self.with(|conn| {
                conn.execute_batch("VACUUM; PRAGMA optimize;")?;
                Ok(())
            });
        }
        Ok(removed)
    }

    /// One-click wipe: sample rows, rollups and tags. Settings stay.
    pub fn wipe(&self) -> Result<()> {
        self.with(|conn| {
            conn.execute_batch(
                "DELETE FROM minute_samples;
                 DELETE FROM days;
                 DELETE FROM session_tags;
                 VACUUM;",
            )?;
            Ok(())
        })
    }

    pub fn export_bundle(&self) -> Result<crate::export::ExportBundle> {
        let days = self.all_days()?;
        let mut minutes = Vec::new();
        for day in &days {
            minutes.extend(self.samples(&day.day)?);
        }
        let mut tags = Vec::new();
        self.with(|conn| {
            let mut stmt =
                conn.prepare("SELECT day, start_minute, tag FROM session_tags ORDER BY day")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            for row in rows {
                tags.push(row?);
            }
            Ok(())
        })?;
        Ok(crate::export::ExportBundle {
            app: "FocusMap".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            note: "Timing counts only. No keystrokes, no text, no window titles.".into(),
            days,
            minutes,
            tags,
        })
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE IF NOT EXISTS minute_samples (
            minute      INTEGER PRIMARY KEY,
            day         TEXT NOT NULL,
            hour        INTEGER NOT NULL,
            mod         INTEGER NOT NULL,
            score       INTEGER NOT NULL,
            level       TEXT NOT NULL,
            keys        INTEGER NOT NULL,
            backspaces  INTEGER NOT NULL,
            clicks      INTEGER NOT NULL,
            mouse_px    INTEGER NOT NULL,
            switches    INTEGER NOT NULL,
            active_secs INTEGER NOT NULL,
            pauses      INTEGER NOT NULL,
            app         TEXT NOT NULL,
            category    TEXT NOT NULL,
            manual      INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX IF NOT EXISTS idx_minute_day ON minute_samples(day);
         CREATE INDEX IF NOT EXISTS idx_minute_day_hour ON minute_samples(day, hour);

         CREATE TABLE IF NOT EXISTS days (
            day                    TEXT PRIMARY KEY,
            score                  INTEGER NOT NULL,
            deep_minutes           INTEGER NOT NULL,
            focus_minutes          INTEGER NOT NULL,
            tracked_minutes        INTEGER NOT NULL,
            longest_session_minutes INTEGER NOT NULL,
            switches               INTEGER NOT NULL,
            top_app                TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS session_tags (
            day          TEXT NOT NULL,
            start_minute INTEGER NOT NULL,
            tag          TEXT NOT NULL,
            PRIMARY KEY (day, start_minute)
         );
         PRAGMA user_version = 1;
         COMMIT;",
    )?;
    Ok(())
}

/// Rebuild the daily rollup from the minute rows. Called after every minute so
/// the aggregate can never drift from the source data.
fn refresh_day(conn: &Connection, day: &str) -> Result<DaySummary> {
    let mut stmt = conn.prepare(
        "SELECT level, score, switches, app FROM minute_samples WHERE day = ?1 ORDER BY minute",
    )?;
    let mut apps: HashMap<String, u32> = HashMap::new();
    let mut score_sum: u64 = 0;
    let mut tracked = 0u32;
    let mut deep = 0u32;
    let mut focus = 0u32;
    let mut switches = 0u32;
    let rows = stmt.query_map(params![day], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, u8>(1)?,
            r.get::<_, u32>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (level, score, sw, app) = row?;
        tracked += 1;
        score_sum += score as u64;
        switches += sw;
        *apps.entry(app).or_default() += 1;
        match FocusLevel::from_str(&level) {
            FocusLevel::Deep => {
                deep += 1;
                focus += 1;
            }
            FocusLevel::Medium => focus += 1,
            _ => {}
        }
    }
    let top_app = apps
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(a, _)| a)
        .unwrap_or_default();
    let score = if tracked > 0 {
        (score_sum / tracked as u64) as u8
    } else {
        0
    };
    let mut samples = Vec::new();
    {
        let mut s = conn.prepare(
            "SELECT minute, mod, score, level FROM minute_samples WHERE day = ?1 ORDER BY minute",
        )?;
        let rows = s.query_map(params![day], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, u16>(1)?,
                r.get::<_, u8>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (minute, minute_of_day, score, level) = row?;
            samples.push(MinuteSample {
                minute,
                day: day.to_string(),
                minute_of_day,
                score,
                level: FocusLevel::from_str(&level),
                keys: 0,
                backspaces: 0,
                clicks: 0,
                mouse_px: 0,
                switches: 0,
                active_secs: 0,
                pauses: 0,
                app: String::new(),
                category: crate::apps::AppCategory::Other,
                manual: false,
            });
        }
    }
    let longest = sessions_from(&samples, &HashMap::new())
        .into_iter()
        .map(|s| s.minutes)
        .max()
        .unwrap_or(0);

    let summary = DaySummary {
        day: day.to_string(),
        score,
        deep_minutes: deep,
        focus_minutes: focus,
        tracked_minutes: tracked,
        longest_session_minutes: longest,
        switches,
        top_app,
    };
    conn.execute(
        "INSERT INTO days
           (day, score, deep_minutes, focus_minutes, tracked_minutes,
            longest_session_minutes, switches, top_app)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(day) DO UPDATE SET
           score=excluded.score, deep_minutes=excluded.deep_minutes,
           focus_minutes=excluded.focus_minutes, tracked_minutes=excluded.tracked_minutes,
           longest_session_minutes=excluded.longest_session_minutes,
           switches=excluded.switches, top_app=excluded.top_app",
        params![
            summary.day,
            summary.score,
            summary.deep_minutes,
            summary.focus_minutes,
            summary.tracked_minutes,
            summary.longest_session_minutes,
            summary.switches,
            summary.top_app
        ],
    )?;
    Ok(summary)
}

fn row_to_day(row: &rusqlite::Row<'_>) -> rusqlite::Result<DaySummary> {
    Ok(DaySummary {
        day: row.get(0)?,
        score: row.get(1)?,
        deep_minutes: row.get(2)?,
        focus_minutes: row.get(3)?,
        tracked_minutes: row.get(4)?,
        longest_session_minutes: row.get(5)?,
        switches: row.get(6)?,
        top_app: row.get(7)?,
    })
}

/// Open (or create) the database for these settings.
pub fn open_for(settings: &Settings) -> Result<Db> {
    Db::open(&settings.db_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::AppCategory;
    use crate::model::{shift_day, today};

    struct TempDb {
        db: Db,
        dir: PathBuf,
    }

    impl TempDb {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "focusmap-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let db = Db::open(&dir.join("test.db")).unwrap();
            Self { db, dir }
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn sample(minute: i64, mod_min: u16, score: u8, switches: u32) -> MinuteSample {
        MinuteSample {
            minute,
            day: today(),
            minute_of_day: mod_min,
            score,
            level: FocusLevel::from_score(score, 70, 45),
            keys: 100,
            backspaces: 4,
            clicks: 2,
            mouse_px: 500,
            switches,
            active_secs: 55,
            pauses: 2,
            app: "Code.exe".into(),
            category: AppCategory::Editor,
            manual: false,
        }
    }

    #[test]
    fn stores_minutes_and_rolls_up_the_day() {
        let t = TempDb::new();
        let day = today();
        let base = 29_000_000i64;
        for i in 0..4 {
            let s = sample(base + i, 600 + i as u16, 80, 1);
            let day_sum = t.db.record_minute(&s).unwrap();
            assert_eq!(day_sum.tracked_minutes, (i + 1) as u32);
        }
        let day_sum = t.db.day(&day).unwrap().unwrap();
        assert_eq!(day_sum.deep_minutes, 4);
        assert_eq!(day_sum.score, 80);
        assert_eq!(day_sum.longest_session_minutes, 4);
        assert_eq!(day_sum.top_app, "Code.exe");
        assert_eq!(t.db.samples(&day).unwrap().len(), 4);
    }

    #[test]
    fn minute_writes_are_idempotent() {
        let t = TempDb::new();
        let s = sample(29_000_000, 600, 80, 0);
        t.db.record_minute(&s).unwrap();
        t.db.record_minute(&s).unwrap();
        assert_eq!(t.db.counts().unwrap().minutes, 1);
    }

    #[test]
    fn tags_stick_to_sessions() {
        let t = TempDb::new();
        let day = today();
        let base = 29_000_000i64;
        for i in 0..3 {
            t.db.record_minute(&sample(base + i, 600 + i as u16, 85, 0))
                .unwrap();
        }
        let sessions = t.db.sessions(&day).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].minutes, 3);
        let start = sessions[0].start_minute;
        t.db.set_tag(&day, start, Some("Coding")).unwrap();
        let sessions = t.db.sessions(&day).unwrap();
        assert_eq!(sessions[0].tag.as_deref(), Some("Coding"));
        t.db.set_tag(&day, start, None).unwrap();
        assert!(t.db.sessions(&day).unwrap()[0].tag.is_none());
    }

    #[test]
    fn hourly_and_app_stats_aggregate() {
        let t = TempDb::new();
        let base = 29_000_000i64;
        t.db.record_minute(&sample(base, 600, 80, 2)).unwrap();
        t.db.record_minute(&sample(base + 1, 601, 60, 1)).unwrap();
        let hours = t.db.hour_stats(&shift_day(&today(), -1)).unwrap();
        assert_eq!(hours.len(), 1);
        assert_eq!(hours[0].hour, 10);
        assert_eq!(hours[0].minutes, 2);
        let apps = t.db.app_stats(&shift_day(&today(), -1)).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].minutes, 2);
        assert_eq!(apps[0].switches, 3);
    }

    #[test]
    fn compaction_drops_old_minutes_but_keeps_days() {
        let t = TempDb::new();
        let old_day = shift_day(&today(), -120);
        let mut old = sample(20_000_000, 600, 70, 0);
        old.day = old_day.clone();
        t.db.record_minute(&old).unwrap();
        let today_sample = sample(29_000_000, 600, 70, 0);
        t.db.record_minute(&today_sample).unwrap();
        let removed = t.db.compact(&today()).unwrap();
        assert_eq!(removed, 1);
        assert!(t.db.samples(&old_day).unwrap().is_empty());
        assert!(t.db.day(&old_day).unwrap().is_some(), "daily rollup survives");
        assert_eq!(t.db.samples(&today()).unwrap().len(), 1);
    }

    #[test]
    fn wipe_leaves_an_empty_database() {
        let t = TempDb::new();
        t.db.record_minute(&sample(29_000_000, 600, 70, 0))
            .unwrap();
        t.db.wipe().unwrap();
        let counts = t.db.counts().unwrap();
        assert_eq!(counts.minutes, 0);
        assert_eq!(counts.days, 0);
    }

    #[test]
    fn export_bundle_contains_everything() {
        let t = TempDb::new();
        t.db.record_minute(&sample(29_000_000, 600, 70, 0))
            .unwrap();
        let bundle = t.db.export_bundle().unwrap();
        assert_eq!(bundle.days.len(), 1);
        assert_eq!(bundle.minutes.len(), 1);
        let json = bundle.to_json().unwrap();
        assert!(json.contains("\"minutes\""));
        let csv = bundle.minutes_csv();
        assert!(csv.starts_with("minute,day"));
    }
}
