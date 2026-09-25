use crate::ui::widget::time_text::with_current_time_text;
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{Painter, Rect, Rgba};

pub(super) fn draw(painter: Painter<'_>, rect: Rect, scale: f32, alpha: u8) {
    let size = (rect.height() * 0.43).clamp(10.0 * scale, 12.0 * scale);
    let color = Rgba::from_argb(alpha, 255, 255, 255);
    with_current_time_text(|text| {
        let text_width = FontManager::global().measure_text_cached(
            text,
            size,
            winisland_render::FontStyle::bold(),
        );
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            painter,
            text,
            x: rect.center_x() - text_width / 2.0,
            y: rect.center_y() + size * 0.36,
            size,
            bold: true,
            color,
            blur: None,
        });
    });
}
