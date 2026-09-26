pub mod app;
mod backdrop;
pub mod settings;
pub mod tray;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use winisland_render::NativeSurface;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

static DWM_COMPOSITION_CHANGED: AtomicBool = AtomicBool::new(false);

pub(crate) fn signal_dwm_composition_changed() {
    DWM_COMPOSITION_CHANGED.store(true, Ordering::Release);
}

pub(crate) fn take_dwm_composition_changed() -> bool {
    DWM_COMPOSITION_CHANGED.swap(false, Ordering::AcqRel)
}

pub(crate) fn native_surface(window: &Arc<Window>) -> Result<NativeSurface, String> {
    let handle = window
        .window_handle()
        .map_err(|error| format!("Window handle unavailable: {error}"))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("D3D12 rendering requires a Win32 window".to_string());
    };
    Ok(NativeSurface::from_win32_hwnd(
        handle.hwnd.get() as usize,
        Some(Arc::new(window.clone())),
    ))
}
