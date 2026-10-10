use super::draw_widget_rounded_background;
use crate::ui::widget::resource_usage::{
    MetricUsage, RateUsage, alpha_color, draw_rate_arrow, gpu_count, metric_color, metric_label,
    preview_usage, usage_color, visible_metrics, with_expanded_config, with_resource_usage,
};
use winisland_core::config::{
    ResourceMetricConfig, ResourceMetricStyle, default_resource_metrics, resource_widget_span,
};
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{Angle, FontStyle, Painter, Point, Radius, Rect, Rgba, StrokeCap};

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
    usage: impl Fn(&ResourceMetricConfig) -> MetricUsage<'a>,
) {
    draw_widget_rounded_background(painter, x, y, w, h, scale, alpha);
    draw_metric_grid(
        painter,
        Rect::from_xywh(x, y, w, h),
        scale,
        alpha,
        text_color,
        metrics,
        usage,
    );
}

pub(crate) fn draw_metric_grid<'a>(
    painter: Painter<'_>,
    rect: Rect,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
    metrics: &[ResourceMetricConfig],
    usage: impl Fn(&ResourceMetricConfig) -> MetricUsage<'a>,
) {
    let (columns, configured_rows) = resource_widget_span();
    let capacity = columns * configured_rows;
    let enabled: Vec<_> = visible_metrics(metrics).take(capacity).collect();
    if enabled.is_empty() {
        return;
    }
    let gpu_count = gpu_count();
    let rows = enabled.len().div_ceil(columns).min(configured_rows);
    let gap = CELL_GAP * scale;
    let cell_w = (rect.width() - gap * (columns - 1) as f32) / columns as f32;
    let cell_h = (rect.height() - gap * (rows - 1) as f32) / rows as f32;
    for (index, metric) in enabled.into_iter().enumerate() {
        let column = index % columns;
        let row = index / columns;
        let bounds = Rect::from_xywh(
            rect.left + column as f32 * (cell_w + gap),
            rect.top + row as f32 * (cell_h + gap),
            cell_w,
            cell_h,
        );
        let label = metric_label(metric, gpu_count);
        draw_metric(
            painter,
            bounds,
            metric,
            &label,
            usage(metric),
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
    label: &str,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    let save_count = painter.save();
    painter.clip_rect(bounds);
    match usage {
        MetricUsage::Rates { upload, download } => draw_rates(
            painter, bounds, config, upload, download, scale, alpha, text_color,
        ),
        MetricUsage::Percent { value, text } => match config.style {
            ResourceMetricStyle::Bar => draw_bar(
                painter, bounds, config, label, value, text, scale, alpha, text_color,
            ),
            ResourceMetricStyle::Ring => draw_ring(
                painter, bounds, config, label, value, text, scale, alpha, text_color,
            ),
        },
    }
    painter.restore_to(save_count);
}

#[allow(clippy::too_many_arguments)]
fn draw_bar(
    painter: Painter<'_>,
    bounds: Rect,
    config: &ResourceMetricConfig,
    label: &str,
    usage: Option<f32>,
    text: &str,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    let value = usage.unwrap_or_default();
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
    if usage.is_some() && value > 0.0 {
        let fill_w = (track.width() * value).max(track_h).min(track.width());
        painter.fill_round_rect(
            Rect::from_xywh(track.left, track.top, fill_w, track_h),
            Radius::uniform(track_h / 2.0),
            alpha_color(accent, (alpha as f32 * 0.92) as u8),
        );
    }
    let fonts = FontManager::global();
    let label_width = fonts.measure_text_cached(label, font_size, FontStyle::bold());
    let measured_value_width = fonts.measure_text_cached(text, value_size, FontStyle::bold());
    let available = (right - left - 2.0 * scale).max(1.0);
    let fit = (available / (label_width + measured_value_width).max(1.0)).min(1.0);
    font_size = (font_size * fit).max(3.8 * scale);
    value_size = (value_size * fit).max(3.8 * scale);
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: label,
        x: left,
        y: baseline,
        size: font_size,
        bold: true,
        color: alpha_color(accent, (alpha as f32 * 0.82) as u8),
        blur: None,
    });
    let value_w = fonts.measure_text_cached(text, value_size, FontStyle::bold());
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text,
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
    label: &str,
    usage: Option<f32>,
    text: &str,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    let value = usage.unwrap_or_default();
    let accent = usage_color(metric_color(config.color), value);
    let fonts = FontManager::global();
    let inset = 5.0 * scale;
    let label_gap = 3.0 * scale;
    let mut label_size = (bounds.height() * 0.16).clamp(4.0 * scale, 9.0 * scale);
    let label_space = (bounds.width() - inset * 2.0).max(1.0);
    let measured_label = fonts.measure_text_cached(label, label_size, FontStyle::bold());
    if measured_label > label_space {
        label_size = (label_size * label_space / measured_label).max(3.5 * scale);
    }
    let diameter = (bounds.height() - inset * 2.0 - label_size - label_gap)
        .min(bounds.width() - inset * 2.0)
        .max(12.0 * scale);
    let group_top = bounds.center_y() - (diameter + label_gap + label_size) / 2.0;
    let center = Point::new(bounds.center_x(), group_top + diameter / 2.0);
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
    if usage.is_some() && value > 0.0 {
        painter.stroke_arc(
            ring,
            Angle::ZERO,
            Angle::from_degrees(value * 360.0),
            stroke_width,
            alpha_color(accent, (alpha as f32 * 0.92) as u8),
            StrokeCap::Round,
        );
    }
    let mut value_size = (diameter * 0.22).clamp(4.0 * scale, 9.0 * scale);
    let max_value_width = diameter * 0.78;
    let mut value_width = fonts.measure_text_cached(text, value_size, FontStyle::bold());
    if value_width > max_value_width {
        value_size = (value_size * max_value_width / value_width).max(3.2 * scale);
        value_width = fonts.measure_text_cached(text, value_size, FontStyle::bold());
    }
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text,
        x: center.x - value_width / 2.0,
        y: center.y + value_size * 0.32,
        size: value_size,
        bold: true,
        color: alpha_color(text_color, alpha),
        blur: None,
    });
    let label_width = fonts.measure_text_cached(label, label_size, FontStyle::bold());
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: label,
        x: center.x - label_width / 2.0,
        y: ring.bottom + label_gap + label_size * 0.8,
        size: label_size,
        bold: true,
        color: alpha_color(accent, (alpha as f32 * 0.84) as u8),
        blur: None,
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_rates(
    painter: Painter<'_>,
    bounds: Rect,
    config: &ResourceMetricConfig,
    upload: RateUsage<'_>,
    download: RateUsage<'_>,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    let accent = alpha_color(metric_color(config.color), (alpha as f32 * 0.92) as u8);
    let fonts = FontManager::global();
    let inset = 7.0 * scale;
    let mut value_size = (bounds.height() * 0.24).clamp(4.5 * scale, 10.5 * scale);
    let mut unit_size = value_size * 0.74;
    let arrow_gap = 3.0 * scale;
    let unit_gap = 2.0 * scale;
    let row_width = |rate: RateUsage<'_>, value_size: f32, unit_size: f32| {
        value_size * 0.8
            + arrow_gap
            + fonts.measure_text_cached(rate.value, value_size, FontStyle::bold())
            + unit_gap
            + fonts.measure_text_cached(rate.unit, unit_size, FontStyle::normal())
    };
    let widest =
        row_width(upload, value_size, unit_size).max(row_width(download, value_size, unit_size));
    let available = (bounds.width() - inset * 2.0).max(1.0);
    if widest > available {
        value_size *= available / widest;
        unit_size *= available / widest;
    }
    let arrow_size = value_size * 0.8;
    let cap = value_size * 0.72;
    let line_gap = (bounds.height() * 0.14).clamp(2.0 * scale, 6.0 * scale);
    let first_baseline = bounds.center_y() - line_gap / 2.0;
    let left = bounds.left + inset;
    for (rate, upward, baseline) in [
        (upload, true, first_baseline),
        (download, false, first_baseline + line_gap + cap),
    ] {
        draw_rate_arrow(
            painter,
            Point::new(left + arrow_size / 2.0, baseline - cap / 2.0),
            arrow_size,
            upward,
            accent,
        );
        let value_x = left + arrow_size + arrow_gap;
        fonts.draw_text_cached(DrawTextCachedParams {
            painter,
            text: rate.value,
            x: value_x,
            y: baseline,
            size: value_size,
            bold: true,
            color: alpha_color(text_color, alpha),
            blur: None,
        });
        let value_w = fonts.measure_text_cached(rate.value, value_size, FontStyle::bold());
        fonts.draw_text_cached(DrawTextCachedParams {
            painter,
            text: rate.unit,
            x: value_x + value_w + unit_gap,
            y: baseline,
            size: unit_size,
            bold: false,
            color: alpha_color(text_color, (alpha as f32 * 0.58) as u8),
            blur: None,
        });
    }
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
                |metric| usage.metric(metric),
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
        let metrics = if visible_metrics(configured).next().is_some() {
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
            preview_usage,
        );
    });
}
