//! Settings: appearance, tracking, the score model, storage and about.

use eframe::egui;
use egui::{Align, Layout};

use focusmap_core::settings::{CityQuality, MotionMode, ThemeMode};
use focusmap_core::scoring::ScoreConfig;

use crate::app::{open_folder, FocusMapApp};
use crate::theme::{self, SPACE};
use crate::widgets;

use super::{card_header, page_gap, two_columns};

impl FocusMapApp {
    pub(crate) fn screen_settings(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        let ctx = ui.ctx().clone();

        widgets::card(ui, &theme, |ui| {
            card_header(ui, &theme, "Appearance", "High contrast and reduced motion are first-class, not an afterthought.");
            ui.add_space(12.0);
            let mut mode = self.settings.theme;
            widgets::setting_row(ui, &theme, "Theme", "Match the system, or pin it.", |ui| {
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
                self.appearance_changed(&ctx);
            }
            ui.add_space(8.0);
            let mut contrast = self.settings.high_contrast;
            let mut motion = self.settings.motion;
            let mut patterns = self.settings.patterns;
            widgets::setting_row(
                ui,
                &theme,
                "High contrast",
                "Stronger borders and text, for bright rooms and low vision.",
                |ui| widgets::toggle(ui, &theme, &mut contrast),
            );
            if contrast != self.settings.high_contrast {
                self.settings.high_contrast = contrast;
                self.appearance_changed(&ctx);
            }
            ui.add_space(8.0);
            widgets::setting_row(
                ui,
                &theme,
                "Motion",
                "Reduced motion removes ring and toggle animation.",
                |ui| {
                    widgets::segmented(
                        ui,
                        &theme,
                        &[
                            (MotionMode::Auto, "System"),
                            (MotionMode::Full, "Full"),
                            (MotionMode::Reduced, "Reduced"),
                        ],
                        &mut motion,
                    )
                },
            );
            if motion != self.settings.motion {
                self.settings.motion = motion;
                self.appearance_changed(&ctx);
            }
            ui.add_space(8.0);
            widgets::setting_row(
                ui,
                &theme,
                "Pattern fills",
                "Hatches medium work and dots fragmented work, so colour is not the only signal.",
                |ui| widgets::toggle(ui, &theme, &mut patterns),
            );
            if patterns != self.settings.patterns {
                self.settings.patterns = patterns;
                self.settings_changed();
            }
            ui.add_space(8.0);
            let mut quality = self.settings.city_quality;
            widgets::setting_row(
                ui,
                &theme,
                "City detail",
                "High targets 60 fps, balanced 30, low renders smaller and drops traffic.",
                |ui| {
                    widgets::segmented(
                        ui,
                        &theme,
                        &[
                            (CityQuality::High, "High"),
                            (CityQuality::Balanced, "Balanced"),
                            (CityQuality::Low, "Low"),
                        ],
                        &mut quality,
                    )
                },
            );
            if quality != self.settings.city_quality {
                self.settings.city_quality = quality;
                self.settings_changed();
            }
            ui.add_space(8.0);
            let mut sound = self.settings.ambient_sound;
            widgets::setting_row(
                ui,
                &theme,
                "Ambient sound",
                "A quiet local drone. No audio files, no network, off by default.",
                |ui| widgets::toggle(ui, &theme, &mut sound),
            );
            if sound != self.settings.ambient_sound {
                self.settings.ambient_sound = sound;
                self.settings_changed();
            }
            ui.add_space(8.0);
            let mut scale = self.settings.text_scale;
            widgets::setting_row(
                ui,
                &theme,
                "Text size",
                "Scales the whole interface, not just labels.",
                |ui| {
                    ui.add(
                        egui::Slider::new(&mut scale, 0.8..=1.4)
                            .fixed_decimals(2)
                            .suffix("×"),
                    )
                },
            );
            if (scale - self.settings.text_scale).abs() > f32::EPSILON {
                self.settings.text_scale = scale;
                self.appearance_changed(&ctx);
            }
        });

        page_gap(ui);
        let data_dir = self.settings.data_dir();
        let mut use_default = false;
        let mut open_storage = false;
        two_columns(
            ui,
            0.5,
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(ui, &theme, "Tracking", "The engine applies these on the next minute.");
                    ui.add_space(12.0);
                    let mut idle = (self.settings.idle_timeout_secs / 60).clamp(1, 30);
                    widgets::setting_row(
                        ui,
                        &theme,
                        "Idle timeout",
                        "No input for this long closes the session and is not scored.",
                        |ui| {
                            ui.add(
                                egui::Slider::new(&mut idle, 1..=30)
                                    .step_by(1.0)
                                    .suffix(" min"),
                            )
                        },
                    );
                    if idle * 60 != self.settings.idle_timeout_secs {
                        self.settings.idle_timeout_secs = idle * 60;
                        self.settings_changed();
                    }
                    ui.add_space(12.0);
                    let mut boot = self.settings.start_on_boot;
                    widgets::setting_row(
                        ui,
                        &theme,
                        "Start on boot",
                        "Per-user Windows Run key. No admin rights, no service.",
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
                    ui.add_space(12.0);
                    let mut focus_mode = self.settings.focus_mode;
                    widgets::setting_row(
                        ui,
                        &theme,
                        "Mute system notifications",
                        "In-app toasts still work.",
                        |ui| widgets::toggle(ui, &theme, &mut focus_mode),
                    );
                    if focus_mode != self.settings.focus_mode {
                        self.settings.focus_mode = focus_mode;
                        self.settings_changed();
                    }
                });
            },
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(ui, &theme, "Storage", "One file, bounded forever.");
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(data_dir.display().to_string())
                            .monospace()
                            .size(10.0)
                            .color(p.muted),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if widgets::ghost_button(ui, &theme, "Open folder").clicked() {
                            open_storage = true;
                        }
                        if widgets::ghost_button(ui, &theme, "Use default").clicked() {
                            use_default = true;
                        }
                    });
                    ui.add_space(8.0);
                    ui.label(
                        theme::regular(10.5, "Minute rows older than 90 days are compacted away. Daily totals and streaks survive.")
                            .color(p.faint),
                    );
                });
            },
        );
        if open_storage {
            open_folder(&data_dir);
        }
        if use_default {
            self.data_dir_draft.clear();
            self.apply_data_dir();
        }

        page_gap(ui);
        widgets::card(ui, &theme, |ui| {
            card_header(
                ui,
                &theme,
                "Focus score model",
                "Every minute is scored from timing counts. The weights are yours.",
            );
            ui.add_space(12.0);
            score_term(ui, &theme, "Activity", self.settings.score.activity_weight, "Input per active second. Peaks around 1.8 events/s.");
            score_term(ui, &theme, "Continuity", self.settings.score.continuity_weight, "How much of the minute had input at all.");
            score_term(ui, &theme, "Steadiness", self.settings.score.steadiness_weight, "Low backspace ratio and no dead stops.");
            score_term(ui, &theme, "Switches", -self.settings.score.switch_penalty, "Penalty per five foreground changes.");
            score_term(ui, &theme, "Fragmentation", -self.settings.score.fragmentation_penalty, "Penalty for many short bursts.");
            score_term(ui, &theme, "Mouse jitter", -self.settings.score.mouse_penalty, "Penalty for large pointer jumps per event.");
            ui.add_space(8.0);
            ui.label(
                theme::regular(10.5, "Positive weights are normalised. Category caps apply last: games and video 25, chat 50, meetings 45.")
                    .color(p.faint),
            );
            ui.add_space(10.0);
            let mut cfg = self.settings.score.clone();
            let mut changed = false;
            egui::CollapsingHeader::new("Tune the weights")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    changed |= weight_slider(ui, "Activity weight", &mut cfg.activity_weight);
                    changed |= weight_slider(ui, "Continuity weight", &mut cfg.continuity_weight);
                    changed |= weight_slider(ui, "Steadiness weight", &mut cfg.steadiness_weight);
                    changed |= weight_slider(ui, "Switch penalty", &mut cfg.switch_penalty);
                    changed |= weight_slider(ui, "Fragmentation penalty", &mut cfg.fragmentation_penalty);
                    changed |= weight_slider(ui, "Mouse penalty", &mut cfg.mouse_penalty);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(theme::regular(11.5, "Peak input rate").color(p.muted));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut cfg.peak_rate, 0.4..=6.0)
                                        .fixed_decimals(1)
                                        .suffix(" ev/s"),
                                )
                                .changed();
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(theme::regular(11.5, "Deep threshold").color(p.muted));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            changed |= ui
                                .add(egui::Slider::new(&mut cfg.deep_threshold, 50..=95))
                                .changed();
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(theme::regular(11.5, "Steady threshold").color(p.muted));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            changed |= ui
                                .add(egui::Slider::new(&mut cfg.medium_threshold, 15..=80))
                                .changed();
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(theme::regular(11.5, "Manual focus floor").color(p.muted));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            changed |= ui
                                .add(egui::Slider::new(&mut cfg.manual_floor, 40..=100))
                                .changed();
                        });
                    });
                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, &theme, "Reset weights").clicked() {
                        cfg = ScoreConfig::default();
                        changed = true;
                    }
                });
            if changed {
                self.settings.score = cfg;
                self.settings_changed();
            }
        });

        page_gap(ui);
        let data_dir = self.settings.data_dir();
        let source = self.live.source.clone();
        let font = self
            .font_name
            .clone()
            .unwrap_or_else(|| "built-in default".to_string());
        let mem = format!(
            "{} · target < 30 MB",
            memory(self.resources.working_set)
        );
        let cpu = format!("{:.1}% · target < 1%", self.resources.cpu_pct);
        let db_size = memory(self.cache.counts.bytes);
        let mut show_welcome = false;
        let mut open_data = false;
        two_columns(
            ui,
            0.55,
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(ui, &theme, "Data folder", "Changing it writes settings only; the database is not moved for you.");
                    ui.add_space(10.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.data_dir_draft)
                            .desired_width(ui.available_width())
                            .hint_text("Default local app data folder"),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, &theme, "Apply folder").clicked() {
                            self.apply_data_dir();
                        }
                        ui.label(
                            theme::regular(10.0, "Restart FocusMap to use it.")
                                .color(p.faint),
                        );
                    });
                });
            },
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(ui, &theme, "About", "Built to stay small and quiet.");
                    ui.add_space(10.0);
                    about_row(ui, &theme, "Version", env!("CARGO_PKG_VERSION"));
                    about_row(ui, &theme, "Renderer", "egui + wgpu");
                    about_row(ui, &theme, "Capture", &source);
                    about_row(ui, &theme, "Font", &font);
                    about_row(ui, &theme, "Memory", &mem);
                    about_row(ui, &theme, "Idle CPU", &cpu);
                    about_row(ui, &theme, "Database", &db_size);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if widgets::ghost_button(ui, &theme, "Show welcome again").clicked() {
                            show_welcome = true;
                        }
                        if widgets::ghost_button(ui, &theme, "Open data folder").clicked() {
                            open_data = true;
                        }
                    });
                });
            },
        );
        if open_data {
            open_folder(&data_dir);
        }
        if show_welcome {
            self.settings.onboarded = false;
            self.onboarding_step = 0;
            self.settings_changed();
            self.persist_now();
        }

        ui.add_space(SPACE);
    }
}

fn weight_slider(ui: &mut egui::Ui, label: &str, value: &mut f32) -> bool {
    let mut changed = ui
        .add(
            egui::Slider::new(value, 0.0..=1.0)
                .fixed_decimals(2)
                .text(label),
        )
        .changed();
    if changed {
        changed = true;
    }
    changed
}

fn score_term(
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
    label: &str,
    weight: f32,
    detail: &str,
) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        ui.label(theme::bold(11.5, label).color(p.text));
        ui.add_space(6.0);
        ui.label(
            theme::regular(10.5, format!("{:+.2}", weight))
                .color(if weight >= 0.0 { p.good } else { p.fragmented }),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::regular(10.0, detail).color(p.faint));
        });
    });
    ui.add_space(5.0);
}

fn about_row(ui: &mut egui::Ui, theme: &crate::theme::Theme, label: &str, value: &str) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        ui.label(theme::regular(10.5, label).color(p.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::regular(10.5, value).color(p.text));
        });
    });
    ui.add_space(4.0);
}

fn memory(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
