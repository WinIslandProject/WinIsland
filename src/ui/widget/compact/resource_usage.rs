use skia_safe::{Canvas, Color, Paint, Rect};

use crate::core::config::{ResourceMetricConfig, ResourceMetricStyle};
use crate::ui::widget::resource_usage::{
    MetricUsage, alpha_color, metric_color, usage_color, with_compact_config, with_resource_usage,
};
use crate::utils::font::{DrawTextCachedParams, FontManager};

const METRIC_GAP: f32 = 4.0;

pub(super) fn draw(canvas: &Canvas, rect: Rect, scale: f32, alpha: u8) {
    with_compact_config(|config| {
        let enabled: Vec<_> = config.iter().filter(|metric| metric.enabled).collect();
        if enabled.is_empty() {
            return;
        }
        with_resource_usage(|usage| {
            let gap = METRIC_GAP * scale;
            let metric_width = (rect.width() - gap * enabled.len().saturating_sub(1) as f32)
                / enabled.len() as f32;
            for (index, metric) in enabled.into_iter().enumerate() {
                let bounds = Rect::from_xywh(
                    rect.left + index as f32 * (metric_width + gap),
                    rect.top,
                    metric_width,
                    rect.height(),
                );
                draw_metric(
                    canvas,
                    bounds,
                    metric,
                    usage.metric(metric.kind),
                    scale,
                    alpha,
                );
            }
        });
    });
}

fn draw_metric(
    canvas: &Canvas,
    rect: Rect,
    config: &ResourceMetricConfig,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
) {
    let save_count = canvas.save();
    canvas.clip_rect(rect, None, true);
    match config.style {
        ResourceMetricStyle::Bar => draw_bar(canvas, rect, config, usage, scale, alpha),
        ResourceMetricStyle::Ring => draw_ring(canvas, rect, config, usage, scale, alpha),
    }
    canvas.restore_to_count(save_count);
}

fn draw_bar(
    canvas: &Canvas,
    rect: Rect,
    config: &ResourceMetricConfig,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
) {
    let value = usage.value.unwrap_or_default();
    let accent = usage_color(metric_color(config.color), value);
    let inset = 1.0 * scale;
    let left = rect.left + inset;
    let width = (rect.width() - inset * 2.0).max(0.0);
    let baseline = rect.center_y() + scale;
    let label_size = (6.5 * scale).min(rect.width() * 0.22).max(4.5);
    let value_size = (8.5 * scale).min(rect.width() * 0.28).max(5.5);
    let track_h = (2.0 * scale).max(1.5);
    let track = Rect::from_xywh(left, baseline + 4.5 * scale, width, track_h);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb((alpha as f32 * 0.13) as u8, 255, 255, 255));
    canvas.draw_round_rect(track, track_h / 2.0, track_h / 2.0, &paint);
    if usage.value.is_some() && value > 0.0 {
        let fill_w = (track.width() * value).max(track_h).min(track.width());
        paint.set_color(alpha_color(accent, (alpha as f32 * 0.9) as u8));
        canvas.draw_round_rect(
            Rect::from_xywh(track.left, track.top, fill_w, track_h),
            track_h / 2.0,
            track_h / 2.0,
            &paint,
        );
    }
    let fonts = FontManager::global();
    paint.set_color(alpha_color(accent, (alpha as f32 * 0.78) as u8));
    fonts.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: config.kind.label(),
        x: left,
        y: baseline,
        size: label_size,
        bold: true,
        paint: &paint,
    });
    let value_w = fonts.measure_text_cached(usage.text, value_size, skia_safe::FontStyle::bold());
    paint.set_color(Color::from_argb(alpha, 255, 255, 255));
    fonts.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: usage.text,
        x: rect.right - inset - value_w,
        y: baseline,
        size: value_size,
        bold: true,
        paint: &paint,
    });
}

fn draw_ring(
    canvas: &Canvas,
    rect: Rect,
    config: &ResourceMetricConfig,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
) {
    let value = usage.value.unwrap_or_default();
    let accent = usage_color(metric_color(config.color), value);
    let diameter = (rect.height() * 0.72)
        .min(rect.width() * 0.42)
        .max(10.0 * scale);
    let inset = 1.0 * scale;
    let center = (rect.right - inset - diameter / 2.0, rect.center_y());
    let ring = Rect::from_xywh(
        center.0 - diameter / 2.0,
        center.1 - diameter / 2.0,
        diameter,
        diameter,
    );
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width((2.0 * scale).min(diameter * 0.13));
    paint.set_stroke_cap(skia_safe::paint::Cap::Round);
    paint.set_color(Color::from_argb((alpha as f32 * 0.14) as u8, 255, 255, 255));
    canvas.draw_circle(center, diameter / 2.0, &paint);
    if usage.value.is_some() && value > 0.0 {
        paint.set_color(alpha_color(accent, (alpha as f32 * 0.92) as u8));
        canvas.draw_arc(ring, -90.0, value * 360.0, false, &paint);
    }
    let fonts = FontManager::global();
    let mut value_size = (diameter * 0.22).max(4.0);
    let max_value_width = diameter * 0.78;
    let mut value_w =
        fonts.measure_text_cached(usage.text, value_size, skia_safe::FontStyle::bold());
    if value_w > max_value_width {
        value_size = (value_size * max_value_width / value_w).max(3.2);
        value_w = fonts.measure_text_cached(usage.text, value_size, skia_safe::FontStyle::bold());
    }
    paint.set_style(skia_safe::paint::Style::Fill);
    paint.set_color(Color::from_argb(alpha, 255, 255, 255));
    fonts.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: usage.text,
        x: center.0 - value_w / 2.0,
        y: center.1 + value_size * 0.32,
        size: value_size,
        bold: true,
        paint: &paint,
    });
    let label_size = (6.0 * scale).max(4.5);
    paint.set_color(alpha_color(accent, (alpha as f32 * 0.8) as u8));
    fonts.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: config.kind.label(),
        x: rect.left + inset,
        y: rect.center_y() + label_size * 0.34,
        size: label_size,
        bold: true,
        paint: &paint,
    });
}
