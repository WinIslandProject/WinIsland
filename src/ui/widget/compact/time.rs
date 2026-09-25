use skia_safe::{Canvas, Color, Paint, Rect};

use crate::ui::widget::time_text::with_current_time_text;
use winisland_render::Painter;
use winisland_render::text::{DrawTextCachedParams, FontManager};

pub(super) fn draw(canvas: &Canvas, rect: Rect, scale: f32, alpha: u8) {
    let size = (rect.height() * 0.43).clamp(10.0 * scale, 12.0 * scale);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb(alpha, 255, 255, 255));
    with_current_time_text(|text| {
        let text_width = FontManager::global().measure_text_cached(
            text,
            size,
            winisland_render::FontStyle::bold(),
        );
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            painter: Painter::from_canvas(canvas),
            text,
            x: rect.center_x() - text_width / 2.0,
            y: rect.center_y() + size * 0.36,
            size,
            bold: true,
            paint: &paint,
        });
    });
}
