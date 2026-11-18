//! The focus score.
//!
//! One minute of timing data becomes a 0-100 score. Every weight lives in
//! [`ScoreConfig`] so the user can tune the model in Settings, and every term is
//! explained here because a score you cannot explain is a score you cannot trust.
//!
//! The model has three positive terms and three penalties:
//!
//! | term | default | what it rewards / punishes |
//! |------|---------|----------------------------|
//! | activity  | 0.35 | steady input (~2 events/s). Too little means passive consumption, too much means mashing. |
//! | continuity| 0.40 | how much of the minute had input. Deep work is sustained, not a 5 second burst. |
//! | steadiness| 0.25 | low backspace ratio and no dead stops. Rework is real work, but thrash is not flow. |
//! | switches  | 0.28 | every window switch costs points: it is the clearest distraction signal there is. |
//! | fragment  | 0.12 | many short bursts in one minute. |
//! | mouse     | 0.06 | erratic pointer movement (large jumps per event). |
//!
//! Weights are normalised, so the positive part is always 0..1 before penalties.
//! Category ceilings (game, video, chat) are applied last, then the manual
//! focus floor.

use serde::{Deserialize, Serialize};

use crate::apps::AppCategory;

/// Tunable weights and thresholds. Serialized with the rest of the settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScoreConfig {
    pub activity_weight: f32,
    pub continuity_weight: f32,
    pub steadiness_weight: f32,
    pub switch_penalty: f32,
    pub fragmentation_penalty: f32,
    pub mouse_penalty: f32,
    /// Events per active second that score best.
    pub peak_rate: f32,
    pub deep_threshold: u8,
    pub medium_threshold: u8,
    /// A manual focus session never scores below this (reading, thinking).
    pub manual_floor: u8,
}

impl Default for ScoreConfig {
    fn default() -> Self {
        Self {
            activity_weight: 0.35,
            continuity_weight: 0.40,
            steadiness_weight: 0.25,
            switch_penalty: 0.28,
            fragmentation_penalty: 0.12,
            mouse_penalty: 0.06,
            peak_rate: 1.8,
            deep_threshold: 70,
            medium_threshold: 45,
            manual_floor: 72,
        }
    }
}

impl ScoreConfig {
    /// Clamp everything into sane ranges after the user edits sliders.
    pub fn sanitize(&mut self) {
        for w in [
            &mut self.activity_weight,
            &mut self.continuity_weight,
            &mut self.steadiness_weight,
        ] {
            *w = w.clamp(0.0, 1.0);
        }
        for w in [
            &mut self.switch_penalty,
            &mut self.fragmentation_penalty,
            &mut self.mouse_penalty,
        ] {
            *w = w.clamp(0.0, 1.0);
        }
        self.peak_rate = self.peak_rate.clamp(0.2, 10.0);
        self.deep_threshold = self.deep_threshold.clamp(40, 95);
        self.medium_threshold = self.medium_threshold.clamp(10, self.deep_threshold - 1);
        self.manual_floor = self.manual_floor.clamp(40, 100);
    }
}

/// Raw per-minute metrics. All counts, never content.
#[derive(Clone, Debug, Default)]
pub struct MinuteMetrics {
    pub keys: u32,
    pub backspaces: u32,
    pub clicks: u32,
    pub mouse_px: u32,
    pub mouse_events: u32,
    pub switches: u32,
    pub active_secs: u32,
    pub pauses: u32,
    pub max_pause_secs: u32,
    pub category: AppCategory,
    pub manual: bool,
}

