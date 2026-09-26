use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MonitorId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub id: MonitorId,
    pub bounds: Rect,
    pub work_area: Rect,
    pub primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuProfile {
    Discrete,
    Integrated,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemSample {
    pub cpu_idle_ticks: Option<u64>,
    pub cpu_total_ticks: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub memory_load_percent: Option<u32>,
    pub network_bytes: Option<u64>,
    pub network_link_bits_per_second: Option<u64>,
    pub disk_free_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub gpu_memory_used_bytes: Option<u64>,
    pub gpu_memory_budget_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MetricSelection {
    pub cpu: bool,
    pub memory: bool,
    pub network: bool,
    pub disk: bool,
    pub gpu: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VolumeState {
    pub level: f32,
    pub muted: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum VolumeCommand {
    StepUp,
    StepDown,
    ToggleMute,
    SetLevel(f32),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BrightnessSnapshot {
    pub level: f32,
    pub revision: u64,
    pub available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeKey {
    Up,
    Down,
    Mute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeKeyEvent {
    pub key: VolumeKey,
    pub is_down: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualKey {
    LeftMouseButton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    ToggleVisibility,
    OpenSettings,
    Restart,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayTheme {
    Light,
    Dark,
}

#[derive(Clone, Debug)]
pub struct TrayLabels {
    pub toggle: String,
    pub settings: String,
    pub restart: String,
    pub exit: String,
    pub tooltip: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CaptureId(pub u64);

#[derive(Clone, Debug)]
pub struct CaptureSpec {
    pub target_app_id: Option<String>,
    pub band_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MediaSessionId(pub String);

#[derive(Clone, Debug)]
pub struct MediaSession {
    pub id: MediaSessionId,
    pub source_app_id: String,
}

#[derive(Clone, Debug, Default)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub thumbnail: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Timeline {
    pub position: Duration,
    pub duration: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Paused,
    Playing,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MediaCapabilities {
    pub play: bool,
    pub pause: bool,
    pub next: bool,
    pub previous: bool,
    pub seek: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum MediaCommand {
    Toggle,
    Play,
    Pause,
    Next,
    Previous,
    Seek(Duration),
}

#[derive(Clone, Debug)]
pub enum MediaEvent {
    SessionsChanged,
    TrackChanged(MediaSessionId),
    PlaybackChanged(MediaSessionId),
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u32,
    pub title: String,
    pub body: String,
    pub app_name: String,
    pub icon: Option<Vec<u8>>,
}

pub struct NotificationPayload {
    pub notification_id: u32,
    pub app_name: String,
    pub app_user_model_id: Option<String>,
    pub title: String,
    pub detail: String,
    pub icon: Option<NotificationIconData>,
}

pub enum NotificationMonitorUpdate {
    Notification(NotificationPayload),
    Icon {
        notification_id: u32,
        icon: NotificationIconData,
    },
}

pub struct NotificationIconData {
    pub bytes: Vec<u8>,
    pub visible_bounds: Option<IconBounds>,
}

#[derive(Clone, Copy)]
pub struct IconBounds {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationAccess {
    Allowed,
    Denied,
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct PlatformPaths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub logs: PathBuf,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LocalDateTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub day_of_week: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
}
