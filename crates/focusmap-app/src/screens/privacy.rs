//! Privacy and data: exactly what is stored, export, and the wipe switch.

use eframe::egui;
use egui::{Align, Layout, Margin, Stroke, Vec2};

use crate::app::{compact_number, open_folder, FocusMapApp};
use crate::theme::{self, SPACE};
use crate::widgets;

use super::{card_header, page_gap, two_columns};

impl FocusMapApp {
    pub(crate) fn screen_privacy(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        let counts = self.cache.counts.clone();
        let data_dir = self.settings.data_dir();
        let data_path = self.db.path().to_path_buf();
        let last_export = self.last_export.clone();
        let mut confirm = self.confirm_wipe;

        egui::Frame::NONE
            .fill(p.accent.gamma_multiply(if p.dark { 0.13 } else { 0.07 }))
            .stroke(Stroke::new(1.0, p.accent.gamma_multiply(0.35)))
            .corner_radius(16)
            .inner_margin(Margin::same(20))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(52.0), egui::Sense::hover());
                    let painter = ui.painter();
                    let center = rect.center();
                    painter.circle_filled(center, 24.0, p.accent.gamma_multiply(0.22));
                    let body = egui::Rect::from_center_size(
                        center + Vec2::new(0.0, 4.0),
                        Vec2::new(22.0, 17.0),
                    );
                    painter.rect_filled(body, egui::CornerRadius::same(4), p.accent);
                    painter.circle_stroke(
                        center + Vec2::new(0.0, -4.0),
                        7.0,
                        Stroke::new(2.6, p.accent),
                    );
                    painter.circle_filled(
                        center + Vec2::new(0.0, 4.0),
                        2.2,
                        if p.dark { p.bg } else { p.card },
                    );
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.label(
                            theme::bold(20.0, "Your data never leaves this PC.")
                                .color(p.text),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            theme::regular(12.0, "No account, no cloud, no telemetry, no network calls. The whole product is one local database and this window.")
                                .color(p.muted),
                        );
                    });
                });
            });

        page_gap(ui);
        ui.columns(4, |columns| {
            widgets::stat_card(
                &mut columns[0],
                &theme,
                "Minute rows",
                &compact_number(counts.minutes),
                "scored minutes",
                p.accent,
            );
            widgets::stat_card(
                &mut columns[1],
                &theme,
                "Days",
                &compact_number(counts.days),
                "daily rollups",
                p.deep,
            );
            widgets::stat_card(
                &mut columns[2],
                &theme,
                "Session tags",
                &compact_number(counts.tags),
                "labels you wrote",
                p.medium,
            );
            widgets::stat_card(
                &mut columns[3],
                &theme,
                "Database",
                &bytes(counts.bytes),
                "on disk, including WAL",
                p.fragmented,
            );
        });

        page_gap(ui);
        two_columns(
            ui,
            0.5,
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "What is stored",
                        "Numbers about timing, nothing about content.",
                    );
                    ui.add_space(10.0);
                    for (title, body) in [
                        ("Key presses", "A count per minute. The key itself is compared and discarded inside the hook."),
                        ("Backspaces", "A count, used for the steadiness term. No text is read."),
                        ("Mouse", "Total pointer distance and click count. Not a path, not a position history."),
                        ("Foreground app", "The executable name, e.g. Code.exe. Never the window title or document name."),
                        ("Sessions and tags", "Derived minutes plus the labels you add yourself."),
                    ] {
                        bullet(ui, &theme, title, body);
                    }
                });
            },
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "What is never touched",
                        "The line the design does not cross.",
                    );
                    ui.add_space(10.0);
                    for (title, body) in [
                        ("Keystrokes", "No characters, shortcuts, passwords or clipboard content."),
                        ("Screen", "No screenshots, OCR, webcam or microphone."),
                        ("Titles and URLs", "No page addresses, document names or chat contents."),
                        ("Network", "No analytics, sync, account or crash upload."),
                        ("Other people", "Only foreground-app names and input timing on this machine."),
                    ] {
                        bullet(ui, &theme, title, body);
                    }
                });
            },
        );

        page_gap(ui);
        two_columns(
            ui,
            0.55,
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(
                        ui,
                        &theme,
                        "Export your data",
                        "Two files, written locally: minute CSV and a full JSON bundle.",
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, &theme, "Export CSV + JSON").clicked() {
                            self.export_data();
                        }
                        if widgets::ghost_button(ui, &theme, "Open export folder").clicked() {
                            open_folder(&data_dir.join("exports"));
                        }
                    });
                    ui.add_space(8.0);
                    if let Some(path) = &last_export {
                        ui.label(
                            theme::regular(10.0, format!("Last file: {}", path.display()))
                                .color(p.good),
                        );
                    } else {
                        ui.label(
                            theme::regular(10.5, "Exports include every minute, daily rollup and tag. Nothing is redacted because nothing sensitive is there.")
                                .color(p.faint),
                        );
                    }
                });
            },
            |ui| {
                widgets::card(ui, &theme, |ui| {
                    card_header(ui, &theme, "Data folder", "One SQLite file and its WAL.");
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(data_path.display().to_string())
                            .monospace()
                            .size(10.0)
                            .color(p.muted),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if widgets::ghost_button(ui, &theme, "Open folder").clicked() {
                            open_folder(&data_dir);
                        }
                        ui.label(
                            theme::regular(10.0, "Minute rows compact after 90 days; daily totals stay.")
                                .color(p.faint),
                        );
                    });
                });
            },
        );

        page_gap(ui);
        let danger_fill = p.danger.gamma_multiply(if p.dark { 0.10 } else { 0.06 });
        egui::Frame::NONE
            .fill(danger_fill)
            .stroke(Stroke::new(1.0, p.danger.gamma_multiply(0.35)))
            .corner_radius(14)
            .inner_margin(Margin::same(18))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(theme::bold(14.0, "Wipe all tracking data").color(p.text));
                        ui.add_space(2.0);
                        ui.label(
                            theme::regular(11.0, "Deletes every minute, daily rollup and tag. Settings and your goal stay. This cannot be undone.")
                                .color(p.muted),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if confirm {
                            if ui
                                .add(
                                    egui::Button::new(
                                        theme::bold(12.0, "Yes, wipe everything")
                                            .color(egui::Color32::WHITE),
                                    )
                                    .fill(p.danger)
                                    .corner_radius(9)
                                    .min_size(Vec2::new(156.0, 32.0)),
                                )
                                .clicked()
                            {
                                self.wipe_all();
                                confirm = false;
                            }
                            if widgets::ghost_button(ui, &theme, "Cancel").clicked() {
                                confirm = false;
                            }
                        } else if ui
                            .add(
                                egui::Button::new(theme::regular(12.0, "Wipe data").color(p.danger))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, p.danger.gamma_multiply(0.6)))
                                    .corner_radius(9)
                                    .min_size(Vec2::new(112.0, 32.0)),
                            )
                            .clicked()
                        {
                            confirm = true;
                        }
                    });
                });
            });
        self.confirm_wipe = confirm;

        ui.add_space(12.0);
        ui.label(
            theme::regular(10.0, "FocusMap v1.0 · MIT licensed · built with Rust and egui · no network dependency for tracking")
                .color(p.faint),
        );
        ui.add_space(SPACE);
    }
}

fn bullet(ui: &mut egui::Ui, theme: &crate::theme::Theme, title: &str, body: &str) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        widgets::dot(ui, p.accent, 3.0);
        ui.vertical(|ui| {
            ui.label(theme::bold(11.5, title).color(p.text));
            ui.label(theme::regular(10.5, body).color(p.muted));
        });
    });
    ui.add_space(8.0);
}

fn bytes(value: u64) -> String {
    if value >= 1_048_576 {
        format!("{:.1} MB", value as f64 / 1_048_576.0)
    } else if value >= 1024 {
        format!("{:.0} KB", value as f64 / 1024.0)
    } else {
        format!("{value} B")
    }
}
