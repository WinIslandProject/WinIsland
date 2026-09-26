mod lyrics;
mod migrate;
mod visual;
mod widget_layout;
mod widgets;

use serde::{Deserialize, Serialize};
use settings_schema::Settings;

pub use lyrics::*;
pub use migrate::*;
pub use visual::*;
pub use widget_layout::*;
pub use widgets::*;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_AUTHOR: &str = "Eatgrapes";
pub const APP_HOMEPAGE: &str = "https://github.com/WinIslandProject/WinIsland";
pub const WINDOW_TITLE: &str = "WinIsland";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Settings)]
pub struct AppConfig {
    #[serde(alias = "global_scale")]
    #[setting(number(min = 0.5, max = 5.0, step = 0.05, precision = 2))]
    pub compact_scale: f32,
    #[serde(default = "default_expanded_scale")]
    #[setting(number(min = 0.5, max = 5.0, step = 0.05, precision = 2))]
    pub expanded_scale: f32,
    #[setting(number(min = 40.0, max = 400.0, step = 5.0))]
    pub base_width: f32,
    #[setting(number(min = 15.0, max = 200.0, step = 2.0))]
    pub base_height: f32,
    #[setting(number(min = 200.0, max = 2000.0, step = 10.0))]
    pub expanded_width: f32,
    #[setting(number(min = 100.0, max = 1000.0, step = 10.0))]
    pub expanded_height: f32,
    #[setting(toggle)]
    pub motion_blur: bool,
    #[serde(default = "default_animation_fps")]
    #[setting(choice(
        30 => "30 FPS",
        60 => "60 FPS",
        90 => "90 FPS",
        120 => "120 FPS",
        0 => "frame_rate_native",
    ))]
    pub animation_fps: u32,
    #[serde(default = "default_expanded_idle_fps")]
    #[setting(choice(30 => "30 FPS", 45 => "45 FPS", 60 => "60 FPS", 90 => "90 FPS"))]
    pub expanded_idle_fps: u32,
    #[serde(default = "default_island_style")]
    pub island_style: String,
    #[setting(toggle, label = "smtc_control")]
    pub smtc_enabled: bool,
    #[serde(default)]
    pub music_notice_acknowledged: bool,
    pub smtc_apps: Vec<String>,
    #[serde(default)]
    pub smtc_known_apps: Vec<String>,
    #[serde(default = "default_show_lyrics")]
    #[setting(toggle)]
    pub show_lyrics: bool,
    #[serde(default)]
    #[setting(toggle)]
    pub show_secondary_lyrics: bool,
    #[serde(default = "default_lyrics_mode")]
    #[setting(choice("online" => "lyrics_mode_online", "lrc" => "lyrics_mode_lrc"))]
    pub lyrics_mode: String,
    #[serde(default)]
    pub lyrics_local_dir: Option<String>,
    #[serde(default)]
    pub custom_font_path: Option<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    #[setting(toggle)]
    pub auto_hide: bool,
    #[serde(default)]
    #[setting(toggle)]
    pub fullscreen_auto_hide: bool,
    #[serde(default = "default_auto_hide_delay")]
    #[setting(number(min = 1.0, max = 60.0, step = 1.0), label = "hide_delay")]
    pub auto_hide_delay: f32,
    #[serde(default = "default_hidden_width")]
    #[setting(number(min = MIN_HIDDEN_WIDTH, max = MAX_HIDDEN_WIDTH, step = 1.0))]
    pub hidden_width: f32,
    #[serde(default = "default_check_for_updates")]
    #[setting(toggle, label = "check_updates")]
    pub check_for_updates: bool,
    #[serde(default = "default_update_check_interval")]
    #[setting(number(min = 1.0, max = 24.0, step = 1.0), label = "update_interval")]
    pub update_check_interval: f32,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_lyrics_source")]
    #[setting(choice(
        "163" => "lyrics_source_163",
        "qq" => "lyrics_source_qq",
        "kugou" => "lyrics_source_kugou",
        "lrclib" => "lyrics_source_lrclib",
        "amll" => "AMLL",
    ))]
    pub lyrics_source: String,
    #[serde(default)]
    #[setting(number(min = -10.0, max = 10.0, step = 0.1, precision = 1))]
    pub lyrics_delay: f64,
    #[serde(default)]
    #[setting(toggle)]
    pub lyrics_scroll: bool,
    #[serde(default = "default_lyrics_scroll_max_width")]
    #[setting(number(min = 100.0, max = 500.0, step = 10.0))]
    pub lyrics_scroll_max_width: f32,
    #[serde(default = "default_lyrics_side_gap")]
    #[setting(number(min = 0.0, max = 32.0, step = 1.0))]
    pub lyrics_side_gap: f32,
    #[serde(default)]
    #[setting(choice(
        "random" => "lyrics_transition_random",
        "blur" => "lyrics_transition_blur",
        "slide" => "lyrics_transition_slide",
        "fade" => "lyrics_transition_fade",
    ))]
    pub lyrics_transition_animation: LyricTransitionMode,
    #[serde(default)]
    #[setting(number(step = 5.0))]
    pub position_x_offset: i32,
    #[serde(default)]
    #[setting(number(step = 5.0))]
    pub position_y_offset: i32,
    #[serde(
        rename = "dock_position",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub legacy_dock_position: Option<DockPosition>,
    #[serde(default)]
    pub monitor_index: i32,
    #[serde(default)]
    #[setting(number(min = 0.0, max = 30.0, step = 1.0))]
    pub font_size: f32,
    #[serde(default = "default_settings_theme")]
    #[setting(choice("system" => "theme_system", "light" => "theme_light", "dark" => "theme_dark"))]
    pub settings_theme: String,
    #[serde(default = "default_mini_cover_shape")]
    pub mini_cover_shape: String,
    #[serde(default = "default_expanded_cover_shape")]
    pub expanded_cover_shape: String,
    #[serde(default = "default_cover_rotate")]
    pub cover_rotate: bool,
    #[serde(default = "default_update_channel")]
    #[setting(choice("stable" => "channel_stable", "beta" => "channel_beta"))]
    pub update_channel: String,
    #[serde(default)]
    #[setting(toggle)]
    pub right_click_drag: bool,
    #[serde(default)]
    #[setting(toggle)]
    pub notification_display: bool,
    #[serde(default = "default_replace_native_volume_flyout")]
    #[setting(toggle)]
    pub replace_native_volume_flyout: bool,
    #[serde(default = "default_brightness_overlay_enabled")]
    #[setting(toggle, label = "brightness_overlay")]
    pub brightness_overlay_enabled: bool,
    #[serde(default = "default_widget_layout")]
    pub widget_layout: Vec<WidgetSlot>,
    #[serde(default)]
    pub plugin_widget_layout: Vec<PluginWidgetSlot>,
    #[serde(default)]
    pub compact_widget_layout: Vec<CompactWidgetSlot>,
    #[serde(default = "default_resource_metrics")]
    pub resource_metrics: Vec<ResourceMetricConfig>,
    #[serde(default = "default_resource_metrics")]
    pub compact_resource_metrics: Vec<ResourceMetricConfig>,
    #[serde(default = "default_resource_widget_columns")]
    pub resource_widget_columns: usize,
    #[serde(default = "default_resource_widget_rows")]
    pub resource_widget_rows: usize,
    #[serde(default)]
    pub config_version: u32,
}

