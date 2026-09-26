//! Platform capability contracts and value types used by the application.
//!
//! This crate has no platform implementation or operating system dependency.

mod audio;
mod caps;
mod display;
mod error;
mod events;
mod input;
mod media;
mod metrics;
mod notify;
mod shell;
mod values;
mod window;

pub use audio::{AudioMeter, AudioProvider, ProcessCapture, VolumeEndpoint};
pub use caps::Capabilities;
pub use display::{BrightnessFeed, DisplayProvider};
pub use error::PlatformError;
pub use events::{AppHandler, PlatformEvent};
pub use input::InputHooks;
pub use media::{MediaContext, MediaProvider, MediaSessionHandle, ThumbnailError};
pub use metrics::SystemMetrics;
pub use notify::{NotificationFeed, NotificationProvider};
pub use shell::{InstanceLock, ShellIntegration};
pub use values::*;
pub use window::{NativeSurface, SURFACE_TAG_WIN32_HWND, WindowSystem};

pub struct Platform {
    pub shell: Box<dyn ShellIntegration>,
    pub metrics: Box<dyn SystemMetrics>,
    pub display: Box<dyn DisplayProvider>,
    pub audio: Box<dyn AudioProvider>,
    pub media: Box<dyn MediaProvider>,
    pub notify: Box<dyn NotificationProvider>,
    pub input: Box<dyn InputHooks>,
    pub capabilities: Capabilities,
}
