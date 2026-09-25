use winisland_render::{Painter, Radius, Rect, Rgba};

pub fn draw_arrow_right(
    painter: Painter<'_>,
    cx: f32,
    cy: f32,
    alpha: u8,
    scale: f32,
    color: Rgba,
) {
    let color = color.with_alpha((alpha as f32 * 0.4) as u8);
    let w = 3.0 * scale;
    let h = 14.0 * scale;
    let rect = Rect::from_xywh(cx - w / 2.0, cy - h / 2.0, w, h);
    painter.fill_round_rect(rect, Radius::uniform(2.0 * scale), color);
}

pub fn draw_arrow_left(painter: Painter<'_>, cx: f32, cy: f32, alpha: u8, scale: f32, color: Rgba) {
    let color = color.with_alpha((alpha as f32 * 0.4) as u8);
    let w = 3.0 * scale;
    let h = 14.0 * scale;
    let rect = Rect::from_xywh(cx - w / 2.0, cy - h / 2.0, w, h);
    painter.fill_round_rect(rect, Radius::uniform(2.0 * scale), color);
}
