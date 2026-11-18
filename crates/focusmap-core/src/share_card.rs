//! Share card: a 1080x1080 PNG rendered locally, ready to post.
//!
//! No network, no template service, no upload. tiny-skia draws the shapes and
//! the system font rasterises the type, so the card matches the app exactly and
//! costs one short-lived 4.6 MB pixmap.

use std::path::Path;

use ab_glyph::{point, Font, FontRef, GlyphId, ScaleFont};
use anyhow::{anyhow, Result};
use tiny_skia::{
    Color, FillRule, GradientStop, IntSize, LineCap, LineJoin, LinearGradient, Paint,
    PathBuilder, Pixmap, Point, Rect, SpreadMode, Stroke, Transform,
};

use crate::fonts;
use crate::model::format_minutes;

pub const CARD_SIZE: u32 = 1080;

const BG: (u8, u8, u8) = (7, 9, 18);
const DUSK: (u8, u8, u8) = (20, 15, 42);
const CARD: (u8, u8, u8) = (16, 21, 36);
const CARD_ALT: (u8, u8, u8) = (25, 31, 49);
const TRACK: (u8, u8, u8) = (36, 44, 66);
const BORDER: (u8, u8, u8) = (44, 54, 82);
const TEXT: (u8, u8, u8) = (244, 246, 252);
const MUTED: (u8, u8, u8) = (154, 164, 186);
const ACCENT: (u8, u8, u8) = (139, 92, 246);
const DEEP: (u8, u8, u8) = (139, 92, 246);
const MEDIUM: (u8, u8, u8) = (34, 211, 238);
const FRAGMENTED: (u8, u8, u8) = (245, 158, 11);

#[derive(Clone, Debug)]
pub struct ShareData {
    pub day: String,
    pub score: u8,
    pub deep_minutes: u32,
    pub longest_session_minutes: u32,
    pub streak: u32,
    pub goal_minutes: u32,
    /// Average score per hour, `None` when no minute was recorded.
    pub hour_scores: [Option<u8>; 24],
}

pub fn render_png(data: &ShareData) -> Result<Vec<u8>> {
    let pm = draw(data)?;
    pm.encode_png().map_err(|e| anyhow!("png encode failed: {e}"))
}

/// Straight RGBA pixels for an in-app preview. Opaque canvas, so premultiplied
/// and straight alpha are identical here.
pub fn render_rgba(data: &ShareData) -> Result<(Vec<u8>, u32, u32)> {
    let pm = draw(data)?;
    Ok((pm.data().to_vec(), pm.width(), pm.height()))
}

