use skia_safe::{Canvas, Color, Paint, Rect};

use crate::ui::widget::resource_usage::{
    CPU_COLOR, RAM_COLOR, alpha_color, usage_color, with_resource_usage,
};
use crate::utils::font::{DrawTextCachedParams, FontManager};

pub(super) fn draw(canvas: &Canvas, rect: Rect, scale: f32, alpha: u8) {
    with_resource_usage(|usage| {
        let gap = 5.0 * scale;
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
        divider.set_stroke_width(scale.max(1.0));
        divider.set_color(Color::from_argb((alpha as f32 * 0.12) as u8, 255, 255, 255));
        let x = rect.left + metric_width + gap / 2.0;
        canvas.draw_line(
            (x, rect.top + rect.height() * 0.3),
            (x, rect.bottom - rect.height() * 0.3),
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
    let track_width = (2.5 * scale).clamp(2.0, 4.0);
    let track_height = (14.0 * scale).min(rect.height() * 0.52);
    let track = Rect::from_xywh(
        rect.left + scale,
        rect.center_y() - track_height / 2.0,
        track_width,
        track_height,
    );
    let radius = track_width / 2.0;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb((alpha as f32 * 0.14) as u8, 255, 255, 255));
    canvas.draw_round_rect(track, radius, radius, &paint);
    if value.is_some() && usage > 0.0 {
        let fill_height = (track.height() * usage).max(track_width);
        let fill = Rect::from_xywh(
            track.left,
            track.bottom - fill_height,
            track.width(),
            fill_height,
        );
        paint.set_color(alpha_color(accent, alpha));
        canvas.draw_round_rect(fill, radius, radius, &paint);
    }

    let text_x = track.right + 5.0 * scale;
    let font_manager = FontManager::global();
    paint.set_color(Color::from_argb((alpha as f32 * 0.56) as u8, 255, 255, 255));
    font_manager.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: label,
        x: text_x,
        y: rect.center_y() - 1.5 * scale,
        size: (6.5 * scale).clamp(6.5, 10.0),
        bold: true,
        paint: &paint,
    });
    paint.set_color(Color::from_argb(alpha, 255, 255, 255));
    font_manager.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: value_text,
        x: text_x,
        y: rect.center_y() + 8.0 * scale,
        size: (9.0 * scale).clamp(8.0, 13.0),
        bold: true,
        paint: &paint,
    });
}
