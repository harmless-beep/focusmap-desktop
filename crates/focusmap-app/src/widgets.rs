//! Reusable pieces: cards, the score ring, pills, toggles, segmented tabs,
//! empty states and small charts. Screens compose these instead of re-inventing
//! spacing and colour on every page.

use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, Ui, Vec2,
};
use focusmap_core::model::FocusLevel;

use crate::theme::{self, Theme, SPACE};

pub fn card<R>(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    theme.card_frame().show(ui, add).inner
}

pub fn pill(ui: &mut Ui, theme: &Theme, text: &str, tint: Color32) -> Response {
    let p = theme.palette;
    egui::Frame::NONE
        .fill(tint.gamma_multiply(if p.dark { 0.18 } else { 0.12 }))
        .stroke(Stroke::new(1.0, tint.gamma_multiply(0.45)))
        .corner_radius(100)
        .inner_margin(egui::Margin::symmetric(9, 3))
        .show(ui, |ui| {
            ui.label(theme::bold(11.0, text).color(tint));
        })
        .response
}

pub fn dot(ui: &mut Ui, color: Color32, radius: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(radius * 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), radius, color);
}

/// Triangle strip along a circular arc, colour-lerped from `from` to `to`.
fn arc_mesh(
    center: egui::Pos2,
    radius: f32,
    width: f32,
    start: f32,
    sweep: f32,
    from: Color32,
    to: Color32,
) -> egui::Mesh {
    let steps = 160u32;
    let half = width / 2.0;
    // egui feathers paths but not meshes, so the strip carries its own
    // one-pixel fade on both rims. Without it the arc is visibly harder than
    // the circle drawn under it.
    let feather = 1.0;
    let mut mesh = egui::Mesh::default();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let dir = Vec2::angled(start + sweep * t);
        let color = theme::mix(from, to, t);
        let fade = theme::alpha(color, 0);
        mesh.colored_vertex(center + dir * (radius + half + feather), fade);
        mesh.colored_vertex(center + dir * (radius + half), color);
        mesh.colored_vertex(center + dir * (radius - half), color);
        mesh.colored_vertex(center + dir * (radius - half - feather), fade);
    }
    for i in 0..steps {
        let a = i * 4;
        for row in 0..3 {
            mesh.add_triangle(a + row, a + row + 1, a + row + 4);
            mesh.add_triangle(a + row + 1, a + row + 5, a + row + 4);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_has_a_soft_rim_and_solid_core() {
        let mesh = arc_mesh(
            egui::Pos2::ZERO,
            100.0,
            10.0,
            0.0,
            std::f32::consts::TAU,
            Color32::WHITE,
            Color32::WHITE,
        );
        assert_eq!(mesh.vertices.len(), 644);
        assert_eq!(mesh.indices.len(), 160 * 3 * 6);
        // Outer rim fades, the two middle rows are solid, inner rim fades.
        assert_eq!(mesh.vertices[0].color.a(), 0);
        assert_eq!(mesh.vertices[1].color.a(), 255);
        assert_eq!(mesh.vertices[2].color.a(), 255);
        assert_eq!(mesh.vertices[3].color.a(), 0);
        assert!((mesh.vertices[0].pos.x - 106.0).abs() < 0.01);
        assert!((mesh.vertices[3].pos.x - 94.0).abs() < 0.01);
    }
}

/// A card with a label, a big number and a caption.
pub fn stat_card(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    value: &str,
    caption: &str,
    tint: Color32,
) {
    let p = theme.palette;
    theme.card_frame().show(ui, |ui| {
        ui.set_min_height(96.0);
        ui.horizontal(|ui| {
            dot(ui, tint, 3.5);
            ui.label(theme::bold(11.0, label.to_uppercase()).color(p.muted));
        });
        ui.add_space(6.0);
        ui.label(theme::bold(30.0, value).color(p.text));
        ui.add_space(2.0);
        ui.label(theme::regular(11.5, caption).color(p.faint));
    });
}

/// Circular gauge. `fraction` is 0..1 and already eased by the caller.
pub fn ring(
    ui: &mut Ui,
    theme: &Theme,
    diameter: f32,
    fraction: f32,
    center: &str,
    below: &str,
    tint: Color32,
) -> Response {
    let p = theme.palette;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
    let painter = ui.painter();
    let center_pos = rect.center();
    let width = (diameter * 0.055).clamp(7.0, 14.0);
    let radius = diameter / 2.0 - width;

    painter.circle_stroke(center_pos, radius, Stroke::new(width, p.track));

    let frac = fraction.clamp(0.0, 1.0);
    if frac > 0.001 {
        let start = -std::f32::consts::FRAC_PI_2;
        let sweep = frac * std::f32::consts::TAU;
        let from = tint;
        let to = theme::mix(tint, p.medium, 0.55);
        // Soft halo under the arc, then the gradient stroke itself.
        painter.add(egui::Shape::mesh(arc_mesh(
            center_pos,
            radius,
            width * 2.6,
            start,
            sweep,
            theme::alpha(tint, 26),
            theme::alpha(to, 26),
        )));
        painter.add(egui::Shape::mesh(arc_mesh(
            center_pos, radius, width, start, sweep, from, to,
        )));
        // Rounded caps: egui paths have butt ends, two dots finish the arc.
        let cap = |angle: f32| center_pos + Vec2::angled(angle) * radius;
        painter.circle_filled(cap(start), width / 2.0, from);
        painter.circle_filled(cap(start + sweep), width / 2.0, to);
    }

    painter.text(
        center_pos + Vec2::new(0.0, -diameter * 0.02),
        Align2::CENTER_CENTER,
        center,
        FontId::new(diameter * 0.26, theme::bold_family()),
        p.text,
    );
    painter.text(
        center_pos + Vec2::new(0.0, diameter * 0.16),
        Align2::CENTER_CENTER,
        below,
        FontId::proportional((diameter * 0.052).clamp(9.5, 13.0)),
        p.muted,
    );
    response
}

/// Two or three options in a rounded track.
pub fn segmented<T: Copy + PartialEq>(
    ui: &mut Ui,
    theme: &Theme,
    options: &[(T, &str)],
    selected: &mut T,
) -> bool {
    let p = theme.palette;
    let mut changed = false;
    egui::Frame::NONE
        .fill(p.card_alt)
        .corner_radius(12)
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (value, label) in options {
                    let active = *selected == *value;
                    let text = if active {
                        theme::bold(12.5, *label).color(p.text)
                    } else {
                        theme::regular(12.5, *label).color(p.muted)
                    };
                    let button = egui::Button::new(text)
                        .fill(if active { p.card } else { Color32::TRANSPARENT })
                        .corner_radius(9)
                        .min_size(Vec2::new(74.0, 26.0));
                    if ui.add(button).clicked() && !active {
                        *selected = *value;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Pill switch for on/off settings.
pub fn toggle(ui: &mut Ui, theme: &Theme, value: &mut bool) -> Response {
    let p = theme.palette;
    let size = Vec2::new(44.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let t = if theme.reduced_motion {
        if *value { 1.0 } else { 0.0 }
    } else {
        ui.ctx().animate_bool_with_time(response.id, *value, 0.12)
    };
    let painter = ui.painter();
    let track = if *value { p.accent } else { p.track };
    painter.rect_filled(rect, CornerRadius::same(12), track);
    let knob_x = rect.left() + 12.0 + t * (rect.width() - 24.0);
    painter.circle_filled(
        egui::pos2(knob_x, rect.center().y),
        8.5,
        if *value { Color32::WHITE } else { p.muted },
    );
    response
}

/// A setting row: title + explanation on the left, control on the right.
pub fn setting_row<R>(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    detail: &str,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let p = theme.palette;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(theme::regular(13.5, title).color(p.text));
            if !detail.is_empty() {
                ui.add_space(1.0);
                ui.label(theme::regular(11.5, detail).color(p.faint));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add).inner
    })
    .inner
}

pub fn divider(ui: &mut Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, theme.palette.border);
}

pub fn empty_state(ui: &mut Ui, theme: &Theme, title: &str, body: &str) {
    let p = theme.palette;
    ui.vertical_centered(|ui| {
        ui.add_space(SPACE * 2.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(0.0, 44.0), Sense::hover());
        let painter = ui.painter();
        let center = rect.center();
        painter.circle_stroke(center, 15.0, Stroke::new(1.5, p.border));
        painter.circle_filled(center + Vec2::new(4.5, -3.0), 3.0, p.accent);
        ui.add_space(6.0);
        ui.label(theme::bold(15.0, title).color(p.text));
        ui.add_space(4.0);
        ui.label(theme::regular(12.5, body).color(p.muted));
        ui.add_space(SPACE * 2.0);
    });
}

/// A rounded heat block for one slice of the day.
pub fn heat_block(
    painter: &egui::Painter,
    rect: Rect,
    theme: &Theme,
    level: FocusLevel,
    patterns: bool,
) {
    let p = theme.palette;
    let color = match level {
        FocusLevel::Idle => p.idle,
        other => theme.level_color(other),
    };
    painter.rect_filled(rect, CornerRadius::same(3), color);
    if patterns && matches!(level, FocusLevel::Medium | FocusLevel::Fragmented) {
        let ink = p.bg.gamma_multiply(0.55);
        let stroke = Stroke::new(1.0, ink);
        if matches!(level, FocusLevel::Fragmented) {
            painter.line_segment(
                [
                    rect.left_bottom() + Vec2::new(1.0, -2.0),
                    rect.right_top() + Vec2::new(-1.0, 2.0),
                ],
                stroke,
            );
        } else {
            painter.circle_filled(rect.center(), 1.3, ink);
        }
    }
}

/// Bars for a 60 minute detail view.
pub fn minute_bars(
    ui: &mut Ui,
    theme: &Theme,
    values: &[Option<u8>; 60],
    height: f32,
) {
    let p = theme.palette;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter();
    let gap = 1.5;
    let bar_w = ((rect.width() - 59.0 * gap) / 60.0).max(1.0);
    for (i, value) in values.iter().enumerate() {
        let x = rect.left() + i as f32 * (bar_w + gap);
        let bar = Rect::from_min_size(
            egui::pos2(x, rect.top()),
            Vec2::new(bar_w, rect.height()),
        );
        painter.rect_filled(bar, CornerRadius::same(1), p.track);
        if let Some(score) = value {
            let frac = (*score as f32 / 100.0).clamp(0.08, 1.0);
            let filled = Rect::from_min_size(
                egui::pos2(x, rect.top() + rect.height() * (1.0 - frac)),
                Vec2::new(bar_w, rect.height() * frac),
            );
            painter.rect_filled(filled, CornerRadius::same(1), theme.score_color(*score));
        }
    }
    if let Some(pos) = response.hover_pos() {
        let idx = (((pos.x - rect.left()) / (bar_w + gap)).floor() as i32).clamp(0, 59) as usize;
        if let Some(score) = values[idx] {
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                response.id.with(idx),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| {
                ui.label(theme::bold(12.0, format!(":{}  score {}", idx, score)));
            });
        }
    }
}

/// Line chart for daily scores. `points` is `(day label, score)`.
pub fn trend_line(ui: &mut Ui, theme: &Theme, points: &[(String, u8)], height: f32) {
    let p = theme.palette;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), p.card_alt);
    if points.len() < 2 {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Not enough days yet",
            FontId::proportional(12.0),
            p.faint,
        );
        return;
    }
    let inner = rect.shrink2(Vec2::new(16.0, 14.0));
    for frac in [0.0, 0.5, 1.0] {
        let y = inner.top() + inner.height() * frac;
        painter.line_segment(
            [egui::pos2(inner.left(), y), egui::pos2(inner.right(), y)],
            Stroke::new(1.0, p.border.gamma_multiply(0.6)),
        );
    }
    let step = inner.width() / (points.len() - 1) as f32;
    let mut line = Vec::with_capacity(points.len());
    for (i, (_, score)) in points.iter().enumerate() {
        let x = inner.left() + step * i as f32;
        let y = inner.bottom() - inner.height() * (*score as f32 / 100.0);
        line.push(egui::pos2(x, y));
    }
    painter.add(egui::Shape::line(line.clone(), Stroke::new(2.0, p.accent)));
    if let Some(last) = line.last() {
        painter.circle_filled(*last, 4.0, p.accent);
        painter.circle_filled(*last, 2.0, p.card);
    }
    let first = points.first().map(|(d, _)| d.clone()).unwrap_or_default();
    let last = points.last().map(|(d, _)| d.clone()).unwrap_or_default();
    painter.text(
        egui::pos2(inner.left(), rect.bottom() - 4.0),
        Align2::LEFT_BOTTOM,
        first,
        FontId::proportional(10.0),
        p.faint,
    );
    painter.text(
        egui::pos2(inner.right(), rect.bottom() - 4.0),
        Align2::RIGHT_BOTTOM,
        last,
        FontId::proportional(10.0),
        p.faint,
    );
}

