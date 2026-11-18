//! Today: the live score, the shape of the day, deep sessions, and leaks.

use std::collections::HashMap;

use chrono::Timelike;
use eframe::egui;
use egui::{Align, Layout, Margin, Sense, Stroke, Vec2};

use focusmap_city::{CityLook, CityPreview};
use focusmap_core::apps::AppCategory;
use focusmap_core::model::format_minutes;
use focusmap_core::settings::CityStyle;

use crate::app::FocusMapApp;
use crate::theme::{self, SPACE};
use crate::widgets;

use super::{card_header, metric, score_band, two_columns};

impl FocusMapApp {
    pub(crate) fn screen_today(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        let summary = self.live.summary.clone();
        let score = self.live.live_score;
        let paused = self.engine.is_paused();
        let manual = self.engine.is_manual();
        let goal = self.settings.goal_minutes.max(1);
        let current_app = if self.live.current_app.is_empty() {
            "Waiting for input".to_string()
        } else {
            self.live.current_app.clone()
        };
        let capture_error = self.live.last_error.clone();
        let deep = summary.deep_minutes;
        let goal_frac = deep as f32 / goal as f32;
        let band = score_band(
            score,
            self.settings.score.deep_threshold,
            self.settings.score.medium_threshold,
        );
        let target = score as f32 / 100.0;
        let ring_frac = if theme.reduced_motion {
            target
        } else {
            ui.ctx()
                .animate_value_with_time(egui::Id::unique("today-ring"), target, 0.45)
        };
        let tint = if paused { p.faint } else { theme.score_color(score) };

        widgets::card(ui, &theme, |ui| {
            ui.horizontal(|ui| {
                widgets::ring(
                    ui,
                    &theme,
                    172.0,
                    ring_frac,
                    &score.to_string(),
                    "focus score",
                    tint,
                );
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.label(
                        theme::bold(9.5, "TODAY")
                            .color(p.faint)
                            .extra_letter_spacing(0.8),
                    );
                    ui.label(theme::bold(24.0, band.0).color(p.text));
                    ui.add_space(2.0);
                    ui.label(theme::regular(12.0, band.1).color(p.muted));
                    ui.add_space(12.0);
                    widgets::progress(ui, &theme, goal_frac, p.accent, 7.0);
                    ui.add_space(5.0);
                    ui.label(
                        theme::regular(10.5, format!(
                            "{} of {} deep-work goal",
                            format_minutes(deep),
                            format_minutes(goal)
                        ))
                        .color(p.faint),
                    );
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        metric(
                            ui,
                            &theme,
                            "Deep",
                            &format_minutes(deep),
                            "70+ score",
                        );
                        ui.add_space(22.0);
                        metric(
                            ui,
                            &theme,
                            "Focus",
                            &format_minutes(summary.focus_minutes),
                            "deep + steady",
                        );
                        ui.add_space(22.0);
                        metric(
                            ui,
                            &theme,
                            "Switches",
                            &summary.switches.to_string(),
                            "windows today",
                        );
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    egui::Frame::NONE
                        .fill(p.card_alt)
                        .corner_radius(12)
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(206.0);
                            ui.horizontal(|ui| {
                                widgets::dot(
                                    ui,
                                    if paused {
                                        p.warn
                                    } else if manual {
                                        p.medium
                                    } else {
                                        p.good
                                    },
                                    3.5,
                                );
                                let status = if paused {
                                    "Paused"
                                } else if manual {
                                    "Manual focus"
                                } else {
                                    "Recording"
                                };
                                ui.label(theme::bold(10.5, status.to_uppercase()).color(p.text));
                            });
                            ui.add_space(5.0);
                            ui.label(theme::regular(11.0, current_app).color(p.muted));
                            ui.add_space(10.0);
                            let width = ui.available_width();
                            let manual_label = if manual {
                                "End manual focus"
                            } else {
                                "Manual focus"
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        theme::regular(11.5, manual_label).color(p.text),
                                    )
                                    .fill(if manual { p.accent.gamma_multiply(0.3) } else { p.card })
                                    .stroke(Stroke::new(1.0, p.border))
                                    .corner_radius(9)
                                    .min_size(Vec2::new(width, 29.0)),
                                )
                                .clicked()
                            {
                                self.toggle_manual();
                            }
                            ui.add_space(5.0);
                            let pause_label = if paused { "Resume" } else { "Pause" };
                            if ui
                                .add(
                                    egui::Button::new(theme::regular(11.5, pause_label).color(p.text))
                                        .fill(p.card)
                                        .stroke(Stroke::new(1.0, p.border))
                                        .corner_radius(9)
                                        .min_size(Vec2::new(width, 29.0)),
                                )
                                .clicked()
                            {
                                self.toggle_pause();
                            }
                        });
                });
            });
        });

        if let Some(error) = capture_error {
            ui.add_space(8.0);
            egui::Frame::NONE
                .fill(p.danger.gamma_multiply(0.12))
                .stroke(Stroke::new(1.0, p.danger.gamma_multiply(0.4)))
                .corner_radius(10)
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.label(
                        theme::regular(11.5, format!("Capture is not running: {error}"))
                            .color(p.danger),
                    );
                });
        }

        super::page_gap(ui);
        super::stat_row(
            ui,
            &theme,
            [
                (
                    "Longest session",
                    format_minutes(summary.longest_session_minutes),
                    "unbroken deep work",
                    p.medium,
                ),
                (
                    "Time tracked",
                    format_minutes(summary.tracked_minutes),
                    "scored minutes today",
                    p.accent,
                ),
                (
                    "Window switches",
                    summary.switches.to_string(),
                    "each one costs focus",
                    p.fragmented,
                ),
            ],
        );

        // ---- timeline ----------------------------------------------------
        let mut clicked_hour = None;
        let mut close_hour = false;
        let selected_hour = self.selected_hour;
        let mut city_view = self.city_view;
        widgets::card(ui, &theme, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    card_header(
                        ui,
                        &theme,
                        "The shape of today",
                        "One row per hour, one cell per minute. Click an hour to look closer.",
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(self.city_ready, |ui| {
                        widgets::segmented(
                            ui,
                            &theme,
                            &[(false, "Timeline"), (true, "Appolis")],
                            &mut city_view,
                        );
                    });
                    if city_view {
                        ui.add_space(8.0);
                        let mut blueprint =
                            self.settings.city_style == CityStyle::Blueprint;
                        if widgets::segmented(
                            ui,
                            &theme,
                            &[(true, "Blueprint"), (false, "Daylight")],
                            &mut blueprint,
                        ) {
                            self.settings.city_style = if blueprint {
                                CityStyle::Blueprint
                            } else {
                                CityStyle::Daylight
                            };
                            self.settings_changed();
                        }
                    } else {
                        ui.add_space(12.0);
                        widgets::legend(ui, &theme);
                    }
                });
            });
            ui.add_space(10.0);
            if city_view {
                self.refresh_city();
                let height = 400.0;
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), height),
                    Sense::click(),
                );
                let ppp = ui.ctx().pixels_per_point();
                let size_px = [
                    (rect.width() * ppp).round() as u32,
                    (rect.height() * ppp).round() as u32,
                ];
                let time = ui.input(|i| i.time) as f32;

                // Which tower is under the pointer, and which one was clicked.
                let pointer = ui.ctx().pointer_hover_pos().filter(|p| rect.contains(*p));
                let hovered = pointer.and_then(|p| {
                    focusmap_city::pick(
                        &self.city_buildings,
                        size_px,
                        time,
                        [(p.x - rect.min.x) * ppp, (p.y - rect.min.y) * ppp],
                    )
                });
                if hovered.is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if response.clicked() {
                    self.city_pinned = match hovered {
                        Some(index) => {
                            let name = self.city_buildings[index].app.clone();
                            (self.city_pinned.as_deref() != Some(name.as_str())).then_some(name)
                        }
                        None => None,
                    };
                }
                let pinned = self.city_pinned.as_ref().and_then(|name| {
                    self.city_buildings.iter().position(|b| &b.app == name)
                });

                let now = chrono::Local::now();
                let hour = match self.city_replay {
                    Some(minute) => minute as f32 / 1440.0,
                    None => (now.hour() as f32 + now.minute() as f32 / 60.0) / 24.0,
                };
                ui.painter().add(eframe::egui_wgpu::Callback::new_paint_callback(
                    rect,
                    CityPreview::new(
                        time,
                        size_px,
                        CityLook {
                            mood: self.city_mood,
                            intensity: self.city_intensity,
                            hour,
                            highlight: hovered.or(pinned),
                            render_scale: self.settings.city_quality.scale(),
                            traffic: self.settings.city_quality.traffic(),
                            fps: self.settings.city_quality.fps(),
                            blueprint: self.settings.city_style == CityStyle::Blueprint,
                        },
                        self.city_buildings.clone(),
                    ),
                ));
                // The city is the point of this mode; on switching in, scroll
                // it into view instead of leaving it clipped at the fold.
                if self.city_focus {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    self.city_focus = false;
                }

                if let Some(index) = hovered.or(pinned) {
                    let b = &self.city_buildings[index];
                    let tint = egui::Color32::from_rgb(
                        (b.glow[0].clamp(0.0, 1.0) * 255.0) as u8,
                        (b.glow[1].clamp(0.0, 1.0) * 255.0) as u8,
                        (b.glow[2].clamp(0.0, 1.0) * 255.0) as u8,
                    );
                    egui::Area::new(egui::Id::unique("appolis-detail"))
                        .fixed_pos(rect.left_top() + Vec2::new(14.0, 14.0))
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::NONE
                                .fill(theme::alpha(p.card, 244))
                                .stroke(Stroke::new(1.0, theme::alpha(tint, 120)))
                                .corner_radius(10)
                                .inner_margin(Margin::symmetric(12, 9))
                                .show(ui, |ui| {
                                    ui.set_width(228.0);
                                    ui.horizontal(|ui| {
                                        widgets::dot(ui, tint, 3.5);
                                        ui.label(theme::bold(13.0, &b.app).color(p.text));
                                        if b.active {
                                            ui.label(
                                                theme::regular(10.5, "live")
                                                    .color(tint),
                                            );
                                        }
                                    });
                                    ui.add_space(2.0);
                                    ui.label(
                                        theme::regular(11.0, format!(
                                            "{} · {}",
                                            b.district.label(),
                                            focusmap_core::model::format_minutes(b.minutes)
                                        ))
                                        .color(p.muted),
                                    );
                                    ui.label(
                                        theme::regular(11.0, format!(
                                            "score {:.0} · {} switches",
                                            b.score, b.switches
                                        ))
                                        .color(p.faint),
                                    );
                                    ui.add_space(3.0);
                                    let hint = if pinned == Some(index) {
                                        "pinned · click to release"
                                    } else {
                                        "click to pin"
                                    };
                                    ui.label(theme::regular(10.0, hint).color(p.faint));
                                });
                        });
                }

                ui.add_space(7.0);
                // Scenery fillers are unnamed; the core is the extra named one.
                let buildings = self
                    .city_buildings
                    .iter()
                    .filter(|b| !b.app.is_empty())
                    .count()
                    .saturating_sub(1);
                let replay = self
                    .city_replay
                    .map(|m| format!("Replay {:02}:{:02} · ", m / 60, m % 60))
                    .unwrap_or_default();
                let caption = if buildings == 0 {
                    "No scored app minutes yet. The city is on standby.".to_string()
                } else {
                    format!(
                        "{replay}{} · {} buildings · hover a tower for details",
                        self.city_mood.label(),
                        buildings
                    )
                };
                ui.label(theme::regular(10.5, caption).color(p.faint));

                if let Some(last) = self.live.samples.iter().map(|s| s.minute_of_day).max() {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(theme::regular(10.5, "Replay").color(p.muted));
                        let mut minute = self.city_replay.unwrap_or(last);
                        ui.spacing_mut().slider_width =
                            (ui.available_width() - 84.0).max(90.0);
                        let slider = ui.add(
                            egui::Slider::new(&mut minute, 0..=last)
                                .show_value(false)
                                .trailing_fill(true),
                        );
                        if slider.changed() {
                            self.set_city_replay(Some(minute));
                        }
                        let label = "Live".to_string();
                        if ui
                            .add_enabled(
                                self.city_replay.is_some(),
                                egui::Button::new(theme::regular(10.5, label).color(p.text))
                                    .small(),
                            )
                            .clicked()
                        {
                            self.set_city_replay(None);
                        }
                    });
                }
            } else {
                clicked_hour =
                    widgets::day_grid(ui, &theme, &self.live.samples, self.settings.patterns);
            }

            if !city_view {
                if let Some(hour) = selected_hour {
                    ui.add_space(10.0);
                    widgets::divider(ui, &theme);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            theme::bold(13.0, format!("{hour:02}:00 - {:02}:00", hour + 1))
                                .color(p.text),
                        );
                        ui.add_space(8.0);
                        if ui.small_button("Close").clicked() {
                            close_hour = true;
                        }
                    });
                    let mut values = [None; 60];
                    let mut total = 0u32;
                    let mut count = 0u32;
                    let mut switches = 0u32;
                    let mut apps: HashMap<&str, u32> = HashMap::new();
                    for sample in &self.live.samples {
                        if sample.minute_of_day / 60 == hour as u16 {
                            values[(sample.minute_of_day % 60) as usize] = Some(sample.score);
                            total += sample.score as u32;
                            count += 1;
                            switches += sample.switches;
                            if !sample.app.is_empty() {
                                *apps.entry(sample.app.as_str()).or_default() += 1;
                            }
                        }
                    }
                    ui.add_space(6.0);
                    widgets::minute_bars(ui, &theme, &values, 58.0);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        metric(
                            ui,
                            &theme,
                            "Average",
                            &if count > 0 {
                                (total / count).to_string()
                            } else {
                                "-".to_string()
                            },
                            "this hour",
                        );
                        ui.add_space(24.0);
                        metric(ui, &theme, "Minutes", &count.to_string(), "with activity");
                        ui.add_space(24.0);
                        metric(ui, &theme, "Switches", &switches.to_string(), "in this hour");
                        let top = apps
                            .into_iter()
                            .max_by_key(|(_, n)| *n)
                            .map(|(a, _)| a.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        ui.add_space(24.0);
                        metric(ui, &theme, "Mostly", &top, "foreground app");
                    });
                }
            }
        });
        self.set_city_view(city_view);        if let Some(hour) = clicked_hour {
            let hour = hour as u8;
            self.selected_hour = if self.selected_hour == Some(hour) {
                None
            } else {
                Some(hour)
            };
            if self.selected_hour.is_none() {
                close_hour = false;
            }
        }
        if close_hour {
            self.selected_hour = None;
        }

        // ---- sessions + leaks -------------------------------------------
        super::page_gap(ui);
        let sessions = self.live.sessions.clone();
        let day = self.live.today.clone();
        let mut leaks: HashMap<String, (AppCategory, u32, u32)> = HashMap::new();
        let mut fragmented = 0u32;
        let mut tracked = 0u32;
        for sample in &self.live.samples {
            if sample.score == 0 {
                continue;
            }
            tracked += 1;
            if matches!(
                sample.category,
                AppCategory::Chat | AppCategory::Media | AppCategory::Game
            ) {
                let entry = leaks
                    .entry(sample.app.clone())
                    .or_insert((sample.category, 0, 0));
                entry.1 += 1;
                entry.2 += sample.switches;
            }
            if sample.level == focusmap_core::model::FocusLevel::Fragmented {
                fragmented += 1;
            }
        }
        let mut leaks: Vec<(String, AppCategory, u32, u32)> = leaks
            .into_iter()
            .map(|(app, (cat, minutes, switches))| (app, cat, minutes, switches))
            .collect();
        leaks.sort_by(|a, b| {
            (b.3 * 2 + b.2)
                .cmp(&(a.3 * 2 + a.2))
                .then_with(|| a.0.cmp(&b.0))
        });

        two_columns(
            ui,
            0.63,
            |ui| {
                self.sessions_block(ui, &day, &sessions, "Deep sessions", 5);
            },
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "Where focus leaked",
                        "Chat, video and games can never score as deep work.",
                    );
                    ui.add_space(10.0);
                    if leaks.is_empty() && fragmented == 0 {
                        ui.label(
                            theme::regular(12.0, "Nothing has pulled at your attention yet today.")
                                .color(p.muted),
                        );
                    } else {
                        for (app, category, minutes, switches) in leaks.iter().take(3) {
                            ui.horizontal(|ui| {
                                widgets::dot(ui, p.fragmented, 3.0);
                                ui.vertical(|ui| {
                                    ui.label(theme::bold(12.0, app).color(p.text));
                                    ui.label(
                                        theme::regular(10.0, category.label()).color(p.faint),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        theme::regular(10.5, format!("{}m · {} sw", minutes, switches))
                                            .color(p.muted),
                                    );
                                });
                            });
                            ui.add_space(7.0);
                        }
                        if fragmented > 0 {
                            ui.add_space(4.0);
                            ui.label(
                                theme::regular(11.0, format!(
                                    "{fragmented} of {tracked} scored minutes were fragmented."
                                ))
                                .color(p.muted),
                            );
                        }
                    }
                    ui.add_space(12.0);
                    widgets::divider(ui, &theme);
                    ui.add_space(8.0);
                    ui.label(
                        theme::regular(10.5, "Every window switch lowers that minute's score. Fewer, longer blocks are the fix.")
                            .color(p.faint),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        theme::bold(9.5, "COUNTS ONLY")
                            .color(p.faint)
                            .extra_letter_spacing(0.6),
                    );
                    ui.label(
                        theme::regular(10.5, "No keys, titles, URLs or screenshots are stored.")
                            .color(p.muted),
                    );
                });
            },
        );

        ui.add_space(SPACE);
    }
}