/// Turn one minute of metrics into a 0..=100 focus score.
pub fn focus_score(m: &MinuteMetrics, cfg: &ScoreConfig) -> u8 {
    if m.active_secs == 0 && !m.manual {
        return 0;
    }

    let activity = m.keys + m.clicks;
    let active = m.active_secs.max(1) as f32;
    let rate = activity as f32 / active;

    // Low input is passive (a video, reading without a manual session), high
    // input is mashing (a game, an argument in chat). The peak is asymmetric on
    // purpose: slow minutes should fade, fast ones should collapse quickly.
    let activity_shape = if rate <= cfg.peak_rate {
        (rate / cfg.peak_rate.max(0.2)).powf(0.9)
    } else {
        let overshoot = (rate - cfg.peak_rate) / 1.8;
        (-overshoot * overshoot).exp()
    };

    let continuity = (m.active_secs as f32 / 60.0).clamp(0.0, 1.0);
    let backspace_ratio = m.backspaces as f32 / m.keys.max(1) as f32;
    let long_pause = (m.max_pause_secs.saturating_sub(25) as f32 / 40.0).clamp(0.0, 1.0);
    let steadiness =
        (1.0 - (backspace_ratio * 2.0).clamp(0.0, 0.6) - long_pause * 0.25).clamp(0.0, 1.0);

    let weights =
        (cfg.activity_weight + cfg.continuity_weight + cfg.steadiness_weight).max(0.001);
    let base = (activity_shape * cfg.activity_weight
        + continuity * cfg.continuity_weight
        + steadiness * cfg.steadiness_weight)
        / weights;

    let switch_norm = (m.switches as f32 / 5.0).clamp(0.0, 1.0);
    let frag_norm = (m.pauses.saturating_sub(10) as f32 / 15.0).clamp(0.0, 1.0);
    let px_per_event = m.mouse_px as f32 / m.mouse_events.max(1) as f32;
    let mouse_norm = ((px_per_event - 30.0) / 60.0).clamp(0.0, 1.0);

    let penalty = switch_norm * cfg.switch_penalty
        + frag_norm * cfg.fragmentation_penalty
        + mouse_norm * cfg.mouse_penalty;

    let mut score = (100.0 * (base - penalty).clamp(0.0, 1.0)).round() as u8;
    if m.manual {
        score = score.max(cfg.manual_floor);
    }
    // The category ceiling wins over the manual floor: a manual session spent in
    // a game or a video is still not deep work.
    score.min(m.category.score_cap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steady() -> MinuteMetrics {
        MinuteMetrics {
            keys: 120,
            backspaces: 6,
            clicks: 4,
            mouse_px: 900,
            mouse_events: 30,
            switches: 1,
            active_secs: 58,
            pauses: 2,
            max_pause_secs: 5,
            category: AppCategory::Editor,
            manual: false,
        }
    }

    #[test]
    fn idle_minute_scores_zero() {
        let m = MinuteMetrics::default();
        assert_eq!(focus_score(&m, &ScoreConfig::default()), 0);
    }

    #[test]
    fn steady_typing_is_deep() {
        let s = focus_score(&steady(), &ScoreConfig::default());
        assert!(s >= 70, "steady coding should be deep work, got {s}");
    }

    #[test]
    fn switching_windows_costs_points() {
        let mut m = steady();
        let clean = focus_score(&m, &ScoreConfig::default());
        m.switches = 8;
        let thrashy = focus_score(&m, &ScoreConfig::default());
        assert!(
            clean as i32 - thrashy as i32 >= 15,
            "8 switches should cost >= 15 points ({clean} -> {thrashy})"
        );
    }

    #[test]
    fn games_and_video_are_never_deep() {
        let cfg = ScoreConfig::default();
        let mut game = steady();
        game.keys = 300;
        game.clicks = 40;
        game.active_secs = 60;
        game.category = AppCategory::Game;
        assert!(focus_score(&game, &cfg) <= 25);

        let mut video = MinuteMetrics {
            keys: 2,
            clicks: 1,
            active_secs: 4,
            mouse_px: 120,
            mouse_events: 3,
            category: AppCategory::Media,
            ..Default::default()
        };
        assert!(focus_score(&video, &cfg) <= 25);
        video.manual = true;
        assert!(focus_score(&video, &cfg) <= 25, "category cap beats manual floor");
    }

    #[test]
    fn chat_stays_out_of_the_deep_band() {
        let cfg = ScoreConfig::default();
        let chat = MinuteMetrics {
            keys: 30,
            backspaces: 2,
            clicks: 6,
            mouse_px: 2500,
            mouse_events: 50,
            switches: 7,
            active_secs: 35,
            pauses: 9,
            max_pause_secs: 12,
            category: AppCategory::Chat,
            manual: false,
        };
        assert!(focus_score(&chat, &cfg) <= 50);
    }

    #[test]
    fn manual_session_covers_reading() {
        let cfg = ScoreConfig::default();
        let reading = MinuteMetrics {
            keys: 3,
            active_secs: 3,
            category: AppCategory::Office,
            manual: true,
            ..Default::default()
        };
        assert!(focus_score(&reading, &cfg) >= cfg.manual_floor);
    }

    #[test]
    fn mashing_keys_scores_lower_than_flow() {
        let cfg = ScoreConfig::default();
        let mut mash = steady();
        mash.keys = 420;
        mash.active_secs = 60;
        assert!(focus_score(&mash, &cfg) < focus_score(&steady(), &cfg));
    }

    #[test]
    fn weights_are_configurable() {
        let m = steady();
        let mut cfg = ScoreConfig::default();
        let before = focus_score(&m, &cfg);
        cfg.activity_weight = 0.0;
        cfg.continuity_weight = 0.0;
        cfg.steadiness_weight = 0.0;
        assert_eq!(focus_score(&m, &cfg), 0);
        assert!(before > 0);
    }

    #[test]
    fn sanitize_keeps_thresholds_ordered() {
        let mut cfg = ScoreConfig {
            deep_threshold: 50,
            medium_threshold: 90,
            ..Default::default()
        };
        cfg.sanitize();
        assert!(cfg.medium_threshold < cfg.deep_threshold);
        assert_eq!(cfg.deep_threshold, 50);
    }
}