pub fn legend(ui: &mut Ui, theme: &Theme) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for (label, color) in [
            ("Deep", p.deep),
            ("Medium", p.medium),
            ("Fragmented", p.fragmented),
            ("Idle", p.idle),
        ] {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                dot(ui, color, 3.5);
                ui.label(theme::regular(10.5, label).color(p.muted));
            });
        }
    });
}

/// Full-day map: 24 hour rows, 60 minute cells. Returns the clicked hour.
pub fn day_grid(
    ui: &mut Ui,
    theme: &Theme,
    samples: &[focusmap_core::model::MinuteSample],
    patterns: bool,
) -> Option<u16> {
    let p = theme.palette;
    let label_w = 30.0;
    let width = ui.available_width().min(1000.0);
    let gap = 1.5;
    let cell_h = 8.0;
    let cell_w = ((width - label_w - 59.0 * gap) / 60.0).max(2.0);
    let row_h = cell_h + gap;
    let height = 24.0 * row_h;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    let mut scores = [0u8; 1440];
    let mut levels = [FocusLevel::Idle; 1440];
    let mut present = [false; 1440];
    for s in samples {
        let idx = s.minute_of_day as usize;
        if idx < 1440 {
            scores[idx] = s.score;
            levels[idx] = s.level;
            present[idx] = true;
        }
    }

    let painter = ui.painter();
    for hour in 0..24u16 {
        let y = rect.top() + hour as f32 * row_h;
        painter.text(
            egui::pos2(rect.left(), y + cell_h / 2.0),
            Align2::LEFT_CENTER,
            format!("{hour:02}"),
            FontId::proportional(10.0),
            p.faint,
        );
        for minute in 0..60u16 {
            let x = rect.left() + label_w + minute as f32 * (cell_w + gap);
            let cell = Rect::from_min_size(egui::pos2(x, y), Vec2::new(cell_w, cell_h));
            let idx = (hour * 60 + minute) as usize;
            if present[idx] {
                heat_block(painter, cell, theme, levels[idx], patterns);
            } else {
                painter.rect_filled(cell, CornerRadius::same(2), p.idle.gamma_multiply(0.45));
            }
        }
    }

    let mut clicked = None;
    if let Some(pos) = response.hover_pos() {
        if pos.x > rect.left() + label_w {
            let col = ((pos.x - rect.left() - label_w) / (cell_w + gap)).floor() as i32;
            let row = ((pos.y - rect.top()) / row_h).floor() as i32;
            if (0..60).contains(&col) && (0..24).contains(&row) {
                let hour = row as u16;
                let minute = col as u16;
                let idx = (hour * 60 + minute) as usize;
                let text = if present[idx] {
                    format!(
                        "{:02}:{:02}  score {}  {}",
                        hour,
                        minute,
                        scores[idx],
                        levels[idx].as_str()
                    )
                } else {
                    format!("{hour:02}:{minute:02}  no activity")
                };
                egui::Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    response.id.with(idx),
                    egui::PopupAnchor::Pointer,
                )
                .show(|ui| {
                    ui.label(theme::bold(12.0, text));
                });
                if response.clicked() {
                    clicked = Some(hour);
                }
            }
        }
    }
    clicked
}

