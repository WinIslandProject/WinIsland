use crate::core::smtc::MediaInfo;

use super::{
    CONTENT_PADDING, COVER_FLIP_ANIM, COVER_FLIP_OLD_IMG, IMG_CACHE, LOCAL_PLAY_STATE,
    NEXT_SKIP_ANIM, PAUSE_CONTROL_PRESS_VELOCITY, PAUSE_SPRING, PREV_SKIP_ANIM, PROGRESS_DRAGGING,
    PROGRESS_HOVER, PROGRESS_SMOOTH,
};

pub fn set_progress_dragging(active: bool) {
    PROGRESS_DRAGGING.with(|cell| {
        *cell.borrow_mut() = active;
    });
}

pub fn snap_progress(progress: f32) {
    PROGRESS_SMOOTH.with(|cell| {
        *cell.borrow_mut() = progress.clamp(0.0, 1.0);
    });
}

pub fn trigger_pause_click(current_is_playing: bool) {
    PAUSE_SPRING.with(|cell| {
        let mut s = cell.borrow_mut();
        s.velocity = PAUSE_CONTROL_PRESS_VELOCITY;
    });
    LOCAL_PLAY_STATE.with(|cell| {
        *cell.borrow_mut() = Some((!current_is_playing, std::time::Instant::now()));
    });
}

pub fn trigger_prev_click() {
    PREV_SKIP_ANIM.with(|cell| {
        *cell.borrow_mut() = Some(std::time::Instant::now());
    });
}

pub fn trigger_next_click() {
    NEXT_SKIP_ANIM.with(|cell| {
        *cell.borrow_mut() = Some(std::time::Instant::now());
    });
}

pub(super) fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158_f32;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

pub fn trigger_cover_flip() {
    let old_img = IMG_CACHE.with(|cache| cache.borrow_mut().take().and_then(|(_, image)| image));
    if let Some(old_img) = old_img {
        COVER_FLIP_OLD_IMG.with(|cell| {
            *cell.borrow_mut() = Some(old_img);
        });
    }
    COVER_FLIP_ANIM.with(|cell| {
        *cell.borrow_mut() = Some(std::time::Instant::now());
    });
}

pub fn set_progress_hover(active: bool) {
    PROGRESS_HOVER.with(|cell| {
        cell.borrow_mut().0 = active;
    });
}

pub fn get_pause_btn_rect(
    ox: f32,
    oy: f32,
    w: f32,
    _h: f32,
    scale: f32,
    _cover_shape: &str,
) -> (f32, f32, f32, f32) {
    let (img_size, img_y) = (72.0 * scale, oy + 24.0 * scale);
    let bar_y = img_y + img_size + 18.0 * scale;
    let btn_cy = bar_y + 42.0 * scale;
    let hit = 40.0 * scale;
    let btn_cx = ox + w / 2.0;
    (btn_cx - hit / 2.0, btn_cy - hit / 2.0, hit, hit)
}

pub fn get_prev_btn_rect(
    ox: f32,
    oy: f32,
    w: f32,
    _h: f32,
    scale: f32,
    _cover_shape: &str,
) -> (f32, f32, f32, f32) {
    let (img_size, img_y) = (72.0 * scale, oy + 24.0 * scale);
    let bar_y = img_y + img_size + 18.0 * scale;
    let btn_cy = bar_y + 42.0 * scale;
    let hit = 36.0 * scale;
    let btn_cx = ox + w / 2.0 - 75.0 * scale;
    (btn_cx - hit / 2.0, btn_cy - hit / 2.0, hit, hit)
}

pub fn get_next_btn_rect(
    ox: f32,
    oy: f32,
    w: f32,
    _h: f32,
    scale: f32,
    _cover_shape: &str,
) -> (f32, f32, f32, f32) {
    let (img_size, img_y) = (72.0 * scale, oy + 24.0 * scale);
    let bar_y = img_y + img_size + 18.0 * scale;
    let btn_cy = bar_y + 42.0 * scale;
    let hit = 36.0 * scale;
    let btn_cx = ox + w / 2.0 + 75.0 * scale;
    (btn_cx - hit / 2.0, btn_cy - hit / 2.0, hit, hit)
}

pub fn get_progress_bar_rect(
    ox: f32,
    oy: f32,
    w: f32,
    _media: &MediaInfo,
    music_active: bool,
    scale: f32,
    _cover_shape: &str,
) -> Option<(f32, f32, f32, f32)> {
    if !music_active {
        return None;
    }
    let (img_size, img_y) = (72.0 * scale, oy + 24.0 * scale);
    let bar_y = img_y + img_size + 18.0 * scale;
    let time_w = 36.0 * scale;
    let bar_full_left = ox + CONTENT_PADDING * scale;
    let bar_full_right = ox + w - CONTENT_PADDING * scale;
    let bar_left = bar_full_left + time_w + 4.0 * scale;
    let bar_right = bar_full_right - time_w - 4.0 * scale;
    let hit_h = 16.0 * scale;
    Some((bar_left, bar_right, bar_y - hit_h / 2.0, hit_h))
}
