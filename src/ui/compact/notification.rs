use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use skia_safe::canvas::SrcRectConstraint;
use skia_safe::{
    Canvas, ClipOp, Color, Data, FilterMode, FontStyle, Image, MipmapMode, Paint, RRect, Rect,
    SamplingOptions,
};
use windows::ApplicationModel::AppDisplayInfo;
use windows::Foundation::Size;
use windows::Storage::Streams::{DataReader, IRandomAccessStreamWithContentType};
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};
use windows::core::HRESULT;

use crate::ui::compact::notification_event::{self, NotificationEventSubscription};
use crate::ui::compact::{CompactOverlayState, CompactSize};
use crate::utils::font::DrawTextCachedParams;
use crate::utils::scroll::{ScrollDrawParams, ScrollText};

const DISPLAY_DURATION: Duration = Duration::from_secs(5);
const ENTER_DURATION: Duration = Duration::from_millis(220);
const FADE_DURATION: Duration = Duration::from_millis(280);
const DETAIL_LINE_GAP: f32 = 21.0;
const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;
const RETRY_INTERVAL: Duration = Duration::from_secs(5);

pub(super) struct NotificationPayload {
    notification_id: u32,
    app_name: String,
    app_user_model_id: Option<String>,
    title: String,
    detail: String,
    icon: Option<NotificationIconData>,
}

pub(super) enum NotificationMonitorUpdate {
    Notification(NotificationPayload),
    Icon {
        notification_id: u32,
        icon: NotificationIconData,
    },
}

pub(super) struct NotificationIconData {
    bytes: Vec<u8>,
    visible_bounds: Option<IconBounds>,
}

struct NotificationIcon {
    image: Image,
    visible_bounds: Option<IconBounds>,
}

#[derive(Clone, Copy)]
struct IconBounds {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy)]
enum NotificationReadKind {
    Baseline,
    Reconcile,
    Event,
}

struct NotificationRecord {
    id: u32,
    creation_time: i64,
}

#[derive(Default)]
struct CancelSlot(Mutex<Option<Box<dyn Fn() + Send + Sync>>>);

impl CancelSlot {
    fn set(&self, cancel: impl Fn() + Send + Sync + 'static) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Box::new(cancel));
    }

    fn cancel(&self) {
        if let Some(cancel) = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            cancel();
        }
    }
}

enum NotificationReadResult {
    Notifications(Vec<NotificationRecord>),
    Failed(HRESULT),
}

#[derive(Default)]
pub(super) struct NotificationMonitor {
    listener: Option<UserNotificationListener>,
    event_subscription: Option<NotificationEventSubscription>,
    access_receiver: Option<Receiver<bool>>,
    access_cancel: Option<Arc<CancelSlot>>,
    read_receiver: Option<Receiver<(NotificationReadKind, NotificationReadResult)>>,
    read_cancel: Option<Arc<CancelSlot>>,
    icon_receiver: Option<Receiver<(u32, Option<NotificationIconData>)>>,
    icon_cancel: Option<Arc<CancelSlot>>,
    access_attempted: bool,
    retry_after: Option<Instant>,
    seen_notifications: HashSet<(u32, i64)>,
    pending_notification_id: Option<u32>,
}

impl NotificationMonitor {
    pub(super) fn update(&mut self, enabled: bool) -> Option<NotificationMonitorUpdate> {
        if !enabled {
            self.stop();
            self.access_attempted = false;
            self.retry_after = None;
            return None;
        }

        self.finish_access_request();
        if self.listener.is_none()
            && self.access_receiver.is_none()
            && !self.access_attempted
            && self
                .retry_after
                .is_none_or(|retry_after| Instant::now() >= retry_after)
        {
            self.access_attempted = true;
            self.request_access();
        }
        self.finish_notification_read();
        self.handle_subscription_error();
        self.read_notifications_when_signaled();
        self.take_update()
    }

