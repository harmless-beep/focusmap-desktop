//! Design tokens and the egui style derived from them.
//!
//! One place for colour, spacing and type. Screens never hard-code a colour;
//! they ask the palette. That is what keeps dark, light and high-contrast modes
//! consistent and makes a redesign a one-file change.

use egui::{
    Color32, FontFamily, FontId, Margin, RichText, Stroke, TextStyle, Visuals,
};

use focusmap_core::model::FocusLevel;
use focusmap_core::settings::ThemeMode;

/// The 8 pixel grid the whole UI snaps to.
pub const SPACE: f32 = 8.0;
pub const CARD_RADIUS: u8 = 22;
pub const CARD_PADDING: i8 = 22;
pub const PAGE_MAX_WIDTH: f32 = 1180.0;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub card_alt: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub deep: Color32,
    pub medium: Color32,
    pub fragmented: Color32,
    pub idle: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub danger: Color32,
    pub track: Color32,
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub palette: Palette,
    pub reduced_motion: bool,
}

impl Theme {
    pub fn resolve(mode: ThemeMode, system_dark: bool, high_contrast: bool, reduced_motion: bool) -> Self {
        let dark = match mode {
            ThemeMode::Auto => system_dark,
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
        };
        let mut palette = if dark { Palette::dark() } else { Palette::light() };
        if high_contrast {
            palette = palette.high_contrast();
        }
        Self {
            palette,
            reduced_motion,
        }
    }

    pub fn level_color(&self, level: FocusLevel) -> Color32 {
        match level {
            FocusLevel::Deep => self.palette.deep,
            FocusLevel::Medium => self.palette.medium,
            FocusLevel::Fragmented => self.palette.fragmented,
            FocusLevel::Idle => self.palette.idle,
        }
    }

    pub fn score_color(&self, score: u8) -> Color32 {
        if score >= 70 {
            self.palette.deep
        } else if score >= 45 {
            self.palette.medium
        } else if score >= 15 {
            self.palette.fragmented
        } else {
            self.palette.faint
        }
    }

    pub fn card_frame(&self) -> egui::Frame {
        let p = self.palette;
        egui::Frame::NONE
            .fill(alpha(p.card, if p.dark { 236 } else { 246 }))
            .stroke(Stroke::new(1.0, alpha(p.border, if p.dark { 200 } else { 160 })))
            .corner_radius(CARD_RADIUS)
            .inner_margin(Margin::same(CARD_PADDING))
            .shadow(egui::epaint::Shadow {
                offset: [0, 8],
                blur: 18,
                spread: 0,
                color: Color32::from_black_alpha(if p.dark { 70 } else { 22 }),
            })
    }

