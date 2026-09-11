use std::cell::RefCell;

use skia_safe::{
    FilterMode, Image, ImageInfo, MipmapMode, Paint, Rect, SamplingOptions, TileMode,
    gpu::{self, Budgeted, DirectContext, SurfaceOrigin},
    image_filters,
};

use crate::core::smtc::MediaInfo;
use crate::ui::expanded::music_view::get_cached_media_image_with_key;

thread_local! {
    static BLURRED_COVER_CACHE: RefCell<Option<BlurredCoverCache>> = const { RefCell::new(None) };
}

struct BlurredCoverCache {
    cache_key: u64,
    blurred_image: Image,
}

pub fn get_blurred_cover_background(
    direct_context: &mut DirectContext,
    media: &MediaInfo,
) -> Option<Image> {
    if media.title.is_empty() {
        return None;
    }
    let (image, cache_key) = get_cached_media_image_with_key(media)?;

    let cached = BLURRED_COVER_CACHE.with(|cell| {
        let cache = cell.borrow();
        cache
            .as_ref()
            .filter(|entry| entry.cache_key == cache_key)
            .map(|entry| entry.blurred_image.clone())
    });
    if cached.is_some() {
        return cached;
    }

    let info = ImageInfo::new_n32_premul((64, 64), None);
    let mut downscaled_surface = gpu::surfaces::render_target(
        direct_context,
        Budgeted::Yes,
        &info,
        None,
        Some(SurfaceOrigin::TopLeft),
        None,
        Some(false),
        Some(false),
    )?;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    downscaled_surface
        .canvas()
        .draw_image_rect_with_sampling_options(
            &image,
            None,
            Rect::from_xywh(0.0, 0.0, 64.0, 64.0),
            SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
            &paint,
        );
    direct_context.flush_and_submit_surface(&mut downscaled_surface, None);
    let downscaled = downscaled_surface.image_snapshot();

    let mut blur_surface = gpu::surfaces::render_target(
        direct_context,
        Budgeted::Yes,
        &info,
        None,
        Some(SurfaceOrigin::TopLeft),
        None,
        Some(false),
        Some(false),
    )?;
    let mut blur_paint = Paint::default();
    blur_paint.set_anti_alias(true);
    if let Some(filter) = image_filters::blur((8.0, 8.0), Some(TileMode::Clamp), None, None) {
        blur_paint.set_image_filter(filter);
    }
    blur_surface
        .canvas()
        .draw_image(&downscaled, (0, 0), Some(&blur_paint));
    direct_context.flush_and_submit_surface(&mut blur_surface, None);
    let blurred_image = blur_surface.image_snapshot();
    gpu::images::get_backend_texture_from_image(&blurred_image, false)?;

    BLURRED_COVER_CACHE.with(|cell| {
        *cell.borrow_mut() = Some(BlurredCoverCache {
            cache_key,
            blurred_image: blurred_image.clone(),
        });
    });

    Some(blurred_image)
}

pub fn clear_blurred_cover_cache() {
    BLURRED_COVER_CACHE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}
