use std::time::Duration;

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetDoubleClickTime, VK_LBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CURSOR_SHOWING, CURSORINFO, GetClassNameW, GetCursorInfo, GetCursorPos, GetForegroundWindow,
    GetWindowRect, GetWindowThreadProcessId, IsIconic,
};

use crate::utils::shape::continuous_rounded_rect_path;

pub fn get_global_cursor_pos() -> (i32, i32) {
    let mut point = POINT::default();
    // SAFETY: GetCursorPos writes to a stack-allocated POINT struct. The pointer
    // is valid for the lifetime of the call. No preconditions or side effects.
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    (point.x, point.y)
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

    continuous_rounded_rect_path(
        skia_safe::Rect::from_xywh(rx as f32, ry as f32, rw as f32, rh as f32),
        radius as f32,
    )
    .contains(skia_safe::Point::new(px as f32, py as f32))
}

pub fn is_left_button_pressed() -> bool {
    // SAFETY: GetAsyncKeyState queries virtual key state. VK_LBUTTON is a constant.
    // No pointers or handles are involved. Thread-safe (per-thread key state).
    unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
}

pub fn double_click_interval() -> Duration {
    // SAFETY: GetDoubleClickTime reads the current system setting and has no parameters.
    Duration::from_millis(u64::from(unsafe { GetDoubleClickTime() }))
}

pub fn is_cursor_hidden() -> bool {
    // SAFETY: GetCursorInfo writes to a stack-allocated CURSORINFO struct with
    // correct cbSize. The pointer is valid for the lifetime of the call. No
    // preconditions — returns current cursor visibility state.
    unsafe {
        let mut info = CURSORINFO {
            cbSize: std::mem::size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        if GetCursorInfo(&mut info).is_ok() {
            return (info.flags.0 & CURSOR_SHOWING.0) == 0;
        }
    }
    false
}

pub fn is_foreground_fullscreen(
    target_x: i32,
    target_y: i32,
    target_width: u32,
    target_height: u32,
) -> bool {
    // SAFETY: All Win32 API calls in this function use valid stack-allocated
    // structs/buffers and query-only operations. GetForegroundWindow returns a
    // handle that may be null (checked). GetWindowThreadProcessId, GetClassNameW,
    // GetWindowRect, MonitorFromWindow, and GetMonitorInfoW all read window/monitor
    // metadata — no mutations to system state. The returned HWND is not stored
    // or used beyond this function's scope.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        if IsIconic(hwnd).as_bool() {
            return false;
        }

        // Skip our own window
        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if process_id == std::process::id() {
            return false;
        }

        // Skip Desktop and Taskbar
        let mut class_name = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_name);
        if len > 0 {
            let name = String::from_utf16_lossy(&class_name[..len as usize]);
            if name == "Progman" || name == "WorkerW" || name == "Shell_TrayWnd" {
                return false;
            }
        }

        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            let monitor_rect = monitor_info.rcMonitor;
            let is_fullscreen = window_rect.left <= monitor_rect.left
                && window_rect.top <= monitor_rect.top
                && window_rect.right >= monitor_rect.right
                && window_rect.bottom >= monitor_rect.bottom;
            let target_right = target_x.saturating_add(target_width as i32);
            let target_bottom = target_y.saturating_add(target_height as i32);
            let is_target_monitor = target_width == 0
                || target_height == 0
                || (monitor_rect.left == target_x
                    && monitor_rect.top == target_y
                    && monitor_rect.right == target_right
                    && monitor_rect.bottom == target_bottom);
            return is_fullscreen && is_target_monitor;
        }
    }
    false
}
