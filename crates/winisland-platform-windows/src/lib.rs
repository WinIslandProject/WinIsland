//! Windows implementations of the platform capability contracts.
//!
//! This crate owns the event loop, windows, and operating system calls; rendering stays in the application.

mod audio;
mod backdrop;
mod com;
mod display;
mod input;
mod media;
mod metrics;
mod notify;
mod process;
mod shell;
pub mod window;

pub use audio::WindowsAudio;
pub use display::WindowsDisplay;
pub use input::WindowsInput;
pub use media::WindowsMedia;
pub use metrics::WindowsMetrics;
pub use notify::WindowsNotifications;
pub use shell::WindowsShell;

use winisland_platform::{AudioProvider, Capabilities, MediaProvider, Platform, ShellIntegration};

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn probe_capabilities() -> Capabilities {
        Capabilities {
            host_backdrop: true,
            tray: true,
            toast_events: notify::events_available(),
            media_session: WindowsMedia.open_context().is_ok(),
            audio_loopback: audio::process_loopback_available(),
            volume_control: WindowsAudio.volume().is_ok(),
            brightness_control: display::brightness_available(),
            autostart: WindowsShell.autostart_enabled().is_ok(),
            input_hooks: true,
        }
    }

    pub fn create() -> Platform {
        Platform {
            shell: Box::new(WindowsShell),
            metrics: Box::new(WindowsMetrics),
            display: Box::new(WindowsDisplay),
            audio: Box::new(WindowsAudio),
            media: Box::new(WindowsMedia),
            notify: Box::new(WindowsNotifications),
            input: Box::new(WindowsInput),
            capabilities: Self::probe_capabilities(),
        }
    }
}
