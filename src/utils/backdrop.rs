use std::cell::RefCell;

use winisland_render::{Image, TileMode};

use crate::core::smtc::MediaInfo;
use crate::ui::expanded::music_view::get_cached_media_image_with_key;
use winisland_render::DrawingContext;

thread_local! {
    static BLURRED_COVER_CACHE: RefCell<Option<BlurredCoverCache>> = const { RefCell::new(None) };
}

struct BlurredCoverCache {
    cache_key: u64,
    blurred_image: Image,
}

pub fn get_blurred_cover_background(
    drawing_context: &mut DrawingContext<'_>,
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

    let downscaled = drawing_context.scale_image(&image, 64, 64)?;
    let blurred_image =
        drawing_context.blur_image(&downscaled, 64, 64, (8.0, 8.0), Some(TileMode::Clamp))?;

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
