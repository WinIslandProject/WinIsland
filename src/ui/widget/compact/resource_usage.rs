use skia_safe::{Canvas, Color, Paint, Rect};

use crate::ui::widget::resource_usage::{
    CPU_COLOR, RAM_COLOR, alpha_color, usage_color, with_resource_usage,
};
use crate::utils::font::{DrawTextCachedParams, FontManager};

const METRIC_GAP: f32 = 8.0;
const METRIC_HORIZONTAL_INSET: f32 = 2.0;
const METRIC_LABEL_SIZE: f32 = 7.0;
const METRIC_VALUE_SIZE: f32 = 10.0;
const METRIC_TRACK_HEIGHT: f32 = 2.0;
const METRIC_TRACK_GAP: f32 = 5.0;
const DIVIDER_HALF_HEIGHT: f32 = 6.0;

pub(super) fn draw(canvas: &Canvas, rect: Rect, scale: f32, alpha: u8) {
    with_resource_usage(|usage| {
        let gap = METRIC_GAP * scale;
        let metric_width = (rect.width() - gap) / 2.0;
        draw_metric(
            canvas,
            Rect::from_xywh(rect.left, rect.top, metric_width, rect.height()),
            "CPU",
            usage.cpu,
            usage.cpu_text,
            CPU_COLOR,
            scale,
            alpha,
        );
        draw_metric(
            canvas,
            Rect::from_xywh(
                rect.left + metric_width + gap,
                rect.top,
                metric_width,
                rect.height(),
            ),
            "RAM",
            usage.ram,
            usage.ram_text,
            RAM_COLOR,
            scale,
            alpha,
        );

        let mut divider = Paint::default();
        divider.set_anti_alias(true);
        divider.set_stroke_width((0.75 * scale).max(0.75));
        divider.set_color(Color::from_argb((alpha as f32 * 0.1) as u8, 255, 255, 255));
        let x = rect.left + metric_width + gap / 2.0;
        let center_y = rect.center_y();
        canvas.draw_line(
            (x, center_y - DIVIDER_HALF_HEIGHT * scale),
            (x, center_y + DIVIDER_HALF_HEIGHT * scale),
            &divider,
        );
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_metric(
    canvas: &Canvas,
    rect: Rect,
    label: &str,
    value: Option<f32>,
    value_text: &str,
    base_color: Color,
    scale: f32,
    alpha: u8,
) {
    let usage = value.unwrap_or_default();
    let accent = usage_color(base_color, usage);
    let inset = METRIC_HORIZONTAL_INSET * scale;
    let content_left = rect.left + inset;
    let content_right = rect.right - inset;
    let content_width = (content_right - content_left).max(0.0);
    let label_size = METRIC_LABEL_SIZE * scale;
    let value_size = METRIC_VALUE_SIZE * scale;
    let text_baseline = rect.center_y() + scale;
    let track_height = METRIC_TRACK_HEIGHT * scale;
    let track = Rect::from_xywh(
        content_left,
        text_baseline + METRIC_TRACK_GAP * scale,
        content_width,
        track_height,
    );
    let radius = track_height / 2.0;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb((alpha as f32 * 0.13) as u8, 255, 255, 255));
    canvas.draw_round_rect(track, radius, radius, &paint);
    if value.is_some() && usage > 0.0 {
        let fill_width = (track.width() * usage).max(track_height).min(track.width());
        let fill = Rect::from_xywh(track.left, track.top, fill_width, track_height);
        paint.set_color(alpha_color(accent, (alpha as f32 * 0.9) as u8));
        canvas.draw_round_rect(fill, radius, radius, &paint);
    }

    let font_manager = FontManager::global();
    paint.set_color(alpha_color(accent, (alpha as f32 * 0.78) as u8));
    font_manager.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: label,
        x: content_left,
        y: text_baseline,
        size: label_size,
        bold: true,
        paint: &paint,
    });
    let value_width =
        font_manager.measure_text_cached(value_text, value_size, skia_safe::FontStyle::bold());
    let value_alpha = if value.is_some() {
        alpha
    } else {
        (alpha as f32 * 0.5) as u8
    };
    paint.set_color(Color::from_argb(value_alpha, 255, 255, 255));
    font_manager.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: value_text,
        x: content_right - value_width,
        y: text_baseline,
        size: value_size,
        bold: true,
        paint: &paint,
    });
}
