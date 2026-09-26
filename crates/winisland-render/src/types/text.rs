#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FontWidth(u8);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FontWeight(u16);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Slant {
    #[default]
    Upright,
    Italic,
    Oblique,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FontStyle {
    weight: FontWeight,
    width: FontWidth,
    slant: Slant,
}

impl FontStyle {
    pub const fn normal() -> Self {
        Self {
            weight: FontWeight(400),
            width: FontWidth(5),
            slant: Slant::Upright,
        }
    }

    pub const fn bold() -> Self {
        Self {
            weight: FontWeight(700),
            width: FontWidth(5),
            slant: Slant::Upright,
        }
    }

    pub const fn italic() -> Self {
        Self {
            weight: FontWeight(400),
            width: FontWidth(5),
            slant: Slant::Italic,
        }
    }

    pub const fn bold_italic() -> Self {
        Self {
            weight: FontWeight(700),
            width: FontWidth(5),
            slant: Slant::Italic,
        }
    }

    pub const fn weight(self) -> FontWeight {
        self.weight
    }

    pub const fn width(self) -> FontWidth {
        self.width
    }

    pub const fn slant(self) -> Slant {
        self.slant
    }

    pub(crate) const fn cache_key(self) -> u32 {
        ((self.weight.0 as u32) << 16) | ((self.width.0 as u32) << 8) | self.slant as u32
    }
}

impl FontWeight {
    pub const fn new(weight: u16) -> Self {
        Self(weight)
    }

    pub const fn value(self) -> u16 {
        self.0
    }

    pub const fn is_at_least_semibold(self) -> bool {
        self.0 >= 600
    }
}

impl FontWidth {
    pub const fn new(width: u8) -> Self {
        Self(width)
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}
