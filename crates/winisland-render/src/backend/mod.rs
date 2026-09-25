mod d3d12;

use crate::error::{RenderError, RenderResult};
use crate::surface::{NativeSurface, SURFACE_TAG_WIN32_HWND};

pub(crate) use d3d12::{D3DDevice, RenderTarget};

pub(crate) fn check_surface_supported(surface: &NativeSurface) -> RenderResult<()> {
    if surface.tag() == SURFACE_TAG_WIN32_HWND {
        return Ok(());
    }
    Err(RenderError::Backend(format!(
        "Unsupported surface tag: {}",
        surface.tag()
    )))
}
