use super::{draw_widget_rounded_background, draw_widget_text_centered};
use crate::ui::widget::time_text::with_current_time_text;
use winisland_render::{Painter, Rect, Rgba};

#[allow(clippy::too_many_arguments)]
pub fn draw_time_widget(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    draw_widget_rounded_background(painter, x, y, w, h, scale, alpha);

    let size = (h * 0.60).min(w * 0.31).max(13.0 * scale);
    let color = text_color.with_alpha(alpha);

    with_current_time_text(|text| {
        draw_widget_text_centered(
            painter,
            text,
            Rect::from_xywh(x, y, w, h),
            size,
            true,
            color,
        );
    });
}
