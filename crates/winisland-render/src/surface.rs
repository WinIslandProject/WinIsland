use skia_safe::Surface;

pub use winisland_platform::{NativeSurface, SURFACE_TAG_WIN32_HWND};

use crate::convert::to_skia_color;
use crate::image::Image;
use crate::painter::Painter;
use crate::types::Rgba;

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
