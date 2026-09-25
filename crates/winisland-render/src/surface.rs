use std::any::Any;
use std::sync::Arc;

/// 表面标签：Win32 `HWND`。Windows 上唯一支持的值。
pub const SURFACE_TAG_WIN32_HWND: u32 = 1;

/// 后端原生的绘制目标描述。
///
/// 不变量：`handle` 的解释由 `tag` 决定（`SURFACE_TAG_WIN32_HWND` 时为 `HWND` 的数值）。
/// `keepalive` 是该原生句柄的所有者，渲染层不解析它，只保证句柄在 `NativeSurface`
/// 存活期间有效；它承担的是原先 `RenderTarget` 持有 `Arc<Window>` 的同一职责。
#[derive(Clone)]
pub struct NativeSurface {
    tag: u32,
    handle: usize,
    keepalive: Option<Arc<dyn Any + Send + Sync>>,
}

impl NativeSurface {
    /// `hwnd` 为 `HWND` 的数值（`HWND.0 as usize`）；`keepalive` 必须持有该窗口的所有权。
    pub fn from_win32_hwnd(hwnd: usize, keepalive: Option<Arc<dyn Any + Send + Sync>>) -> Self {
        Self {
            tag: SURFACE_TAG_WIN32_HWND,
            handle: hwnd,
            keepalive,
        }
    }

    pub fn tag(&self) -> u32 {
        self.tag
    }

    pub fn handle(&self) -> usize {
        self.handle
    }

    pub(crate) fn into_keepalive(self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.keepalive
    }
}