macro_rules! defaults {
    ($($name:ident: $ty:ty = $value:expr),* $(,)?) => {
        $(fn $name() -> $ty { $value })*
    };
}

defaults! {
    default_true: bool = true,
    default_resource_widget_columns: usize = 2,
    default_resource_widget_rows: usize = 1,
    default_expanded_scale: f32 = 1.0,
    default_animation_fps: u32 = 90,
    default_expanded_idle_fps: u32 = 60,
    default_replace_native_volume_flyout: bool = true,
    default_brightness_overlay_enabled: bool = true,
    default_island_style: String = "default".to_string(),
    default_show_lyrics: bool = true,
    default_lyrics_mode: String = "online".to_string(),
    default_auto_hide_delay: f32 = 5.0,
    default_hidden_width: f32 = 5.0,
    default_check_for_updates: bool = true,
    default_update_check_interval: f32 = 4.0,
    default_language: String = "auto".to_string(),
    default_lyrics_source: String = "163".to_string(),
    default_lyrics_scroll_max_width: f32 = 300.0,
    default_lyrics_side_gap: f32 = 6.0,
    default_settings_theme: String = "system".to_string(),
    default_mini_cover_shape: String = "square".to_string(),
    default_expanded_cover_shape: String = "square".to_string(),
    default_cover_rotate: bool = false,
    default_update_channel: String = "stable".to_string(),
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            compact_scale: 1.0,
            expanded_scale: default_expanded_scale(),
            base_width: 120.0,
            base_height: 27.0,
            expanded_width: 360.0,
            expanded_height: 200.0,
            motion_blur: true,
            animation_fps: default_animation_fps(),
            expanded_idle_fps: default_expanded_idle_fps(),
            island_style: default_island_style(),
            smtc_enabled: true,
            music_notice_acknowledged: false,
            smtc_apps: Vec::new(),
            smtc_known_apps: Vec::new(),
            show_lyrics: default_show_lyrics(),
            show_secondary_lyrics: false,
            lyrics_mode: default_lyrics_mode(),
            lyrics_local_dir: None,
            custom_font_path: None,
            auto_start: false,
            auto_hide: false,
            fullscreen_auto_hide: false,
            auto_hide_delay: default_auto_hide_delay(),
            hidden_width: default_hidden_width(),
            check_for_updates: default_check_for_updates(),
            update_check_interval: default_update_check_interval(),
            language: default_language(),
            lyrics_source: default_lyrics_source(),
            lyrics_delay: 0.0,
            lyrics_scroll: false,
            lyrics_scroll_max_width: default_lyrics_scroll_max_width(),
            lyrics_side_gap: default_lyrics_side_gap(),
            lyrics_transition_animation: LyricTransitionMode::default(),
            position_x_offset: 0,
            position_y_offset: 0,
            legacy_dock_position: None,
            monitor_index: 0,
            font_size: 0.0,
            settings_theme: default_settings_theme(),
            mini_cover_shape: default_mini_cover_shape(),
            expanded_cover_shape: default_expanded_cover_shape(),
            cover_rotate: default_cover_rotate(),
            update_channel: default_update_channel(),
            right_click_drag: false,
            notification_display: false,
            replace_native_volume_flyout: default_replace_native_volume_flyout(),
            brightness_overlay_enabled: default_brightness_overlay_enabled(),
            widget_layout: default_widget_layout(),
            plugin_widget_layout: Vec::new(),
            compact_widget_layout: Vec::new(),
            resource_metrics: default_resource_metrics(),
            compact_resource_metrics: default_resource_metrics(),
            resource_widget_columns: default_resource_widget_columns(),
            resource_widget_rows: default_resource_widget_rows(),
            config_version: CONFIG_VERSION,
        }
    }
}
