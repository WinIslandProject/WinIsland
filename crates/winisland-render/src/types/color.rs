/// 非预乘 8 位颜色，打包为 `0xAARRGGBB`。
///
/// 不变量：所有构造与访问都按非预乘语义；预乘只在 `painter` 内部按 GPU 面（N32 premul）要求完成。
/// `from_rgb`/`from_rgba`/`from_argb`/`with_alpha` 均为 `const`，因为全仓有 11 个
/// `const COLOR_*` 常量依赖常量构造。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rgba(u32);

impl Rgba {
    /// 全透明（`0x00000000`）。
    pub const TRANSPARENT: Self = Self(0x0000_0000);
    /// 不透明黑。
    pub const BLACK: Self = Self::from_rgb(0, 0, 0);
    /// 不透明白。
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);

    /// 任意 alpha，参数顺序与 `skia_safe::Color::from_argb` 一致。
    pub const fn from_argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Self(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    /// 不透明色，参数顺序为 `r, g, b, a`。
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::from_argb(a, r, g, b)
    }

    /// 不透明色，alpha 固定为 255。
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from_argb(255, r, g, b)
    }

    /// 打包值 `0xAARRGGBB`。
    pub const fn to_argb(self) -> u32 {
        self.0
    }

    pub const fn a(self) -> u8 {
        (self.0 >> 24) as u8
    }

    pub const fn r(self) -> u8 {
        (self.0 >> 16) as u8
    }

    pub const fn g(self) -> u8 {
        (self.0 >> 8) as u8
    }

    pub const fn b(self) -> u8 {
        self.0 as u8
    }

    /// 替换 alpha，保留 r/g/b。
    pub const fn with_alpha(self, a: u8) -> Self {
        Self((self.0 & 0x00FF_FFFF) | ((a as u32) << 24))
    }

    /// 按最近 u8 量化设置 alpha，超出 `0.0..=1.0` 的输入先夹取。
    ///
    /// 与 `skia_safe::Paint::set_alpha_f` 的差别：Skia 在 `Color4f` 中保留 f32 alpha，
    /// 本方法量化到 8 位，最坏偏差 0.5/255（约 0.2% alpha）。
    pub fn with_alpha_f(self, alpha: f32) -> Self {
        let clamped = alpha.clamp(0.0, 1.0);
        self.with_alpha((clamped * 255.0).round() as u8)
    }
}
