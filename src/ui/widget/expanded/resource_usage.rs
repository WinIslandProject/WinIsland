use skia_safe::{Canvas, Color, Paint, Rect};

use super::{draw_widget_rounded_background, draw_widget_text_centered};
use crate::ui::widget::resource_usage::{
    CPU_COLOR, RAM_COLOR, alpha_color, usage_color, with_resource_usage,
};

#[allow(clippy::too_many_arguments)]
fn draw_metric(
    canvas: &Canvas,
    bounds: Rect,
    label: &str,
    value: Option<f32>,
    value_text: &str,
    base_color: Color,
    scale: f32,
    alpha: u8,
    text_color: Color,
) {
    let center_x = bounds.center_x();
    let center_y = bounds.top() + bounds.height() * 0.43;
    let diameter = (bounds.height() * 0.58)
        .min(bounds.width() * 0.62)
        .max(20.0 * scale);
    let ring = Rect::from_xywh(
        center_x - diameter / 2.0,
        center_y - diameter / 2.0,
        diameter,
        diameter,
    );
    let usage = value.unwrap_or(0.0);
    let accent = usage_color(base_color, usage);

    let mut glow = Paint::default();
    glow.set_anti_alias(true);
    glow.set_color(alpha_color(accent, (alpha as f32 * 0.07) as u8));
    canvas.draw_circle((center_x, center_y), diameter * 0.57, &glow);

    let mut ring_paint = Paint::default();
    ring_paint.set_anti_alias(true);
    ring_paint.set_style(skia_safe::paint::Style::Stroke);
    ring_paint.set_stroke_width((3.0 * scale).min(diameter * 0.12));
    ring_paint.set_stroke_cap(skia_safe::paint::Cap::Round);
    ring_paint.set_color(alpha_color(text_color, (alpha as f32 * 0.13) as u8));
    canvas.draw_circle((center_x, center_y), diameter / 2.0, &ring_paint);

    if value.is_some() && usage > 0.0 {
        ring_paint.set_color(alpha_color(accent, (alpha as f32 * 0.92) as u8));
        canvas.draw_arc(ring, -90.0, usage * 360.0, false, &ring_paint);
    }

    let mut value_paint = Paint::default();
    value_paint.set_anti_alias(true);
    value_paint.set_color(alpha_color(text_color, alpha));
    draw_widget_text_centered(
        canvas,
        value_text,
        Rect::from_xywh(
            center_x - diameter * 0.42,
            center_y - diameter * 0.24,
            diameter * 0.84,
            diameter * 0.48,
        ),
        (diameter * 0.27).clamp(7.0 * scale, 10.5 * scale),
        true,
        &value_paint,
    );

    let mut label_paint = Paint::default();
    label_paint.set_anti_alias(true);
    label_paint.set_color(alpha_color(text_color, (alpha as f32 * 0.58) as u8));
    draw_widget_text_centered(
        canvas,
        label,
        Rect::from_xywh(
            bounds.left(),
            bounds.top() + bounds.height() * 0.76,
            bounds.width(),
            bounds.height() * 0.16,
        ),
        (bounds.height() * 0.12).clamp(6.0 * scale, 8.0 * scale),
        true,
        &label_paint,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_resource_usage(
    canvas: &Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Color,
    cpu: Option<f32>,
    ram: Option<f32>,
    cpu_text: &str,
    ram_text: &str,
) {
    draw_widget_rounded_background(canvas, x, y, w, h, scale, alpha);

    let mut divider = Paint::default();
    divider.set_anti_alias(true);
    divider.set_color(alpha_color(text_color, (alpha as f32 * 0.09) as u8));
    divider.set_stroke_width(scale.max(0.75));
    canvas.draw_line(
        (x + w / 2.0, y + h * 0.2),
        (x + w / 2.0, y + h * 0.8),
        &divider,
    );

    let inset = 3.0 * scale;
    let metric_w = (w - inset * 2.0) / 2.0;
    draw_metric(
        canvas,
        Rect::from_xywh(x + inset, y, metric_w, h),
        "CPU",
        cpu,
        cpu_text,
        CPU_COLOR,
        scale,
        alpha,
        text_color,
    );
    draw_metric(
        canvas,
        Rect::from_xywh(x + inset + metric_w, y, metric_w, h),
        "RAM",
        ram,
        ram_text,
        RAM_COLOR,
        scale,
        alpha,
        text_color,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn draw_resource_usage_widget(
    canvas: &Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Color,
) {
    with_resource_usage(|usage| {
        draw_resource_usage(
            canvas,
            x,
            y,
            w,
            h,
            scale,
            alpha,
            text_color,
            usage.cpu,
            usage.ram,
            usage.cpu_text,
            usage.ram_text,
        );
    });
}

#[allow(clippy::too_many_arguments)]
pub fn draw_resource_usage_preview(
    canvas: &Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Color,
) {
    draw_resource_usage(
        canvas,
        x,
        y,
        w,
        h,
        scale,
        alpha,
        text_color,
        Some(0.37),
        Some(0.62),
        "37%",
        "62%",
    );
}
