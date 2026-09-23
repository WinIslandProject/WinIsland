use std::cell::LazyCell;

use image::imageops::FilterType;
use skia_safe::{AlphaType, Canvas, ColorType, Data, Image, ImageInfo, Paint, Point, Rect, images};

thread_local! {
    static SUN_ICONS: LazyCell<[Option<Image>; 2]> = LazyCell::new(|| [
        load_icon(include_bytes!("../../resources/in_app/icons/sun.min.png")),
        load_icon(include_bytes!("../../resources/in_app/icons/sun.max.png")),
    ]);
}

fn load_icon(bytes: &[u8]) -> Option<Image> {
    let source = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = source.dimensions();
    let mut bounds = (width, height, 0, 0);
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] > 8 {
            bounds.0 = bounds.0.min(x);
            bounds.1 = bounds.1.min(y);
            bounds.2 = bounds.2.max(x);
            bounds.3 = bounds.3.max(y);
        }
    }
    if bounds.0 > bounds.2 || bounds.1 > bounds.3 {
        return None;
    }
    let cropped = image::imageops::crop_imm(
        &source,
        bounds.0,
        bounds.1,
        bounds.2 - bounds.0 + 1,
        bounds.3 - bounds.1 + 1,
    )
    .to_image();
    let small = image::imageops::resize(&cropped, 64, 64, FilterType::Lanczos3);
    let mut pixels = small.into_raw();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[0] = pixel[3];
        pixel[1] = pixel[3];
        pixel[2] = pixel[3];
    }
    let info = ImageInfo::new((64, 64), ColorType::RGBA8888, AlphaType::Premul, None);
    images::raster_from_data(&info, Data::new_copy(&pixels), 64 * 4)
}

pub fn draw_brightness_icon(canvas: &Canvas, center: Point, size: f32, alpha: u8, level: f32) {
    let index = usize::from(level >= 0.5);
    SUN_ICONS.with(|icons| {
        if let Some(icon) = icons[index].as_ref() {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_alpha(alpha);
            canvas.draw_image_rect(
                icon,
                None,
                Rect::from_xywh(center.x - size / 2.0, center.y - size / 2.0, size, size),
                &paint,
            );
        }
    });
}
