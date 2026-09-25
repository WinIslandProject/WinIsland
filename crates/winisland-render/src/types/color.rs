/// An unpremultiplied 8-bit color packed as `0xAARRGGBB`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rgba(u32);

impl Rgba {
    pub const TRANSPARENT: Self = Self(0x0000_0000);
    pub const BLACK: Self = Self::from_rgb(0, 0, 0);
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);

    pub const fn from_argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Self(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::from_argb(a, r, g, b)
    }

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from_argb(255, r, g, b)
    }

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

    pub const fn with_alpha(self, a: u8) -> Self {
        Self((self.0 & 0x00FF_FFFF) | ((a as u32) << 24))
    }

    pub fn with_alpha_f(self, alpha: f32) -> Self {
        let clamped = alpha.clamp(0.0, 1.0);
        self.with_alpha((clamped * 255.0).round() as u8)
    }
}
