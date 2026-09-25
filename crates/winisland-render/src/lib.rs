//! WinIsland 的渲染出口 crate。
//!
//! **是什么**：`skia_safe` 的唯一出口。本 crate 定义渲染语义值类型（`Rgba`/`Rect`/`Point`/
//! `Radius`/`Angle`/`Path`/`Image`/`Sampling` 等），并提供语义绘制接口 `Painter` 与持有设备、
//! 渲染目标、帧生命周期的 `Renderer`（B2 起）。
//!
//! **不做什么**：不认识平台窗口，不做布局、命中测试或 UI 状态缓存，不定义插件 ABI，
//! 不把 `skia_safe` 类型放进公开签名。
//!
//! **依赖谁**：`skia-safe`（`features = ["d3d"]`）、`image`。**不允许**依赖 `winit`、
//! `winisland-platform*`、`winisland-ui`、`winisland-plugin-*`（`04` 第 2.1 节依赖矩阵）。
//!
//! **被谁依赖**：app crate（`WinIsland`）。`winisland-core` **不依赖**本 crate。
//!
//! **坐标与单位**：长度一律为逻辑像素（logical px）。`Angle` 的 0° 指向 12 点钟方向、
//! 顺时针为正；`skia_safe::Canvas::draw_arc` 的 0° 在 3 点钟方向，换算 `skia = ours - 90`
//! 只在 `painter` 内部发生。`skia_safe::Canvas::rotate` 与 `draw_arc` 的角度单位都是**度**，
//! 但 `rotate` 不接受 12 点钟偏移。
//!
//! **线程模型**：`Image` 是 `Send + Sync + Clone` 的句柄，解码（`Image::decode`/
//! `Image::from_encoded`/`Image::from_rgba8`）可在任意线程调用；GPU 上传
//! （`Renderer::prepare_image`）只允许在渲染线程调用。

mod backend;
mod convert;
mod error;
mod frame;
mod image;
#[cfg(feature = "legacy-canvas-bridge")]
mod legacy;
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
pub use surface::{NativeSurface, SURFACE_TAG_WIN32_HWND};
pub use types::{
    Angle, BlurSpec, FontStyle, FontWeight, FontWidth, GradientStop, ImageFit, ImageOptions,
    LayerSpec, Mipmapped, PaintStyle, Point, Radius, Rect, Rgba, Sampling, Slant, SrcConstraint,
    StrokeCap, StrokeJoin, TileMode, Vec2,
};
