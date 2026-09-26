use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winisland_platform::{
    IconBounds, NotificationFeed, NotificationIconData, NotificationMonitorUpdate,
    NotificationPayload,
};
use winisland_render::FontStyle;
use winisland_render::text::DrawTextCachedParams;
use winisland_render::{Image, ImageOptions, Painter, Radius, Rect, Rgba, Sampling, Vec2};

use crate::ui::compact::{CompactOverlayState, CompactSize};
use crate::utils::scroll::{ScrollDrawParams, ScrollText};

const DISPLAY_DURATION: Duration = Duration::from_secs(5);
const ENTER_DURATION: Duration = Duration::from_millis(220);
const FADE_DURATION: Duration = Duration::from_millis(280);
const DETAIL_LINE_GAP: f32 = 21.0;

struct NotificationIcon {
    image: Image,
    visible_bounds: Option<IconBounds>,
}

#[derive(Default)]
pub(super) struct NotificationMonitor {
    feed: Option<Box<dyn NotificationFeed>>,
}

impl NotificationMonitor {
    pub(super) fn update(&mut self, enabled: bool) -> Option<NotificationMonitorUpdate> {
        if self.feed.is_none() {
            self.feed =
                Some(crate::platform::notify().open_feed(Arc::new(crate::utils::event_loop::wake)));
        }
        let feed = self.feed.as_mut()?;
        let update = feed.update(enabled);
        if feed.events_available() == Some(false) {
            crate::platform::update_capabilities(|caps| caps.toast_events = false);
        }
        update
    }

    pub(super) fn remove_notification(&self, id: u32) {
        if let Some(feed) = &self.feed {
            feed.dismiss(id);
        }
    }
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
        if crate::platform::shell()
            .activate_app(app_user_model_id)
            .unwrap_or_else(|error| {
                log::warn!("Application activation failed: {error}");
                false
            })
        {
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

    pub(super) fn draw(&self, painter: Painter<'_>, rect: Rect, scale: f32, alpha: f32) {
        let (opacity, offset_y) = self.presentation();
        let alpha = (alpha * opacity * 255.0).round().clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            return;
        }

        painter.save();
        painter.translate(Vec2::new(0.0, offset_y * scale));

        let has_icon = self.icon.is_some();
        let content_left = if has_icon {
            rect.left + 72.0 * scale
        } else {
            rect.left + 20.0 * scale
        };
        let content_width = rect.right - 18.0 * scale - content_left;
        if content_width <= 0.0 {
            painter.restore();
            return;
        }

        if let Some(icon) = &self.icon {
            draw_notification_icon(painter, icon, rect, scale, alpha);
        }

        let top = rect.top;
        if !self.app_name.is_empty() {
            draw_notification_text(
                &self.app_name_scroll,
                DrawTextCachedParams {
                    painter,
                    text: &self.app_name,
                    x: content_left,
                    y: top + 22.0 * scale,
                    size: 11.0 * scale,
                    bold: false,
                    color: Rgba::WHITE.with_alpha((alpha as f32 * 0.65) as u8),
                    blur: None,
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
                painter,
                text: &self.title,
                x: content_left,
                y: title_y,
                size: 13.0 * scale,
                bold: true,
                color: Rgba::WHITE.with_alpha(alpha),
                blur: None,
            },
            content_width,
            scale,
        );
        if !self.detail.is_empty() {
            draw_notification_text(
                &self.detail_scroll,
                DrawTextCachedParams {
                    painter,
                    text: &self.detail,
                    x: content_left,
                    y: title_y + DETAIL_LINE_GAP * scale,
                    size: 11.0 * scale,
                    bold: false,
                    color: Rgba::WHITE.with_alpha((alpha as f32 * 0.72) as u8),
                    blur: None,
                },
                content_width,
                scale,
            );
        }
        painter.restore();
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
    Image::from_encoded(&icon.bytes).map(|image| NotificationIcon {
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
        painter: params.painter,
        text: params.text,
        x: params.x,
        y: params.y,
        max_w: max_width,
        size: params.size,
        style,
        color: params.color,
        blur: params.blur,
        scale,
        render_as_paths: false,
    });
}

fn draw_notification_icon(
    painter: Painter<'_>,
    icon: &NotificationIcon,
    rect: Rect,
    scale: f32,
    alpha: u8,
) {
    let size = 42.0 * scale;
    let icon_rect = Rect::from_xywh(
        rect.left + 18.0 * scale,
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
    painter.save();
    painter.clip_round_rect(icon_rect, Radius::uniform(11.0 * scale));
    painter.draw_image(
        &icon.image,
        destination,
        &ImageOptions::default()
            .with_src(source)
            .with_sampling(Sampling::LinearLinear)
            .with_alpha(alpha),
    );
    painter.restore();
}
