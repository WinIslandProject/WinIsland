use super::draw_widget_rounded_background;
use crate::ui::widget::resource_usage::{
    MetricUsage, alpha_color, metric_color, usage_color, with_expanded_config, with_resource_usage,
};
use winisland_core::config::{
    ResourceMetricConfig, ResourceMetricKind, ResourceMetricStyle, default_resource_metrics,
    resource_widget_span,
};
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{Angle, Painter, Point, Radius, Rect, Rgba, StrokeCap};

const CELL_GAP: f32 = 3.0;

#[allow(clippy::too_many_arguments)]
fn draw_resource_usage<'a>(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
    metrics: &[ResourceMetricConfig],
    values: impl Fn(ResourceMetricKind) -> MetricUsage<'a>,
) {
    draw_widget_rounded_background(painter, x, y, w, h, scale, alpha);
    let (columns, configured_rows) = resource_widget_span();
    let capacity = columns * configured_rows;
    let enabled: Vec<_> = metrics
        .iter()
        .filter(|metric| metric.enabled)
        .take(capacity)
        .collect();
    if enabled.is_empty() {
        return;
    }
    let rows = enabled.len().div_ceil(columns).min(configured_rows);
    let gap = CELL_GAP * scale;
    let cell_w = (w - gap * (columns - 1) as f32) / columns as f32;
    let cell_h = (h - gap * (rows - 1) as f32) / rows as f32;
    for (index, metric) in enabled.into_iter().enumerate() {
        let column = index % columns;
        let row = index / columns;
        let bounds = Rect::from_xywh(
            x + column as f32 * (cell_w + gap),
            y + row as f32 * (cell_h + gap),
            cell_w,
            cell_h,
        );
        draw_metric(
            painter,
            bounds,
            metric,
            values(metric.kind),
            scale,
            alpha,
            text_color,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_metric(
    painter: Painter<'_>,
    bounds: Rect,
    config: &ResourceMetricConfig,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    let save_count = painter.save();
    painter.clip_rect(bounds);
    match config.style {
        ResourceMetricStyle::Bar => {
            draw_bar(painter, bounds, config, usage, scale, alpha, text_color)
        }
        ResourceMetricStyle::Ring => {
            draw_ring(painter, bounds, config, usage, scale, alpha, text_color)
        }
    }
    painter.restore_to(save_count);
}

#[allow(clippy::too_many_arguments)]
fn draw_bar(
    painter: Painter<'_>,
    bounds: Rect,
    config: &ResourceMetricConfig,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    let value = usage.value.unwrap_or_default();
    let accent = usage_color(metric_color(config.color), value);
    let inset = 7.0 * scale;
    let left = bounds.left + inset;
    let right = bounds.right - inset;
    let baseline = bounds.top + bounds.height() * 0.46;
    let mut font_size = (bounds.height() * 0.23).clamp(4.5 * scale, 10.0 * scale);
    let mut value_size = (font_size * 1.08).min(11.0 * scale);
    let track_h = (3.0 * scale).min(bounds.height() * 0.12).max(1.5);
    let track = Rect::from_xywh(
        left,
        bounds.top + bounds.height() * 0.68,
        (right - left).max(0.0),
        track_h,
    );
    painter.fill_round_rect(
        track,
        Radius::uniform(track_h / 2.0),
        alpha_color(text_color, (alpha as f32 * 0.13) as u8),
    );
    if usage.value.is_some() && value > 0.0 {
        let fill_w = (track.width() * value).max(track_h).min(track.width());
        painter.fill_round_rect(
            Rect::from_xywh(track.left, track.top, fill_w, track_h),
            Radius::uniform(track_h / 2.0),
            alpha_color(accent, (alpha as f32 * 0.92) as u8),
        );
    }
    let fonts = FontManager::global();
    let label_width = fonts.measure_text_cached(
        config.kind.label(),
        font_size,
        winisland_render::FontStyle::bold(),
    );
    let measured_value_width =
        fonts.measure_text_cached(usage.text, value_size, winisland_render::FontStyle::bold());
    let available = (right - left - 2.0 * scale).max(1.0);
    let fit = (available / (label_width + measured_value_width).max(1.0)).min(1.0);
    font_size = (font_size * fit).max(3.8 * scale);
    value_size = (value_size * fit).max(3.8 * scale);
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: config.kind.label(),
        x: left,
        y: baseline,
        size: font_size,
        bold: true,
        color: alpha_color(accent, (alpha as f32 * 0.82) as u8),
        blur: None,
    });
    let value_w =
        fonts.measure_text_cached(usage.text, value_size, winisland_render::FontStyle::bold());
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: usage.text,
        x: right - value_w,
        y: baseline,
        size: value_size,
        bold: true,
        color: alpha_color(text_color, alpha),
        blur: None,
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_ring(
    painter: Painter<'_>,
    bounds: Rect,
    config: &ResourceMetricConfig,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    let value = usage.value.unwrap_or_default();
    let accent = usage_color(metric_color(config.color), value);
    let inset = 7.0 * scale;
    let diameter = (bounds.height() * 0.64)
        .min(bounds.width() * 0.38)
        .max(12.0 * scale);
    let center = Point::new(bounds.right - inset - diameter / 2.0, bounds.center_y());
    let ring = Rect::from_xywh(
        center.x - diameter / 2.0,
        center.y - diameter / 2.0,
        diameter,
        diameter,
    );
    let stroke_width = (2.5 * scale).min(diameter * 0.13);
    painter.stroke_circle(
        center,
        diameter / 2.0,
        stroke_width,
        alpha_color(text_color, (alpha as f32 * 0.13) as u8),
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
    let mut value_size = (diameter * 0.22).clamp(4.0 * scale, 9.0 * scale);
    let max_value_width = diameter * 0.78;
    let mut value_width =
        fonts.measure_text_cached(usage.text, value_size, winisland_render::FontStyle::bold());
    if value_width > max_value_width {
        value_size = (value_size * max_value_width / value_width).max(3.2 * scale);
        value_width =
            fonts.measure_text_cached(usage.text, value_size, winisland_render::FontStyle::bold());
    }
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: usage.text,
        x: center.x - value_width / 2.0,
        y: center.y + value_size * 0.32,
        size: value_size,
        bold: true,
        color: alpha_color(text_color, alpha),
        blur: None,
    });
    let mut label_size = (bounds.height() * 0.22).clamp(4.0 * scale, 10.0 * scale);
    let label_space = (ring.left - bounds.left - inset - 2.0 * scale).max(1.0);
    let measured_label = fonts.measure_text_cached(
        config.kind.label(),
        label_size,
        winisland_render::FontStyle::bold(),
    );
    if measured_label > label_space {
        label_size = (label_size * label_space / measured_label).max(3.5 * scale);
    }
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: config.kind.label(),
        x: bounds.left + inset,
        y: bounds.center_y() + label_size * 0.34,
        size: label_size,
        bold: true,
        color: alpha_color(accent, (alpha as f32 * 0.84) as u8),
        blur: None,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn draw_resource_usage_widget(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    with_expanded_config(|metrics| {
        with_resource_usage(metrics, |usage| {
            draw_resource_usage(
                painter,
                x,
                y,
                w,
                h,
                scale,
                alpha,
                text_color,
                metrics,
                |kind| usage.metric(kind),
            );
        });
    });
}

#[allow(clippy::too_many_arguments)]
pub fn draw_resource_usage_preview(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    with_expanded_config(|configured| {
        let defaults;
        let metrics = if configured.iter().any(|metric| metric.enabled) {
            configured
        } else {
            defaults = default_resource_metrics();
            &defaults
        };
        draw_resource_usage(
            painter,
            x,
            y,
            w,
            h,
            scale,
            alpha,
            text_color,
            metrics,
            |kind| match kind {
                ResourceMetricKind::Cpu => MetricUsage {
                    value: Some(0.37),
                    text: "37%",
                },
                ResourceMetricKind::Ram => MetricUsage {
                    value: Some(0.62),
                    text: "62%",
                },
                ResourceMetricKind::Gpu => MetricUsage {
                    value: Some(0.48),
                    text: "48%",
                },
                ResourceMetricKind::Network => MetricUsage {
                    value: Some(0.24),
                    text: "2.4M/s",
                },
                ResourceMetricKind::Disk => MetricUsage {
                    value: Some(0.71),
                    text: "71%",
                },
            },
        );
    });
}
