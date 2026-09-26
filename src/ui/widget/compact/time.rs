use skia_safe::{Canvas, Color, Paint, Rect};

use crate::ui::widget::time_text::with_current_time_text;
use crate::utils::font::{DrawTextCachedParams, FontManager};

pub(super) fn draw(canvas: &Canvas, rect: Rect, scale: f32, alpha: u8) {
    let mut size = (rect.height() * 0.43).clamp(10.0 * scale, 12.0 * scale);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb(alpha, 255, 255, 255));
    with_current_time_text(|text| {
        let max_w = (rect.width() - 4.0 * scale).max(0.0);
        let mut text_width =
            FontManager::global().measure_text_cached(text, size, skia_safe::FontStyle::bold());
        if text_width > max_w && text_width > 0.0 {
            size = (size * (max_w / text_width)).max(8.0 * scale);
            text_width =
                FontManager::global().measure_text_cached(text, size, skia_safe::FontStyle::bold());
        }
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            canvas,
            text,
            x: rect.center_x() - text_width / 2.0,
            y: rect.center_y() + size * 0.36,
            size,
            bold: true,
            paint: &paint,
        });
    });
}
