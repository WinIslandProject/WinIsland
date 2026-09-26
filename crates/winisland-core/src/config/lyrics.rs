use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, IntoStaticStr};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(from = "String", into = "&'static str")]
#[derive(Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum LyricTransitionMode {
    Random,
    #[default]
    Blur,
    Slide,
    Fade,
}

impl LyricTransitionMode {
    pub const fn animation(self, random_value: u64) -> LyricTransitionAnimation {
        match self {
            Self::Random => match random_value % 3 {
                0 => LyricTransitionAnimation::Blur,
                1 => LyricTransitionAnimation::Slide,
                _ => LyricTransitionAnimation::Fade,
            },
            Self::Blur => LyricTransitionAnimation::Blur,
            Self::Slide => LyricTransitionAnimation::Slide,
            Self::Fade => LyricTransitionAnimation::Fade,
        }
    }
}

impl From<String> for LyricTransitionMode {
    fn from(value: String) -> Self {
        value.parse().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricTransitionAnimation {
    Blur,
    Slide,
    Fade,
}
