use super::geom::Rect;

/// 图像采样方式。
///
/// **三态，不是两态**：显式构造的 `SamplingOptions` 只有两种组合
/// （`Linear+Linear` 7 处、`Linear+None` 4 处），但另有 5 处绘制不带采样参数
/// （`draw_image_rect` 3 处 + `draw_image` 2 处）走 `SamplingOptions::default()`，
/// 即 `Nearest + None`。`Default` 与 `LinearNone` 的像素结果不同，禁止合并。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Sampling {
    /// `FilterMode::Nearest` + `MipmapMode::None`，等价于 Skia 的 `SamplingOptions::default()`。
    #[default]
    Default,
    /// `FilterMode::Linear` + `MipmapMode::None`。
    LinearNone,
    /// `FilterMode::Linear` + `MipmapMode::Linear`。
    LinearLinear,
}

/// 源矩形约束，对应 `skia_safe::canvas::SrcRectConstraint`。
///
/// `Fast` 允许后端在缩放时读取源矩形外的像素；实测用到 `Fast`（3 处），
/// 默认值与其一致。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum SrcConstraint {
    #[default]
    Fast,
    Strict,
}

/// 源图像到目标矩形的适配方式。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ImageFit {
    /// 直接把源矩形铺满目标矩形，允许拉伸。
    #[default]
    Fill,
    /// 完整显示源图像，多出的方向留空（等比缩小）。
    Contain,
    /// 铺满目标矩形，超出方向居中裁剪（等比放大）。
    Cover,
}

/// 图像纹理是否生成 mipmap，对应 `skia_safe::gpu::Mipmapped`。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Mipmapped {
    #[default]
    No,
    Yes,
}

/// 一次图像绘制的完整参数。
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ImageOptions {
    /// 源矩形；`None` 表示使用整张图像（对应 `drawImageRect` 的单矩形重载）。
    pub src: Option<Rect>,
    pub sampling: Sampling,
    pub constraint: SrcConstraint,
    pub fit: ImageFit,
    /// 整体不透明度（0..=255）。后端图像绘制只用 Paint 的 alpha 调制像素、RGB 不参与，
    /// 因此这里只表达 alpha（对应原 `Paint::set_alpha` / `set_alpha_f`）。
    pub alpha: u8,
}

impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            src: None,
            sampling: Sampling::Default,
            constraint: SrcConstraint::Fast,
            fit: ImageFit::Fill,
            alpha: 255,
        }
    }
}

impl ImageOptions {
    pub fn with_sampling(sampling: Sampling) -> Self {
        Self {
            sampling,
            ..Self::default()
        }
    }

    pub fn with_src(mut self, src: Rect) -> Self {
        self.src = Some(src);
        self
    }

    pub fn with_constraint(mut self, constraint: SrcConstraint) -> Self {
        self.constraint = constraint;
        self
    }

    pub fn with_fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn with_alpha(mut self, alpha: u8) -> Self {
        self.alpha = alpha;
        self
    }
}
