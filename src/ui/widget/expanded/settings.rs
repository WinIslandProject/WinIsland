use super::draw_widget_rounded_background;
use crate::icons::settings::draw_settings_icon;
use crate::utils::color::rgba;
use skia_safe::{Canvas, Color};
use winisland_render::Painter;

#[allow(clippy::too_many_arguments)]
pub fn draw_settings_widget(
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
    let icon_scale = w.min(h) * 0.38 / 24.0;
    draw_settings_icon(
        Painter::from_canvas(canvas),
        x + w / 2.0,
        y + h / 2.0,
        (alpha as f32 * 0.72) as u8,
        icon_scale,
        rgba(text_color),
    );
}