    /// Apply the palette to egui. Fonts are installed separately at startup.
    pub fn apply(&self, ctx: &egui::Context) {
        let p = self.palette;
        ctx.set_theme(if p.dark {
            egui::ThemePreference::Dark
        } else {
            egui::ThemePreference::Light
        });
        let mut visuals = if p.dark { Visuals::dark() } else { Visuals::light() };
        visuals.panel_fill = p.bg;
        visuals.window_fill = p.card;
        visuals.extreme_bg_color = p.card_alt;
        visuals.faint_bg_color = p.card_alt;
        visuals.override_text_color = Some(p.text);
        visuals.hyperlink_color = p.accent;
        visuals.selection.bg_fill = p.accent.gamma_multiply(0.35);
        visuals.selection.stroke = Stroke::new(1.0, p.text);
        visuals.widgets.noninteractive.bg_fill = p.card;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        visuals.widgets.inactive.bg_fill = p.card_alt;
        visuals.widgets.inactive.weak_bg_fill = p.card_alt;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.muted);
        visuals.widgets.hovered.bg_fill = p.card_alt;
        visuals.widgets.hovered.weak_bg_fill = p.card_alt;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
        visuals.widgets.active.bg_fill = p.accent.gamma_multiply(0.25);
        visuals.widgets.active.weak_bg_fill = p.accent.gamma_multiply(0.25);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
        visuals.window_stroke = Stroke::new(1.0, p.border);
        ctx.global_style_mut(|style| {
            style.visuals = visuals;
            style.spacing.item_spacing = egui::vec2(SPACE, SPACE);
            style.spacing.button_padding = egui::vec2(14.0, 8.0);
            style.spacing.interact_size.y = 30.0;
            style.spacing.indent = 16.0;
            style.spacing.slider_width = 220.0;
            style.spacing.scroll.bar_width = 8.0;
            style.text_styles = [
                (
                    TextStyle::Heading,
                    FontId::new(24.0, bold_family()),
                ),
                (TextStyle::Body, FontId::proportional(14.0)),
                (TextStyle::Button, FontId::proportional(14.0)),
                (TextStyle::Small, FontId::proportional(12.0)),
                (TextStyle::Monospace, FontId::monospace(13.0)),
            ]
            .into();
        });
    }
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: Color32::from_rgb(0x06, 0x08, 0x0F),
            panel: Color32::from_rgb(0x0A, 0x0D, 0x18),
            card: Color32::from_rgb(0x12, 0x17, 0x26),
            card_alt: Color32::from_rgb(0x1A, 0x20, 0x33),
            border: Color32::from_rgb(0x2A, 0x33, 0x4C),
            text: Color32::from_rgb(0xF4, 0xF6, 0xFC),
            muted: Color32::from_rgb(0x9A, 0xA4, 0xBA),
            faint: Color32::from_rgb(0x67, 0x72, 0x8C),
            accent: Color32::from_rgb(0x8B, 0x5C, 0xF6),
            deep: Color32::from_rgb(0x8B, 0x5C, 0xF6),
            medium: Color32::from_rgb(0x22, 0xD3, 0xEE),
            fragmented: Color32::from_rgb(0xF5, 0x9E, 0x0B),
            idle: Color32::from_rgb(0x16, 0x1B, 0x29),
            good: Color32::from_rgb(0x34, 0xD3, 0x99),
            warn: Color32::from_rgb(0xFB, 0xBF, 0x24),
            danger: Color32::from_rgb(0xFB, 0x71, 0x85),
            track: Color32::from_rgb(0x1C, 0x22, 0x33),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            bg: Color32::from_rgb(0xF4, 0xF6, 0xFC),
            panel: Color32::from_rgb(0xFB, 0xFC, 0xFE),
            card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            card_alt: Color32::from_rgb(0xEE, 0xF1, 0xF9),
            border: Color32::from_rgb(0xDC, 0xE2, 0xF0),
            text: Color32::from_rgb(0x0B, 0x10, 0x20),
            muted: Color32::from_rgb(0x55, 0x60, 0x7A),
            faint: Color32::from_rgb(0x8A, 0x94, 0xAC),
            accent: Color32::from_rgb(0x6D, 0x4A, 0xFF),
            deep: Color32::from_rgb(0x6D, 0x4A, 0xFF),
            medium: Color32::from_rgb(0x08, 0x91, 0xB2),
            fragmented: Color32::from_rgb(0xD9, 0x77, 0x06),
            idle: Color32::from_rgb(0xE6, 0xEB, 0xF7),
            good: Color32::from_rgb(0x05, 0x96, 0x69),
            warn: Color32::from_rgb(0xB4, 0x53, 0x09),
            danger: Color32::from_rgb(0xDC, 0x26, 0x26),
            track: Color32::from_rgb(0xE4, 0xE9, 0xF5),
        }
    }

    /// Stronger borders and text, for users who need more contrast.
    pub fn high_contrast(mut self) -> Self {
        if self.dark {
            self.bg = Color32::BLACK;
            self.panel = Color32::BLACK;
            self.card = Color32::from_rgb(0x0A, 0x0A, 0x0A);
            self.card_alt = Color32::from_rgb(0x16, 0x16, 0x16);
            self.border = Color32::from_rgb(0x6B, 0x74, 0x86);
            self.text = Color32::WHITE;
            self.muted = Color32::from_rgb(0xC2, 0xC8, 0xD4);
            self.faint = Color32::from_rgb(0x9A, 0xA2, 0xB2);
            self.deep = Color32::from_rgb(0x9B, 0x8C, 0xFF);
            self.medium = Color32::from_rgb(0x53, 0xE6, 0xD6);
            self.fragmented = Color32::from_rgb(0xFF, 0xB3, 0x66);
        } else {
            self.bg = Color32::WHITE;
            self.card = Color32::WHITE;
            self.card_alt = Color32::from_rgb(0xEE, 0xEE, 0xEE);
            self.border = Color32::from_rgb(0x5C, 0x64, 0x74);
            self.text = Color32::BLACK;
            self.muted = Color32::from_rgb(0x3A, 0x41, 0x4F);
            self.faint = Color32::from_rgb(0x55, 0x5D, 0x6B);
        }
        self
    }
}

pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn bold(size: f32, text: impl Into<String>) -> RichText {
    RichText::new(text).size(size).family(bold_family())
}

pub fn regular(size: f32, text: impl Into<String>) -> RichText {
    RichText::new(text).size(size)
}

/// Same colour, new opacity. Keeps RGB untouched so blends stay predictable.
pub fn alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    // Premultiplied interpolates correctly channel by channel; dropping alpha
    // would turn translucent bands (the ring halo) into opaque ones.
    Color32::from_rgba_premultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}

/// Vertical two-stop gradient, one quad.
pub fn gradient_rect(painter: &egui::Painter, rect: egui::Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(egui::Shape::mesh(mesh));
}

/// Soft radial light: a triangle fan fading to zero alpha at the rim.
pub fn glow(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: Color32, strength: u8) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, alpha(color, strength));
    let edge = alpha(color, 0);
    let steps = 36;
    for i in 0..=steps {
        let angle = std::f32::consts::TAU * i as f32 / steps as f32;
        let pos = center + egui::Vec2::angled(angle) * radius;
        mesh.colored_vertex(pos, edge);
    }
    for i in 0..steps {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Window backdrop: cool gradient with three slow aurora blooms.
/// Painted once per frame into the root layer; panels sit above it.
pub fn paint_background(ui: &egui::Ui, theme: &Theme) {
    let p = theme.palette;
    let rect = ui.max_rect();
    let painter = ui.painter();
    let top = if p.dark {
        mix(p.bg, p.accent, 0.10)
    } else {
        mix(p.bg, p.accent, 0.05)
    };
    gradient_rect(painter, rect, top, p.bg);

    let strength = if p.dark { 46 } else { 22 };
    glow(
        painter,
        rect.left_top() + egui::Vec2::new(rect.width() * 0.16, rect.height() * 0.02),
        rect.width() * 0.55,
        p.accent,
        strength,
    );
    glow(
        painter,
        rect.right_top() + egui::Vec2::new(-rect.width() * 0.08, rect.height() * 0.30),
        rect.width() * 0.42,
        p.medium,
        (strength as f32 * 0.70) as u8,
    );
    glow(
        painter,
        rect.left_bottom() + egui::Vec2::new(rect.width() * 0.42, -rect.height() * 0.05),
        rect.width() * 0.48,
        p.deep,
        (strength as f32 * 0.55) as u8,
    );
}

/// Install the system sans font (and its bold face) into egui.
/// Returns the family name that was used, for the About screen.
pub fn install_fonts(ctx: &egui::Context) -> Option<String> {
    let set = focusmap_core::fonts::load()?;
    let name = set.name.clone();
    let mut defs = egui::FontDefinitions::default();
    defs.font_data.insert(
        "focusmap-sans".to_owned(),
        std::sync::Arc::new(egui::FontData::from_owned(set.regular)),
    );
    defs.families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "focusmap-sans".to_owned());
    defs.families
        .entry(FontFamily::Monospace)
        .or_default()
        .push("focusmap-sans".to_owned());
    if let Some(bold) = set.bold {
        defs.font_data.insert(
            "focusmap-sans-bold".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bold)),
        );
        defs.families.insert(
            bold_family(),
            vec!["focusmap-sans-bold".to_owned(), "focusmap-sans".to_owned()],
        );
    } else {
        defs.families
            .insert(bold_family(), vec!["focusmap-sans".to_owned()]);
    }
    ctx.set_fonts(defs);
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_keeps_alpha() {
        let halo = mix(alpha(Color32::from_rgb(200, 100, 50), 26), Color32::TRANSPARENT, 0.0);
        assert_eq!(halo.a(), 26);
        let solid = mix(Color32::from_rgb(200, 100, 50), Color32::WHITE, 0.5);
        assert_eq!(solid.a(), 255);
    }
}
