use super::color::Rgba;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PaintStyle {
    #[default]
    Fill,
    Stroke,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StrokeCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StrokeJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TileMode {
    #[default]
    Clamp,
    Mirror,
    Repeat,
    Decal,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Rgba,
}

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LayerSpec {
    Blur(BlurSpec),
}
