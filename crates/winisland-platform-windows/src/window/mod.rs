mod attrs;
mod hit_test;
mod r#loop;
pub(crate) mod styles;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use winisland_platform::{
    AppHandler, CursorKind, DisplayProvider, HitRegion, HostBackdropParams, MonitorId, MonitorInfo,
    NativeSurface, OverlaySpec, OverlayStyles, PlatformError, SettingsSpec, Theme, WindowId,
    WindowPosition, WindowSize, WindowSystem,
};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::platform::windows::{MonitorHandleExtWindows, WindowExtWindows};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{CursorIcon, Theme as WinitTheme, Window, WindowAttributes};

use crate::backdrop::HostBackdrop;
use crate::display::WindowsDisplay;

pub use r#loop::wake;

pub struct WindowsWindowSystem;

struct WindowRecord {
    host_backdrop: Option<Rc<HostBackdrop>>,
    window: Arc<Window>,
    backdrop: Option<Arc<Window>>,
}

thread_local! {
    static WINDOWS: RefCell<HashMap<WindowId, WindowRecord>> = RefCell::new(HashMap::new());
}

static SYSTEM: WindowsWindowSystem = WindowsWindowSystem;

pub fn system() -> &'static WindowsWindowSystem {
    &SYSTEM
}

fn get_window(id: WindowId) -> Option<Arc<Window>> {
    WINDOWS.with(|windows| {
        windows
            .borrow()
            .get(&id)
            .map(|record| record.window.clone())
    })
}

fn with_window<T>(id: WindowId, f: impl FnOnce(&Window) -> T) -> Option<T> {
    let window = get_window(id)?;
    Some(f(&window))
}

fn create_window(
    event_loop: &ActiveEventLoop,
    attributes: WindowAttributes,
) -> Result<Arc<Window>, PlatformError> {
    event_loop
        .create_window(attributes)
        .map(Arc::new)
        .map_err(PlatformError::backend)
}

fn register_window(window: Arc<Window>, backdrop: Option<Arc<Window>>) -> WindowId {
    let id = WindowId(u64::from(window.id()));
    let record = WindowRecord {
        host_backdrop: None,
        window,
        backdrop,
    };
    WINDOWS.with(|windows| {
        windows.borrow_mut().insert(id, record);
    });
    id
}

fn monitor_id(monitor: &MonitorHandle) -> MonitorId {
    MonitorId(monitor.hmonitor() as usize as u64)
}

fn available_monitors() -> Vec<MonitorHandle> {
    if let Some(handles) =
        r#loop::with_active_event_loop(|event_loop| event_loop.available_monitors().collect())
    {
        return handles;
    }
    let window = WINDOWS.with(|windows| {
        windows
            .borrow()
            .values()
            .next()
            .map(|record| record.window.clone())
    });
    window
        .map(|window| window.available_monitors().collect())
        .unwrap_or_default()
}

fn target_monitor_handle(window: &Window, index: i32) -> Option<MonitorHandle> {
    if index < 0 {
        return window
            .primary_monitor()
            .or_else(|| window.current_monitor());
    }
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_STATE_FLAGS, DISPLAY_DEVICEW, EnumDisplayDevicesW,
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use windows::core::BOOL;

    let mut active_index = 0usize;
    let mut device_index = 0u32;
    let target_name = loop {
        let mut device = DISPLAY_DEVICEW {
            cb: size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        // SAFETY: device is initialized with the required structure size and is writable.
        let found = unsafe { EnumDisplayDevicesW(None, device_index, &mut device, 0) }.as_bool();
        if !found {
            break None;
        }
        device_index += 1;
        if (device.StateFlags & DISPLAY_DEVICE_ACTIVE) != DISPLAY_DEVICE_STATE_FLAGS(0) {
            if active_index == index as usize {
                break Some(
                    String::from_utf16_lossy(&device.DeviceName)
                        .trim_end_matches('\0')
                        .to_string(),
                );
            }
            active_index += 1;
        }
    }?;
    unsafe extern "system" fn collect_monitor(
        monitor: HMONITOR,
        _dc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: EnumDisplayMonitors invokes this synchronously while the Vec is alive.
        let found = unsafe { &mut *(data.0 as *mut Vec<(String, RECT)>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: MONITORINFOEXW begins with MONITORINFO and cbSize includes the full struct.
        if unsafe { GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO) }.as_bool() {
            found.push((
                String::from_utf16_lossy(&info.szDevice)
                    .trim_end_matches('\0')
                    .to_string(),
                info.monitorInfo.rcMonitor,
            ));
        }
        BOOL(1)
    }
    let mut native_monitors = Vec::<(String, RECT)>::new();
    // SAFETY: The callback is synchronous and receives a valid pointer to native_monitors.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM((&mut native_monitors as *mut Vec<(String, RECT)>) as isize),
        )
    };
    if let Some((_, rect)) = native_monitors
        .iter()
        .find(|(name, _)| name == &target_name)
        && let Some(monitor) = window.available_monitors().find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            position.x == rect.left
                && position.y == rect.top
                && size.width == (rect.right - rect.left) as u32
                && size.height == (rect.bottom - rect.top) as u32
        })
    {
        return Some(monitor);
    }
    window
        .primary_monitor()
        .or_else(|| window.current_monitor())
}

