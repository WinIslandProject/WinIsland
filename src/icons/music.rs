use winisland_render::{Painter, Path, Rgba, Vec2};

const MUSIC_PATH: &str = "M812.6 96.4c-9.4 1.8-404 78.4-412.4 80-8.4 1.6-16.2 7.2-16.2 16v480.2c0 3.2-0.2 14.4-4.8 23.4-6.2 11.8-17 20.4-32.2 25.4-6.6 2.2-15.6 4.2-26.2 6.6-48.2 10.8-128.8 29.2-128.8 103.6 0 62.2 44.8 90.2 83.4 95 4.2 0.6 9 1.4 14.2 1.4 13.4 0 72-6.6 102.4-26.4 22-14.4 48.2-42.8 48.2-95.6V381c0-7.6 5.4-14.2 12.8-15.6l304-61.4c10-2 19.2 5.6 19.2 15.6v261.8c0 8.2-0.4 17.8-5 26.8-6.2 11.8-17 20.4-32.4 25.4-6.6 2.2-17.6 4.2-28.2 6.6-48.2 10.8-128.8 29-128.8 103.4 0 67.4 50.8 94.4 83.6 96.6 13 0.8 22.4 0.6 38.8-1.8s47-11 73-26c35.8-20.6 55-53.6 55-96.4V111.8c-0.2-8.8-7.6-17.8-19.6-15.4z";

pub fn draw_music_icon(painter: Painter<'_>, cx: f32, cy: f32, alpha: u8, scale: f32, color: Rgba) {
    let Some(path) = Path::from_svg(MUSIC_PATH) else {
        return;
    };
    painter.save();
    painter.translate(Vec2::new(cx, cy));
    let s = 0.024 * scale;
    painter.scale(Vec2::new(s, s));
    painter.translate(Vec2::new(-512.0 + 15.0, -512.0 + 35.0));
    painter.fill_path(&path, color.with_alpha(alpha));
    painter.restore();
}
