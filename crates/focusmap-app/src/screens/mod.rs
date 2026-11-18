//! One file per page. Each page is an extension of [`FocusMapApp`], so it can
//! read the cached live state without a second layer of view models.

mod goals;
mod insights;
mod onboarding;
mod privacy;
mod settings;
mod share;
mod today;
mod week;

use eframe::egui;

use crate::theme::{self, Theme};

use focusmap_core::apps::AppCategory;

pub(crate) fn card_header(ui: &mut egui::Ui, theme: &Theme, title: &str, caption: &str) {
    let p = theme.palette;
    ui.label(theme::bold(15.0, title).color(p.text));
    if !caption.is_empty() {
        ui.add_space(1.0);
        ui.label(theme::regular(11.0, caption).color(p.faint));
    }
}

pub(crate) fn metric(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str, caption: &str) {
    let p = theme.palette;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.label(theme::bold(9.5, label.to_uppercase()).color(p.faint));
        ui.label(theme::bold(18.0, value).color(p.text));
        if !caption.is_empty() {
            ui.label(theme::regular(9.5, caption).color(p.muted));
        }
    });
}

pub(crate) fn score_band(score: u8, deep: u8, medium: u8) -> (&'static str, &'static str) {
    if score >= deep {
        (
            "In flow",
            "Sustained input, few switches. This is the work that compounds.",
        )
    } else if score >= medium {
        (
            "Steady",
            "Good rhythm with some interruption. One longer block would lift this.",
        )
    } else if score >= 15 {
        (
            "Scattered",
            "Lots of starts and stops. The next deep block is worth protecting.",
        )
    } else {
        (
            "Quiet",
            "Not much happened yet. Your map fills itself while you work.",
        )
    }
}

pub(crate) fn page_gap(ui: &mut egui::Ui) {
    ui.add_space(12.0);
}

pub(crate) fn stat_row(ui: &mut egui::Ui, theme: &Theme, cards: [(&str, String, &str, egui::Color32); 3]) {
    ui.columns(3, |columns| {
        for (column, (label, value, caption, tint)) in columns.iter_mut().zip(cards) {
            crate::widgets::stat_card(column, theme, label, &value, caption, tint);
        }
    });
}

pub(crate) fn two_columns(
    ui: &mut egui::Ui,
    left_ratio: f32,
    left: impl FnOnce(&mut egui::Ui),
    right: impl FnOnce(&mut egui::Ui),
) {
    let width = ui.available_width();
    let gap = 12.0;
    let left_width = (width - gap) * left_ratio;
    let right_width = width - gap - left_width;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(left_width);
            left(ui);
        });
        ui.add_space(gap);
        ui.vertical(|ui| {
            ui.set_width(right_width);
            right(ui);
        });
    });
}

pub(crate) fn category_tint(theme: &Theme, category: AppCategory) -> egui::Color32 {
    let p = theme.palette;
    match category {
        AppCategory::Editor | AppCategory::Terminal | AppCategory::Office => p.deep,
        AppCategory::Browser => p.accent,
        AppCategory::Chat => p.fragmented,
        AppCategory::Meeting => p.warn,
        AppCategory::Media | AppCategory::Game => p.danger,
        AppCategory::Other => p.muted,
    }
}