fn draw(data: &ShareData) -> Result<Pixmap> {
    let loaded = fonts::load().ok_or_else(|| anyhow!("no system font available for the card"))?;
    let regular = FontRef::try_from_slice(&loaded.regular)
        .map_err(|_| anyhow!("could not read {}", loaded.name))?;
    let bold_bytes = loaded.bold.as_deref().unwrap_or(&loaded.regular);
    let bold = FontRef::try_from_slice(bold_bytes).unwrap_or(regular.clone());

    let mut pm = Pixmap::new(CARD_SIZE, CARD_SIZE)
        .ok_or_else(|| anyhow!("could not allocate the share card"))?;

    // Night gradient with brand glows outside the main card.
    let bg = gradient_paint(0.0, 0.0, 1080.0, 1080.0, BG, DUSK);
    if let Some(rect) = Rect::from_xywh(0.0, 0.0, 1080.0, 1080.0) {
        pm.fill_rect(rect, &bg, Transform::identity(), None);
    }
    circle(&mut pm, 1010.0, 60.0, 380.0, rgba(ACCENT, 0.14));
    circle(&mut pm, 60.0, 1000.0, 300.0, rgba(MEDIUM, 0.08));
    circle(&mut pm, 940.0, 1000.0, 240.0, rgba(FRAGMENTED, 0.05));

    round_rect(&mut pm, 56.0, 56.0, 968.0, 968.0, 44.0, color(CARD));
    stroke_round_rect(
        &mut pm,
        56.0,
        56.0,
        968.0,
        968.0,
        44.0,
        color(BORDER),
        2.0,
    );

    // A faint plaza-dot grid gives the card the Appolis texture.
    let mut dots = PathBuilder::new();
    let mut gy = 92.0;
    while gy < 1000.0 {
        let mut gx = 92.0;
        while gx < 1000.0 {
            dots.push_circle(gx, gy, 1.2);
            gx += 44.0;
        }
        gy += 44.0;
    }
    if let Some(path) = dots.finish() {
        let mut paint = Paint::default();
        paint.set_color(rgba((120, 145, 190), 0.07));
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // Brand row.
    circle(&mut pm, 116.0, 132.0, 11.0, color(ACCENT));
    draw_text(&mut pm, &bold, 30.0, 140.0, 144.0, color(TEXT), "FocusMap");
    gradient_bar(&mut pm, 104.0, 154.0, 180.0, 3.0, ACCENT, MEDIUM);
    let date = crate::model::pretty_day(&data.day);
    let date_w = text_width(&regular, 26.0, &date);
    draw_text(
        &mut pm,
        &regular,
        26.0,
        976.0 - date_w,
        142.0,
        color(MUTED),
        &date,
    );

    // Score.
    let score_text = data.score.to_string();
    draw_text(&mut pm, &bold, 210.0, 104.0, 470.0, color(TEXT), &score_text);
    draw_text(
        &mut pm,
        &regular,
        30.0,
        110.0,
        522.0,
        color(MUTED),
        "focus score / 100",
    );

    // Mood chip, so the number has a plain-language read.
    let (mood_label, mood_tint) = mood(data.score);
    let chip = mood_label.to_uppercase();
    let chip_w = text_width(&regular, 17.0, &chip) + 36.0;
    round_rect(&mut pm, 104.0, 546.0, chip_w, 36.0, 18.0, rgba(mood_tint, 0.16));
    stroke_round_rect(
        &mut pm,
        104.0,
        546.0,
        chip_w,
        36.0,
        18.0,
        rgba(mood_tint, 0.55),
        1.5,
    );
    draw_text(
        &mut pm,
        &regular,
        17.0,
        122.0,
        570.0,
        color(mood_tint),
        &chip,
    );

    // Goal ring.
    let goal_frac = if data.goal_minutes > 0 {
        (data.deep_minutes as f32 / data.goal_minutes as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy, radius) = (800.0, 340.0, 120.0);
    circle_stroke(&mut pm, cx, cy, radius, 24.0, color(TRACK));

    // Instrument ticks around the ring.
    let mut ticks = PathBuilder::new();
    for i in 0..48 {
        let a = -std::f32::consts::FRAC_PI_2
            + i as f32 / 48.0 * std::f32::consts::TAU;
        let r0 = radius + 20.0;
        let r1 = r0 + if i % 4 == 0 { 9.0 } else { 4.0 };
        ticks.move_to(cx + a.cos() * r0, cy + a.sin() * r0);
        ticks.line_to(cx + a.cos() * r1, cy + a.sin() * r1);
    }
    if let Some(path) = ticks.finish() {
        let mut paint = Paint::default();
        paint.set_color(rgba(MUTED, 0.40));
        pm.stroke_path(
            &path,
            &paint,
            &Stroke {
                width: 2.0,
                line_cap: LineCap::Round,
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
    }
    if goal_frac > 0.0 {
        arc(
            &mut pm,
            cx,
            cy,
            radius,
            24.0,
            goal_frac,
            gradient_paint(cx - radius, cy - radius, cx + radius, cy + radius, DEEP, MEDIUM),
        );
    }
    let deep = format_minutes(data.deep_minutes);
    let deep_w = text_width(&bold, 44.0, &deep);
    draw_text(&mut pm, &bold, 44.0, cx - deep_w / 2.0, cy + 8.0, color(TEXT), &deep);
    let label = format!("of {} goal", format_minutes(data.goal_minutes));
    let label_w = text_width(&regular, 22.0, &label);
    draw_text(
        &mut pm,
        &regular,
        22.0,
        cx - label_w / 2.0,
        cy + 46.0,
        color(MUTED),
        &label,
    );

    // Timeline: one bar per hour.
    draw_text(
        &mut pm,
        &regular,
        24.0,
        104.0,
        616.0,
        color(MUTED),
        "DEEP WORK, HOUR BY HOUR",
    );
    let track_x = 104.0;
    let track_w = 872.0;
    let gap = 6.0;
    let bar_w = (track_w - gap * 23.0) / 24.0;
    let peak = data
        .hour_scores
        .iter()
        .enumerate()
        .filter_map(|(hour, score)| score.map(|score| (score, hour)))
        .max();
    if let Some((score, hour)) = peak {
        let label = format!("PEAK {hour:02}:00");
        let w = text_width(&bold, 20.0, &label);
        draw_text(
            &mut pm,
            &bold,
            20.0,
            976.0 - w,
            616.0,
            color(score_color(score)),
            &label,
        );
    }
    for hour in 0..24 {
        let x = track_x + hour as f32 * (bar_w + gap);
        if let Some((score, best)) = peak {
            if hour == best {
                round_rect(
                    &mut pm,
                    x - 3.0,
                    632.0,
                    bar_w + 6.0,
                    100.0,
                    10.0,
                    rgba(score_color(score), 0.20),
                );
            }
        }
        let value = data.hour_scores[hour];
        let color = match value {
            None => rgba(TRACK, 0.55),
            Some(s) => color(score_color(s)),
        };
        round_rect(&mut pm, x, 636.0, bar_w, 92.0, 7.0, color);
    }
    for hour in [0u32, 6, 12, 18, 23] {
        let x = track_x + hour as f32 * (bar_w + gap);
        let label = format!("{hour:02}");
        draw_text(&mut pm, &regular, 20.0, x, 758.0, color(MUTED), &label);
    }

    // Stats.
    let stats = [
        ("DEEP WORK", format_minutes(data.deep_minutes)),
        ("LONGEST SESSION", format_minutes(data.longest_session_minutes)),
        ("STREAK", format!("{} days", data.streak)),
    ];
    let card_w = (track_w - 44.0) / 3.0;
    let stat_tints = [DEEP, MEDIUM, FRAGMENTED];
    for (i, (label, value)) in stats.iter().enumerate() {
        let x = track_x + i as f32 * (card_w + 22.0);
        round_rect(&mut pm, x, 800.0, card_w, 128.0, 18.0, color(CARD_ALT));
        circle(&mut pm, x + 28.0, 834.0, 4.0, color(stat_tints[i]));
        draw_text(
            &mut pm,
            &regular,
            20.0,
            x + 40.0,
            840.0,
            color(MUTED),
            label,
        );
        draw_text(&mut pm, &bold, 46.0, x + 24.0, 896.0, color(TEXT), value);
    }

    // Footer.
    hline(&mut pm, 104.0, 946.0, 872.0, rgba(BORDER, 0.9));
    draw_text(
        &mut pm,
        &regular,
        22.0,
        104.0,
        976.0,
        color(MUTED),
        "Tracked locally. Nothing left this device.",
    );
    let mark = "focusmap";
    let mark_w = text_width(&bold, 22.0, mark);
    draw_text(
        &mut pm,
        &bold,
        22.0,
        976.0 - mark_w,
        976.0,
        rgba(ACCENT, 0.9),
        mark,
    );

    Ok(pm)
}

/// Encode already-rendered straight RGBA pixels, used by the Appolis GPU
/// export. The pixmap is short-lived, like the card's own.
pub fn encode_rgba_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let size = IntSize::from_wh(width, height)
        .ok_or_else(|| anyhow!("invalid export image size"))?;
    let pm = Pixmap::from_vec(rgba.to_vec(), size)
        .ok_or_else(|| anyhow!("could not allocate the export image"))?;
    pm.encode_png().map_err(|e| anyhow!("png encode failed: {e}"))
}

pub fn write_png(data: &ShareData, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, render_png(data)?)?;
    Ok(())
}

fn color(rgb: (u8, u8, u8)) -> Color {
    Color::from_rgba8(rgb.0, rgb.1, rgb.2, 255)
}

fn rgba(rgb: (u8, u8, u8), alpha: f32) -> Color {
    Color::from_rgba8(rgb.0, rgb.1, rgb.2, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn round_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, fill: Color) {
    let mut pb = PathBuilder::new();
    let r = r.min(w / 2.0).min(h / 2.0);
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(fill);
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn stroke_round_rect(
    pm: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    stroke: Color,
    width: f32,
) {
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(stroke);
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, fill: Color) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(fill);
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn circle_stroke(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, width: f32, stroke: Color) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(stroke);
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn arc(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, width: f32, frac: f32, paint: Paint) {
    let steps = 90;
    let sweep = frac.clamp(0.0, 1.0) * std::f32::consts::TAU;
    let start = -std::f32::consts::FRAC_PI_2;
    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let angle = start + sweep * i as f32 / steps as f32;
        let x = cx + r * angle.cos();
        let y = cy + r * angle.sin();
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    if let Some(path) = pb.finish() {
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn gradient_paint(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    from: (u8, u8, u8),
    to: (u8, u8, u8),
) -> Paint<'static> {
    let mut paint = Paint::default();
    if let Some(shader) = LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        vec![
            GradientStop::new(0.0, color(from)),
            GradientStop::new(1.0, color(to)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        paint.shader = shader;
    }
    paint.anti_alias = true;
    paint
}

fn gradient_bar(
    pm: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    from: (u8, u8, u8),
    to: (u8, u8, u8),
) {
    let paint = gradient_paint(x, y, x + w, y, from, to);
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn hline(pm: &mut Pixmap, x: f32, y: f32, w: f32, stroke: Color) {
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x + w, y);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(stroke);
        pm.stroke_path(
            &path,
            &paint,
            &Stroke {
                width: 1.0,
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
    }
}

fn mood(score: u8) -> (&'static str, (u8, u8, u8)) {
    if score >= 70 {
        ("Deep focus", DEEP)
    } else if score >= 45 {
        ("Steady", MEDIUM)
    } else {
        ("Scattered", FRAGMENTED)
    }
}

fn score_color(score: u8) -> (u8, u8, u8) {
    mood(score).1
}

fn text_width(font: &FontRef, size: f32, text: &str) -> f32 {
    let scaled = font.as_scaled(size);
    let mut width = 0.0;
    let mut prev: Option<GlyphId> = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(prev) = prev {
            width += scaled.kern(prev, id);
        }
        width += scaled.h_advance(id);
        prev = Some(id);
    }
    width
}

fn draw_text(
    pm: &mut Pixmap,
    font: &FontRef,
    size: f32,
    x: f32,
    baseline: f32,
    fill: Color,
    text: &str,
) {
    let scaled = font.as_scaled(size);
    let mut pen = x;
    let mut prev: Option<GlyphId> = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(prev) = prev {
            pen += scaled.kern(prev, id);
        }
        let glyph = id.with_scale_and_position(size, point(pen, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                blend(
                    pm,
                    bounds.min.x + gx as f32,
                    bounds.min.y + gy as f32,
                    fill,
                    coverage,
                );
            });
        }
        pen += scaled.h_advance(id);
        prev = Some(id);
    }
}

/// Composite glyph coverage over the (opaque) canvas.
fn blend(pm: &mut Pixmap, x: f32, y: f32, fill: Color, coverage: f32) {
    let xi = x as i32;
    let yi = y as i32;
    let w = pm.width() as i32;
    let h = pm.height() as i32;
    if xi < 0 || yi < 0 || xi >= w || yi >= h {
        return;
    }
    let alpha = (coverage * fill.alpha()).clamp(0.0, 1.0);
    if alpha <= 0.004 {
        return;
    }
    let c = fill.to_color_u8();
    let idx = ((yi as u32 * pm.width() + xi as u32) * 4) as usize;
    let data = pm.data_mut();
    let src = [c.red(), c.green(), c.blue()];
    for (i, s) in src.iter().enumerate() {
        let dst = data[idx + i] as f32;
        data[idx + i] = (dst * (1.0 - alpha) + *s as f32 * alpha).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_1080_square_png() {
        if fonts::load().is_none() {
            return; // no system font in this environment: nothing to verify
        }
        let mut hours = [None; 24];
        for (i, hour) in hours.iter_mut().enumerate().skip(9).take(7) {
            *hour = Some(if i % 3 == 0 { 82 } else { 40 });
        }
        let data = ShareData {
            day: "2026-11-15".into(),
            score: 81,
            deep_minutes: 135,
            longest_session_minutes: 55,
            streak: 5,
            goal_minutes: 120,
            hour_scores: hours,
        };
        let png = render_png(&data).expect("card renders");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!((width, height), (CARD_SIZE, CARD_SIZE));
    }

    #[test]
    fn encodes_exported_rgba_as_a_square_png() {
        let rgba = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255];
        let png = encode_rgba_png(&rgba, 2, 2).expect("export encodes");
        let decoded = Pixmap::decode_png(&png).expect("png decodes");
        assert_eq!((decoded.width(), decoded.height()), (2, 2));
        assert_eq!(decoded.pixels()[0].red(), 255);
        assert_eq!(decoded.pixels()[3].blue(), 255);
    }
}
