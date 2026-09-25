use super::color::Rgba;

/// 填充或描边。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PaintStyle {
    #[default]
    Fill,
    Stroke,
}

/// 线端样式，对应 `skia_safe::paint::Cap`。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StrokeCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// 线连接样式，对应 `skia_safe::paint::Join`。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StrokeJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// 着色器与图像过滤器的边缘延展方式，对应 `skia_safe::TileMode`。
///
/// 实测用到 `Clamp`（blur 与一处渐变）与 `Mirror`（频谱渐变），另两种为保留变体。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TileMode {
    #[default]
    Clamp,
    Mirror,
    Repeat,
    Decal,
}

/// 线性渐变的一个色标。
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GradientStop {
    /// 归一化位置，`0.0` 为渐变起点、`1.0` 为终点。
    pub offset: f32,
    pub color: Rgba,
}

/// 模糊参数。
///
/// `sigma` 为 `(x, y)`，允许各向异性（实测有 `(s, s * 0.3)`、`(12.0, 10.0)` 等）。
/// `tile` 为 `None` 时交给后端默认（Skia 视调用点取 `Decal`），实测 8 处 blur 中
/// 仅玻璃背景一处显式传 `Some(Clamp)`，其余为 `None`；两者不可合并。
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BlurSpec {
    pub sigma: (f32, f32),
    pub tile: Option<TileMode>,
}

impl BlurSpec {
    pub const fn uniform(sigma: f32) -> Self {
        Self {
            sigma: (sigma, sigma),
            tile: None,
        }
    }
}

/// 图层描述，对应 `SaveLayerRec` + `image_filter` 的组合。
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LayerSpec {
    Blur(BlurSpec),
}