/// Score per day as bars, used for the week and the month.
pub fn day_bars(
    ui: &mut Ui,
    theme: &Theme,
    days: &[(String, u8)],
    selected: Option<usize>,
    height: f32,
) -> Option<usize> {
    let p = theme.palette;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    if days.is_empty() {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No days yet",
            FontId::proportional(12.0),
            p.faint,
        );
        return None;
    }
    let inner = rect.shrink2(Vec2::new(4.0, 10.0));
    let painter = ui.painter();
    for frac in [0.0, 0.5, 1.0] {
        let y = inner.top() + inner.height() * frac;
        painter.line_segment(
            [egui::pos2(inner.left(), y), egui::pos2(inner.right(), y)],
            Stroke::new(1.0, p.border.gamma_multiply(0.55)),
        );
    }
    let gap = if days.len() > 14 { 3.0 } else { 8.0 };
    let bar_w = ((inner.width() - gap * (days.len() - 1) as f32) / days.len() as f32).max(2.0);
    let mut clicked = None;
    for (i, (_, score)) in days.iter().enumerate() {
        let x = inner.left() + i as f32 * (bar_w + gap);
        let track = Rect::from_min_max(
            egui::pos2(x, inner.top()),
            egui::pos2(x + bar_w, inner.bottom()),
        );
        painter.rect_filled(track, CornerRadius::same(3), p.track);
        if *score > 0 {
            let h = inner.height() * (*score as f32 / 100.0).max(0.06);
            let bar = Rect::from_min_max(
                egui::pos2(x, inner.bottom() - h),
                egui::pos2(x + bar_w, inner.bottom()),
            );
            painter.rect_filled(bar, CornerRadius::same(3), theme.score_color(*score));
        }
        if selected == Some(i) {
            painter.rect_stroke(
                track.expand(1.5),
                CornerRadius::same(4),
                Stroke::new(1.5, p.accent),
                egui::StrokeKind::Outside,
            );
        }
    }
    if let Some(pos) = response.hover_pos() {
        let idx = ((pos.x - inner.left()) / (bar_w + gap)).floor() as i32;
        if (0..days.len() as i32).contains(&idx) {
            let i = idx as usize;
            let (day, score) = &days[i];
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                response.id.with(i),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| {
                ui.label(
                    theme::bold(12.0, format!("{}  score {}", pretty_day(day), score)),
                );
            });
            if response.clicked() {
                clicked = Some(i);
            }
        }
    }
    clicked
}

