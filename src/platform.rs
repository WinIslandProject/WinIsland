use std::sync::Mutex;

use winisland_platform::{
    AudioProvider, Capabilities, DisplayProvider, HitRegion, InputHooks, MediaProvider,
    MonitorInfo, NotificationProvider, ShellIntegration, SystemMetrics, TrayLabels, TrayTheme,
    WindowId, WindowPosition, WindowSize, WindowSystem,
};
use winisland_platform_windows as backend;

static CAPABILITIES: Mutex<Capabilities> = Mutex::new(Capabilities {
    host_backdrop: true,
    tray: true,
    toast_events: true,
    media_session: true,
    audio_loopback: true,
    volume_control: true,
    brightness_control: true,
    autostart: true,
    input_hooks: true,
});

pub(crate) fn capabilities() -> Capabilities {
    *CAPABILITIES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn update_capabilities(update: impl FnOnce(&mut Capabilities)) {
    update(
        &mut CAPABILITIES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
}

pub(crate) fn shell() -> &'static dyn ShellIntegration {
    &backend::WindowsShell
}

pub(crate) fn metrics() -> &'static dyn SystemMetrics {
    &backend::WindowsMetrics
}

pub(crate) fn display() -> &'static dyn DisplayProvider {
    &backend::WindowsDisplay
}

pub(crate) fn input() -> &'static dyn InputHooks {
    &backend::WindowsInput
}

pub(crate) fn window() -> &'static dyn WindowSystem {
    backend::window::system()
}

pub(crate) fn wake() {
    backend::window::wake();
}

#[derive(Clone, Copy)]
pub(crate) struct WindowRef(pub(crate) WindowId);

impl WindowRef {
    pub(crate) fn id(self) -> WindowId {
        self.0
    }

    pub(crate) fn request_redraw(self) {
        window().request_redraw(self.0);
    }

    pub(crate) fn set_cursor_hittest(self, enabled: bool) {
        window().set_hit_regions(self.0, &[HitRegion::WholeWindow(enabled)]);
    }

    pub(crate) fn inner_size(self) -> WindowSize {
        window().inner_size(self.0)
    }

    pub(crate) fn request_inner_size(self, size: WindowSize) -> bool {
        window().request_inner_size(self.0, size)
    }

    pub(crate) fn scale_factor(self) -> f64 {
        window().scale_factor(self.0)
    }

    pub(crate) fn set_outer_position(self, position: WindowPosition) {
        window().set_position(self.0, position);
    }

    pub(crate) fn outer_position(self) -> Option<WindowPosition> {
        window().position(self.0)
    }

    pub(crate) fn outer_size(self) -> Option<WindowSize> {
        window().size(self.0)
    }

    pub(crate) fn set_visible(self, visible: bool) {
        window().set_visible(self.0, visible);
    }

    pub(crate) fn set_minimized(self, minimized: bool) {
        window().set_minimized(self.0, minimized);
    }

    pub(crate) fn set_maximized(self, maximized: bool) {
        window().set_maximized(self.0, maximized);
    }

    pub(crate) fn is_maximized(self) -> bool {
        window().is_maximized(self.0)
    }

    pub(crate) fn is_minimized(self) -> Option<bool> {
        window().is_minimized(self.0)
    }

    pub(crate) fn target_monitor(self, index: i32) -> Option<MonitorRef> {
        let id = window().target_monitor(self.0, index)?;
        window()
            .monitors()
            .into_iter()
            .find(|monitor| monitor.id == id)
            .map(MonitorRef)
    }
}

#[derive(Clone)]
pub(crate) struct MonitorRef(pub(crate) MonitorInfo);

impl MonitorRef {
    pub(crate) fn id(&self) -> winisland_platform::MonitorId {
        self.0.id
    }

    pub(crate) fn position(&self) -> WindowPosition {
        WindowPosition {
            x: self.0.bounds.left,
            y: self.0.bounds.top,
        }
    }

    pub(crate) fn size(&self) -> WindowSize {
        WindowSize {
            width: self.0.bounds.right.saturating_sub(self.0.bounds.left) as u32,
            height: self.0.bounds.bottom.saturating_sub(self.0.bounds.top) as u32,
        }
    }

    pub(crate) fn refresh_rate_millihertz(&self) -> Option<u32> {
        self.0.refresh_rate_millihertz
    }
}

pub(crate) fn audio() -> &'static dyn AudioProvider {
    &backend::WindowsAudio
}

pub(crate) fn notify() -> &'static dyn NotificationProvider {
    &backend::WindowsNotifications
}

pub(crate) fn media() -> &'static dyn MediaProvider {
    &backend::WindowsMedia
}

pub(crate) fn system_locale() -> String {
    shell().locale()
}

pub(crate) fn to_simplified(text: &str) -> String {
    shell().to_simplified(text)
}

pub(crate) fn tray_theme(is_light: bool) -> TrayTheme {
    if is_light {
        TrayTheme::Light
    } else {
        TrayTheme::Dark
    }
}

pub(crate) fn tray_labels(visible: bool) -> TrayLabels {
    use winisland_core::i18n::tr;
    TrayLabels {
        toggle: tr(if visible { "tray_hide" } else { "tray_show" }),
        settings: tr("tray_settings"),
        restart: tr("tray_restart"),
        exit: tr("tray_exit"),
        tooltip: winisland_core::config::WINDOW_TITLE.to_string(),
    }
}
