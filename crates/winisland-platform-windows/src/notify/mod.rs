mod events;
mod read;
mod toast;

use std::sync::OnceLock;
use winisland_platform::{
    NotificationFeed, NotificationIconData, NotificationMonitorUpdate, NotificationProvider,
};

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use windows::ApplicationModel::AppDisplayInfo;
use windows::Foundation::Size;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::NotificationKinds;
use windows::core::HRESULT;

use self::events::{self as notification_event, NotificationEventSubscription};

const RETRY_INTERVAL: Duration = Duration::from_secs(5);

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
        *self.0.lock() = Some(Box::new(cancel));
    }

    fn cancel(&self) {
        if let Some(cancel) = self.0.lock().take() {
            cancel();
        }
    }
}

enum NotificationReadResult {
    Notifications(Vec<NotificationRecord>),
    Failed(HRESULT),
}

#[derive(Default)]
pub struct WindowsNotificationFeed {
    listener: Option<UserNotificationListener>,
    event_subscription: Option<NotificationEventSubscription>,
    subscription_status: Option<bool>,
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

impl WindowsNotificationFeed {
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
                        wake();
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
                Ok(subscription) => {
                    self.event_subscription = Some(subscription);
                    self.subscription_status = Some(true);
                }
                Err(error) => {
                    self.subscription_status = Some(false);
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
        self.subscription_status = Some(false);
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
                wake();
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
                wake();
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

impl Drop for WindowsNotificationFeed {
    fn drop(&mut self) {
        self.stop();
    }
}

use read::{read_app_icon, read_notification};

static WAKER: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();

fn wake() {
    if let Some(waker) = WAKER.get() {
        waker();
    }
}

pub struct WindowsNotifications;

pub(crate) fn events_available() -> bool {
    events::NotificationEventSubscription::subscribe().is_ok()
}

impl NotificationProvider for WindowsNotifications {
    fn open_feed(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Box<dyn NotificationFeed> {
        let _ = WAKER.set(waker);
        Box::new(WindowsNotificationFeed::default())
    }

    fn set_app_identity(&self) {
        toast::set_app_identity();
    }

    fn show_toast(&self, title: &str, message: &str) {
        toast::show(title, message);
    }
}

impl NotificationFeed for WindowsNotificationFeed {
    fn update(&mut self, enabled: bool) -> Option<NotificationMonitorUpdate> {
        WindowsNotificationFeed::update(self, enabled)
    }
    fn dismiss(&self, id: u32) {
        self.remove_notification(id);
    }

    fn events_available(&self) -> Option<bool> {
        self.subscription_status
    }
}
