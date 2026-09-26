use std::sync::Mutex;

use winisland_platform::{
    AudioProvider, Capabilities, DisplayProvider, InputHooks, MediaProvider, NotificationProvider,
    ShellIntegration, SystemMetrics, TrayLabels, TrayTheme,
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
