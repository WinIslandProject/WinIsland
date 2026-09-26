use std::sync::OnceLock;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_ADAPTER_FLAG, DXGI_ADAPTER_FLAG_NONE, DXGI_ADAPTER_FLAG_SOFTWARE,
    DXGI_CREATE_FACTORY_FLAGS, IDXGIFactory4,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CURSOR_SHOWING, CURSORINFO, FindWindowW, GetClassNameW, GetCursorInfo, GetCursorPos,
    GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};
use windows::core::{BOOL, PCWSTR};
use winisland_platform::{
    DisplayProvider, GpuProfile, MonitorId, MonitorInfo, PlatformError, Point, Rect,
};

mod brightness;

const INTEGRATED_GPU_MEMORY_THRESHOLD: usize = 1_073_741_824;
static GPU_PROFILE: OnceLock<GpuProfile> = OnceLock::new();

pub struct WindowsDisplay;

pub(crate) fn brightness_available() -> bool {
    brightness::available()
}

impl DisplayProvider for WindowsDisplay {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        let mut monitors = Vec::new();
        // SAFETY: The callback is synchronous and receives a pointer to monitors while it is live.
        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(collect_monitor),
                LPARAM((&mut monitors as *mut Vec<MonitorInfo>) as isize),
            )
        }
        .ok()
        .map_err(|error| PlatformError::Backend(error.to_string()))?;
        Ok(monitors)
    }

    fn cursor_position(&self) -> Result<Option<Point>, PlatformError> {
        let mut point = POINT::default();
        // SAFETY: point is an initialized writable output.
        if unsafe { GetCursorPos(&mut point) }.is_ok() {
            Ok(Some(Point {
                x: point.x,
                y: point.y,
            }))
        } else {
            Ok(None)
        }
    }

    fn foreign_fullscreen_on(&self, target: Rect) -> bool {
        foreground_fullscreen(target)
    }

    fn cursor_hidden(&self) -> bool {
        let mut info = CURSORINFO {
            cbSize: size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: info declares its size and is writable for the duration of the call.
        unsafe { GetCursorInfo(&mut info) }.is_ok() && (info.flags.0 & CURSOR_SHOWING.0) == 0
    }

    fn start_brightness_feed(
        &self,
    ) -> Result<Box<dyn winisland_platform::BrightnessFeed>, PlatformError> {
        Ok(Box::new(brightness::WindowsBrightnessFeed::new()))
    }

    fn gpu_profile(&self) -> GpuProfile {
        *GPU_PROFILE.get_or_init(detect_gpu_profile)
    }

    fn foreign_window_exists(&self, title: &str) -> bool {
        find_window(title).is_some()
    }

    fn bring_foreign_window_to_front(&self, title: &str) -> bool {
        let Some(hwnd) = find_window(title) else {
            return false;
        };
        // SAFETY: hwnd was returned by FindWindowW and is used only synchronously.
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd).as_bool()
        }
    }
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: EnumDisplayMonitors invokes this synchronously with the vector pointer supplied above.
    let found = unsafe { &mut *(data.0 as *mut Vec<MonitorInfo>) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: info is writable and monitor is supplied by EnumDisplayMonitors.
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        found.push(MonitorInfo {
            id: MonitorId(monitor.0 as usize as u64),
            bounds: rect(info.rcMonitor),
            work_area: rect(info.rcWork),
            primary: info.dwFlags & 1 != 0,
        });
    }
    true.into()
}

fn rect(value: RECT) -> Rect {
    Rect {
        left: value.left,
        top: value.top,
        right: value.right,
        bottom: value.bottom,
    }
}

fn foreground_fullscreen(target: Rect) -> bool {
    // SAFETY: All handles and buffers are used only for synchronous read-only queries.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() || IsIconic(hwnd).as_bool() {
            return false;
        }
        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if process_id == std::process::id() {
            return false;
        }
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
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            return false;
        }
        let bounds = monitor_info.rcMonitor;
        let fullscreen = window_rect.left <= bounds.left
            && window_rect.top <= bounds.top
            && window_rect.right >= bounds.right
            && window_rect.bottom >= bounds.bottom;
        let target_width = target.right.saturating_sub(target.left);
        let target_height = target.bottom.saturating_sub(target.top);
        let same_monitor = target_width == 0
            || target_height == 0
            || (bounds.left == target.left
                && bounds.top == target.top
                && bounds.right == target.right
                && bounds.bottom == target.bottom);
        fullscreen && same_monitor
    }
}

fn find_window(title: &str) -> Option<HWND> {
    let wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    // SAFETY: The title is NUL-terminated and remains live for the call.
    unsafe {
        FindWindowW(None, PCWSTR(wide.as_ptr()))
            .ok()
            .filter(|hwnd| !hwnd.is_invalid())
    }
}

fn detect_gpu_profile() -> GpuProfile {
    if let Ok(value) = std::env::var("WINISLAND_GPU_PROFILE") {
        if value.eq_ignore_ascii_case("integrated") {
            return GpuProfile::Integrated;
        }
        if value.eq_ignore_ascii_case("discrete") {
            return GpuProfile::Discrete;
        }
    }
    // SAFETY: Factory creation and adapter enumeration retain no caller-owned pointers.
    unsafe {
        let Ok(factory) = CreateDXGIFactory2::<IDXGIFactory4>(DXGI_CREATE_FACTORY_FLAGS(0)) else {
            return GpuProfile::Discrete;
        };
        for index in 0.. {
            let Ok(adapter) = factory.EnumAdapters1(index) else {
                break;
            };
            let Ok(desc) = adapter.GetDesc1() else {
                continue;
            };
            if (DXGI_ADAPTER_FLAG(desc.Flags as _) & DXGI_ADAPTER_FLAG_SOFTWARE)
                != DXGI_ADAPTER_FLAG_NONE
            {
                continue;
            }
            return if desc.DedicatedVideoMemory < INTEGRATED_GPU_MEMORY_THRESHOLD {
                GpuProfile::Integrated
            } else {
                GpuProfile::Discrete
            };
        }
    }
    GpuProfile::Discrete
}
