use windows::ApplicationModel::AppDisplayInfo;
use windows::Storage::Streams::{DataReader, IRandomAccessStreamWithContentType};
use windows::UI::Notifications::Management::UserNotificationListener;
use windows::UI::Notifications::{KnownNotificationBindings, UserNotification};
use winisland_platform::{IconBounds, NotificationIconData, NotificationPayload};

use super::CancelSlot;

const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;
pub(super) fn read_notification(
    listener: &UserNotificationListener,
    notification_id: u32,
) -> Option<(NotificationPayload, Option<AppDisplayInfo>)> {
    let notification = listener.GetNotification(notification_id).ok()?;
    let (mut title, detail) = read_notification_text(&notification);
    let (app_name, app_user_model_id, display) = notification
        .AppInfo()
        .ok()
        .and_then(|app| {
            let display = app.DisplayInfo().ok()?;
            let name = display
                .DisplayName()
                .map(|name| name.to_string())
                .unwrap_or_default();
            let app_user_model_id = app
                .AppUserModelId()
                .ok()
                .map(|app_user_model_id| app_user_model_id.to_string())
                .filter(|app_user_model_id| !app_user_model_id.is_empty());
            Some((name, app_user_model_id, Some(display)))
        })
        .unwrap_or_default();

    if title.is_empty() {
        title = app_name.clone();
    }
    (!title.is_empty()).then_some((
        NotificationPayload {
            notification_id,
            app_name,
            app_user_model_id,
            title,
            detail,
            icon: None,
        },
        display,
    ))
}

fn read_notification_text(notification: &UserNotification) -> (String, String) {
    let Some(binding_name) = KnownNotificationBindings::ToastGeneric().ok() else {
        return (String::new(), String::new());
    };
    let Some(binding) = notification
        .Notification()
        .ok()
        .and_then(|notification| notification.Visual().ok())
        .and_then(|visual| visual.GetBinding(&binding_name).ok())
    else {
        return (String::new(), String::new());
    };
    let Some(text_elements) = binding.GetTextElements().ok() else {
        return (String::new(), String::new());
    };
    let mut lines = Vec::new();
    for index in 0..text_elements.Size().unwrap_or(0) {
        let Some(text) = text_elements
            .GetAt(index)
            .ok()
            .and_then(|element| element.Text().ok())
            .map(|text| text.to_string())
        else {
            continue;
        };
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            lines.push(text);
        }
    }
    (
        lines.first().cloned().unwrap_or_default(),
        lines.into_iter().skip(1).collect::<Vec<_>>().join(" "),
    )
}

pub(super) fn read_app_icon(
    stream: IRandomAccessStreamWithContentType,
    cancel: &CancelSlot,
) -> Option<NotificationIconData> {
    let size = stream.Size().ok()?;
    if size == 0 || size > MAX_ICON_BYTES {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    let operation = reader.LoadAsync(size as u32).ok()?;
    let cancel_operation = operation.clone();
    cancel.set(move || {
        let _ = cancel_operation.Cancel();
    });
    operation.join().ok()?;
    let mut bytes = vec![0; size as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(NotificationIconData {
        visible_bounds: visible_icon_bounds(&bytes),
        bytes,
    })
}

fn visible_icon_bounds(bytes: &[u8]) -> Option<IconBounds> {
    const ALPHA_THRESHOLD: u8 = 8;

    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (image_width, image_height) = image.dimensions();
    let mut bounds: Option<IconBounds> = None;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] < ALPHA_THRESHOLD {
            continue;
        }
        bounds = Some(match bounds {
            Some(bounds) => {
                let right = bounds.left + bounds.width - 1;
                let bottom = bounds.top + bounds.height - 1;
                let left = bounds.left.min(x);
                let top = bounds.top.min(y);
                IconBounds {
                    left,
                    top,
                    width: right.max(x) - left + 1,
                    height: bottom.max(y) - top + 1,
                }
            }
            None => IconBounds {
                left: x,
                top: y,
                width: 1,
                height: 1,
            },
        });
    }
    bounds.filter(|bounds| {
        bounds.width > 0
            && bounds.height > 0
            && bounds.left + bounds.width <= image_width
            && bounds.top + bounds.height <= image_height
    })
}
