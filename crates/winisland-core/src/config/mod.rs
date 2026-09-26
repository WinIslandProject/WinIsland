mod lyrics;
mod migrate;
mod visual;
mod widget_layout;
mod widgets;

use educe::Educe;
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Educe, Settings)]
#[educe(Default)]
#[serde(default)]
pub struct AppConfig {
    #[serde(alias = "global_scale")]
    #[educe(Default = 1.0)]
    #[setting(number(min = 0.5, max = 5.0, step = 0.05, precision = 2))]
    pub compact_scale: f32,
    #[educe(Default = 1.0)]
    #[setting(number(min = 0.5, max = 5.0, step = 0.05, precision = 2))]
    pub expanded_scale: f32,
    #[educe(Default = 120.0)]
    #[setting(number(min = 40.0, max = 400.0, step = 5.0))]
    pub base_width: f32,
    #[educe(Default = 27.0)]
    #[setting(number(min = 15.0, max = 200.0, step = 2.0))]
    pub base_height: f32,
    #[educe(Default = 360.0)]
    #[setting(number(min = 200.0, max = 2000.0, step = 10.0))]
    pub expanded_width: f32,
    #[educe(Default = 200.0)]
    #[setting(number(min = 100.0, max = 1000.0, step = 10.0))]
    pub expanded_height: f32,
    #[educe(Default = true)]
    #[setting(toggle)]
    pub motion_blur: bool,
    #[educe(Default = 90)]
    #[setting(choice(
        30 => "30 FPS",
        60 => "60 FPS",
        90 => "90 FPS",
        120 => "120 FPS",
        0 => "frame_rate_native",
    ))]
    pub animation_fps: u32,
    #[educe(Default = 60)]
    #[setting(choice(30 => "30 FPS", 45 => "45 FPS", 60 => "60 FPS", 90 => "90 FPS"))]
    pub expanded_idle_fps: u32,
    #[educe(Default = "default")]
    pub island_style: String,
    #[educe(Default = true)]
    #[setting(toggle, label = "smtc_control")]
    pub smtc_enabled: bool,
    pub music_notice_acknowledged: bool,
    pub smtc_apps: Vec<String>,
    pub smtc_known_apps: Vec<String>,
    #[educe(Default = true)]
    #[setting(toggle)]
    pub show_lyrics: bool,
    #[setting(toggle)]
    pub show_secondary_lyrics: bool,
    #[educe(Default = "online")]
    #[setting(choice("online" => "lyrics_mode_online", "lrc" => "lyrics_mode_lrc"))]
    pub lyrics_mode: String,
    pub lyrics_local_dir: Option<String>,
    pub custom_font_path: Option<String>,
    pub auto_start: bool,
    #[setting(toggle)]
    pub auto_hide: bool,
    #[setting(toggle)]
    pub fullscreen_auto_hide: bool,
    #[educe(Default = 5.0)]
    #[setting(number(min = 1.0, max = 60.0, step = 1.0), label = "hide_delay")]
    pub auto_hide_delay: f32,
    #[educe(Default = 5.0)]
    #[setting(number(min = MIN_HIDDEN_WIDTH, max = MAX_HIDDEN_WIDTH, step = 1.0))]
    pub hidden_width: f32,
    #[educe(Default = true)]
    #[setting(toggle, label = "check_updates")]
    pub check_for_updates: bool,
    #[educe(Default = 4.0)]
    #[setting(number(min = 1.0, max = 24.0, step = 1.0), label = "update_interval")]
    pub update_check_interval: f32,
    #[educe(Default = "auto")]
    pub language: String,
    #[educe(Default = "163")]
    #[setting(choice(
        "163" => "lyrics_source_163",
        "qq" => "lyrics_source_qq",
        "kugou" => "lyrics_source_kugou",
        "lrclib" => "lyrics_source_lrclib",
        "amll" => "AMLL",
    ))]
    pub lyrics_source: String,
    #[setting(number(min = -10.0, max = 10.0, step = 0.1, precision = 1))]
    pub lyrics_delay: f64,
    #[setting(toggle)]
    pub lyrics_scroll: bool,
    #[educe(Default = 300.0)]
    #[setting(number(min = 100.0, max = 500.0, step = 10.0))]
    pub lyrics_scroll_max_width: f32,
    #[educe(Default = 6.0)]
    #[setting(number(min = 0.0, max = 32.0, step = 1.0))]
    pub lyrics_side_gap: f32,
    #[setting(choice(
        "random" => "lyrics_transition_random",
        "blur" => "lyrics_transition_blur",
        "slide" => "lyrics_transition_slide",
        "fade" => "lyrics_transition_fade",
    ))]
    pub lyrics_transition_animation: LyricTransitionMode,
    #[setting(number(step = 5.0))]
    pub position_x_offset: i32,
    #[setting(number(step = 5.0))]
    pub position_y_offset: i32,
    #[serde(rename = "dock_position", skip_serializing_if = "Option::is_none")]
    pub legacy_dock_position: Option<DockPosition>,
    pub monitor_index: i32,
    #[setting(number(min = 0.0, max = 30.0, step = 1.0))]
    pub font_size: f32,
    #[educe(Default = "system")]
    #[setting(choice("system" => "theme_system", "light" => "theme_light", "dark" => "theme_dark"))]
    pub settings_theme: String,
    #[educe(Default = "square")]
    pub mini_cover_shape: String,
    #[educe(Default = "square")]
    pub expanded_cover_shape: String,
    pub cover_rotate: bool,
    #[educe(Default = "stable")]
    #[setting(choice("stable" => "channel_stable", "beta" => "channel_beta"))]
    pub update_channel: String,
    #[setting(toggle)]
    pub right_click_drag: bool,
    #[setting(toggle)]
    pub notification_display: bool,
    #[educe(Default = true)]
    #[setting(toggle)]
    pub replace_native_volume_flyout: bool,
    #[educe(Default = true)]
    #[setting(toggle, label = "brightness_overlay")]
    pub brightness_overlay_enabled: bool,
    #[educe(Default(expression = default_widget_layout()))]
    pub widget_layout: Vec<WidgetSlot>,
    pub plugin_widget_layout: Vec<PluginWidgetSlot>,
    pub compact_widget_layout: Vec<CompactWidgetSlot>,
    #[educe(Default(expression = default_resource_metrics()))]
    pub resource_metrics: Vec<ResourceMetricConfig>,
    #[educe(Default(expression = default_resource_metrics()))]
    pub compact_resource_metrics: Vec<ResourceMetricConfig>,
    #[educe(Default = 2)]
    pub resource_widget_columns: usize,
    #[educe(Default = 1)]
    pub resource_widget_rows: usize,
    #[serde(default)]
    #[educe(Default(expression = CONFIG_VERSION))]
    pub config_version: u32,
}