fn window_hwnd(window: &Window) -> Option<usize> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as usize),
        _ => None,
    }
}

impl WindowSystem for WindowsWindowSystem {
    fn run(&self, handler: &mut dyn AppHandler) -> Result<(), PlatformError> {
        let result = r#loop::run(handler);
        let records = WINDOWS.with(|windows| std::mem::take(&mut *windows.borrow_mut()));
        drop(records);
        result
    }

    fn exit(&self) {
        r#loop::exit();
    }

    fn create_overlay(&self, spec: OverlaySpec) -> Result<WindowId, PlatformError> {
        r#loop::with_active_event_loop(|event_loop| {
            let backdrop = create_window(event_loop, attrs::backdrop_attributes())?;
            let _ = backdrop.set_cursor_hittest(false);
            let owner = window_hwnd(&backdrop)
                .ok_or(PlatformError::Unavailable("Win32 backdrop window handle"))?
                as isize;
            let window = create_window(event_loop, attrs::overlay_attributes(spec, owner))?;
            Ok(register_window(window, Some(backdrop)))
        })
        .ok_or(PlatformError::Unavailable("active event loop"))?
    }

    fn create_settings(&self, spec: SettingsSpec) -> Result<WindowId, PlatformError> {
        r#loop::with_active_event_loop(|event_loop| {
            let monitor = spec.monitor.and_then(|id| {
                event_loop
                    .available_monitors()
                    .find(|monitor| monitor_id(monitor) == id)
            });
            let attributes = attrs::settings_attributes(spec, monitor.as_ref());
            Ok(register_window(
                create_window(event_loop, attributes)?,
                None,
            ))
        })
        .ok_or(PlatformError::Unavailable("active event loop"))?
    }

    fn destroy_window(&self, id: WindowId) {
        let record = WINDOWS.with(|windows| windows.borrow_mut().remove(&id));
        drop(record);
    }

    fn request_redraw(&self, id: WindowId) {
        let _ = with_window(id, Window::request_redraw);
    }

    fn set_visible(&self, id: WindowId, visible: bool) {
        let _ = with_window(id, |window| window.set_visible(visible));
    }

    fn is_visible(&self, id: WindowId) -> bool {
        with_window(id, |window| window.is_visible().unwrap_or(false)).unwrap_or(false)
    }

    fn set_position(&self, id: WindowId, position: WindowPosition) {
        let _ = with_window(id, |window| {
            window.set_outer_position(PhysicalPosition::new(position.x, position.y));
        });
    }

    fn position(&self, id: WindowId) -> Option<WindowPosition> {
        with_window(id, |window| {
            window.outer_position().ok().map(|position| WindowPosition {
                x: position.x,
                y: position.y,
            })
        })
        .flatten()
    }

    fn size(&self, id: WindowId) -> Option<WindowSize> {
        with_window(id, |window| {
            let size = window.outer_size();
            WindowSize {
                width: size.width,
                height: size.height,
            }
        })
    }

    fn inner_size(&self, id: WindowId) -> WindowSize {
        with_window(id, |window| {
            let size = window.inner_size();
            WindowSize {
                width: size.width,
                height: size.height,
            }
        })
        .unwrap_or_default()
    }

    fn request_inner_size(&self, id: WindowId, size: WindowSize) -> bool {
        let settings_window = WINDOWS.with(|windows| {
            windows
                .borrow()
                .get(&id)
                .is_some_and(|record| record.backdrop.is_none())
        });
        if settings_window && let Some(accepted) = r#loop::request_dpi_inner_size(id, size) {
            return accepted;
        }
        with_window(id, |window| {
            let _ = window.request_inner_size(PhysicalSize::new(size.width, size.height));
        })
        .is_some()
    }

    fn scale_factor(&self, id: WindowId) -> f64 {
        with_window(id, Window::scale_factor).unwrap_or(1.0)
    }

    fn set_hit_regions(&self, id: WindowId, regions: &[HitRegion]) {
        let _ = with_window(id, |window| hit_test::set_hit_regions(window, regions));
    }

    fn begin_drag(&self, id: WindowId) {
        let _ = with_window(id, |window| window.drag_window());
    }

    fn set_minimized(&self, id: WindowId, minimized: bool) {
        let _ = with_window(id, |window| window.set_minimized(minimized));
    }

    fn set_maximized(&self, id: WindowId, maximized: bool) {
        let _ = with_window(id, |window| window.set_maximized(maximized));
    }

    fn is_maximized(&self, id: WindowId) -> bool {
        with_window(id, Window::is_maximized).unwrap_or(false)
    }

    fn is_minimized(&self, id: WindowId) -> Option<bool> {
        with_window(id, Window::is_minimized).flatten()
    }

    fn monitor_of(&self, id: WindowId) -> Option<MonitorId> {
        with_window(id, |window| {
            window.current_monitor().as_ref().map(monitor_id)
        })
        .flatten()
    }

    fn primary_monitor(&self) -> Option<MonitorId> {
        if let Some(monitor) =
            r#loop::with_active_event_loop(|event_loop| event_loop.primary_monitor()).flatten()
        {
            return Some(monitor_id(&monitor));
        }
        WINDOWS.with(|windows| {
            windows
                .borrow()
                .values()
                .next()
                .and_then(|record| record.window.primary_monitor())
                .as_ref()
                .map(monitor_id)
        })
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        let Ok(native) = WindowsDisplay.monitors() else {
            return Vec::new();
        };
        let handles = available_monitors();
        if handles.is_empty() {
            return native;
        }
        handles
            .into_iter()
            .filter_map(|handle| {
                native
                    .iter()
                    .find(|info| info.id == monitor_id(&handle))
                    .map(|info| {
                        let mut info = info.clone();
                        info.scale_factor = handle.scale_factor();
                        info.refresh_rate_millihertz = handle.refresh_rate_millihertz();
                        info
                    })
            })
            .collect()
    }

    fn target_monitor(&self, id: WindowId, index: i32) -> Option<MonitorId> {
        with_window(id, |window| {
            target_monitor_handle(window, index)
                .as_ref()
                .map(monitor_id)
        })
        .flatten()
    }

    fn native_surface(&self, id: WindowId) -> Option<NativeSurface> {
        let window = get_window(id)?;
        let hwnd = window_hwnd(&window)?;
        Some(NativeSurface::from_win32_hwnd(hwnd, Some(window)))
    }

    fn apply_overlay_styles(&self, id: WindowId, styles: OverlayStyles) {
        let _ = with_window(id, |window| {
            if let Some(hwnd) = window_hwnd(window) {
                styles::enforce_overlay_window_styles(
                    windows::Win32::Foundation::HWND(hwnd as *mut _),
                    styles.topmost,
                );
            }
            window.set_skip_taskbar(styles.skip_taskbar);
        });
    }

    fn set_topmost(&self, id: WindowId, topmost: bool) {
        let _ = with_window(id, |window| {
            if let Some(hwnd) = window_hwnd(window) {
                styles::set_window_topmost(
                    windows::Win32::Foundation::HWND(hwnd as *mut _),
                    topmost,
                );
            }
        });
    }

    fn theme(&self, id: WindowId) -> Option<Theme> {
        with_window(id, |window| {
            window.theme().map(|theme| match theme {
                WinitTheme::Light => Theme::Light,
                WinitTheme::Dark => Theme::Dark,
            })
        })
        .flatten()
    }

    fn set_cursor(&self, id: WindowId, cursor: CursorKind) {
        let _ = with_window(id, |window| {
            window.set_cursor(match cursor {
                CursorKind::Default => CursorIcon::Default,
                CursorKind::Pointer => CursorIcon::Pointer,
            });
        });
    }

    fn set_titlebar_theme(&self, id: WindowId, is_light: bool) {
        let _ = with_window(id, |window| {
            if let Some(hwnd) = window_hwnd(window) {
                styles::set_titlebar_theme(
                    windows::Win32::Foundation::HWND(hwnd as *mut _),
                    is_light,
                );
            }
        });
    }

    fn start_host_backdrop(&self, id: WindowId) -> Result<(), PlatformError> {
        let (window, backdrop) = WINDOWS
            .with(|windows| {
                windows
                    .borrow()
                    .get(&id)
                    .map(|record| (record.window.clone(), record.backdrop.clone()))
            })
            .ok_or(PlatformError::Unavailable("overlay window"))?;
        let backdrop = backdrop.ok_or(PlatformError::Unavailable("backdrop window"))?;
        let host_backdrop =
            Rc::new(HostBackdrop::new(&window, &backdrop).map_err(PlatformError::Backend)?);
        WINDOWS.with(|windows| {
            if let Some(record) = windows.borrow_mut().get_mut(&id) {
                record.host_backdrop = Some(host_backdrop);
            }
        });
        Ok(())
    }

    fn update_host_backdrop(
        &self,
        id: WindowId,
        params: HostBackdropParams,
    ) -> Result<bool, PlatformError> {
        let host_backdrop = WINDOWS.with(|windows| {
            windows
                .borrow()
                .get(&id)
                .and_then(|record| record.host_backdrop.clone())
        });
        let Some(host_backdrop) = host_backdrop else {
            return Ok(false);
        };
        if let Err(error) = host_backdrop.update(params) {
            self.release_host_backdrop(id);
            return Err(PlatformError::backend(error));
        }
        Ok(true)
    }

    fn hide_host_backdrop(&self, id: WindowId) {
        let host_backdrop = WINDOWS.with(|windows| {
            windows
                .borrow()
                .get(&id)
                .and_then(|record| record.host_backdrop.clone())
        });
        if let Some(host_backdrop) = host_backdrop {
            host_backdrop.hide();
        }
    }

    fn release_host_backdrop(&self, id: WindowId) {
        let host_backdrop = WINDOWS.with(|windows| {
            windows
                .borrow_mut()
                .get_mut(&id)
                .and_then(|record| record.host_backdrop.take())
        });
        drop(host_backdrop);
    }
}
