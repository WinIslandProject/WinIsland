use std::path::{Path, PathBuf};

use crate::{LocalDateTime, PlatformError, TrayAction, TrayLabels, TrayTheme};

/// Shell services are synchronous. Call blocking operations away from rendering;
/// returned guards own their resources, errors never panic, and methods are not reentrant.
pub trait ShellIntegration {
    /// Returns the configuration directory, using a fallback if OS lookup fails.
    fn config_dir(&self) -> PathBuf;
    /// Returns the data directory, using a fallback if OS lookup fails.
    fn data_dir(&self) -> PathBuf;
    /// Returns the log directory, using a fallback if OS lookup fails.
    fn log_dir(&self) -> PathBuf;
    /// Returns the OS locale, or the default locale on failure.
    fn locale(&self) -> String;
    /// Reads local wall time from the OS.
    fn local_datetime(&self) -> LocalDateTime;
    /// Converts text to simplified Chinese, or returns original text on failure.
    fn to_simplified(&self, text: &str) -> String;
    /// Opens a URL, or returns a shell error.
    fn open_url(&self, url: &str) -> Result<(), PlatformError>;
    /// Reveals a path, or returns a shell error.
    fn reveal_path(&self, path: &Path) -> Result<(), PlatformError>;
    /// Reads the autostart entry, or returns a registry error.
    fn autostart_enabled(&self) -> Result<bool, PlatformError>;
    /// Changes the autostart entry, or returns a registry error.
    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError>;
    /// Acquires a process lock; `None` means another instance owns it.
    fn acquire_single_instance(
        &self,
        key: &str,
    ) -> Result<Option<Box<dyn InstanceLock>>, PlatformError>;
    /// Starts a replacement process, or returns a spawn error.
    fn restart(&self, args: &[String]) -> Result<(), PlatformError>;
    /// Shows a blocking fatal error dialog; returns after dismissal.
    fn fatal_dialog(&self, title: &str, body: &str);
    /// Shows a blocking information dialog; returns after dismissal.
    fn information_dialog(&self, title: &str, body: &str);
    /// Shows a blocking confirmation dialog; false means cancel or backend failure.
    fn confirm_information(&self, title: &str, body: &str) -> bool;
    /// Installs an update package, or returns an install error.
    fn install_update(&self, package: &Path) -> Result<(), PlatformError>;
    /// Installs the tray icon, or returns an unavailable/backend error.
    fn tray_install(&self, theme: TrayTheme, labels: TrayLabels) -> Result<(), PlatformError>;
    /// Updates the tray icon, or returns a backend error.
    fn tray_update(&self, theme: TrayTheme, labels: TrayLabels) -> Result<(), PlatformError>;
    /// Drains pending tray actions; empty means no action.
    fn poll_tray_events(&self) -> Vec<TrayAction>;
    /// Activates a packaged app on a dedicated STA thread; false means not found.
    fn activate_app(&self, app_user_model_id: &str) -> Result<bool, PlatformError>;
    /// Activates a media app on a dedicated STA thread; false means not found.
    fn activate_media_app(&self, source_app_id: &str) -> Result<bool, PlatformError>;
}

/// Owns a single-instance OS lock until dropped.
pub trait InstanceLock {}
