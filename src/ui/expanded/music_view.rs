#![allow(deprecated)]
mod controls;
mod palette;
mod visualizer;

pub use controls::{
    get_cover_rect, get_next_btn_rect, get_pause_btn_rect, get_prev_btn_rect,
    get_progress_bar_rect, set_progress_dragging, set_progress_hover, snap_progress,
    trigger_cover_flip, trigger_next_click, trigger_pause_click, trigger_prev_click,
};
pub use visualizer::{DrawVisualizerParams, draw_visualizer};

use self::controls::ease_out_back;
use self::palette::get_palette_from_image;
use crate::core::smtc::MediaInfo;
use crate::icons::arrows::draw_arrow_right;
use crate::icons::controls::{draw_control_triangle, draw_pause_button, draw_play_button};
use crate::utils::cover::decode_cover_image;
use crate::utils::scroll::{ScrollDrawParams, ScrollText};
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{
    BlurSpec, FontStyle, Image, ImageOptions, LayerSpec, Painter, Path, Radius, Rect, Rgba,
    Sampling, Vec2,
};

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};
use winisland_core::physics::Spring;

const CONTENT_PADDING: f32 = 24.0;
const PAGE_ARROW_RIGHT_INSET: f32 = 7.5;
const PAGE_ARROW_FADE_RATE: f32 = 5.0;
const COVER_SIZE: f32 = 64.0;
const TRACK_TEXT_GAP: f32 = 18.0;
const TRACK_TEXT_RIGHT_INSET: f32 = 70.0;
const TRACK_TITLE_BASELINE_OFFSET: f32 = 26.0;
const PROGRESS_TOP_GAP: f32 = 24.0;
const PROGRESS_TIME_FONT_SCALE: f32 = 0.67;
const DEFAULT_PROGRESS_TIME_FONT_SIZE: f32 = 10.0;
const PROGRESS_TIME_WIDTH: f32 = 28.0;
const PROGRESS_TIME_GAP: f32 = 4.0;
const PROGRESS_START_THRESHOLD: f32 = 0.02;
const PROGRESS_JUMP_THRESHOLD: f32 = 0.3;
const PROGRESS_SMOOTHING: f32 = 0.15;
const PROGRESS_HOVER_SMOOTHING: f32 = 0.18;
const PROGRESS_HOVER_SNAP_THRESHOLD: f32 = 0.005;
const PROGRESS_BAR_HEIGHT: f32 = 6.5;
const PROGRESS_BAR_HOVER_GROWTH: f32 = 3.5;
const PROGRESS_FILL_IDLE_BRIGHTNESS: f32 = 0.72;
const PROGRESS_FILL_BRIGHTEN_DELAY: f32 = 0.18;
const PROGRESS_TIME_BASELINE_SCALE: f32 = 0.35;
const PROGRESS_TIME_IDLE_ALPHA: f32 = 0.5;
const PROGRESS_TRACK_ALPHA: f32 = 0.12;
const PLAYBACK_CONTROLS_TOP_GAP: f32 = 42.0;
const SKIP_BUTTON_GAP: f32 = 75.0;
const SKIP_ANIMATION_DURATION_SECS: f32 = 0.5;
const PLAY_STATE_RESPONSE: f32 = 0.18;
pub(super) const PAUSE_CONTROL_PRESS_VELOCITY: f32 = -0.18;
const PAUSE_CONTROL_STIFFNESS: f32 = 0.18;
const PAUSE_CONTROL_DAMPING: f32 = 0.64;
const PAUSE_CONTROL_MIN_SCALE: f32 = 0.8;
const PAUSE_CONTROL_MAX_SCALE: f32 = 1.03;
const PAUSE_CONTROL_BLUR_SCALE: f32 = 16.0;
const PAUSE_CONTROL_MAX_BLUR: f32 = 2.5;
const COLLAPSED_VISUALIZER_INSET: f32 = 17.0;
const EXPANDED_VISUALIZER_INSET: f32 = 37.0;
const VISUALIZER_TITLE_OFFSET: f32 = 4.0;
const VISUALIZER_SMOOTHING: (f32, f32) = (0.6, 0.08);

