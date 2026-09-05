use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use skia_safe::{
    AlphaType, Color, ColorType, FilterMode, ISize, Image, ImageInfo, MipmapMode, Paint, Rect,
    SamplingOptions,
    gpu::{Budgeted, DirectContext, SurfaceOrigin, SyncCpu, surfaces},
};

const PALETTE_SAMPLE_SIZE: i32 = 8;

thread_local! {
    static COLOR_CACHE: RefCell<HashMap<u64, Arc<[Color]>>> = RefCell::new(HashMap::new());
}

pub(super) fn get_palette_from_image(
    direct_context: &mut DirectContext,
    img: &Image,
    cache_key: u64,
) -> Arc<[Color]> {
    COLOR_CACHE.with(|cache| {
        let mut cache_mut = cache.borrow_mut();
        if cache_mut.len() > 50
            && let Some(oldest_key) = cache_mut.keys().next().cloned()
        {
            cache_mut.remove(&oldest_key);
        }
        if let Some(palette) = cache_mut.get(&cache_key) {
            return palette.clone();
        }
        let mut palette = Vec::with_capacity(3);
        if let Some((r_avg, g_avg, b_avg)) = average_image_color(direct_context, img) {
            let brighten = |r: f32, g: f32, b: f32, factor: f32| -> Color {
                let mut r = r * factor;
                let mut g = g * factor;
                let mut b = b * factor;

                let brightness = r * 0.299 + g * 0.587 + b * 0.114;
                if brightness < 80.0 {
                    let boost = 80.0 - brightness;
                    r += boost;
                    g += boost;
                    b += boost;
                }

                Color::from_rgb(r.min(255.0) as u8, g.min(255.0) as u8, b.min(255.0) as u8)
            };

            let primary = brighten(r_avg, g_avg, b_avg, 1.3);
            let secondary = brighten(r_avg, g_avg, b_avg, 1.5);

            palette.push(primary);
            palette.push(secondary);
            palette.push(primary);
        }
        if palette.is_empty() {
            palette.push(Color::from_rgb(200, 200, 200));
        }
        let palette: Arc<[Color]> = Arc::from(palette);
        cache_mut.insert(cache_key, palette.clone());
        palette
    })
}

fn average_image_color(
    direct_context: &mut DirectContext,
    image: &Image,
) -> Option<(f32, f32, f32)> {
    if image.width() <= 0 || image.height() <= 0 {
        return None;
    }
    let info = ImageInfo::new(
        ISize::new(PALETTE_SAMPLE_SIZE, PALETTE_SAMPLE_SIZE),
        ColorType::BGRA8888,
        AlphaType::Premul,
        None,
    );
    let mut surface = surfaces::render_target(
        direct_context,
        Budgeted::Yes,
        &info,
        None,
        Some(SurfaceOrigin::TopLeft),
        None,
        Some(false),
        Some(false),
    )?;
    let paint = Paint::default();
    surface.canvas().draw_image_rect_with_sampling_options(
        image,
        None,
        Rect::from_wh(PALETTE_SAMPLE_SIZE as f32, PALETTE_SAMPLE_SIZE as f32),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        &paint,
    );
    direct_context.flush_and_submit_surface(&mut surface, Some(SyncCpu::Yes));

    let mut pixels = [0u8; (PALETTE_SAMPLE_SIZE as usize).pow(2) * 4];
    if !surface.read_pixels(&info, &mut pixels, info.min_row_bytes(), (0, 0)) {
        return None;
    }
    let mut red = 0u32;
    let mut green = 0u32;
    let mut blue = 0u32;
    for pixel in pixels.as_chunks::<4>().0 {
        blue += pixel[0] as u32;
        green += pixel[1] as u32;
        red += pixel[2] as u32;
    }
    let count = (pixels.len() / 4) as f32;
    Some((
        red as f32 / count,
        green as f32 / count,
        blue as f32 / count,
    ))
}
