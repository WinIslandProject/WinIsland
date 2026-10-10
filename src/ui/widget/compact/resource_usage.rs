use crate::ui::widget::resource_usage::{
    COMPACT_METRIC_GAP, MetricUsage, RateUsage, alpha_color, compact_metric_width, draw_rate_arrow,
    gpu_count, metric_color, metric_label, usage_color, visible_metrics, with_compact_config,
    with_resource_usage,
};
use winisland_core::config::{ResourceMetricConfig, ResourceMetricStyle};
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{Angle, FontStyle, Painter, Point, Radius, Rect, Rgba, StrokeCap};

const RING_LABEL_GAP: f32 = 4.0;

pub(super) fn draw(painter: Painter<'_>, rect: Rect, scale: f32, alpha: u8) {
    with_compact_config(|config| {
        with_resource_usage(config, |usage| {
            draw_metrics(painter, rect, config, scale, alpha, |metric| {
                usage.metric(metric)
            });
        });
    });
}

pub(crate) fn draw_metrics<'a>(
    painter: Painter<'_>,
    rect: Rect,
    metrics: &[ResourceMetricConfig],
    scale: f32,
    alpha: u8,
    usage: impl Fn(&ResourceMetricConfig) -> MetricUsage<'a>,
) {
    let enabled: Vec<_> = visible_metrics(metrics).collect();
    if enabled.is_empty() {
        return;
    }
    let gpu_count = gpu_count();
    let gap = COMPACT_METRIC_GAP * scale;
    let natural_width = enabled
        .iter()
        .map(|metric| compact_metric_width(metric, gpu_count) * scale)
        .sum::<f32>()
        + gap * enabled.len().saturating_sub(1) as f32;
    let fit = (rect.width() / natural_width.max(f32::EPSILON)).min(1.0);
    let mut x = rect.left + (rect.width() - natural_width * fit).max(0.0) / 2.0;
    for metric in enabled {
        let metric_width = compact_metric_width(metric, gpu_count) * scale * fit;
        let bounds = Rect::from_xywh(x, rect.top, metric_width, rect.height());
        x += metric_width + gap * fit;
        let label = metric_label(metric, gpu_count);
        draw_metric(painter, bounds, metric, &label, usage(metric), scale, alpha);
    }
}

fn draw_metric(
    painter: Painter<'_>,
    rect: Rect,
    config: &ResourceMetricConfig,
    label: &str,
    usage: MetricUsage<'_>,
    scale: f32,
    alpha: u8,
) {
    let save_count = painter.save();
    painter.clip_rect(rect);
    match usage {
        MetricUsage::Rates { upload, download } => {
            draw_rates(painter, rect, config, upload, download, scale, alpha)
        }
        MetricUsage::Percent { value, text } => match config.style {
            ResourceMetricStyle::Bar => {
                draw_bar(painter, rect, config, label, value, text, scale, alpha)
            }
            ResourceMetricStyle::Ring => {
                draw_ring(painter, rect, config, label, value, text, scale, alpha)
            }
        },
    }
    painter.restore_to(save_count);
}