struct ProgressTextCache {
    elapsed_secs: u32,
    remaining_secs: Option<u32>,
    remaining_initialized: bool,
    elapsed_text: String,
    remaining_text: String,
}

thread_local! {
    static IMG_CACHE: RefCell<Option<(u64, Option<Image>)>> = const { RefCell::new(None) };
    static PROGRESS_SMOOTH: RefCell<f32> = const { RefCell::new(-1.0) };
    static PAUSE_ANIM: RefCell<f32> = const { RefCell::new(0.0) };
    static PAUSE_SPRING: RefCell<Spring> = RefCell::new(Spring::new(1.0));
    static PREV_SKIP_ANIM: RefCell<Option<std::time::Instant>> = const { RefCell::new(None) };
    static NEXT_SKIP_ANIM: RefCell<Option<std::time::Instant>> = const { RefCell::new(None) };
    static LOCAL_PLAY_STATE: RefCell<Option<(bool, std::time::Instant)>> = const { RefCell::new(None) };
    static TITLE_SCROLL: RefCell<ScrollText> = RefCell::new(ScrollText::new());
    static ARTIST_SCROLL: RefCell<ScrollText> = RefCell::new(ScrollText::new());
    static COVER_FLIP_ANIM: RefCell<Option<std::time::Instant>> = const { RefCell::new(None) };
    static COVER_FLIP_OLD_IMG: RefCell<Option<Image>> = const { RefCell::new(None) };
    static PROGRESS_HOVER: RefCell<(bool, f32)> = const { RefCell::new((false, 0.0)) };
    static PROGRESS_DRAGGING: RefCell<bool> = const { RefCell::new(false) };
    static COVER_ROTATION: RefCell<f32> = const { RefCell::new(0.0) };
    static PROGRESS_TEXT_CACHE: RefCell<ProgressTextCache> = const { RefCell::new(ProgressTextCache {
        elapsed_secs: u32::MAX,
        remaining_secs: None,
        remaining_initialized: false,
        elapsed_text: String::new(),
        remaining_text: String::new(),
    }) };
}

pub fn draw_text_cached(params: DrawTextCachedParams<'_>) {
    FontManager::global().draw_text_cached(params);
}

pub fn get_cached_media_image(media: &MediaInfo) -> Option<Image> {
    get_cached_media_image_with_key(media).map(|(img, _)| img)
}

