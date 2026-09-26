use super::geom::Rect;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Sampling {
    #[default]
    Default,
    /// `FilterMode::Linear` with `MipmapMode::None`.
    LinearNone,
    /// `FilterMode::Linear` with `MipmapMode::Linear`.
    LinearLinear,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum SrcConstraint {
    #[default]
    Fast,
    Strict,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ImageFit {
    #[default]
    Fill,
    Contain,
    Cover,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Mipmapped {
    #[default]
    No,
    Yes,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ImageOptions {
    pub src: Option<Rect>,
    pub sampling: Sampling,
    pub constraint: SrcConstraint,
    pub fit: ImageFit,
    pub alpha: u8,
    pub alpha_f: Option<f32>,
    pub anti_alias: bool,
}

impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            src: None,
            sampling: Sampling::Default,
            constraint: SrcConstraint::Fast,
            fit: ImageFit::Fill,
            alpha: 255,
            alpha_f: None,
            anti_alias: true,
        }
    }
}

impl ImageOptions {
    pub fn with_sampling(mut self, sampling: Sampling) -> Self {
        self.sampling = sampling;
        self
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
        self.alpha_f = None;
        self
    }

    pub fn with_alpha_f(mut self, alpha: f32) -> Self {
        self.alpha_f = Some(alpha);
        self
    }

    pub fn with_anti_alias(mut self, anti_alias: bool) -> Self {
        self.anti_alias = anti_alias;
        self
    }
}
