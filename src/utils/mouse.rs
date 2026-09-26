use std::time::Duration;

use winisland_platform::VirtualKey;
use winisland_render::{Path, Point, Rect};

pub fn get_global_cursor_pos() -> (i32, i32) {
    crate::platform::display()
        .cursor_position()
        .ok()
        .flatten()
        .map(|point| (point.x, point.y))
        .unwrap_or_default()
}

pub fn is_point_in_rect(px: f64, py: f64, rx: f64, ry: f64, rw: f64, rh: f64) -> bool {
    px >= rx && px <= rx + rw && py >= ry && py <= ry + rh
}

pub fn is_point_in_continuous_rounded_rect(
    px: f64,
    py: f64,
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
    radius: f64,
) -> bool {
    if !px.is_finite()
        || !py.is_finite()
        || !rx.is_finite()
        || !ry.is_finite()
        || !rw.is_finite()
        || !rh.is_finite()
        || !radius.is_finite()
        || rw <= 0.0
        || rh <= 0.0
        || !is_point_in_rect(px, py, rx, ry, rw, rh)
    {
        return false;
    }

    Path::continuous_rounded_rect(
        Rect::from_xywh(rx as f32, ry as f32, rw as f32, rh as f32),
        radius as f32,
    )
    .contains(Point::new(px as f32, py as f32))
}

pub fn is_left_button_pressed() -> bool {
    crate::platform::input()
        .key_down(VirtualKey::LeftMouseButton)
        .unwrap_or(false)
}

pub fn double_click_interval() -> Duration {
    crate::platform::input().double_click_interval()
}

pub fn is_cursor_hidden() -> bool {
    crate::platform::display().cursor_hidden()
}

pub fn is_foreground_fullscreen(
    target_x: i32,
    target_y: i32,
    target_width: u32,
    target_height: u32,
) -> bool {
    crate::platform::display().foreign_fullscreen_on(winisland_platform::Rect {
        left: target_x,
        top: target_y,
        right: target_x.saturating_add(target_width as i32),
        bottom: target_y.saturating_add(target_height as i32),
    })
}