    fn request_access(&mut self) {
        let Ok(listener) = UserNotificationListener::Current() else {
            log::warn!("Notification listener is unavailable");
            self.schedule_retry();
            return;
        };
        match listener.GetAccessStatus() {
            Ok(UserNotificationListenerAccessStatus::Allowed) => self.start_monitor(listener),
            Ok(UserNotificationListenerAccessStatus::Unspecified) => {
                let Ok(operation) = listener.RequestAccessAsync() else {
                    log::warn!("Notification access request could not be started");
                    self.schedule_retry();
                    return;
                };
                let (sender, receiver) = mpsc::sync_channel(1);
                let cancel_operation = operation.clone();
                let cancel = Arc::new(CancelSlot::default());
                cancel.set(move || {
                    let _ = cancel_operation.Cancel();
                });
                self.access_cancel = Some(cancel);
                tokio::task::spawn_blocking(move || {
                    let granted = matches!(
                        operation.join(),
                        Ok(UserNotificationListenerAccessStatus::Allowed)
                    );
                    if sender.send(granted).is_ok() {
                        crate::utils::event_loop::wake();
                    }
                });
                self.access_receiver = Some(receiver);
            }
            Ok(status) => log::warn!("Notification access was not granted: {status:?}"),
            Err(error) => {
                log::warn!("Notification access status is unavailable: {error:?}");
                self.schedule_retry();
            }
        }
    }

    fn finish_access_request(&mut self) {
        let result = self
            .access_receiver
            .as_ref()
            .map(std::sync::mpsc::Receiver::try_recv);
        match result {
            Some(Ok(true)) => {
                self.access_receiver = None;
                self.access_cancel = None;
                let Ok(listener) = UserNotificationListener::Current() else {
                    log::warn!("Notification listener is unavailable after access was granted");
                    self.schedule_retry();
                    return;
                };
                self.start_monitor(listener);
            }
            Some(Ok(false)) => {
                self.access_receiver = None;
                self.access_cancel = None;
                log::warn!("Notification access was not granted");
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.access_receiver = None;
                self.access_cancel = None;
                log::warn!("Notification access request ended unexpectedly");
                self.schedule_retry();
            }
            Some(Err(mpsc::TryRecvError::Empty)) | None => {}
        }
    }

    fn start_monitor(&mut self, listener: UserNotificationListener) {
        self.listener = Some(listener);
        self.retry_after = None;
        self.seen_notifications.clear();
        self.pending_notification_id = None;
        notification_event::reset_signals();
        if !self.start_notification_read(NotificationReadKind::Baseline) {
            self.restart_monitor();
        }
    }

    fn finish_notification_read(&mut self) {
        let result = self
            .read_receiver
            .as_ref()
            .map(std::sync::mpsc::Receiver::try_recv);
        match result {
            Some(Ok((kind, NotificationReadResult::Notifications(notifications)))) => {
                self.read_receiver = None;
                self.read_cancel = None;
                self.handle_notification_snapshot(kind, notifications);
            }
            Some(Ok((_, NotificationReadResult::Failed(error)))) => {
                self.read_receiver = None;
                self.read_cancel = None;
                log::warn!("Notification history could not be read: {error:?}");
                self.restart_monitor();
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.read_receiver = None;
                self.read_cancel = None;
                log::warn!("Notification history request ended unexpectedly");
                self.restart_monitor();
            }
            Some(Err(mpsc::TryRecvError::Empty)) | None => {}
        }
    }

    fn handle_notification_snapshot(
        &mut self,
        kind: NotificationReadKind,
        mut notifications: Vec<NotificationRecord>,
    ) {
        notifications
            .sort_unstable_by_key(|notification| (notification.creation_time, notification.id));
        if matches!(kind, NotificationReadKind::Baseline) {
            for notification in notifications {
                self.seen_notifications
                    .insert((notification.id, notification.creation_time));
            }
            match NotificationEventSubscription::subscribe() {
                Ok(subscription) => self.event_subscription = Some(subscription),
                Err(error) => {
                    log::warn!("Notification event subscription could not be started: {error}");
                    self.restart_monitor();
                    return;
                }
            }
            if !self.start_notification_read(NotificationReadKind::Reconcile) {
                self.restart_monitor();
            }
            return;
        }

        let mut current_notifications = HashSet::with_capacity(notifications.len());
        let mut newest_notification_id = None;
        for notification in notifications {
            let identity = (notification.id, notification.creation_time);
            current_notifications.insert(identity);
            if !self.seen_notifications.contains(&identity) {
                newest_notification_id = Some(notification.id);
            }
        }
        self.seen_notifications = current_notifications;
        if newest_notification_id.is_some() {
            self.pending_notification_id = newest_notification_id;
        }
    }

