use super::{draw_widget_rounded_background, draw_widget_text_centered};
use crate::ui::widget::time_text::with_current_time_text;
use crate::utils::font::FontManager;
use skia_safe::{Canvas, Color, Paint, Rect};

#[allow(clippy::too_many_arguments)]
pub fn draw_time_widget(
    canvas: &Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Color,
) {
    draw_widget_rounded_background(canvas, x, y, w, h, scale, alpha);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb(
        alpha,
        text_color.r(),
        text_color.g(),
        text_color.b(),
    ));

    with_current_time_text(|text| {
        let max_w = (w - 14.0 * scale).max(0.0);
        let max_h = h * 0.60;
        let mut size = (max_h).min(w * 0.31).max(13.0 * scale);
        let text_width =
            FontManager::global().measure_text_cached(text, size, skia_safe::FontStyle::bold());
        if text_width > max_w && text_width > 0.0 {
            size = (size * (max_w / text_width)).max(10.0 * scale);
        }
        draw_widget_text_centered(
            canvas,
            text,
            Rect::from_xywh(x, y, w, h),
            size,
            true,
            &paint,
        );
    });
}
