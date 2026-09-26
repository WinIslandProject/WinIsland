//! Rendering types and frame lifecycle for WinIsland.
//!
//! Skia remains private to this crate except for the hidden plugin ABI v1 adapter.
//! Geometry uses logical pixels. Arc angles start at 12 o'clock and increase clockwise.
//! Images may be decoded on any thread; GPU uploads belong on the rendering thread.

mod backend;
mod convert;
mod error;
mod frame;
mod image;
mod painter;
mod path;
mod surface;
pub mod text;
mod types;

pub use error::{RenderError, RenderResult};
pub use frame::{DrawingContext, Renderer, RendererOptions, RendererTargetId};
pub use image::Image;
pub use painter::Painter;
pub use path::{Path, PathBuilder};
#[doc(hidden)]
pub use surface::{NativeSurface, RasterSurface, SURFACE_TAG_WIN32_HWND};
pub use types::{
    Angle, BlurSpec, FontStyle, FontWeight, FontWidth, GradientStop, ImageFit, ImageOptions,
    LayerSpec, Mipmapped, PaintStyle, Point, Radius, Rect, Rgba, Sampling, Slant, SrcConstraint,
    StrokeCap, StrokeJoin, TileMode, Vec2,
};
