use std::any::Any;
use std::sync::Arc;

use skia_safe::Surface;

use crate::convert::to_skia_color;
use crate::image::Image;
use crate::painter::Painter;
use crate::types::Rgba;

pub const SURFACE_TAG_WIN32_HWND: u32 = 1;

#[derive(Clone)]
pub struct NativeSurface {
    tag: u32,
    handle: usize,
    keepalive: Option<Arc<dyn Any + Send + Sync>>,
}

impl NativeSurface {
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

pub struct RasterSurface {
    surface: Surface,
}

impl RasterSurface {
    pub fn new(width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        skia_safe::surfaces::raster_n32_premul((width, height)).map(|surface| Self { surface })
    }

    pub fn painter(&mut self) -> Painter<'_> {
        Painter {
            canvas: self.surface.canvas(),
        }
    }

    pub fn clear(&mut self, color: Rgba) {
        self.surface.canvas().clear(to_skia_color(color));
    }

    pub fn snapshot(&mut self) -> Image {
        Image::from_skia(self.surface.image_snapshot())
    }
}
