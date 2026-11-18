//! Week and month: the trend, the best day, and any day opened in full.

use eframe::egui;
use egui::{Align, Layout};

use focusmap_core::model::{format_minutes, pretty_day, DaySummary};

use crate::app::{day_label, last_days, FocusMapApp, Period};
use crate::theme::{self, SPACE};
use crate::widgets;

use super::{card_header, metric, page_gap};

impl FocusMapApp {
    pub(crate) fn screen_week(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        let today = self.live.today.clone();
        let count = self.period.days();
        let range = last_days(&today, count);
        if !range.iter().any(|d| d == &self.week_day) {
            self.week_day = today.clone();
        }
        let day = self.week_day.clone();
        if day != today {
            self.ensure_detail(&day);
        }

        let mut period = self.period;
        let mut new_day: Option<String> = None;

        widgets::card(ui, &theme, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    card_header(
                        ui,
                        &theme,
                        match period {
                            Period::Week => "This week",
                            Period::Month => "The last 30 days",
                        },
                        &format!(
                            "{} to {}",
                            pretty_day(&range[0]),
                            pretty_day(range.last().unwrap_or(&today))
                        ),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::segmented(
                        ui,
                        &theme,
                        &[(Period::Week, "7 days"), (Period::Month, "30 days")],
                        &mut period,
                    );
                });
            });

            let in_range: Vec<&DaySummary> = range
                .iter()
                .filter_map(|d| self.cache.summaries.iter().find(|s| &s.day == d))
                .collect();
            let active_count = in_range.iter().filter(|d| d.tracked_minutes > 0).count();
            let avg = if active_count == 0 {
                0
            } else {
                in_range
                    .iter()
                    .filter(|d| d.tracked_minutes > 0)
                    .map(|d| d.score as u32)
                    .sum::<u32>()
                    / active_count as u32
            };
            let deep: u32 = in_range.iter().map(|d| d.deep_minutes).sum();
            let tracked: u32 = in_range.iter().map(|d| d.tracked_minutes).sum();
            let switches: u32 = in_range.iter().map(|d| d.switches).sum();

            ui.add_space(14.0);
            ui.columns(4, |columns| {
                widgets::stat_card(
                    &mut columns[0],
                    &theme,
                    "Average score",
                    &avg.to_string(),
                    "per active day",
                    theme.score_color(avg as u8),
                );
                widgets::stat_card(
                    &mut columns[1],
                    &theme,
                    "Deep work",
                    &format_minutes(deep),
                    "across the period",
                    p.deep,
                );
                widgets::stat_card(
                    &mut columns[2],
                    &theme,
                    "Time tracked",
                    &format_minutes(tracked),
                    &format!("{} active days", active_count),
                    p.accent,
                );
                widgets::stat_card(
                    &mut columns[3],
                    &theme,
                    "Switches",
                    &switches.to_string(),
                    "context changes",
                    p.fragmented,
                );
            });
        });

        page_gap(ui);
        let scores: Vec<(String, u8)> = range
            .iter()
            .map(|d| {
                let score = self.summary_for(d).score;
                (d.clone(), score)
            })
            .collect();
        let selected_index = range.iter().position(|d| d == &day);
        let bars = scores.clone();
        widgets::card(ui, &theme, |ui| {
            card_header(
                ui,
                &theme,
                "Score by day",
                "A bar is a full day. Click one to open its map and sessions.",
            );
            ui.add_space(10.0);
            if let Some(index) = widgets::day_bars(ui, &theme, &bars, selected_index, 168.0) {
                new_day = range.get(index).cloned();
            }
            ui.add_space(6.0);
            if period == Period::Week {
                ui.columns(range.len().max(1), |columns| {
                    for (column, day) in columns.iter_mut().zip(range.iter()) {
                        column.vertical_centered(|ui| {
                            ui.label(theme::regular(10.0, weekday(day)).color(p.faint));
                        });
                    }
                });
            } else {
                ui.label(
                    theme::regular(10.0, format!("{} days ending today", range.len()))
                        .color(p.faint),
                );
            }
        });
        if let Some(day) = new_day {
            if day != self.week_day {
                self.week_day = day;
            }
        }
        if period != self.period {
            self.period = period;
            self.week_day = today.clone();
        }

        page_gap(ui);
        let day = self.week_day.clone();
        self.day_detail(ui, &day, &today);
        ui.add_space(SPACE);
    }

    fn day_detail(&mut self, ui: &mut egui::Ui, day: &str, today: &str) {
        let theme = self.theme;
        let p = theme.palette;
        let summary = if day == today {
            self.live.summary.clone()
        } else {
            self.summary_for(day)
        };
        let sessions = if day == today {
            self.live.sessions.clone()
        } else {
            self.cache.detail_sessions.clone()
        };
        let has_minutes = if day == today {
            !self.live.samples.is_empty()
        } else {
            !self.cache.detail_samples.is_empty()
        };
        let score = summary.score;

        widgets::card(ui, &theme, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(theme::bold(18.0, day_label(day, today)).color(p.text));
                    ui.label(theme::regular(11.0, pretty_day(day)).color(p.faint));
                });
                ui.add_space(12.0);
                widgets::pill(ui, &theme, &format!("score {score}"), theme.score_color(score));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        theme::regular(10.5, "Shown in local time").color(p.faint),
                    );
                });
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                metric(
                    ui,
                    &theme,
                    "Deep",
                    &format_minutes(summary.deep_minutes),
                    "70+ minutes",
                );
                ui.add_space(26.0);
                metric(
                    ui,
                    &theme,
                    "Focus",
                    &format_minutes(summary.focus_minutes),
                    "deep + steady",
                );
                ui.add_space(26.0);
                metric(
                    ui,
                    &theme,
                    "Tracked",
                    &format_minutes(summary.tracked_minutes),
                    "scored minutes",
                );
                ui.add_space(26.0);
                metric(
                    ui,
                    &theme,
                    "Longest",
                    &format_minutes(summary.longest_session_minutes),
                    "deep session",
                );
                ui.add_space(26.0);
                metric(ui, &theme, "Switches", &summary.switches.to_string(), "window changes");
            });
            ui.add_space(12.0);
            if has_minutes {
                let samples = if day == today {
                    &self.live.samples
                } else {
                    &self.cache.detail_samples
                };
                widgets::day_grid(ui, &theme, samples, self.settings.patterns);
            } else {
                widgets::empty_state(
                    ui,
                    &theme,
                    "No minutes recorded",
                    "This day has no minute data. Compaction keeps old daily totals but drops detail.",
                );
            }
        });

        ui.add_space(12.0);
        self.sessions_block(ui, day, &sessions, "Sessions that day", 4);
    }
}

fn weekday(day: &str) -> String {
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|d| d.format("%a").to_string())
        .unwrap_or_default()
}
