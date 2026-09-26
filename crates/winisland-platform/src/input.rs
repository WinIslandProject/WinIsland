use crate::{PlatformError, VirtualKey, VolumeKeyEvent};

/// Hook installation must run on a message-pumping thread and is not reentrant. The hook
/// lives until uninstall or drop; denied installation returns an error without intercepting keys.
pub trait InputHooks {
    /// Installs the volume hook on a message-pumping thread; errors leave keys native.
    fn install_volume_keys(
        &self,
        callback: Box<dyn Fn(VolumeKeyEvent) -> bool + Send + Sync>,
    ) -> Result<(), PlatformError>;
    /// Removes the hook; an error means removal could not be confirmed.
    fn uninstall_volume_keys(&self) -> Result<(), PlatformError>;
    /// Reads the current key state, or returns an OS error.
    fn key_down(&self, key: VirtualKey) -> Result<bool, PlatformError>;
    /// Reads the system double-click interval, falling back to its default.
    fn double_click_interval(&self) -> std::time::Duration;
}
