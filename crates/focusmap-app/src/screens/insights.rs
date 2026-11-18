//! Insights: the longer pattern, records, best hours and where time went.

use eframe::egui;
use egui::{Align, Layout, Margin, Stroke};

use focusmap_core::apps::AppCategory;
use focusmap_core::model::{format_minutes, pretty_day, shift_day};
use focusmap_core::stats::{self, AppStat};

use crate::app::FocusMapApp;
use crate::theme::{self, SPACE};
use crate::widgets;

use super::{card_header, category_tint, metric, page_gap, two_columns};

impl FocusMapApp {
    pub(crate) fn screen_insights(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        let today = self.live.today.clone();
        let goal = self.settings.goal_minutes.max(1);
        let from = shift_day(&today, -27);
        let range: Vec<String> = (0..28)
            .map(|i| shift_day(&today, -i))
            .collect();
        let summaries: Vec<_> = self
            .cache
            .summaries
            .iter()
            .filter(|d| d.day >= from)
            .cloned()
            .collect();
        let points: Vec<(String, u8)> = range
            .iter()
            .rev()
            .map(|day| {
                let score = summaries
                    .iter()
                    .find(|d| &d.day == day)
                    .map(|d| d.score)
                    .unwrap_or(0);
                (day.clone(), score)
            })
            .collect();
        let tip = stats::tip(&self.cache.hours, &self.cache.apps, &self.cache.summaries, &today, goal);
        let burnout = stats::burnout(&self.cache.summaries, &today);
        let best_weekday = stats::best_weekday(&summaries);
        let streak_now = stats::streak(&self.cache.summaries, &today, goal);
        let records = stats::records(&self.cache.summaries, streak_now, goal);
        let best_hours = stats::predicted_hours(&self.cache.hours, 4);
        let best_day = records
            .best_day
            .as_ref()
            .map(|(day, _)| pretty_day(day))
            .unwrap_or_else(|| "-".to_string());
        let best_score = records
            .best_day
            .as_ref()
            .map(|(_, score)| score.to_string())
            .unwrap_or_else(|| "-".to_string());
        let total_deep = records.total_deep_minutes as u32;
        let apps = self.cache.apps.clone();
        let tags = self.cache.tags.clone();

        egui::Frame::NONE
            .fill(p.accent.gamma_multiply(if p.dark { 0.14 } else { 0.08 }))
            .stroke(Stroke::new(1.0, p.accent.gamma_multiply(0.35)))
            .corner_radius(14)
            .inner_margin(Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    widgets::dot(ui, p.accent, 4.0);
                    ui.vertical(|ui| {
                        ui.label(
                            theme::bold(9.5, "ONE THING TO TRY")
                                .color(p.accent)
                                .extra_letter_spacing(0.7),
                        );
                        ui.add_space(2.0);
                        ui.label(theme::regular(13.0, tip).color(p.text));
                    });
                });
            });

        if let Some(message) = &burnout {
            page_gap(ui);
            widgets::card(ui, &theme, |ui| {
                card_header(
                    ui,
                    &theme,
                    "A gentle check-in",
                    "Only shown when the last five tracked days really are lower than the five before.",
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    widgets::dot(ui, p.accent, 4.0);
                    ui.label(theme::regular(12.0, message).color(p.text));
                });
            });
        }

        page_gap(ui);
        widgets::card(ui, &theme, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    card_header(
                        ui,
                        &theme,
                        "The last four weeks",
                        "Daily focus score. The line is the trend, not a target.",
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let change = stats::trend(&self.cache.summaries, &today).change();
                    if let Some(change) = change {
                        let (label, tint) = if change >= 0.0 {
                            (format!("+{:.0}% vs last week", change), p.good)
                        } else {
                            (format!("{:.0}% vs last week", change), p.danger)
                        };
                        widgets::pill(ui, &theme, &label, tint);
                    }
                });
            });
            ui.add_space(12.0);
            widgets::trend_line(ui, &theme, &points, 190.0);
            ui.add_space(6.0);
            ui.label(
                theme::regular(10.5, format!(
                    "{} days tracked · {} of deep work",
                    records.total_days,
                    format_minutes(total_deep)
                ))
                .color(p.faint),
            );
        });

        page_gap(ui);
        ui.columns(5, |columns| {
            widgets::stat_card(
                &mut columns[0],
                &theme,
                "Current streak",
                &format!("{} days", streak_now),
                "goal days in a row",
                p.good,
            );
            widgets::stat_card(
                &mut columns[1],
                &theme,
                "Best streak",
                &format!("{} days", records.best_streak),
                "your record",
                p.accent,
            );
            widgets::stat_card(
                &mut columns[2],
                &theme,
                "Best day",
                &best_score,
                &best_day,
                p.deep,
            );
            widgets::stat_card(
                &mut columns[3],
                &theme,
                "Longest session",
                &format_minutes(records.longest_session_minutes),
                "unbroken deep work",
                p.medium,
            );
            widgets::stat_card(
                &mut columns[4],
                &theme,
                "Best weekday",
                &best_weekday
                    .map(|(_, avg)| format!("{avg:.0}"))
                    .unwrap_or_else(|| "-".to_string()),
                best_weekday.map_or("-", |(name, _)| name),
                p.deep,
            );
        });

        page_gap(ui);
        two_columns(
            ui,
            0.46,
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "Your best windows",
                        "Average score by hour over the last 28 days.",
                    );
                    ui.add_space(10.0);
                    if best_hours.is_empty() {
                        widgets::empty_state(
                            ui,
                            &theme,
                            "Not enough hours yet",
                            "Track a few normal days and the pattern appears.",
                        );
                    } else {
                        for hour in &best_hours {
                            let tint = theme.score_color(hour.avg_score as u8);
                            ui.horizontal(|ui| {
                                ui.label(
                                    theme::bold(11.5, format!("{:02}:00", hour.hour))
                                        .color(p.text),
                                );
                                ui.add_space(6.0);
                                let bar_width = (ui.available_width() - 78.0).max(40.0);
                                ui.scope(|ui| {
                                    ui.set_width(bar_width);
                                    widgets::progress(
                                        ui,
                                        &theme,
                                        hour.avg_score / 100.0,
                                        tint,
                                        7.0,
                                    );
                                });
                                ui.with_layout(
                                    Layout::right_to_left(Align::Center),
                                    |ui| {
                                        ui.label(
                                            theme::bold(11.5, format!("{:.0}", hour.avg_score))
                                                .color(tint),
                                        );
                                    },
                                );
                            });
                            ui.label(
                                theme::regular(9.5, format!("{} minutes sampled", hour.minutes))
                                    .color(p.faint),
                            );
                            ui.add_space(6.0);
                        }
                        ui.add_space(4.0);
                        ui.label(
                            theme::regular(10.5, "Protect the top window tomorrow before email and chat get it.")
                                .color(p.muted),
                        );
                    }
                });
            },
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "Where the time went",
                        "Foreground app minutes over 28 days.",
                    );
                    ui.add_space(10.0);
                    if apps.is_empty() {
                        widgets::empty_state(
                            ui,
                            &theme,
                            "No app minutes yet",
                            "The foreground app is recorded as a name, never a window title.",
                        );
                    } else {
                        let max_minutes = apps.iter().map(|a| a.minutes).max().unwrap_or(1);
                        for app in apps.iter().take(6) {
                            app_row(ui, &theme, app, max_minutes);
                            ui.add_space(7.0);
                        }
                    }
                });
            },
        );

        page_gap(ui);
        let chat: u32 = apps
            .iter()
            .filter(|a| a.category == AppCategory::Chat)
            .map(|a| a.minutes)
            .sum();
        let passive: u32 = apps
            .iter()
            .filter(|a| matches!(a.category, AppCategory::Media | AppCategory::Game))
            .map(|a| a.minutes)
            .sum();
        let noisy: u32 = apps
            .iter()
            .filter(|a| {
                matches!(
                    a.category,
                    AppCategory::Chat | AppCategory::Media | AppCategory::Game
                )
            })
            .map(|a| a.switches)
            .sum();
        widgets::card(ui, &theme, |ui| {
            card_header(
                ui,
                &theme,
                "Distraction report",
                "The categories that are capped by the score, and what they cost.",
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                metric(ui, &theme, "Chat", &format_minutes(chat), "message apps");
                ui.add_space(30.0);
                metric(ui, &theme, "Passive", &format_minutes(passive), "video + games");
                ui.add_space(30.0);
                metric(ui, &theme, "Switches", &noisy.to_string(), "in those apps");
                ui.add_space(30.0);
                metric(
                    ui,
                    &theme,
                    "Longest block",
                    &format_minutes(records.longest_session_minutes),
                    "your best run",
                );
            });
            ui.add_space(10.0);
            let sentence = if chat + passive == 0 {
                "No capped-app time in the last four weeks. That is unusually clean."
            } else if noisy >= 20 {
                "Most of the leak is switching, not the apps themselves. Batch chat and video after a block instead of between blocks."
            } else {
                "Capped apps cost less than they feel like. One protected deep block is worth more than removing them."
            };
            ui.label(theme::regular(11.5, sentence).color(p.muted));
        });

        page_gap(ui);
        widgets::card(ui, &theme, |ui| {
            card_header(
                ui,
                &theme,
                "Projects",
                "Deep minutes by tag over the last four weeks.",
            );
            ui.add_space(10.0);
            if tags.is_empty() {
                widgets::empty_state(
                    ui,
                    &theme,
                    "No tagged sessions yet",
                    "Add a tag to a deep session on Today and it will show up here.",
                );
            } else {
                let max_minutes = tags.iter().map(|t| t.minutes).max().unwrap_or(1);
                for tag in tags.iter().take(6) {
                    ui.horizontal(|ui| {
                        widgets::dot(ui, p.accent, 3.0);
                        ui.label(theme::bold(11.5, &tag.tag).color(p.text));
                        ui.add_space(6.0);
                        ui.scope(|ui| {
                            ui.set_width((ui.available_width() - 190.0).max(40.0));
                            widgets::progress(
                                ui,
                                &theme,
                                tag.minutes as f32 / max_minutes.max(1) as f32,
                                p.deep,
                                6.0,
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                theme::regular(10.5, format!("score {:.0}", tag.avg_score))
                                    .color(p.faint),
                            );
                            ui.label(
                                theme::regular(10.5, format_minutes(tag.minutes)).color(p.muted),
                            );
                        });
                    });
                    ui.add_space(7.0);
                }
            }
        });

        ui.add_space(SPACE);
    }
}

fn app_row(ui: &mut egui::Ui, theme: &crate::theme::Theme, app: &AppStat, max_minutes: u32) {
    let p = theme.palette;
    let tint = category_tint(theme, app.category);
    ui.horizontal(|ui| {
        widgets::dot(ui, tint, 3.0);
        ui.vertical(|ui| {
            ui.label(theme::bold(11.5, &app.app).color(p.text));
            ui.label(
                theme::regular(9.5, format!("{} · {} sw", app.category.label(), app.switches))
                    .color(p.faint),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::regular(10.5, format_minutes(app.minutes)).color(p.muted));
            let width = 68.0;
            ui.scope(|ui| {
                ui.set_width(width);
                widgets::progress(ui, theme, app.minutes as f32 / max_minutes.max(1) as f32, tint, 5.0);
            });
        });
    });
}
