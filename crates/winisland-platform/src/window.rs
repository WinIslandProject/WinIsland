use std::any::Any;
use std::sync::Arc;

use crate::{
    AppHandler, CursorKind, HitRegion, HostBackdropParams, MonitorId, MonitorInfo, OverlaySpec,
    OverlayStyles, PlatformError, SettingsSpec, Theme, WindowId, WindowPosition, WindowSize,
};

/// Surface tag for a native Win32 window handle.
pub const SURFACE_TAG_WIN32_HWND: u32 = 1;

/// Opaque native surface value; its keepalive holds the window until rendering releases it.
#[derive(Clone)]
pub struct NativeSurface {
    tag: u32,
    handle: usize,
    keepalive: Option<Arc<dyn Any + Send + Sync>>,
}

impl NativeSurface {
    /// Wraps a Win32 handle; without an owner, the caller must keep the handle valid.
    pub fn from_win32_hwnd(hwnd: usize, keepalive: Option<Arc<dyn Any + Send + Sync>>) -> Self {
        Self {
            tag: SURFACE_TAG_WIN32_HWND,
            handle: hwnd,
            keepalive,
        }
    }

    /// Returns the backend tag without transferring ownership.
    pub fn tag(&self) -> u32 {
        self.tag
    }

    /// Returns the opaque native handle without transferring ownership.
    pub fn handle(&self) -> usize {
        self.handle
    }

    /// Transfers the owner to the renderer while consuming this surface value.
    pub fn into_keepalive(self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.keepalive
    }
}

/// Owns the event loop and application windows.
///
/// Every method must run on the event-loop thread; callbacks may call these methods synchronously,
/// but `run` must not be nested or called twice. Window IDs are valid from successful creation
/// until `destroy_window` or `run` returns. Unless documented otherwise, a stale ID makes a
/// command a no-op and a query return its absent/default value; commands do not report OS errors.
pub trait WindowSystem {
    /// Runs the loop on the current thread until exit; setup or loop failure returns an error.
    fn run(&self, handler: &mut dyn AppHandler) -> Result<(), PlatformError>;
    /// Requests loop termination; does nothing outside an active callback.
    fn exit(&self);
    /// Creates the hidden backdrop before its owned overlay; creation failure returns an error.
    fn create_overlay(&self, spec: OverlaySpec) -> Result<WindowId, PlatformError>;
    /// Creates a settings window on the selected monitor; unavailable loop or creation fails.
    fn create_settings(&self, spec: SettingsSpec) -> Result<WindowId, PlatformError>;
    /// Releases an owned window and its registry entry; repeated release is harmless.
    fn destroy_window(&self, id: WindowId);
    /// Requests a redraw synchronously, without allocation or a blocking lock; stale IDs do nothing.
    fn request_redraw(&self, id: WindowId);
    /// Changes visibility; a missing window is ignored.
    fn set_visible(&self, id: WindowId, visible: bool);
    /// Returns visibility, or false when unknown or the window is missing.
    fn is_visible(&self, id: WindowId) -> bool;
    /// Sets outer screen position in physical pixels; a missing window is ignored.
    fn set_position(&self, id: WindowId, position: WindowPosition);
    /// Reads outer screen position in physical pixels; `None` means unavailable.
    fn position(&self, id: WindowId) -> Option<WindowPosition>;
    /// Reads outer size in physical pixels; `None` means the window is missing.
    fn size(&self, id: WindowId) -> Option<WindowSize>;
    /// Reads inner size in physical pixels; a missing window yields zero size.
    fn inner_size(&self, id: WindowId) -> WindowSize;
    /// Requests inner size in physical pixels; true means dispatched or accepted by the active DPI writer.
    fn request_inner_size(&self, id: WindowId, size: WindowSize) -> bool;
    /// Reads the current scale factor; a missing window yields 1.0.
    fn scale_factor(&self, id: WindowId) -> f64;
    /// Applies whole-window cursor hit testing; a missing window is ignored.
    fn set_hit_regions(&self, id: WindowId, regions: &[HitRegion]);
    /// Starts native window dragging; missing or denied windows are ignored.
    fn begin_drag(&self, id: WindowId);
    /// Changes minimization; a missing window is ignored.
    fn set_minimized(&self, id: WindowId, minimized: bool);
    /// Changes maximization; a missing window is ignored.
    fn set_maximized(&self, id: WindowId, maximized: bool);
    /// Reports maximization, or false if the window is missing.
    fn is_maximized(&self, id: WindowId) -> bool;
    /// Reports minimization; `None` means unavailable or the window is missing.
    fn is_minimized(&self, id: WindowId) -> Option<bool>;
    /// Returns the current monitor ID, or `None` if unresolved.
    fn monitor_of(&self, id: WindowId) -> Option<MonitorId>;
    /// Returns the primary monitor ID, or `None` if unavailable.
    fn primary_monitor(&self) -> Option<MonitorId>;
    /// Returns an owned monitor snapshot; enumeration failure yields an empty list.
    fn monitors(&self) -> Vec<MonitorInfo>;
    /// Resolves a configured native monitor index; `None` means no usable monitor.
    fn target_monitor(&self, id: WindowId, index: i32) -> Option<MonitorId>;
    /// Returns a surface retaining its window; `None` means no valid native handle.
    fn native_surface(&self, id: WindowId) -> Option<NativeSurface>;
    /// Reapplies overlay styles, including skip-taskbar; a missing window is ignored.
    fn apply_overlay_styles(&self, id: WindowId, styles: OverlayStyles);
    /// Changes topmost order without activation; a missing window is ignored.
    fn set_topmost(&self, id: WindowId, topmost: bool);
    /// Reads the theme; `None` includes unknown theme and missing window.
    fn theme(&self, id: WindowId) -> Option<Theme>;
    /// Sets the cursor icon; a missing window is ignored.
    fn set_cursor(&self, id: WindowId, cursor: CursorKind);
    /// Applies the titlebar theme; unsupported or missing windows are ignored.
    fn set_titlebar_theme(&self, id: WindowId, is_light: bool);
    /// Starts composition backdrop for a live overlay; unavailable support returns an error.
    fn start_host_backdrop(&self, id: WindowId) -> Result<(), PlatformError>;
    /// Updates backdrop geometry in physical pixels; false means absent, error releases it.
    fn update_host_backdrop(
        &self,
        id: WindowId,
        params: HostBackdropParams,
    ) -> Result<bool, PlatformError>;
    /// Hides a live backdrop without releasing it; an absent backdrop is ignored.
    fn hide_host_backdrop(&self, id: WindowId);
    /// Releases backdrop resources before renderer drop; repeated release is harmless.
    fn release_host_backdrop(&self, id: WindowId);
}
