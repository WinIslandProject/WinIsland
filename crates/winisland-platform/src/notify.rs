use std::sync::Arc;

use crate::NotificationMonitorUpdate;

/// Worker-owned notification subscription. Drop cancels subscriptions.
pub trait NotificationFeed {
    /// Enables or disables forwarding and returns a new snapshot when available.
    fn update(&mut self, enabled: bool) -> Option<NotificationMonitorUpdate>;
    /// Removes a notification; stale IDs are ignored.
    fn dismiss(&self, id: u32);
    /// Returns the known subscription state, or `None` before initialization.
    fn events_available(&self) -> Option<bool>;
}

/// Notification calls block and belong on a worker thread. Subscription state lives until
/// drop; denied access returns an explicit value and methods are not reentrant.
pub trait NotificationProvider {
    /// Opens a feed on a worker; the wake callback only signals the app loop.
    fn open_feed(&self, wake: Arc<dyn Fn() + Send + Sync>) -> Box<dyn NotificationFeed>;
    /// Sets the process toast identity; backend failures are logged.
    fn set_app_identity(&self);
    /// Requests a toast; backend failures are logged.
    fn show_toast(&self, title: &str, message: &str);
}
