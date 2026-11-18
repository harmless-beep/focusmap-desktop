//! Share card: a locally rendered 1080 square, previewed before it is written.

use eframe::egui;
use egui::{Align, Layout, Margin, Stroke, Vec2};

use focusmap_core::model::format_minutes;

use crate::app::{open_folder, pictures_dir, FocusMapApp};
use crate::theme::{self, SPACE};
use crate::widgets;

use super::card_header;

impl FocusMapApp {
    pub(crate) fn screen_share(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        let has_data = self.live.summary.tracked_minutes > 0;
        if has_data {
            self.ensure_share_texture(ui.ctx());
        } else {
            self.share.texture = None;
        }
        let texture_id = self.share.texture.as_ref().map(|t| t.id());
        let error = self.share.error.clone();
        let saved = self.share.saved.clone();
        let summary = self.live.summary.clone();
        let streak = self.share_data().streak;
        let goal = self.settings.goal_minutes;
        let day = self.live.today.clone();

        let mut save_clicked = false;
        let mut city_clicked = false;
        let mut open_clicked = false;
        let mut refresh_clicked = false;
        let mut go_today = false;

        let width = ui.available_width();
        let gap = 12.0;
        let left_width = (width - gap) * 0.52;
        let right_width = width - gap - left_width;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(left_width);
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "Preview",
                        "1080 × 1080 PNG. Dark on purpose, readable in any feed.",
                    );
                    ui.add_space(10.0);
                    if let Some(id) = texture_id {
                        let size = ui.available_width().min(470.0);
                        ui.add(egui::Image::new(egui::load::SizedTexture::new(
                            id,
                            Vec2::splat(size),
                        )));
                    } else if let Some(error) = &error {
                        egui::Frame::NONE
                            .fill(p.danger.gamma_multiply(0.12))
                            .stroke(Stroke::new(1.0, p.danger.gamma_multiply(0.35)))
                            .corner_radius(10)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(
                                    theme::regular(11.5, format!("Could not render the card: {error}"))
                                        .color(p.danger),
                                );
                            });
                    } else {
                        widgets::empty_state(
                            ui,
                            &theme,
                            "Nothing to share yet",
                            "Track a few minutes first. The card is built from today's local data.",
                        );
                        if widgets::ghost_button(ui, &theme, "Open Today").clicked() {
                            go_today = true;
                        }
                    }
                });
            });
            ui.add_space(gap);
            ui.vertical(|ui| {
                ui.set_width(right_width);
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "Today's card",
                        &focusmap_core::model::pretty_day(&day),
                    );
                    ui.add_space(12.0);
                    share_row(
                        ui,
                        &theme,
                        "Focus score",
                        &summary.score.to_string(),
                    );
                    share_row(
                        ui,
                        &theme,
                        "Deep work",
                        &format_minutes(summary.deep_minutes),
                    );
                    share_row(
                        ui,
                        &theme,
                        "Longest session",
                        &format_minutes(summary.longest_session_minutes),
                    );
                    share_row(ui, &theme, "Streak", &format!("{streak} days"));
                    share_row(
                        ui,
                        &theme,
                        "Goal",
                        &format_minutes(goal),
                    );
                    ui.add_space(12.0);
                    widgets::divider(ui, &theme);
                    ui.add_space(10.0);
                    if widgets::primary_button(ui, &theme, "Save PNG").clicked() && has_data {
                        save_clicked = true;
                    }
                    ui.add_space(6.0);
                    let city_button = egui::Button::new(
                        theme::regular(13.0, "Save Appolis card").color(p.text),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0, p.border))
                    .corner_radius(100)
                    .min_size(Vec2::new(92.0, 36.0));
                    if ui
                        .add_enabled(self.city_ready && has_data, city_button)
                        .clicked()
                    {
                        city_clicked = true;
                    }
                    ui.add_space(6.0);
                    if widgets::ghost_button(ui, &theme, "Open folder").clicked() {
                        open_clicked = true;
                    }
                    ui.add_space(6.0);
                    if widgets::ghost_button(ui, &theme, "Refresh preview").clicked() {
                        refresh_clicked = true;
                    }
                    ui.add_space(10.0);
                    ui.label(
                        theme::regular(10.5, "No upload, no account, no watermark. The PNG is written to Pictures\\FocusMap.")
                            .color(p.faint),
                    );
                    if let Some(saved) = &saved {
                        ui.add_space(8.0);
                        ui.label(
                            theme::regular(10.0, format!("Last save: {}", saved.display()))
                                .color(p.good),
                        );
                    }
                });
                ui.add_space(12.0);
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "What is on it",
                        "Only numbers you can already see in the app.",
                    );
                    ui.add_space(8.0);
                    for line in [
                        "Focus score and deep-work minutes",
                        "Longest unbroken session and streak",
                        "One bar per hour of the day",
                        "The date, in local time",
                        "The Appolis skyline, with no app names drawn on it",
                    ] {
                        ui.horizontal(|ui| {
                            widgets::dot(ui, p.accent, 2.8);
                            ui.label(theme::regular(11.0, line).color(p.muted));
                        });
                        ui.add_space(3.0);
                    }
                    ui.add_space(4.0);
                    ui.label(
                        theme::regular(10.5, "No app names, tags or window titles are drawn on the card.")
                            .color(p.faint),
                    );
                });
            });
        });

        if save_clicked {
            self.save_share();
        }
        if city_clicked {
            self.save_city_share();
        }
        if open_clicked {
            open_folder(&pictures_dir().join("FocusMap"));
        }
        if refresh_clicked {
            self.share.texture = None;
            self.share.error = None;
        }
        if go_today {
            self.go(crate::app::Screen::Today);
        }
        ui.add_space(SPACE);
    }
}

fn share_row(ui: &mut egui::Ui, theme: &crate::theme::Theme, label: &str, value: &str) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        ui.label(theme::regular(11.5, label).color(p.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::bold(12.5, value).color(p.text));
        });
    });
    ui.add_space(7.0);
}