fn media_image_key(media: &MediaInfo) -> u64 {
    if media.thumbnail_hash != 0 {
        return media.thumbnail_hash;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    media.title.hash(&mut hasher);
    media.album.hash(&mut hasher);
    hasher.finish()
}

pub fn get_cached_media_image_with_key(media: &MediaInfo) -> Option<(Image, u64)> {
    if media.title.is_empty() {
        clear_cover_cache();
        return None;
    }
    let cache_key = media_image_key(media);

    let mut result: Option<(Image, u64)> = None;
    let mut has_current_image = false;
    IMG_CACHE.with(|cache| {
        let mut cache_mut = cache.borrow_mut();
        if let Some((key, img)) = cache_mut.as_ref()
            && *key == cache_key
        {
            result = img.clone().map(|image| (image, *key));
            has_current_image = true;
            return;
        }
        if let Some(data) = media.thumbnail.as_ref() {
            let image = decode_cover_image(data.as_ref());
            *cache_mut = Some((cache_key, image.clone()));
            result = image.map(|image| (image, cache_key));
            has_current_image = true;
        }
    });
    if has_current_image {
        let flip_finished = COVER_FLIP_ANIM.with(|cell| {
            cell.borrow()
                .is_none_or(|started| started.elapsed().as_secs_f32() >= 0.6)
        });
        if flip_finished {
            COVER_FLIP_OLD_IMG.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
    if result.is_none() {
        COVER_FLIP_OLD_IMG.with(|cell| {
            if let Some(old_img) = cell.borrow().as_ref() {
                result = Some((old_img.clone(), old_img.unique_id() as u64));
            }
        });
    }
    result
}

pub fn get_media_palette(media: &MediaInfo) -> Arc<[Rgba]> {
    if let Some((img, cache_key)) = get_cached_media_image_with_key(media) {
        get_palette_from_image(&img, cache_key)
    } else {
        default_media_palette()
    }
}

pub fn default_media_palette() -> Arc<[Rgba]> {
    static DEFAULT_PALETTE: OnceLock<Arc<[Rgba]>> = OnceLock::new();
    DEFAULT_PALETTE
        .get_or_init(|| Arc::from([Rgba::from_rgb(180, 180, 180), Rgba::from_rgb(100, 100, 100)]))
        .clone()
}

pub fn clear_cover_cache() {
    IMG_CACHE.with(|cell| {
        *cell.borrow_mut() = None;
    });
    COVER_FLIP_OLD_IMG.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub struct DrawMusicPageParams<'a> {
    pub painter: Painter<'a>,
    pub ox: f32,
    pub oy: f32,
    pub w: f32,
    pub h: f32,
    pub alpha: u8,
    pub media: &'a MediaInfo,
    pub music_active: bool,
    pub available_controls: u32,
    pub view_offset: f32,
    pub scale: f32,
    pub expansion_progress: f32,
    pub viz_h_scale: f32,
    pub use_blur: bool,
    pub font_size: f32,
    pub dt: f32,
    pub text_color: Rgba,
    pub text_color_sec: Rgba,
    pub palette: &'a [Rgba],
}

pub fn draw_music_page(params: DrawMusicPageParams<'_>) {
    let DrawMusicPageParams {
        painter,
        ox,
        oy,
        w,
        h,
        alpha,
        media,
        music_active,
        available_controls,
        view_offset,
        scale,
        expansion_progress,
        viz_h_scale,
        use_blur,
        font_size,
        dt,
        text_color,
        text_color_sec,
        palette,
    } = params;

    let arrow_alpha =
        (alpha as f32 * (1.0 - view_offset * PAGE_ARROW_FADE_RATE).clamp(0.0, 1.0)) as u8;
    if arrow_alpha > 0 {
        draw_arrow_right(
            painter,
            ox + w - PAGE_ARROW_RIGHT_INSET * scale,
            oy + h / 2.0,
            arrow_alpha,
            scale,
            text_color,
        );
    }
    let base_img_size = COVER_SIZE * scale;
    let (img_size, img_x, img_y) = (
        base_img_size,
        ox + CONTENT_PADDING * scale,
        oy + CONTENT_PADDING * scale,
    );
    let pause_t = draw_cover(CoverParams {
        painter,
        media,
        music_active,
        img_x,
        img_y,
        img_size,
        alpha,
        scale,
        use_blur,
        dt,
        text_color,
    });

    let text_x = img_x + img_size + TRACK_TEXT_GAP * scale;
    let max_text_w = w - (text_x - ox) - TRACK_TEXT_RIGHT_INSET * scale;
    let title_y = img_y + TRACK_TITLE_BASELINE_OFFSET * scale;
    draw_track_text(TrackTextParams {
        painter,
        media,
        music_active,
        text_x,
        max_text_w,
        title_y,
        alpha,
        font_size,
        scale,
        text_color,
        text_color_sec,
    });

    if music_active {
        let bar_y = img_y + img_size + PROGRESS_TOP_GAP * scale;
        let time_font_size = if font_size > 0.0 {
            font_size * PROGRESS_TIME_FONT_SCALE * scale
        } else {
            DEFAULT_PROGRESS_TIME_FONT_SIZE * scale
        };
        let time_w = PROGRESS_TIME_WIDTH * scale;

        let current_pos_ms = if media.is_playing {
            media
                .position_ms
                .saturating_add(media.last_update.elapsed().as_millis() as u64)
        } else {
            media.position_ms
        };
        let duration_ms = media.effective_duration_ms();
        let current_pos_ms = if duration_ms > 0 {
            current_pos_ms.min(duration_ms)
        } else {
            current_pos_ms
        };
        let raw_progress = if duration_ms > 0 {
            current_pos_ms as f32 / duration_ms as f32
        } else {
            0.0
        };

        let progress = PROGRESS_SMOOTH.with(|cell| {
            let mut smooth = cell.borrow_mut();
            let dragging = PROGRESS_DRAGGING.with(|d| *d.borrow());
            if current_pos_ms < 1000
                || raw_progress <= 0.0
                || dragging
                || *smooth < 0.0
                || (*smooth < PROGRESS_START_THRESHOLD && raw_progress > PROGRESS_START_THRESHOLD)
            {
                *smooth = raw_progress;
            } else {
                let diff = (raw_progress - *smooth).abs();
                if diff > PROGRESS_JUMP_THRESHOLD {
                    *smooth = raw_progress;
                } else {
                    *smooth += (raw_progress - *smooth) * PROGRESS_SMOOTHING;
                }
            }
            *smooth
        });

        let elapsed_secs = (current_pos_ms / 1000) as u32;
        let remaining_secs = if duration_ms > 0 {
            Some((duration_ms.saturating_sub(current_pos_ms) / 1000) as u32)
        } else {
            None
        };

        let bar_full_left = ox + CONTENT_PADDING * scale;
        let bar_full_right = ox + w - CONTENT_PADDING * scale;

        let bar_left = bar_full_left + time_w + PROGRESS_TIME_GAP * scale;
        let bar_right = bar_full_right - time_w - PROGRESS_TIME_GAP * scale;
        let bar_total_w = bar_right - bar_left;

        let hover_t = PROGRESS_HOVER.with(|cell| {
            let mut state = cell.borrow_mut();
            let target = if state.0 { 1.0_f32 } else { 0.0 };
            state.1 += (target - state.1) * PROGRESS_HOVER_SMOOTHING;
            if (state.1 - target).abs() < PROGRESS_HOVER_SNAP_THRESHOLD {
                state.1 = target;
            }
            state.1
        });

        let bar_h = (PROGRESS_BAR_HEIGHT + PROGRESS_BAR_HOVER_GROWTH * hover_t) * scale;
        let bar_center_y = bar_y;
        let bar_radius = bar_h / 2.0;

        let text_baseline_y = bar_center_y + time_font_size * PROGRESS_TIME_BASELINE_SCALE;

        let time_alpha_factor =
            PROGRESS_TIME_IDLE_ALPHA + (1.0 - PROGRESS_TIME_IDLE_ALPHA) * hover_t;
        let time_color = text_color.with_alpha((alpha as f32 * time_alpha_factor) as u8);

        PROGRESS_TEXT_CACHE.with(|cell| {
            let mut cache = cell.borrow_mut();
            if cache.elapsed_secs != elapsed_secs {
                cache.elapsed_secs = elapsed_secs;
                cache.elapsed_text = format!("{}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
            }
            if !cache.remaining_initialized || cache.remaining_secs != remaining_secs {
                cache.remaining_initialized = true;
                cache.remaining_secs = remaining_secs;
                cache.remaining_text = remaining_secs.map_or_else(
                    || "--:--".to_string(),
                    |secs| format!("-{}:{:02}", secs / 60, secs % 60),
                );
            }

            draw_text_cached(DrawTextCachedParams {
                painter,
                text: &cache.elapsed_text,
                x: bar_full_left,
                y: text_baseline_y,
                size: time_font_size,
                bold: false,
                color: time_color,
                blur: None,
            });

            let remaining_w = FontManager::global().measure_text_cached(
                &cache.remaining_text,
                time_font_size,
                FontStyle::normal(),
            );
            draw_text_cached(DrawTextCachedParams {
                painter,
                text: &cache.remaining_text,
                x: bar_full_right - remaining_w,
                y: text_baseline_y,
                size: time_font_size,
                bold: false,
                color: time_color,
                blur: None,
            });
        });

        let track_color = text_color.with_alpha((alpha as f32 * PROGRESS_TRACK_ALPHA) as u8);
        let track_rect = Rect::from_xywh(bar_left, bar_center_y - bar_h / 2.0, bar_total_w, bar_h);
        painter.fill_round_rect(track_rect, Radius::uniform(bar_radius), track_color);

        let filled_w = (bar_total_w * progress.clamp(0.0, 1.0)).min(bar_total_w);
        let fill_hover_t = ((hover_t - PROGRESS_FILL_BRIGHTEN_DELAY)
            / (1.0 - PROGRESS_FILL_BRIGHTEN_DELAY))
            .clamp(0.0, 1.0);
        let fill_hover_t = fill_hover_t * fill_hover_t * (3.0 - 2.0 * fill_hover_t);
        let fill_brightness =
            PROGRESS_FILL_IDLE_BRIGHTNESS + (1.0 - PROGRESS_FILL_IDLE_BRIGHTNESS) * fill_hover_t;
        let fill_color = Rgba::from_argb(
            alpha,
            (text_color.r() as f32 * fill_brightness).round() as u8,
            (text_color.g() as f32 * fill_brightness).round() as u8,
            (text_color.b() as f32 * fill_brightness).round() as u8,
        );
        if filled_w > 0.0 {
            let fill_rect = Rect::from_xywh(bar_left, bar_center_y - bar_h / 2.0, filled_w, bar_h);
            let fill_radius = bar_radius.min(filled_w / 2.0);
            let radius = Radius {
                top_left: Vec2::new(fill_radius, fill_radius),
                top_right: Vec2::new(0.0, 0.0),
                bottom_right: Vec2::new(0.0, 0.0),
                bottom_left: Vec2::new(fill_radius, fill_radius),
            };
            painter.save();
            painter.clip_round_rect(track_rect, Radius::uniform(bar_radius));
            painter.fill_round_rect(fill_rect, radius, fill_color);
            painter.restore();
        }

        let btn_cx = ox + w / 2.0;
        let btn_cy = bar_center_y + bar_h / 2.0 + PLAYBACK_CONTROLS_TOP_GAP * scale;
        let skip_gap = SKIP_BUTTON_GAP * scale;

        let prev_t = PREV_SKIP_ANIM.with(|cell| {
            let start = *cell.borrow();
            match start {
                Some(s) => {
                    let t = s.elapsed().as_secs_f32() / SKIP_ANIMATION_DURATION_SECS;
                    if t >= 1.0 {
                        *cell.borrow_mut() = None;
                        return None;
                    }
                    Some(t)
                }
                None => None,
            }
        });

        if available_controls & winisland_plugin_api::types::v2::context::MEDIA_CONTROL_PREVIOUS
            != 0
        {
            draw_skip_button(
                painter,
                btn_cx - skip_gap,
                btn_cy,
                true,
                prev_t,
                alpha,
                scale,
                use_blur,
                text_color,
            );
        }

        if available_controls & winisland_plugin_api::types::v2::context::MEDIA_CONTROL_TOGGLE_PLAY
            != 0
        {
            draw_pause_control(
                painter, btn_cx, btn_cy, pause_t, alpha, scale, use_blur, dt, text_color,
            );
        }

        let next_t = NEXT_SKIP_ANIM.with(|cell| {
            let start = *cell.borrow();
            match start {
                Some(s) => {
                    let t = s.elapsed().as_secs_f32() / SKIP_ANIMATION_DURATION_SECS;
                    if t >= 1.0 {
                        *cell.borrow_mut() = None;
                        return None;
                    }
                    Some(t)
                }
                None => None,
            }
        });

        if available_controls & winisland_plugin_api::types::v2::context::MEDIA_CONTROL_NEXT != 0 {
            draw_skip_button(
                painter,
                btn_cx + skip_gap,
                btn_cy,
                false,
                next_t,
                alpha,
                scale,
                use_blur,
                text_color,
            );
        }
    }

    let viz_x_offset = COLLAPSED_VISUALIZER_INSET
        + (EXPANDED_VISUALIZER_INSET - COLLAPSED_VISUALIZER_INSET) * expansion_progress;
    draw_visualizer(DrawVisualizerParams {
        painter,
        x: ox + w - viz_x_offset * scale,
        y: title_y - VISUALIZER_TITLE_OFFSET * scale,
        alpha,
        is_playing: music_active && media.is_playing,
        palette,
        spectrum: &media.spectrum,
        w_scale: scale,
        h_scale: viz_h_scale,
        smooth_factors: VISUALIZER_SMOOTHING,
    });
}

struct CoverParams<'a> {
    painter: Painter<'a>,
    media: &'a MediaInfo,
    music_active: bool,
    img_x: f32,
    img_y: f32,
    img_size: f32,
    alpha: u8,
    scale: f32,
    use_blur: bool,
    dt: f32,
    text_color: Rgba,
}

fn draw_cover(params: CoverParams) -> f32 {
    let CoverParams {
        painter,
        media,
        music_active,
        img_x,
        img_y,
        img_size,
        alpha,
        scale,
        use_blur,
        dt,
        text_color,
    } = params;

    let image_to_draw = if music_active {
        get_cached_media_image(media)
    } else {
        None
    };

    let mut effective_is_playing = media.is_playing;
    LOCAL_PLAY_STATE.with(|cell| {
        let mut opt = cell.borrow_mut();
        if let Some((opt_val, time)) = *opt {
            if media.is_playing == opt_val || time.elapsed().as_millis() > 2000 {
                *opt = None;
            } else {
                effective_is_playing = opt_val;
            }
        }
    });

    let pause_t = PAUSE_ANIM.with(|cell| {
        let mut v = cell.borrow_mut();
        let target = if effective_is_playing { 1.0_f32 } else { 0.0 };
        let factor = 1.0 - (1.0 - PLAY_STATE_RESPONSE).powf(dt);
        *v += (target - *v) * factor;
        if (*v - target).abs() < 0.003 {
            *v = target;
        }
        *v
    });

    let cover_scale = 0.85 + 0.15 * pause_t;
    let cover_brightness = 0.75 + 0.25 * pause_t;

    let (flip_scale_x, flip_blur_sigma, flip_use_old) = COVER_FLIP_ANIM.with(|cell| {
        let start = *cell.borrow();
        match start {
            Some(s) => {
                let t = (s.elapsed().as_secs_f32() / 0.6).min(1.0);
                if t >= 1.0 {
                    *cell.borrow_mut() = None;
                    (1.0_f32, 0.0_f32, false)
                } else {
                    let eased = if t < 0.5 {
                        let t2 = t * 2.0;
                        t2 * t2 * 0.5
                    } else {
                        let t2 = (t - 0.5) * 2.0;
                        let c1 = 1.2_f32;
                        0.5 + (1.0 + c1 * (t2 - 1.0).powi(2) + (t2 - 1.0).powi(3) * (c1 + 1.0))
                            * 0.5
                    };
                    let cos_val = (eased * std::f32::consts::PI).cos();
                    let sx = cos_val.abs().max(0.02);
                    let blur = (1.0 - cos_val.abs()).powf(0.6) * 8.0 * scale;
                    (sx, blur, cos_val > 0.0)
                }
            }
            None => (1.0, 0.0, false),
        }
    });

    let flip_old_img = if flip_use_old {
        COVER_FLIP_OLD_IMG.with(|cell| cell.borrow().clone())
    } else {
        None
    };

    let cover_img = if flip_use_old {
        flip_old_img.or(image_to_draw.clone())
    } else {
        image_to_draw.clone()
    };

    painter.save();
    let img_cx = img_x + img_size / 2.0;
    let img_cy = img_y + img_size / 2.0;
    painter.translate(Vec2::new(img_cx, img_cy));

    painter.scale(Vec2::new(cover_scale * flip_scale_x, cover_scale));
    painter.translate(Vec2::new(-img_cx, -img_cy));

    if flip_blur_sigma > 0.1 && use_blur {
        painter.begin_layer(LayerSpec::Blur(BlurSpec {
            sigma: (flip_blur_sigma, flip_blur_sigma * 0.3),
            tile: None,
        }));
    }

    painter.clip_path(&Path::continuous_rounded_rect(
        Rect::from_xywh(img_x, img_y, img_size, img_size),
        16.0 * scale,
    ));
    if let Some(img) = cover_img {
        let final_alpha = (alpha as f32 * cover_brightness) / 255.0;
        let img_w = img.width() as f32;
        let img_h = img.height() as f32;
        let src_rect = if img_w > 0.0 && img_h > 0.0 {
            let aspect = img_w / img_h;
            let src: Rect = if aspect > 1.0 {
                let crop_w = img_h;
                let offset_x = (img_w - crop_w) / 2.0;
                Rect::from_xywh(offset_x, 0.0, crop_w, img_h)
            } else {
                let crop_h = img_w;
                let offset_y = (img_h - crop_h) / 2.0;
                Rect::from_xywh(0.0, offset_y, img_w, crop_h)
            };
            Some(src)
        } else {
            None
        };
        let mut options = ImageOptions::default()
            .with_sampling(Sampling::LinearLinear)
            .with_alpha_f(final_alpha);
        if let Some(source) = src_rect {
            options = options.with_src(Rect::from_ltrb(
                source.left,
                source.top,
                source.right,
                source.bottom,
            ));
        }
        painter.draw_image(
            &img,
            Rect::from_xywh(img_x, img_y, img_size, img_size),
            &options,
        );
    } else {
        draw_placeholder(painter, img_x, img_y, img_size, alpha, scale, text_color);
    }
    if flip_blur_sigma > 0.1 && use_blur {
        painter.restore();
    }
    painter.restore();

    pause_t
}

struct TrackTextParams<'a> {
    painter: Painter<'a>,
    media: &'a MediaInfo,
    music_active: bool,
    text_x: f32,
    max_text_w: f32,
    title_y: f32,
    alpha: u8,
    font_size: f32,
    scale: f32,
    text_color: Rgba,
    text_color_sec: Rgba,
}

fn draw_track_text(params: TrackTextParams) {
    let TrackTextParams {
        painter,
        media,
        music_active,
        text_x,
        max_text_w,
        title_y,
        alpha,
        font_size,
        scale,
        text_color,
        text_color_sec,
    } = params;

    let title = if !music_active || media.title.is_empty() {
        "No Music playing"
    } else {
        &media.title
    };
    let artist = if !music_active || media.artist.is_empty() {
        "Unknown Artist"
    } else {
        &media.artist
    };

    let title_font_size = if font_size > 0.0 {
        font_size * scale
    } else {
        15.0 * scale
    };
    let title_style = FontStyle::bold();

    TITLE_SCROLL.with(|cell| {
        let mut scroll = cell.borrow_mut();
        scroll.draw(ScrollDrawParams {
            painter,
            text: title,
            x: text_x,
            y: title_y,
            max_w: max_text_w,
            size: title_font_size,
            style: title_style,
            color: text_color.with_alpha(alpha),
            blur: None,
            scale,
            render_as_paths: true,
        });
    });

    let artist_y = title_y + 22.0 * scale;
    let artist_font_size = if font_size > 0.0 {
        font_size * scale
    } else {
        15.0 * scale
    };
    let artist_style = FontStyle::normal();

    ARTIST_SCROLL.with(|cell| {
        let mut scroll = cell.borrow_mut();
        scroll.draw(ScrollDrawParams {
            painter,
            text: artist,
            x: text_x,
            y: artist_y,
            max_w: max_text_w,
            size: artist_font_size,
            style: artist_style,
            color: text_color_sec.with_alpha((alpha as f32 * 0.6) as u8),
            blur: None,
            scale,
            render_as_paths: true,
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_pause_control(
    painter: Painter<'_>,
    btn_cx: f32,
    btn_cy: f32,
    pause_t: f32,
    alpha: u8,
    scale: f32,
    use_blur: bool,
    dt: f32,
    text_color: Rgba,
) {
    let (pause_s, pause_blur) = PAUSE_SPRING.with(|cell| {
        let mut s = cell.borrow_mut();
        s.update_dt(1.0, PAUSE_CONTROL_STIFFNESS, PAUSE_CONTROL_DAMPING, dt);
        if (s.value - 1.0).abs() < 0.001 && s.velocity.abs() < 0.001 {
            s.value = 1.0;
            s.velocity = 0.0;
        }
        (
            s.value
                .clamp(PAUSE_CONTROL_MIN_SCALE, PAUSE_CONTROL_MAX_SCALE),
            (s.velocity.abs() * PAUSE_CONTROL_BLUR_SCALE * scale)
                .min(PAUSE_CONTROL_MAX_BLUR * scale),
        )
    });

    painter.save();
    if pause_blur > 0.1 && use_blur {
        painter.begin_layer(LayerSpec::Blur(BlurSpec::uniform(pause_blur)));
    }
    painter.translate(Vec2::new(btn_cx, btn_cy));
    painter.scale(Vec2::new(pause_s, pause_s));
    let icon_progress = ((pause_t - 0.5).abs() * 2.0).clamp(0.0, 1.0);
    let icon_alpha =
        (alpha as f32 * icon_progress * icon_progress * (3.0 - 2.0 * icon_progress)) as u8;
    if icon_alpha > 0 {
        if pause_t >= 0.5 {
            draw_pause_button(painter, 0.0, 0.0, icon_alpha, scale, text_color);
        } else {
            draw_play_button(painter, 0.0, 0.0, icon_alpha, scale, text_color);
        }
    }
    if pause_blur > 0.1 && use_blur {
        painter.restore();
    }
    painter.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_skip_button(
    painter: Painter<'_>,
    cx: f32,
    cy: f32,
    mirror: bool,
    anim_t: Option<f32>,
    alpha: u8,
    scale: f32,
    use_blur: bool,
    text_color: Rgba,
) {
    painter.save();
    painter.translate(Vec2::new(cx, cy));
    if mirror {
        painter.scale(Vec2::new(-1.0, 1.0));
    }
    if let Some(t) = anim_t {
        let skip_blur = (1.0 - t / 0.3).max(0.0) * 6.0 * scale;
        if skip_blur > 0.1 && use_blur {
            painter.begin_layer(LayerSpec::Blur(BlurSpec {
                sigma: (skip_blur, skip_blur * 0.3),
                tile: None,
            }));
        }

        let shoot_t = (t / 0.25).min(1.0);
        let shoot_x = 10.92 * scale + 22.0 * scale * shoot_t;
        let shoot_alpha = ((alpha as f32) * (1.0 - shoot_t)) as u8;
        if shoot_alpha > 0 {
            draw_control_triangle(painter, shoot_x, 0.0, shoot_alpha, 0.055, scale, text_color);
        }

        let move_t = (t / 0.55).min(1.0);
        let mid_x = -10.92 * scale + (10.92 * 2.0) * scale * move_t;
        let mid_s = 0.050 + (0.055 - 0.050) * move_t;
        draw_control_triangle(painter, mid_x, 0.0, alpha, mid_s, scale, text_color);

        let fade_raw = ((t - 0.15) / 0.85).clamp(0.0, 1.0);
        let fade_eased = ease_out_back(fade_raw);
        let new_x = -25.0 * scale + (25.0 - 10.92) * scale * fade_eased;
        let new_alpha = ((alpha as f32) * fade_raw) as u8;
        if new_alpha > 0 {
            draw_control_triangle(painter, new_x, 0.0, new_alpha, 0.050, scale, text_color);
        }

        if skip_blur > 0.1 && use_blur {
            painter.restore();
        }
    } else {
        draw_control_triangle(
            painter,
            -10.92 * scale,
            0.0,
            alpha,
            0.050,
            scale,
            text_color,
        );
        draw_control_triangle(painter, 10.92 * scale, 0.0, alpha, 0.055, scale, text_color);
    }
    painter.restore();
}

fn draw_placeholder(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    size: f32,
    alpha: u8,
    scale: f32,
    text_color: Rgba,
) {
    painter.fill_round_rect(
        Rect::from_xywh(x, y, size, size),
        Radius::uniform(14.0 * scale),
        text_color.with_alpha((alpha as f32 * 0.15) as u8),
    );

    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    crate::icons::music::draw_music_icon(painter, cx, cy, alpha, scale * 1.8, text_color);
}
