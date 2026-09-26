use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, IntoStaticStr};

pub const TOP_OFFSET: i32 = 10;
pub const PADDING: f32 = 80.0;
pub const MIN_HIDDEN_WIDTH: f32 = 0.0;
pub const MAX_HIDDEN_WIDTH: f32 = 400.0;
pub const MAX_LYRIC_WIDTH: f32 = 700.0;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(from = "String", into = "&'static str")]
#[derive(Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
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
}

impl From<String> for DockPosition {
    fn from(value: String) -> Self {
        value.parse().unwrap_or_default()
    }
}
