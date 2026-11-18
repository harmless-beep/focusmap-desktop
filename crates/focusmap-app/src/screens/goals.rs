//! Goals: the daily target, the streak, records and the nudge rules.

use eframe::egui;
use egui::{Align, Layout, Margin, Sense, Stroke, Vec2};

use focusmap_core::model::{format_minutes, pretty_day, shift_day};
use focusmap_core::stats;

use crate::app::FocusMapApp;
use crate::theme::{self, SPACE};
use crate::widgets;

use super::{card_header, page_gap, two_columns};

impl FocusMapApp {
    pub(crate) fn screen_goals(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        let today = self.live.today.clone();
        let goal = self.settings.goal_minutes.max(1);
        let deep = self.live.summary.deep_minutes;
        let streak_now = stats::streak(&self.cache.summaries, &today, goal);
        let records = stats::records(&self.cache.summaries, streak_now, goal);
        let frac = (deep as f32 / goal as f32).clamp(0.0, 1.0);
        let tint = if frac >= 1.0 { p.good } else { p.accent };
        let band = if frac >= 1.0 {
            "Goal reached. The streak is safe."
        } else if frac >= 0.5 {
            "Past halfway. One more block would do it."
        } else {
            "The first block is the one that counts."
        };

        widgets::card(ui, &theme, |ui| {
            ui.horizontal(|ui| {
                widgets::ring(
                    ui,
                    &theme,
                    156.0,
                    frac,
                    &format!("{:.0}%", frac * 100.0),
                    "of daily goal",
                    tint,
                );
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.label(
                        theme::bold(9.5, "TODAY'S GOAL")
                            .color(p.faint)
                            .extra_letter_spacing(0.8),
                    );
                    ui.label(
                        theme::bold(23.0, format!(
                            "{} of {}",
                            format_minutes(deep),
                            format_minutes(goal)
                        ))
                        .color(p.text),
                    );
                    ui.add_space(3.0);
                    ui.label(theme::regular(12.0, band).color(p.muted));
                    ui.add_space(12.0);
                    widgets::progress(ui, &theme, frac, tint, 7.0);
                    ui.add_space(6.0);
                    ui.label(
                        theme::regular(
                            10.5,
                            format!(
                                "A minute is deep at a score of {} or more.",
                                self.settings.score.deep_threshold
                            ),
                        )
                        .color(p.faint),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    egui::Frame::NONE
                        .fill(p.card_alt)
                        .corner_radius(12)
                        .inner_margin(Margin::same(14))
                        .show(ui, |ui| {
                            ui.set_width(178.0);
                            ui.vertical_centered(|ui| {
                                ui.label(theme::bold(10.0, "STREAK").color(p.faint));
                                ui.label(
                                    theme::bold(44.0, streak_now.to_string()).color(p.text),
                                );
                                ui.label(
                                    theme::regular(10.5, if streak_now == 1 { "day in a row" } else { "days in a row" })
                                        .color(p.muted),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    theme::regular(9.5, format!(
                                        "best {} days",
                                        records.best_streak
                                    ))
                                    .color(p.faint),
                                );
                            });
                        });
                });
            });
        });

