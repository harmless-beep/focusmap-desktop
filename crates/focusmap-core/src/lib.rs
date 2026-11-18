//! FocusMap core: capture, scoring, storage, insight, export.
//!
//! The crate is platform-light on purpose. Only [`capture::windows`] talks to
//! the OS directly; everything else is pure logic with tests.

pub mod aggregator;
pub mod apps;
pub mod capture;
pub mod engine;
pub mod export;
pub mod fonts;
pub mod model;
pub mod platform;
pub mod scoring;
pub mod settings;
pub mod share_card;
pub mod stats;
pub mod storage;

pub use apps::AppCategory;
pub use engine::{Engine, LiveState, Nudge, NudgeKind};
pub use model::{DaySummary, FocusLevel, MinuteSample, Session};
pub use scoring::{focus_score, MinuteMetrics, ScoreConfig};
pub use settings::Settings;
pub use storage::Db;