    fn handle_subscription_error(&mut self) {
        let Some(error_code) = notification_event::take_error() else {
            return;
        };
        log::warn!("Notification event subscription failed with Win32 error {error_code}");
        if self
            .event_subscription
            .as_ref()
            .is_some_and(NotificationEventSubscription::has_wnf)
        {
            return;
        }
        self.restart_monitor();
    }

    fn read_notifications_when_signaled(&mut self) {
        if self.event_subscription.is_none()
            || self.read_receiver.is_some()
            || !notification_event::take_delivery()
        {
            return;
        }
        if !self.start_notification_read(NotificationReadKind::Event) {
            self.restart_monitor();
        }
    }

    fn start_notification_read(&mut self, kind: NotificationReadKind) -> bool {
        let Some(listener) = self.listener.as_ref() else {
            return false;
        };
        let Ok(operation) = listener.GetNotificationsAsync(NotificationKinds::Toast) else {
            log::warn!("Notification history request could not be started");
            return false;
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancel_operation = operation.clone();
        let cancel = Arc::new(CancelSlot::default());
        cancel.set(move || {
            let _ = cancel_operation.Cancel();
        });
        self.read_cancel = Some(cancel);
        tokio::task::spawn_blocking(move || {
            let result = operation.join().map(|notifications| {
                let mut records = Vec::new();
                if let Ok(count) = notifications.Size() {
                    for index in 0..count {
                        if let Ok(notification) = notifications.GetAt(index)
                            && let Ok(notification_id) = notification.Id()
                        {
                            let creation_time = notification
                                .CreationTime()
                                .map(|time| time.UniversalTime)
                                .unwrap_or_default();
                            records.push(NotificationRecord {
                                id: notification_id,
                                creation_time,
                            });
                        }
                    }
                }
                records
            });
            let result = match result {
                Ok(notifications) => NotificationReadResult::Notifications(notifications),
                Err(error) => NotificationReadResult::Failed(error.code()),
            };
            if sender.send((kind, result)).is_ok() {
                crate::utils::event_loop::wake();
            }
        });
        self.read_receiver = Some(receiver);
        true
    }

    fn stop(&mut self) {
        for cancel in [
            self.access_cancel.take(),
            self.read_cancel.take(),
            self.icon_cancel.take(),
        ]
        .into_iter()
        .flatten()
        {
            cancel.cancel();
        }
        self.listener = None;
        self.event_subscription = None;
        self.access_receiver = None;
        self.read_receiver = None;
        self.icon_receiver = None;
        self.seen_notifications.clear();
        self.pending_notification_id = None;
        notification_event::reset_signals();
    }

    fn restart_monitor(&mut self) {
        self.stop();
        self.schedule_retry();
    }

    fn schedule_retry(&mut self) {
        self.access_attempted = false;
        self.retry_after = Some(Instant::now() + RETRY_INTERVAL);
    }

    fn take_update(&mut self) -> Option<NotificationMonitorUpdate> {
        if let Some(notification_id) = self.pending_notification_id.take()
            && let Some(listener) = self.listener.as_ref()
            && let Some((payload, display)) = read_notification(listener, notification_id)
        {
            if let Some(display) = display {
                self.start_icon_read(notification_id, display);
            }
            return Some(NotificationMonitorUpdate::Notification(payload));
        }
        self.finish_icon_read()
    }

    fn start_icon_read(&mut self, notification_id: u32, display: AppDisplayInfo) {
        if let Some(cancel) = self.icon_cancel.take() {
            cancel.cancel();
        }
        self.icon_receiver = None;
        let Some(operation) = display
            .GetLogo(Size {
                Width: 64.0,
                Height: 64.0,
            })
            .ok()
            .and_then(|logo| logo.OpenReadAsync().ok())
        else {
            return;
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancel_operation = operation.clone();
        let cancel = Arc::new(CancelSlot::default());
        cancel.set(move || {
            let _ = cancel_operation.Cancel();
        });
        self.icon_cancel = Some(Arc::clone(&cancel));
        tokio::task::spawn_blocking(move || {
            let icon = operation
                .join()
                .ok()
                .and_then(|stream| read_app_icon(stream, &cancel));
            if sender.send((notification_id, icon)).is_ok() {
                crate::utils::event_loop::wake();
            }
        });
        self.icon_receiver = Some(receiver);
    }

    fn finish_icon_read(&mut self) -> Option<NotificationMonitorUpdate> {
        let result = self
            .icon_receiver
            .as_ref()
            .map(std::sync::mpsc::Receiver::try_recv);
        match result {
            Some(Ok((notification_id, icon))) => {
                self.icon_receiver = None;
                self.icon_cancel = None;
                icon.map(|icon| NotificationMonitorUpdate::Icon {
                    notification_id,
                    icon,
                })
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.icon_receiver = None;
                self.icon_cancel = None;
                None
            }
            Some(Err(mpsc::TryRecvError::Empty)) | None => None,
        }
    }

    pub(super) fn remove_notification(&self, notification_id: u32) {
        if let Some(listener) = &self.listener
            && let Err(error) = listener.RemoveNotification(notification_id)
        {
            log::debug!("Notification could not be removed: {error:?}");
        }
    }
}

impl Drop for NotificationMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn read_notification(
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

fn read_app_icon(
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

#[derive(Default)]
pub(super) struct NotificationIndicator {
    notification_id: Option<u32>,
    app_name: String,
    app_user_model_id: Option<String>,
    title: String,
    detail: String,
    icon: Option<NotificationIcon>,
    pending: Option<NotificationPayload>,
    display_started: Option<Instant>,
    display_until: Option<Instant>,
    app_name_scroll: RefCell<ScrollText>,
    title_scroll: RefCell<ScrollText>,
    detail_scroll: RefCell<ScrollText>,
}

impl NotificationIndicator {
    pub(super) fn update(
        &mut self,
        update: Option<NotificationMonitorUpdate>,
        state: CompactOverlayState,
    ) -> bool {
        if self
            .display_until
            .is_some_and(|until| until + FADE_DURATION <= Instant::now())
        {
            self.clear_display();
        }
        let received_notification = match update {
            Some(NotificationMonitorUpdate::Notification(notification)) => {
                self.pending = Some(notification);
                true
            }
            Some(NotificationMonitorUpdate::Icon {
                notification_id,
                icon,
            }) => {
                self.update_icon(notification_id, icon);
                false
            }
            None => false,
        };
        if !matches!(state, CompactOverlayState::Present) {
            self.clear_display();
            if matches!(state, CompactOverlayState::Discard) {
                self.pending = None;
            }
            return received_notification;
        }
        let Some(notification) = self.pending.take() else {
            return received_notification;
        };

        let NotificationPayload {
            notification_id,
            app_name,
            app_user_model_id,
            title,
            detail,
            icon,
        } = notification;
        self.app_name = if title.trim().eq_ignore_ascii_case(app_name.trim()) {
            String::new()
        } else {
            app_name
        };
        self.title = title;
        self.detail = detail;
        self.notification_id = Some(notification_id);
        self.app_user_model_id = app_user_model_id;
        self.icon = icon.and_then(decode_notification_icon);
        self.display_started = Some(Instant::now());
        self.display_until = Some(Instant::now() + DISPLAY_DURATION);
        self.reset_text_scroll();
        received_notification
    }

    fn update_icon(&mut self, notification_id: u32, icon: NotificationIconData) {
        if self.notification_id == Some(notification_id) {
            self.icon = decode_notification_icon(icon);
        } else if let Some(pending) = self
            .pending
            .as_mut()
            .filter(|pending| pending.notification_id == notification_id)
        {
            pending.icon = Some(icon);
        }
    }

    pub(super) fn clear(&mut self) {
        self.pending = None;
        self.clear_display();
    }

    pub(super) fn activate(&mut self) -> Option<u32> {
        let app_user_model_id = self.app_user_model_id.as_deref()?;
        if crate::utils::win32::activate_application(app_user_model_id) {
            self.take_notification_id()
        } else {
            None
        }
    }

    pub(super) fn dismiss(&mut self) -> Option<u32> {
        self.take_notification_id()
    }

    fn clear_display(&mut self) {
        self.notification_id = None;
        self.app_name = String::new();
        self.app_user_model_id = None;
        self.title = String::new();
        self.detail = String::new();
        self.display_started = None;
        self.display_until = None;
        self.icon = None;
        self.reset_text_scroll();
    }

    fn take_notification_id(&mut self) -> Option<u32> {
        let notification_id = self.notification_id.take();
        self.clear_display();
        notification_id
    }

    pub(super) fn is_visible(&self) -> bool {
        self.display_until
            .is_some_and(|until| until + FADE_DURATION > Instant::now())
    }

    pub(super) fn target_size(base_width: f32, base_height: f32, scale: f32) -> CompactSize {
        CompactSize {
            width: base_width.max(330.0) * scale,
            height: base_height.max(82.0) * scale,
        }
    }

    pub(super) fn draw(&self, canvas: &Canvas, rect: Rect, scale: f32, alpha: f32) {
        let (opacity, offset_y) = self.presentation();
        let alpha = (alpha * opacity * 255.0).round().clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            return;
        }

        canvas.save();
        canvas.translate((0.0, offset_y * scale));

        let has_icon = self.icon.is_some();
        let content_left = if has_icon {
            rect.left() + 72.0 * scale
        } else {
            rect.left() + 20.0 * scale
        };
        let content_width = rect.right() - 18.0 * scale - content_left;
        if content_width <= 0.0 {
            canvas.restore();
            return;
        }

        if let Some(icon) = &self.icon {
            draw_notification_icon(canvas, icon, rect, scale, alpha);
        }

        let mut app_paint = Paint::default();
        app_paint.set_anti_alias(true);
        app_paint.set_color(Color::from_argb((alpha as f32 * 0.65) as u8, 255, 255, 255));
        let mut title_paint = Paint::default();
        title_paint.set_anti_alias(true);
        title_paint.set_color(Color::from_argb(alpha, 255, 255, 255));
        let mut detail_paint = Paint::default();
        detail_paint.set_anti_alias(true);
        detail_paint.set_color(Color::from_argb((alpha as f32 * 0.72) as u8, 255, 255, 255));

        let top = rect.top();
        if !self.app_name.is_empty() {
            draw_notification_text(
                &self.app_name_scroll,
                DrawTextCachedParams {
                    canvas,
                    text: &self.app_name,
                    x: content_left,
                    y: top + 22.0 * scale,
                    size: 11.0 * scale,
                    bold: false,
                    paint: &app_paint,
                },
                content_width,
                scale,
            );
        }
        let title_y = if self.app_name.is_empty() && self.detail.is_empty() {
            top + (rect.height() + 13.0 * scale) / 2.0
        } else if self.app_name.is_empty() {
            top + 34.0 * scale
        } else {
            top + 43.0 * scale
        };
        draw_notification_text(
            &self.title_scroll,
            DrawTextCachedParams {
                canvas,
                text: &self.title,
                x: content_left,
                y: title_y,
                size: 13.0 * scale,
                bold: true,
                paint: &title_paint,
            },
            content_width,
            scale,
        );
        if !self.detail.is_empty() {
            draw_notification_text(
                &self.detail_scroll,
                DrawTextCachedParams {
                    canvas,
                    text: &self.detail,
                    x: content_left,
                    y: title_y + DETAIL_LINE_GAP * scale,
                    size: 11.0 * scale,
                    bold: false,
                    paint: &detail_paint,
                },
                content_width,
                scale,
            );
        }
        canvas.restore();
    }

    fn presentation(&self) -> (f32, f32) {
        let Some(started) = self.display_started else {
            return (0.0, 0.0);
        };
        let Some(until) = self.display_until else {
            return (0.0, 0.0);
        };
        let now = Instant::now();
        let enter = ease_out_cubic(
            (now.saturating_duration_since(started).as_secs_f32() / ENTER_DURATION.as_secs_f32())
                .clamp(0.0, 1.0),
        );
        let exit_elapsed = now.saturating_duration_since(until);
        let exit = if exit_elapsed.is_zero() {
            1.0
        } else {
            (1.0 - exit_elapsed.as_secs_f32() / FADE_DURATION.as_secs_f32()).clamp(0.0, 1.0)
        };
        (enter * exit, (1.0 - enter) * 7.0)
    }

    fn reset_text_scroll(&self) {
        self.app_name_scroll.borrow_mut().reset();
        self.title_scroll.borrow_mut().reset();
        self.detail_scroll.borrow_mut().reset();
    }
}

fn decode_notification_icon(icon: NotificationIconData) -> Option<NotificationIcon> {
    Image::from_encoded(Data::new_copy(&icon.bytes)).map(|image| NotificationIcon {
        image,
        visible_bounds: icon.visible_bounds,
    })
}

fn ease_out_cubic(value: f32) -> f32 {
    1.0 - (1.0 - value).powi(3)
}

fn draw_notification_text(
    scroll: &RefCell<ScrollText>,
    params: DrawTextCachedParams<'_>,
    max_width: f32,
    scale: f32,
) {
    let style = if params.bold {
        FontStyle::bold()
    } else {
        FontStyle::normal()
    };
    scroll.borrow_mut().draw(ScrollDrawParams {
        canvas: params.canvas,
        text: params.text,
        x: params.x,
        y: params.y,
        max_w: max_width,
        size: params.size,
        style,
        paint: params.paint,
        scale,
        render_as_paths: false,
    });
}

fn draw_notification_icon(
    canvas: &Canvas,
    icon: &NotificationIcon,
    rect: Rect,
    scale: f32,
    alpha: u8,
) {
    let size = 42.0 * scale;
    let icon_rect = Rect::from_xywh(
        rect.left() + 18.0 * scale,
        rect.center_y() - size / 2.0,
        size,
        size,
    );
    let image_width = icon.image.width() as f32;
    let image_height = icon.image.height() as f32;
    if image_width <= 0.0 || image_height <= 0.0 {
        return;
    }
    let source = icon
        .visible_bounds
        .filter(|bounds| {
            bounds.left < icon.image.width() as u32
                && bounds.top < icon.image.height() as u32
                && bounds.width > 0
                && bounds.height > 0
        })
        .map(|bounds| {
            Rect::from_xywh(
                bounds.left as f32,
                bounds.top as f32,
                bounds.width.min(icon.image.width() as u32 - bounds.left) as f32,
                bounds.height.min(icon.image.height() as u32 - bounds.top) as f32,
            )
        })
        .unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, image_width, image_height));
    let scale = (icon_rect.width() / source.width()).min(icon_rect.height() / source.height());
    let destination = Rect::from_xywh(
        icon_rect.center_x() - source.width() * scale / 2.0,
        icon_rect.center_y() - source.height() * scale / 2.0,
        source.width() * scale,
        source.height() * scale,
    );
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_alpha_f(alpha as f32 / 255.0);
    canvas.save();
    canvas.clip_rrect(
        RRect::new_rect_xy(icon_rect, 11.0 * scale, 11.0 * scale),
        ClipOp::Intersect,
        true,
    );
    canvas.draw_image_rect_with_sampling_options(
        &icon.image,
        Some((&source, SrcRectConstraint::Fast)),
        destination,
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
        &paint,
    );
    canvas.restore();
}
