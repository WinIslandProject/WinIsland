mod brightness;
mod notification;
mod volume;

use winisland_render::{Painter, Rect};

use self::brightness::BrightnessMonitor;
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

pub struct CompactOverlayUpdate {
    pub volume_changed: bool,
    pub brightness_changed: bool,
    pub notification_received: bool,
}

pub struct CompactOverlay {
    volume_monitor: VolumeMonitor,
    volume_indicator: VolumeIndicator,
    brightness_monitor: BrightnessMonitor,
    brightness_indicator: VolumeIndicator,
    brightness_overlay_enabled: bool,
    last_level_is_brightness: bool,
    notification_monitor: NotificationMonitor,
    notification_indicator: NotificationIndicator,
}

enum ActiveCompactOverlay<'a> {
    Notification(&'a NotificationIndicator),
    Volume(&'a VolumeIndicator),
    Brightness(&'a VolumeIndicator),
}

impl ActiveCompactOverlay<'_> {
    fn target_size(&self, base_width: f32, base_height: f32, scale: f32) -> CompactSize {
        match self {
            Self::Notification(_) => {
                NotificationIndicator::target_size(base_width, base_height, scale)
            }
            Self::Volume(_) => VolumeIndicator::target_size(base_width, base_height, scale),
            Self::Brightness(_) => VolumeIndicator::target_size(base_width, base_height, scale),
        }
    }

    fn draw(&self, painter: Painter<'_>, rect: Rect, scale: f32, alpha: f32) {
        match self {
            Self::Notification(indicator) => indicator.draw(painter, rect, scale, alpha),
            Self::Volume(indicator) => indicator.draw(painter, rect, scale, alpha),
            Self::Brightness(indicator) => indicator.draw_brightness(painter, rect, scale, alpha),
        }
    }
}

impl CompactOverlay {
    pub fn new(replace_native_volume_flyout: bool, brightness_overlay_enabled: bool) -> Self {
        Self {
            volume_monitor: VolumeMonitor::new(replace_native_volume_flyout),
            volume_indicator: VolumeIndicator::default(),
            brightness_monitor: BrightnessMonitor::new(),
            brightness_indicator: VolumeIndicator::new_brightness(),
            brightness_overlay_enabled,
            last_level_is_brightness: false,
            notification_monitor: NotificationMonitor::default(),
            notification_indicator: NotificationIndicator::default(),
        }
    }

    pub fn set_native_volume_flyout_replacement_enabled(&mut self, enabled: bool) {
        self.volume_monitor
            .set_native_flyout_replacement_enabled(enabled);
    }

    pub fn set_brightness_overlay_enabled(&mut self, enabled: bool) {
        self.brightness_overlay_enabled = enabled;
    }

    pub fn update(
        &mut self,
        volume_state: CompactOverlayState,
        notification_state: CompactOverlayState,
        notification_display: bool,
    ) -> CompactOverlayUpdate {
        let brightness = self.brightness_monitor.snapshot();
        let mut caps = crate::platform::capabilities();
        if brightness.available && !caps.brightness_control {
            crate::platform::update_capabilities(|caps| caps.brightness_control = true);
            caps.brightness_control = true;
        }
        let volume_state = if caps.volume_control {
            volume_state
        } else {
            CompactOverlayState::Discard
        };
        let notification_display = notification_display && caps.toast_events;
        self.volume_monitor.set_key_handling_enabled(
            caps.input_hooks && !matches!(volume_state, CompactOverlayState::Discard),
        );
        let volume_changed = self
            .volume_indicator
            .update(self.volume_monitor.snapshot(), volume_state);
        let brightness_changed = self.brightness_overlay_enabled
            && caps.brightness_control
            && brightness.available
            && self.brightness_indicator.update_brightness(
                brightness.level,
                brightness.revision,
                volume_state,
            );
        if !self.brightness_overlay_enabled || !caps.brightness_control {
            self.brightness_indicator.update_brightness(
                brightness.level,
                brightness.revision,
                CompactOverlayState::Discard,
            );
        }
        if volume_changed {
            self.last_level_is_brightness = false;
        }
        if brightness_changed {
            self.last_level_is_brightness = true;
        }
        let notification = self.notification_monitor.update(notification_display);
        let notification_received = if notification_display {
            self.notification_indicator
                .update(notification, notification_state)
        } else {
            self.notification_indicator.clear();
            false
        };
        CompactOverlayUpdate {
            volume_changed,
            brightness_changed,
            notification_received,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.active().is_some()
    }

    pub fn begin_volume_drag(&mut self, x: f32, y: f32, rect: Rect, scale: f32) -> bool {
        if self.last_level_is_brightness
            || !self.volume_monitor.can_set_level()
            || !self.volume_indicator.begin_drag(x, y, rect, scale)
        {
            return false;
        }
        self.update_volume_drag(x, rect, scale);
        true
    }

    pub fn update_volume_drag(&mut self, x: f32, rect: Rect, scale: f32) {
        if let Some(level) = self.volume_indicator.drag_to(x, rect, scale) {
            self.volume_monitor.set_level(level);
        }
    }

    pub fn finish_volume_drag(&mut self) -> bool {
        self.volume_indicator.finish_drag()
    }

    pub fn is_volume_dragging(&self) -> bool {
        self.volume_indicator.is_dragging()
    }

    pub fn begin_brightness_drag(&mut self, x: f32, y: f32, rect: Rect, scale: f32) -> bool {
        if !self.last_level_is_brightness
            || !self.brightness_monitor.snapshot().available
            || !self.brightness_indicator.begin_drag(x, y, rect, scale)
        {
            return false;
        }
        self.update_brightness_drag(x, rect, scale);
        true
    }

    pub fn update_brightness_drag(&mut self, x: f32, rect: Rect, scale: f32) {
        if let Some(level) = self.brightness_indicator.drag_to(x, rect, scale) {
            self.brightness_monitor.set_level(level);
        }
    }

    pub fn finish_brightness_drag(&mut self) -> bool {
        self.brightness_indicator.finish_drag()
    }

    pub fn is_brightness_dragging(&self) -> bool {
        self.brightness_indicator.is_dragging()
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

    pub(crate) fn maximum_size(base_width: f32, base_height: f32, scale: f32) -> CompactSize {
        let notification = NotificationIndicator::target_size(base_width, base_height, scale);
        let volume = VolumeIndicator::target_size(base_width, base_height, scale);
        CompactSize {
            width: notification.width.max(volume.width),
            height: notification.height.max(volume.height),
        }
    }

    pub fn draw(&self, painter: Painter<'_>, rect: Rect, scale: f32, alpha: f32) {
        if let Some(overlay) = self.active() {
            overlay.draw(painter, rect, scale, alpha);
        }
    }

    fn active(&self) -> Option<ActiveCompactOverlay<'_>> {
        if self.last_level_is_brightness && self.brightness_indicator.is_visible() {
            Some(ActiveCompactOverlay::Brightness(&self.brightness_indicator))
        } else if self.volume_indicator.is_visible() {
            Some(ActiveCompactOverlay::Volume(&self.volume_indicator))
        } else if self.brightness_indicator.is_visible() {
            Some(ActiveCompactOverlay::Brightness(&self.brightness_indicator))
        } else if self.notification_indicator.is_visible() {
            Some(ActiveCompactOverlay::Notification(
                &self.notification_indicator,
            ))
        } else {
            None
        }
    }
}
