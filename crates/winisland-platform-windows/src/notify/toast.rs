use windows::ApplicationModel::Package;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager, ToastTemplateType};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::{HSTRING, PCWSTR};

pub(super) fn set_app_identity() {
    if Package::Current().is_ok() {
        return;
    }
    let wide: Vec<u16> = "WinIsland.PluginManager"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: The UTF-16 buffer is NUL-terminated and remains live for this synchronous call.
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(PCWSTR::from_raw(wide.as_ptr()));
    }
}

pub(super) fn show(title: &str, message: &str) {
    set_app_identity();
    let tmpl = match ToastNotificationManager::GetTemplateContent(ToastTemplateType::ToastText02) {
        Ok(template) => template,
        Err(error) => {
            log::error!("Toast template failed: {error:?}");
            return;
        }
    };
    if let Ok(nodes) = tmpl.SelectNodes(&HSTRING::from("//text")) {
        if let Ok(node) = nodes.Item(0) {
            let _ = node.SetInnerText(&HSTRING::from(title));
        }
        if let Ok(node) = nodes.Item(1) {
            let _ = node.SetInnerText(&HSTRING::from(message));
        }
    }
    let toast = match ToastNotification::CreateToastNotification(&tmpl) {
        Ok(toast) => toast,
        Err(error) => {
            log::error!("CreateToastNotification failed: {error:?}");
            return;
        }
    };
    let notifier_result = if Package::Current().is_ok() {
        ToastNotificationManager::CreateToastNotifier()
    } else {
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
            "WinIsland.PluginManager",
        ))
    };
    let notifier = match notifier_result {
        Ok(notifier) => notifier,
        Err(error) => {
            log::error!("CreateToastNotifier failed: {error:?}");
            return;
        }
    };
    if let Err(error) = notifier.Show(&toast) {
        log::error!("Toast Show failed: {error:?}");
    }
}
