use windows::ApplicationModel::Package;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager, ToastTemplateType};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::{HSTRING, h, w};

pub(super) fn set_app_identity() {
    if Package::Current().is_ok() {
        return;
    }
    // SAFETY: The app ID is a static NUL-terminated literal.
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(w!("WinIsland.PluginManager"));
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
    if let Ok(nodes) = tmpl.SelectNodes(h!("//text")) {
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
        ToastNotificationManager::CreateToastNotifierWithId(h!("WinIsland.PluginManager"))
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
