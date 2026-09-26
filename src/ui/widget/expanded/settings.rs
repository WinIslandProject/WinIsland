use super::draw_widget_rounded_background;
use crate::icons::settings::draw_settings_icon;
use winisland_render::{Painter, Rgba};

#[allow(clippy::too_many_arguments)]
pub fn draw_settings_widget(
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
    let icon_scale = w.min(h) * 0.38 / 24.0;
    draw_settings_icon(
        painter,
        x + w / 2.0,
        y + h / 2.0,
        (alpha as f32 * 0.72) as u8,
        icon_scale,
        text_color,
    );
}