#[allow(clippy::too_many_arguments)]
fn draw_bar(
    painter: Painter<'_>,
    rect: Rect,
    config: &ResourceMetricConfig,
    label: &str,
    usage: Option<f32>,
    text: &str,
    scale: f32,
    alpha: u8,
) {
    let value = usage.unwrap_or_default();
    let accent = usage_color(metric_color(config.color), value);
    let inset = 1.0 * scale;
    let left = rect.left + inset;
    let width = (rect.width() - inset * 2.0).max(0.0);
    let baseline = rect.center_y() + scale;
    let label_size = (6.5 * scale).min(rect.width() * 0.22).max(4.5);
    let mut value_size = (8.5 * scale).min(rect.width() * 0.28).max(5.5);
    let track_h = (2.0 * scale).max(1.5);
    let track = Rect::from_xywh(left, baseline + 4.5 * scale, width, track_h);
    painter.fill_round_rect(
        track,
        Radius::uniform(track_h / 2.0),
        Rgba::from_argb((alpha as f32 * 0.13) as u8, 255, 255, 255),
    );
    if usage.is_some() && value > 0.0 {
        let fill_w = (track.width() * value).max(track_h).min(track.width());
        painter.fill_round_rect(
            Rect::from_xywh(track.left, track.top, fill_w, track_h),
            Radius::uniform(track_h / 2.0),
            alpha_color(accent, (alpha as f32 * 0.9) as u8),
        );
    }
    let fonts = FontManager::global();
    let label_w = fonts.measure_text_cached(label, label_size, FontStyle::bold());
    let available_value_width = (width - label_w - 3.0 * scale).max(f32::EPSILON);
    let natural_value_width = fonts.measure_text_cached(text, value_size, FontStyle::bold());
    if natural_value_width > available_value_width {
        value_size *= available_value_width / natural_value_width;
    }
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: label,
        x: left,
        y: baseline,
        size: label_size,
        bold: true,
        color: alpha_color(accent, (alpha as f32 * 0.78) as u8),
        blur: None,
    });
    let value_w = fonts.measure_text_cached(text, value_size, FontStyle::bold());
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text,
        x: rect.right - inset - value_w,
        y: baseline,
        size: value_size,
        bold: true,
        color: Rgba::from_argb(alpha, 255, 255, 255),
        blur: None,
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_ring(
    painter: Painter<'_>,
    rect: Rect,
    config: &ResourceMetricConfig,
    label: &str,
    usage: Option<f32>,
    text: &str,
    scale: f32,
    alpha: u8,
) {
    let value = usage.unwrap_or_default();
    let accent = usage_color(metric_color(config.color), value);
    let fonts = FontManager::global();
    let label_size = (6.0 * scale).max(4.5);
    let label_w = fonts.measure_text_cached(label, label_size, FontStyle::bold());
    let label_gap = RING_LABEL_GAP * scale;
    let diameter = (rect.height() * 0.72)
        .min(rect.width() - label_w - label_gap)
        .max(10.0 * scale);
    let group_left = rect.center_x() - (label_w + label_gap + diameter) / 2.0;
    let center = Point::new(
        group_left + label_w + label_gap + diameter / 2.0,
        rect.center_y(),
    );
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
    let mut value_size = (diameter * 0.22).max(4.0);
    let max_value_width = diameter * 0.78;
    let mut value_w = fonts.measure_text_cached(text, value_size, FontStyle::bold());
    if value_w > max_value_width {
        value_size = (value_size * max_value_width / value_w).max(3.2);
        value_w = fonts.measure_text_cached(text, value_size, FontStyle::bold());
    }
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text,
        x: center.x - value_w / 2.0,
        y: center.y + value_size * 0.32,
        size: value_size,
        bold: true,
        color: Rgba::from_argb(alpha, 255, 255, 255),
        blur: None,
    });
    fonts.draw_text_cached(DrawTextCachedParams {
        painter,
        text: label,
        x: group_left,
        y: rect.center_y() + label_size * 0.34,
        size: label_size,
        bold: true,
        color: alpha_color(accent, (alpha as f32 * 0.8) as u8),
        blur: None,
    });
}

fn draw_rates(
    painter: Painter<'_>,
    rect: Rect,
    config: &ResourceMetricConfig,
    upload: RateUsage<'_>,
    download: RateUsage<'_>,
    scale: f32,
    alpha: u8,
) {
    let accent = alpha_color(metric_color(config.color), (alpha as f32 * 0.92) as u8);
    let fonts = FontManager::global();
    let mut value_size = (6.6 * scale).max(4.5);
    let mut unit_size = value_size * 0.78;
    let arrow_size = value_size * 0.78;
    let arrow_gap = 2.0 * scale;
    let unit_gap = 1.4 * scale;
    let row_width = |value: &str, unit: &str, value_size: f32, unit_size: f32| {
        arrow_size
            + arrow_gap
            + fonts.measure_text_cached(value, value_size, FontStyle::bold())
            + unit_gap
            + fonts.measure_text_cached(unit, unit_size, FontStyle::normal())
    };
    let widest = row_width(upload.value, upload.unit, value_size, unit_size).max(row_width(
        download.value,
        download.unit,
        value_size,
        unit_size,
    ));
    let available = (rect.width() - 2.0 * scale).max(1.0);
    if widest > available {
        value_size *= available / widest;
        unit_size *= available / widest;
    }
    let left = rect.left + scale;
    let line_gap = 2.0 * scale;
    let cap = value_size * 0.72;
    let first_baseline = rect.center_y() - line_gap / 2.0;
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
            color: Rgba::from_argb(alpha, 255, 255, 255),
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
            color: Rgba::from_argb((alpha as f32 * 0.6) as u8, 255, 255, 255),
            blur: None,
        });
    }
}
