/// 字宽档位，取值与 Skia `SkFontStyle::Width` 一致（1..=9，5 为 NORMAL）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FontWidth(u8);

/// 字重，取值与 Skia `SkFontStyle::Weight` 一致（100..=900，另允许 0 与 950）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FontWeight(u16);

/// 字形倾斜，数值与 Skia `SkFontStyle::Slant` 一致。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Slant {
    #[default]
    Upright,
    Italic,
    Oblique,
}

/// 字体样式：字重 + 字宽 + 倾斜。
///
/// 不变量：三个分量的取值都等于 Skia 原生枚举值，使文本测量缓存的键
/// `(weight << 16) | (width << 8) | slant` 与迁移前逐位相同。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FontStyle {
    weight: FontWeight,
    width: FontWidth,
    slant: Slant,
}

impl FontStyle {
    /// 对应 Skia `FontStyle::normal()`：字重 400、字宽 5、直立。
    pub const fn normal() -> Self {
        Self {
            weight: FontWeight(400),
            width: FontWidth(5),
            slant: Slant::Upright,
        }
    }

    /// 对应 Skia `FontStyle::bold()`：字重 700、字宽 5、直立。
    pub const fn bold() -> Self {
        Self {
            weight: FontWeight(700),
            width: FontWidth(5),
            slant: Slant::Upright,
        }
    }

    /// 对应 Skia `FontStyle::italic()`：字重 400、字宽 5、斜体。
    pub const fn italic() -> Self {
        Self {
            weight: FontWeight(400),
            width: FontWidth(5),
            slant: Slant::Italic,
        }
    }

    /// 对应 Skia `FontStyle::bold_italic()`：字重 700、字宽 5、斜体。
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

    /// 复刻既有 `utils/font.rs` 的样式缓存键打包：`(weight << 16) | (width << 8) | slant`。
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

    /// `needs_synthetic_bold` 的判定阈值：请求字重 ≥ 600 而字体的原生字重 < 600 时，
    /// 由 `Font::set_embolden` 合成加粗。
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
