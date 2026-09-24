use serde::{Deserialize, Serialize};

pub const TOP_OFFSET: i32 = 10;
pub const PADDING: f32 = 80.0;
pub const MIN_HIDDEN_WIDTH: f32 = 0.0;
pub const MAX_HIDDEN_WIDTH: f32 = 400.0;
pub const MAX_LYRIC_WIDTH: f32 = 700.0;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(from = "String", into = "String")]
#[derive(Default)]
pub enum DockPosition {
    #[default]
    TopCenter,
    TopLeft,
    TopRight,
    BottomCenter,
    BottomLeft,
    BottomRight,
}

impl DockPosition {
    pub const fn is_bottom(self) -> bool {
        matches!(
            self,
            Self::BottomCenter | Self::BottomLeft | Self::BottomRight
        )
    }

    pub const fn is_left(self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }

    pub const fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopCenter => "top_center",
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomCenter => "bottom_center",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        }
    }
}

impl std::fmt::Display for DockPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
    }
}

impl std::str::FromStr for DockPosition {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "top_center" => Ok(Self::TopCenter),
            "top_left" => Ok(Self::TopLeft),
            "top_right" => Ok(Self::TopRight),
            "bottom_center" => Ok(Self::BottomCenter),
            "bottom_left" => Ok(Self::BottomLeft),
            "bottom_right" => Ok(Self::BottomRight),
            _ => Err(()),
        }
    }
}

impl From<String> for DockPosition {
    fn from(value: String) -> Self {
        value.parse().unwrap_or_default()
    }
}

impl From<DockPosition> for String {
    fn from(value: DockPosition) -> Self {
        value.as_str().to_string()
    }
}
