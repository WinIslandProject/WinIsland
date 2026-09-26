use crate::{BrightnessSnapshot, GpuProfile, MonitorInfo, PlatformError, Point, Rect};

/// Worker-owned brightness state. Missing WMI control is reflected in snapshots.
pub trait BrightnessFeed {
    /// Returns the latest cached brightness and availability state without blocking.
    fn snapshot(&self) -> BrightnessSnapshot;
    /// Requests a level change; unavailable controls ignore the request.
    fn set_level(&self, level: f32);
}

/// Display queries block and should run away from rendering. No returned value owns a
/// native handle; unavailable controls return errors and calls are not reentrant.
pub trait DisplayProvider {
    /// Lists usable monitors, or returns a display enumeration error.
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError>;
    /// Reads the cursor position; `None` means the cursor is unavailable.
    fn cursor_position(&self) -> Result<Option<Point>, PlatformError>;
    /// Checks whether a foreign window covers the target; false on lookup failure.
    fn foreign_fullscreen_on(&self, target: Rect) -> bool;
    /// Reports whether the system cursor is hidden; false on lookup failure.
    fn cursor_hidden(&self) -> bool;
    /// Starts WMI brightness tracking on a worker, or returns an unavailable error.
    fn start_brightness_feed(&self) -> Result<Box<dyn BrightnessFeed>, PlatformError>;
    /// Returns the selected GPU profile; falls back to the default on probe failure.
    fn gpu_profile(&self) -> GpuProfile;
    /// Finds a foreign window by title; false means absent or inaccessible.
    fn foreign_window_exists(&self, title: &str) -> bool;
    /// Activates a foreign window; false means absent or activation denied.
    fn bring_foreign_window_to_front(&self, title: &str) -> bool;
}
