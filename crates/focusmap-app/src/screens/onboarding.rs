//! Three-screen welcome. It sets a goal, explains the privacy line, and never
//! blocks the app behind an account.

use eframe::egui;
use egui::{Align, Layout, Margin, Stroke, Vec2};

use focusmap_core::model::format_minutes;
use focusmap_core::settings::ThemeMode;

use crate::app::FocusMapApp;
use crate::theme::{self, SPACE};
use crate::widgets;

impl FocusMapApp {
    pub(crate) fn screen_onboarding(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let palette = theme.palette;
        let step = self.onboarding_step.min(2);
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .inner_margin(Margin::symmetric(28, 24)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let width = ui.available_width().min(760.0);
                        let margin = ((ui.available_width() - width) / 2.0).max(0.0);
                        ui.horizontal(|ui| {
                            ui.add_space(margin);
                            ui.vertical(|ui| {
                                ui.set_width(width);
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        Vec2::splat(38.0),
                                        egui::Sense::hover(),
                                    );
                                    let painter = ui.painter();
                                    painter.circle_filled(rect.center(), 17.0, palette.accent);
                                    painter.circle_stroke(
                                        rect.center(),
                                        9.0,
                                        Stroke::new(2.2, egui::Color32::WHITE),
                                    );
                                    painter.circle_filled(
                                        rect.center() + Vec2::new(9.5, -9.5),
                                        3.0,
                                        egui::Color32::WHITE,
                                    );
                                    ui.add_space(4.0);
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        ui.label(
                                            theme::bold(21.0, "FocusMap").color(palette.text),
                                        );
                                        ui.label(
                                            theme::regular(10.5, "strava for your pc")
                                                .color(palette.faint),
                                        );
                                    });
                                });

                                ui.add_space(SPACE * 3.0);
                                widgets::card(ui, &theme, |ui| {
                                    ui.set_min_height(390.0);
                                    match step {
                                        0 => self.onboarding_privacy(ui),
                                        1 => self.onboarding_goal(ui),
                                        _ => self.onboarding_look(ui, &ctx),
                                    }

                                    ui.add_space(SPACE * 3.0);
                                    widgets::divider(ui, &theme);
                                    ui.add_space(12.0);
                                    ui.horizontal(|ui| {
                                        for index in 0..3 {
                                            let (rect, _) = ui.allocate_exact_size(
                                                Vec2::new(22.0, 8.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                rect,
                                                egui::CornerRadius::same(4),
                                                if index <= step {
                                                    palette.accent
                                                } else {
                                                    palette.track
                                                },
                                            );
                                        }
                                        ui.add_space(6.0);
                                        ui.label(
                                            theme::regular(10.0, format!("Step {} of 3", step + 1))
                                                .color(palette.faint),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if step == 2 {
                                                    if widgets::primary_button(
                                                        ui,
                                                        &theme,
                                                        "Open FocusMap",
                                                    )
                                                    .clicked()
                                                    {
                                                        self.finish_onboarding(&ctx);
                                                    }
                                                } else if widgets::primary_button(
                                                    ui,
                                                    &theme,
                                                    "Continue",
                                                )
                                                .clicked()
                                                {
                                                    self.onboarding_step = step + 1;
                                                }
                                                if step > 0
                                                    && widgets::ghost_button(
                                                        ui,
                                                        &theme,
                                                        "Back",
                                                    )
                                                    .clicked()
                                                {
                                                    self.onboarding_step = step - 1;
                                                }
                                                if step == 0
                                                    && widgets::ghost_button(
                                                        ui,
                                                        &theme,
                                                        "Skip",
                                                    )
                                                    .clicked()
                                                {
                                                    self.finish_onboarding(&ctx);
                                                }
                                            },
                                        );
                                    });
                                });
                                ui.add_space(16.0);
                                ui.label(
                                    theme::regular(10.0, "No account. No cloud. No keystroke content.")
                                        .color(palette.faint),
                                );
                            });
                        });
                    });
            });
    }

    fn onboarding_privacy(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        ui.label(
            theme::bold(9.5, "WELCOME")
                .color(p.accent)
                .extra_letter_spacing(0.9),
        );
        ui.add_space(4.0);
        ui.label(theme::bold(27.0, "Know where your focus goes.").color(p.text));
        ui.add_space(4.0);
        ui.label(
            theme::regular(13.0, "FocusMap reads input timing from your own machine, scores each minute, and draws the day. It never reads what you type.")
                .color(p.muted),
        );
        ui.add_space(20.0);
        ui.columns(3, |columns| {
            for (column, (title, body)) in columns.iter_mut().zip([
                ("Counts only", "Keys and clicks as numbers. Never characters, shortcuts or clipboard."),
                ("One local file", "SQLite in your local app data folder. No server, no sync."),
                ("No account", "No sign-up, no email, no telemetry. Delete everything with one button."),
            ]) {
                egui::Frame::NONE
                    .fill(p.card_alt)
                    .corner_radius(12)
                    .inner_margin(Margin::same(12))
                    .show(column, |ui| {
                        ui.set_min_height(96.0);
                        ui.label(theme::bold(12.0, title).color(p.text));
                        ui.add_space(4.0);
                        ui.label(theme::regular(10.5, body).color(p.muted));
                    });
            }
        });
        ui.add_space(18.0);
        ui.label(theme::bold(11.0, "The line the app does not cross").color(p.text));
        ui.add_space(6.0);
        for line in [
            "No keystroke content, window titles, URLs or screenshots.",
            "No network calls, analytics or crash uploads.",
            "Only the foreground executable name, e.g. Code.exe.",
        ] {
            ui.horizontal(|ui| {
                widgets::dot(ui, p.good, 3.0);
                ui.label(theme::regular(11.0, line).color(p.muted));
            });
            ui.add_space(4.0);
        }
    }

    fn onboarding_goal(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        ui.label(
            theme::bold(9.5, "THE TARGET")
                .color(p.accent)
                .extra_letter_spacing(0.9),
        );
        ui.add_space(4.0);
        ui.label(theme::bold(27.0, "What does a good day look like?").color(p.text));
        ui.add_space(4.0);
        ui.label(
            theme::regular(13.0, "Pick a deep-work goal you can hit on a normal day, not a heroic one. The streak follows the goal, so make it honest.")
                .color(p.muted),
        );
        ui.add_space(20.0);
        ui.label(
            theme::bold(40.0, format_minutes(self.settings.goal_minutes)).color(p.text),
        );
        ui.add_space(2.0);
        let changed = ui
            .add(
                egui::Slider::new(&mut self.settings.goal_minutes, 30..=480)
                    .step_by(15.0)
                    .trailing_fill(true),
            )
            .changed();
        if changed {
            self.settings_changed();
        }
        ui.add_space(16.0);
        let mut idle = (self.settings.idle_timeout_secs / 60).clamp(1, 30);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(theme::bold(12.0, "Idle timeout").color(p.text));
                ui.label(
                    theme::regular(10.5, "No input for this long ends the session and is not scored.")
                        .color(p.muted),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    egui::Slider::new(&mut idle, 1..=30)
                        .step_by(1.0)
                        .suffix(" min"),
                );
            });
        });
        if idle * 60 != self.settings.idle_timeout_secs {
            self.settings.idle_timeout_secs = idle * 60;
            self.settings_changed();
        }
        ui.add_space(18.0);
        egui::Frame::NONE
            .fill(p.card_alt)
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.label(theme::bold(11.5, "A deep minute needs three things").color(p.text));
                ui.add_space(4.0);
                ui.label(
                    theme::regular(10.5, "Steady input, continuity across the minute, and few window switches. Games, video, chat and meetings are capped so they can never pass as deep work.")
                        .color(p.muted),
                );
            });
    }

    fn onboarding_look(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let theme = self.theme;
        let p = theme.palette;
        ui.label(
            theme::bold(9.5, "LAST BIT")
                .color(p.accent)
                .extra_letter_spacing(0.9),
        );
        ui.add_space(4.0);
        ui.label(theme::bold(27.0, "Make it yours.").color(p.text));
        ui.add_space(4.0);
        ui.label(
            theme::regular(13.0, "These live in Settings later. Nothing here is permanent.")
                .color(p.muted),
        );
        ui.add_space(20.0);
        let mut mode = self.settings.theme;
        widgets::setting_row(ui, &theme, "Theme", "Follow the system or pin dark/light.", |ui| {
            widgets::segmented(
                ui,
                &theme,
                &[
                    (ThemeMode::Auto, "System"),
                    (ThemeMode::Dark, "Dark"),
                    (ThemeMode::Light, "Light"),
                ],
                &mut mode,
            )
        });
        if mode != self.settings.theme {
            self.settings.theme = mode;
            self.appearance_changed(ctx);
        }
        ui.add_space(12.0);
        let mut contrast = self.settings.high_contrast;
        widgets::setting_row(
            ui,
            &theme,
            "High contrast",
            "Stronger borders and text.",
            |ui| widgets::toggle(ui, &theme, &mut contrast),
        );
        if contrast != self.settings.high_contrast {
            self.settings.high_contrast = contrast;
            self.appearance_changed(ctx);
        }
        ui.add_space(12.0);
        let mut boot = self.settings.start_on_boot;
        widgets::setting_row(
            ui,
            &theme,
            "Start on boot",
            "Optional. Uses the per-user Run key, no admin rights.",
            |ui| widgets::toggle(ui, &theme, &mut boot),
        );
        if boot != self.settings.start_on_boot {
            self.settings.start_on_boot = boot;
            self.apply_start_on_boot(boot);
        }
        ui.add_space(12.0);
        let mut minimized = self.settings.start_minimized;
        widgets::setting_row(
            ui,
            &theme,
            "Start in the tray",
            "Tracking starts, the window stays hidden.",
            |ui| widgets::toggle(ui, &theme, &mut minimized),
        );
        if minimized != self.settings.start_minimized {
            self.settings.start_minimized = minimized;
            self.settings_changed();
        }
    }

    fn finish_onboarding(&mut self, _ctx: &egui::Context) {
        self.settings.onboarded = true;
        self.settings.score.sanitize();
        self.engine.apply_settings(&self.settings);
        self.settings_dirty = true;
        self.persist_now();
        self.toast(
            "You are set",
            "The first full minute will appear on Today.",
            self.theme.palette.good,
        );
    }
}
