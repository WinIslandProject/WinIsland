mod notification;
mod notification_event;
mod volume;

use skia_safe::{Canvas, Rect};

use self::notification::{NotificationIndicator, NotificationMonitor};
use self::volume::{VolumeIndicator, VolumeMonitor};

#[derive(Clone, Copy)]
pub struct CompactSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy)]
pub enum CompactOverlayState {
    Present,
    Defer,
    Discard,
}

pub struct CompactOverlay {
    volume_monitor: VolumeMonitor,
    volume_indicator: VolumeIndicator,
    notification_monitor: NotificationMonitor,
    notification_indicator: NotificationIndicator,
}

enum ActiveCompactOverlay<'a> {
    Notification(&'a NotificationIndicator),
    Volume(&'a VolumeIndicator),
}

impl ActiveCompactOverlay<'_> {
    fn target_size(&self, base_width: f32, base_height: f32, scale: f32) -> CompactSize {
        match self {
            Self::Notification(_) => {
                NotificationIndicator::target_size(base_width, base_height, scale)
            }
            Self::Volume(_) => VolumeIndicator::target_size(base_width, base_height, scale),
        }
    }

    fn draw(&self, canvas: &Canvas, rect: Rect, scale: f32, alpha: f32) {
        match self {
            Self::Notification(indicator) => indicator.draw(canvas, rect, scale, alpha),
            Self::Volume(indicator) => indicator.draw(canvas, rect, scale, alpha),
        }
    }
}

impl CompactOverlay {
    pub fn new(replace_native_volume_flyout: bool) -> Self {
        Self {
            volume_monitor: VolumeMonitor::new(replace_native_volume_flyout),
            volume_indicator: VolumeIndicator::default(),
            notification_monitor: NotificationMonitor::default(),
            notification_indicator: NotificationIndicator::default(),
        }
    }

    pub fn set_native_volume_flyout_replacement_enabled(&mut self, enabled: bool) {
        self.volume_monitor
            .set_native_flyout_replacement_enabled(enabled);
    }

    pub fn update(&mut self, state: CompactOverlayState, notification_display: bool) -> bool {
        self.volume_monitor
            .set_key_handling_enabled(!matches!(state, CompactOverlayState::Discard));
        let volume_changed = self
            .volume_indicator
            .update(self.volume_monitor.snapshot(), state);
        let notification = self.notification_monitor.update(notification_display);
        if notification_display {
            let notification_received = self.notification_indicator.update(notification, state);
            volume_changed || notification_received
        } else {
            self.notification_indicator.clear();
            volume_changed
        }
    }

    pub fn is_visible(&self) -> bool {
        self.active().is_some()
    }

    pub fn is_notification_visible(&self) -> bool {
        self.notification_indicator.is_visible()
    }

    pub fn activate_notification(&mut self) -> bool {
        let Some(notification_id) = self.notification_indicator.activate() else {
            return false;
        };
        self.notification_monitor
            .remove_notification(notification_id);
        true
    }

    pub fn dismiss_notification(&mut self) -> bool {
        let Some(notification_id) = self.notification_indicator.dismiss() else {
            return false;
        };
        self.notification_monitor
            .remove_notification(notification_id);
        true
    }

    pub fn target_size(
        &self,
        base_width: f32,
        base_height: f32,
        scale: f32,
    ) -> Option<CompactSize> {
        self.active()
            .map(|overlay| overlay.target_size(base_width, base_height, scale))
    }

    pub fn draw(&self, canvas: &Canvas, rect: Rect, scale: f32, alpha: f32) {
        if let Some(overlay) = self.active() {
            overlay.draw(canvas, rect, scale, alpha);
        }
    }

    fn active(&self) -> Option<ActiveCompactOverlay<'_>> {
        if self.volume_indicator.is_visible() {
            Some(ActiveCompactOverlay::Volume(&self.volume_indicator))
        } else if self.notification_indicator.is_visible() {
            Some(ActiveCompactOverlay::Notification(
                &self.notification_indicator,
            ))
        } else {
            None
        }
    }
}
