//! CSV and JSON export. Everything is generated locally from the database and
//! written to a file the user picked; nothing is uploaded.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::model::{DaySummary, MinuteSample};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportBundle {
    pub app: String,
    pub version: String,
    pub generated_at: String,
    pub note: String,
    pub days: Vec<DaySummary>,
    pub minutes: Vec<MinuteSample>,
    /// `(day, session start unix minute, tag)`
    pub tags: Vec<(String, i64, String)>,
}

impl ExportBundle {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn minutes_csv(&self) -> String {
        let mut out = String::from(
            "minute,day,time,score,level,keys,backspaces,clicks,mouse_px,switches,active_secs,pauses,app,category,manual\n",
        );
        for m in &self.minutes {
            out.push_str(&format!(
                "{},{},{:02}:{:02},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                m.minute,
                m.day,
                m.minute_of_day / 60,
                m.minute_of_day % 60,
                m.score,
                m.level.as_str(),
                m.keys,
                m.backspaces,
                m.clicks,
                m.mouse_px,
                m.switches,
                m.active_secs,
                m.pauses,
                csv_field(&m.app),
                m.category.as_str(),
                if m.manual { 1 } else { 0 },
            ));
        }
        out
    }

    pub fn days_csv(&self) -> String {
        let mut out = String::from(
            "day,score,deep_minutes,focus_minutes,tracked_minutes,longest_session_minutes,switches,top_app\n",
        );
        for d in &self.days {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                d.day,
                d.score,
                d.deep_minutes,
                d.focus_minutes,
                d.tracked_minutes,
                d.longest_session_minutes,
                d.switches,
                csv_field(&d.top_app)
            ));
        }
        out
    }

    /// Write both files into `dir` and return their paths.
    pub fn write_files(&self, dir: &Path) -> Result<(PathBuf, PathBuf)> {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let csv_path = dir.join(format!("focusmap-{stamp}.csv"));
        let json_path = dir.join(format!("focusmap-{stamp}.json"));
        std::fs::write(&csv_path, self.minutes_csv())
            .with_context(|| format!("writing {}", csv_path.display()))?;
        std::fs::write(&json_path, self.to_json()?)
            .with_context(|| format!("writing {}", json_path.display()))?;
        Ok((csv_path, json_path))
    }
}

fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::AppCategory;
    use crate::model::FocusLevel;

    fn bundle() -> ExportBundle {
        ExportBundle {
            app: "FocusMap".into(),
            version: "1.0.0".into(),
            generated_at: "2026-11-15T10:00:00Z".into(),
            note: String::new(),
            days: vec![DaySummary {
                day: "2026-11-15".into(),
                score: 81,
                deep_minutes: 120,
                focus_minutes: 180,
                tracked_minutes: 300,
                longest_session_minutes: 55,
                switches: 9,
                top_app: "Code.exe".into(),
            }],
            minutes: vec![MinuteSample {
                minute: 29_000_000,
                day: "2026-11-15".into(),
                minute_of_day: 615,
                score: 84,
                level: FocusLevel::Deep,
                keys: 110,
                backspaces: 5,
                clicks: 2,
                mouse_px: 800,
                switches: 1,
                active_secs: 57,
                pauses: 2,
                app: "Code, Spotless".into(),
                category: AppCategory::Editor,
                manual: false,
            }],
            tags: vec![("2026-11-15".into(), 29_000_000, "Coding".into())],
        }
    }

    #[test]
    fn csv_quotes_commas() {
        let csv = bundle().minutes_csv();
        assert!(csv.contains("\"Code, Spotless\""));
        assert_eq!(csv.lines().count(), 2);
    }

    #[test]
    fn json_round_trips() {
        let json = bundle().to_json().unwrap();
        let back: ExportBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.days.len(), 1);
        assert_eq!(back.minutes[0].score, 84);
    }

    #[test]
    fn writes_both_files() {
        let dir = std::env::temp_dir().join(format!(
            "focusmap-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (csv, json) = bundle().write_files(&dir).unwrap();
        assert!(csv.exists() && json.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