/// Simple horizontal progress track, for goals and budgets.
pub fn progress(
    ui: &mut Ui,
    theme: &Theme,
    fraction: f32,
    tint: Color32,
    height: f32,
) -> Response {
    let p = theme.palette;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same((height / 2.0) as u8), p.track);
    let frac = fraction.clamp(0.0, 1.0);
    if frac > 0.001 {
        let filled = Rect::from_min_max(
            rect.min,
            egui::pos2(rect.left() + rect.width() * frac, rect.bottom()),
        );
        let radius = CornerRadius::same((height / 2.0) as u8);
        painter.rect_filled(filled.expand(2.0), radius, theme::alpha(tint, 30));
        painter.rect_filled(filled, radius, tint);
        if height >= 6.0 && filled.width() > 10.0 {
            let gloss = Rect::from_min_max(
                filled.min + Vec2::new(2.0, 1.0),
                egui::pos2(filled.max.x - 2.0, filled.center().y),
            );
            painter.rect_filled(gloss, CornerRadius::same(2), Color32::from_white_alpha(26));
        }
    }
    response
}

pub fn tag_button(ui: &mut Ui, theme: &Theme, text: &str, tint: Color32) -> Response {
    let p = theme.palette;
    ui.add(
        egui::Button::new(theme::bold(10.5, text).color(tint))
            .fill(tint.gamma_multiply(if p.dark { 0.16 } else { 0.10 }))
            .stroke(Stroke::new(1.0, tint.gamma_multiply(0.45)))
            .corner_radius(100)
            .min_size(Vec2::new(0.0, 23.0)),
    )
}

pub fn primary_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    ui.add(
        egui::Button::new(theme::bold(13.0, text).color(Color32::WHITE))
            .fill(theme.palette.accent)
            .stroke(Stroke::new(1.0, theme::alpha(Color32::WHITE, 46)))
            .corner_radius(100)
            .min_size(Vec2::new(112.0, 36.0)),
    )
}

pub fn ghost_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    ui.add(
        egui::Button::new(theme::regular(13.0, text).color(theme.palette.text))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme.palette.border))
            .corner_radius(100)
            .min_size(Vec2::new(92.0, 36.0)),
    )
}

fn pretty_day(day: &str) -> String {
    focusmap_core::model::pretty_day(day)
}

pub fn center_page(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let width = ui.available_width().min(theme::PAGE_MAX_WIDTH);
    let margin = ((ui.available_width() - width) / 2.0).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.vertical(|ui| {
            ui.set_width(width);
            add(ui);
        });
    });
}
