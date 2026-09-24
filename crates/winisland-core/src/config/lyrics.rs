use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(from = "String", into = "String")]
pub enum LyricTransitionMode {
    Random,
    #[default]
    Blur,
    Slide,
    Fade,
}

impl LyricTransitionMode {
    pub const ALL: [Self; 4] = [Self::Random, Self::Blur, Self::Slide, Self::Fade];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Blur => "blur",
            Self::Slide => "slide",
            Self::Fade => "fade",
        }
    }

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

impl std::str::FromStr for LyricTransitionMode {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "random" => Ok(Self::Random),
            "blur" => Ok(Self::Blur),
            "slide" => Ok(Self::Slide),
            "fade" => Ok(Self::Fade),
            _ => Err(()),
        }
    }
}

impl From<String> for LyricTransitionMode {
    fn from(value: String) -> Self {
        value.parse().unwrap_or_default()
    }
}

impl From<LyricTransitionMode> for String {
    fn from(value: LyricTransitionMode) -> Self {
        value.as_str().to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricTransitionAnimation {
    Blur,
    Slide,
    Fade,
}
