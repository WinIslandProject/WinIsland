use crate::ui::widget::resource_usage::{
    MetricUsage, alpha_color, metric_color, usage_color, with_compact_config, with_resource_usage,
};
use winisland_core::config::{ResourceMetricConfig, ResourceMetricStyle};
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{Angle, Painter, Point, Radius, Rect, Rgba, StrokeCap};

const METRIC_GAP: f32 = 4.0;

pub(super) fn draw(painter: Painter<'_>, rect: Rect, scale: f32, alpha: u8) {
    with_compact_config(|config| {
        let enabled: Vec<_> = config.iter().filter(|metric| metric.enabled).collect();
        if enabled.is_empty() {
            return;
        }
        with_resource_usage(config, |usage| {
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
                    painter,
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
    painter: Painter<'_>,
    rect: Rect,
    config: &ResourceMetricConfig,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
) {
    let save_count = painter.save();
    painter.clip_rect(rect);
    match config.style {
        ResourceMetricStyle::Bar => draw_bar(painter, rect, config, usage, scale, alpha),
        ResourceMetricStyle::Ring => draw_ring(painter, rect, config, usage, scale, alpha),
    }
    painter.restore_to(save_count);
}

fn draw_bar(
    painter: Painter<'_>,
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
    painter.fill_round_rect(
        track,
        Radius::uniform(track_h / 2.0),
        Rgba::from_argb((alpha as f32 * 0.13) as u8, 255, 255, 255),
    );
    if usage.value.is_some() && value > 0.0 {
        let fill_w = (track.width() * value).max(track_h).min(track.width());
        painter.fill_round_rect(
            Rect::from_xywh(track.left, track.top, fill_w, track_h),
            Radius::uniform(track_h / 2.0),
            alpha_color(accent, (alpha as f32 * 0.9) as u8),
        );
    }
    let fonts = FontManager::global();
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: config.kind.label(),
        x: left,
        y: baseline,
        size: label_size,
        bold: true,
        color: alpha_color(accent, (alpha as f32 * 0.78) as u8),
        blur: None,
    });
    let value_w =
        fonts.measure_text_cached(usage.text, value_size, winisland_render::FontStyle::bold());
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: usage.text,
        x: rect.right - inset - value_w,
        y: baseline,
        size: value_size,
        bold: true,
        color: Rgba::from_argb(alpha, 255, 255, 255),
        blur: None,
    });
}

fn draw_ring(
    painter: Painter<'_>,
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
    let center = Point::new(rect.right - inset - diameter / 2.0, rect.center_y());
    let ring = Rect::from_xywh(
        center.x - diameter / 2.0,
        center.y - diameter / 2.0,
        diameter,
        diameter,
    );
    let stroke_width = (2.0 * scale).min(diameter * 0.13);
    painter.stroke_circle(
        center,
        diameter / 2.0,
        stroke_width,
        Rgba::from_argb((alpha as f32 * 0.14) as u8, 255, 255, 255),
    );
    if usage.value.is_some() && value > 0.0 {
        painter.stroke_arc(
            ring,
            Angle::ZERO,
            Angle::from_degrees(value * 360.0),
            stroke_width,
            alpha_color(accent, (alpha as f32 * 0.92) as u8),
            StrokeCap::Round,
        );
    }
    let fonts = FontManager::global();
    let mut value_size = (diameter * 0.22).max(4.0);
    let max_value_width = diameter * 0.78;
    let mut value_w =
        fonts.measure_text_cached(usage.text, value_size, winisland_render::FontStyle::bold());
    if value_w > max_value_width {
        value_size = (value_size * max_value_width / value_w).max(3.2);
        value_w =
            fonts.measure_text_cached(usage.text, value_size, winisland_render::FontStyle::bold());
    }
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: usage.text,
        x: center.x - value_w / 2.0,
        y: center.y + value_size * 0.32,
        size: value_size,
        bold: true,
        color: Rgba::from_argb(alpha, 255, 255, 255),
        blur: None,
    });
    let label_size = (6.0 * scale).max(4.5);
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: config.kind.label(),
        x: rect.left + inset,
        y: rect.center_y() + label_size * 0.34,
        size: label_size,
        bold: true,
        color: alpha_color(accent, (alpha as f32 * 0.8) as u8),
        blur: None,
    });
}