        page_gap(ui);
        widgets::card(ui, &theme, |ui| {
            card_header(
                ui,
                &theme,
                "Daily deep-work goal",
                "Deep minutes, not hours at the desk. The default is two hours.",
            );
            ui.add_space(10.0);
            let slider = ui.add(
                egui::Slider::new(&mut self.settings.goal_minutes, 30..=480)
                    .step_by(15.0)
                    .suffix(" min")
                    .trailing_fill(true),
            );
            if slider.changed() {
                self.settings_changed();
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for preset in [60u32, 90, 120, 180, 240] {
                    let active = self.settings.goal_minutes == preset;
                    let label = format_minutes(preset);
                    let text = if active {
                        theme::bold(11.0, &label).color(p.text)
                    } else {
                        theme::regular(11.0, &label).color(p.muted)
                    };
                    if ui
                        .add(
                            egui::Button::new(text)
                                .fill(if active { p.card_alt } else { p.card })
                                .stroke(Stroke::new(1.0, if active { p.accent } else { p.border }))
                                .corner_radius(9)
                                .min_size(Vec2::new(58.0, 28.0)),
                        )
                        .clicked()
                        && !active
                    {
                        self.settings.goal_minutes = preset;
                        self.settings_changed();
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(
                theme::regular(10.5, "Goal nudges fire once per day. Streak days count only when the goal is met.")
                    .color(p.faint),
            );
        });

        page_gap(ui);
        two_columns(
            ui,
            0.55,
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(ui, &theme, "Records", "Personal bests across every day tracked.");
                    ui.add_space(12.0);
                    record_row(
                        ui,
                        &theme,
                        "Best focus day",
                        &records
                            .best_day
                            .as_ref()
                            .map(|(day, score)| format!("{score} on {}", pretty_day(day)))
                            .unwrap_or_else(|| "-".to_string()),
                    );
                    record_row(
                        ui,
                        &theme,
                        "Longest deep session",
                        &format_minutes(records.longest_session_minutes),
                    );
                    record_row(
                        ui,
                        &theme,
                        "Best streak",
                        &format!("{} days", records.best_streak),
                    );
                    record_row(ui, &theme, "Days tracked", &records.total_days.to_string());
                    record_row(
                        ui,
                        &theme,
                        "Total deep work",
                        &format_minutes(records.total_deep_minutes as u32),
                    );
                });
            },
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(ui, &theme, "Nudges", "Three gentle interruptions, no more.");
                    ui.add_space(12.0);
                    nudge_row(
                        ui,
                        &theme,
                        "Goal reached",
                        "One toast when today's deep work crosses the goal.",
                    );
                    nudge_row(
                        ui,
                        &theme,
                        "Stand up",
                        "After a long unbroken deep run, a reminder to look away.",
                    );
                    nudge_row(
                        ui,
                        &theme,
                        "Ease off",
                        "When five recent days are clearly below the five before.",
                    );
                    ui.add_space(10.0);
                    widgets::divider(ui, &theme);
                    ui.add_space(8.0);
                    let mut focus_mode = self.settings.focus_mode;
                    widgets::setting_row(
                        ui,
                        &theme,
                        "Mute system notifications",
                        "In-app toasts still appear.",
                        |ui| widgets::toggle(ui, &theme, &mut focus_mode).changed(),
                    );
                    if focus_mode != self.settings.focus_mode {
                        self.settings.focus_mode = focus_mode;
                        self.settings_changed();
                    }
                });
            },
        );

        page_gap(ui);
        widgets::card(ui, &theme, |ui| {
            card_header(
                ui,
                &theme,
                "The last 28 days",
                "One square per day. Solid means the goal was met.",
            );
            ui.add_space(10.0);
            streak_strip(ui, &theme, &self.cache.summaries, &today, goal);
        });

        page_gap(ui);
        let manual = self.engine.is_manual();
        widgets::card(ui, &theme, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    card_header(
                        ui,
                        &theme,
                        "Manual focus session",
                        "For reading, design, or thinking with little input. The minute floor is lifted.",
                    );
                    ui.add_space(6.0);
                    ui.label(
                        theme::regular(11.0, if manual {
                            "Manual focus is on. It ends when you turn it off."
                        } else {
                            "Turn it on before you start, not after."
                        })
                        .color(if manual { p.medium } else { p.muted }),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::primary_button(
                        ui,
                        &theme,
                        if manual { "End session" } else { "Start session" },
                    )
                    .clicked()
                    {
                        self.toggle_manual();
                    }
                });
            });
        });

        ui.add_space(SPACE);
    }
}

fn record_row(ui: &mut egui::Ui, theme: &crate::theme::Theme, label: &str, value: &str) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        ui.label(theme::regular(11.5, label).color(p.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::bold(12.0, value).color(p.text));
        });
    });
    ui.add_space(7.0);
}

fn nudge_row(ui: &mut egui::Ui, theme: &crate::theme::Theme, title: &str, body: &str) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        widgets::dot(ui, p.accent, 3.0);
        ui.vertical(|ui| {
            ui.label(theme::bold(11.5, title).color(p.text));
            ui.label(theme::regular(10.0, body).color(p.faint));
        });
    });
    ui.add_space(9.0);
}

fn streak_strip(
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
    summaries: &[focusmap_core::model::DaySummary],
    today: &str,
    goal: u32,
) {
    let p = theme.palette;
    let width = ui.available_width();
    let gap = 4.0;
    let cell = ((width - gap * 27.0) / 28.0).clamp(8.0, 26.0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, cell + 24.0), Sense::hover());
    let painter = ui.painter();
    let days: Vec<String> = (0..28)
        .map(|i| shift_day(today, -(27 - i)))
        .collect();
    for (i, day) in days.iter().enumerate() {
        let summary = summaries.iter().find(|d| &d.day == day);
        let hit = summary.map(|d| d.deep_minutes >= goal).unwrap_or(false);
        let has = summary.map(|d| d.tracked_minutes > 0).unwrap_or(false);
        let color = if hit {
            p.good
        } else if has {
            p.track.gamma_multiply(1.6)
        } else {
            p.idle
        };
        let x = rect.left() + i as f32 * (cell + gap);
        let square = egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::splat(cell));
        painter.rect_filled(square, egui::CornerRadius::same(4), color);
        if day == today {
            painter.rect_stroke(
                square,
                egui::CornerRadius::same(4),
                Stroke::new(1.2, p.accent),
                egui::StrokeKind::Inside,
            );
        }
    }
    painter.text(
        egui::pos2(rect.left(), rect.bottom() - 2.0),
        egui::Align2::LEFT_BOTTOM,
        pretty_day(&days[0]),
        egui::FontId::proportional(9.5),
        p.faint,
    );
    painter.text(
        egui::pos2(rect.right(), rect.bottom() - 2.0),
        egui::Align2::RIGHT_BOTTOM,
        "Today",
        egui::FontId::proportional(9.5),
        p.faint,
    );
    if let Some(pos) = response.hover_pos() {
        let idx = ((pos.x - rect.left()) / (cell + gap)).floor() as i32;
        if (0..28).contains(&idx) {
            let day = &days[idx as usize];
            let summary = summaries.iter().find(|d| &d.day == day);
            let text = match summary {
                Some(d) if d.tracked_minutes > 0 => {
                    format!("{} · {} deep · score {}", pretty_day(day), format_minutes(d.deep_minutes), d.score)
                }
                _ => format!("{} · no data", pretty_day(day)),
            };
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                response.id.with(idx),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| {
                ui.label(theme::bold(11.5, text));
            });
        }
    }
}
