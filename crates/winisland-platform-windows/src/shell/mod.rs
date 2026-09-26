mod activate;
mod autostart;
mod cjk;
mod instance;
mod links;
mod locale;
mod paths;
mod tray;
mod update;

use std::path::{Path, PathBuf};

use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::WindowsAndMessaging::{
    IDOK, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_OKCANCEL, MB_SETFOREGROUND, MB_TOPMOST,
    MESSAGEBOX_STYLE, MessageBoxW,
};
use windows::core::HSTRING;
use winisland_platform::{
    InstanceLock, LocalDateTime, PlatformError, ShellIntegration, TrayAction, TrayLabels, TrayTheme,
};

pub struct WindowsShell;

impl ShellIntegration for WindowsShell {
    fn config_dir(&self) -> PathBuf {
        paths::config_dir()
    }
    fn data_dir(&self) -> PathBuf {
        paths::data_dir()
    }
    fn log_dir(&self) -> PathBuf {
        paths::log_dir()
    }
    fn locale(&self) -> String {
        locale::system_locale()
    }
    fn local_datetime(&self) -> LocalDateTime {
        // SAFETY: GetLocalTime returns a fully initialized SYSTEMTIME value.
        let time = unsafe { GetLocalTime() };
        LocalDateTime {
            year: time.wYear,
            month: time.wMonth,
            day: time.wDay,
            day_of_week: time.wDayOfWeek,
            hour: time.wHour,
            minute: time.wMinute,
            second: time.wSecond,
            millisecond: time.wMilliseconds,
        }
    }
    fn to_simplified(&self, text: &str) -> String {
        cjk::to_simplified(text)
    }
    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        links::open_url(url)
    }
    fn reveal_path(&self, path: &Path) -> Result<(), PlatformError> {
        links::reveal_path(path)
    }
    fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        autostart::enabled()
    }
    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError> {
        autostart::set(enabled)
    }
    fn acquire_single_instance(
        &self,
        key: &str,
    ) -> Result<Option<Box<dyn InstanceLock>>, PlatformError> {
        instance::acquire(key)
    }
    fn restart(&self, args: &[String]) -> Result<(), PlatformError> {
        std::process::Command::new(std::env::current_exe().map_err(PlatformError::backend)?)
            .args(args)
            .spawn()
            .map(|_| ())
            .map_err(PlatformError::backend)
    }
    fn fatal_dialog(&self, title: &str, body: &str) {
        show_dialog(title, body, MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST);
    }
    fn information_dialog(&self, title: &str, body: &str) {
        show_dialog(title, body, MB_ICONINFORMATION | MB_TOPMOST);
    }
    fn confirm_information(&self, title: &str, body: &str) -> bool {
        let result = show_dialog(
            title,
            body,
            MB_OKCANCEL | MB_ICONINFORMATION | MB_TOPMOST | MB_SETFOREGROUND,
        );
        result == IDOK || result == IDYES
    }
    fn install_update(&self, package: &Path) -> Result<(), PlatformError> {
        update::install(package)
    }
    fn tray_install(&self, theme: TrayTheme, labels: TrayLabels) -> Result<(), PlatformError> {
        tray::install(theme, labels)
    }
    fn tray_update(&self, theme: TrayTheme, labels: TrayLabels) -> Result<(), PlatformError> {
        tray::update(theme, labels)
    }
    fn poll_tray_events(&self) -> Vec<TrayAction> {
        tray::poll_events()
    }
    fn activate_app(&self, app_user_model_id: &str) -> Result<bool, PlatformError> {
        activate::application(app_user_model_id)
    }
    fn activate_media_app(&self, source_app_id: &str) -> Result<bool, PlatformError> {
        activate::media_application(source_app_id)
    }
}

fn show_dialog(
    title: &str,
    body: &str,
    style: MESSAGEBOX_STYLE,
) -> windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_RESULT {
    // SAFETY: Both strings remain live and NUL-terminated for the synchronous call.
    unsafe { MessageBoxW(None, &HSTRING::from(body), &HSTRING::from(title), style) }
}
