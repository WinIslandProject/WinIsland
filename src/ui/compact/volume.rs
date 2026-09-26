use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use winisland_platform::{VolumeCommand, VolumeKey, VolumeKeyEvent};
use winisland_render::{Painter, Point, Radius, Rect, Rgba};

use crate::icons::brightness::draw_brightness_icon;
use crate::icons::volume::draw_volume_icon;
use crate::ui::compact::{CompactOverlayState, CompactSize};
use winisland_core::i18n::tr;
use winisland_render::FontStyle;
use winisland_render::text::{DrawTextCachedParams, FontManager};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const ENDPOINT_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const NOTIFIER_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const DISPLAY_DURATION: Duration = Duration::from_millis(1600);
const FADE_DURATION: Duration = Duration::from_millis(240);
const VOLUME_CHANGE_THRESHOLD: f32 = 0.002;
const PREVIEW_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy)]
pub(super) struct VolumeSnapshot {
    pub(super) level: f32,
    pub(super) muted: bool,
    pub(super) revision: u64,
}

struct SharedVolumeState {
    snapshot: Mutex<VolumeSnapshot>,
    pending_level: Mutex<Option<f32>>,
}

pub(super) struct VolumeMonitor {
    state: Arc<SharedVolumeState>,
    cancellation: CancellationToken,
    command_sender: SyncSender<VolumeCommand>,
    endpoint_ready: Arc<AtomicBool>,
    display_enabled: Arc<AtomicBool>,
    keyboard_hook_installed: bool,
}

impl VolumeMonitor {
    pub(super) fn new(replace_native_volume_flyout: bool) -> Self {
        let state = Arc::new(SharedVolumeState {
            pending_level: Mutex::new(None),
            snapshot: Mutex::new(VolumeSnapshot {
                level: 0.0,
                muted: false,
                revision: 0,
            }),
        });
        let cancellation = CancellationToken::new();
        let endpoint_ready = Arc::new(AtomicBool::new(false));
        let display_enabled = Arc::new(AtomicBool::new(false));
        let (command_sender, command_receiver) = mpsc::sync_channel(32);
        spawn_volume_monitor(
            state.clone(),
            cancellation.clone(),
            command_receiver,
            endpoint_ready.clone(),
        );
        let mut monitor = Self {
            state,
            cancellation,
            command_sender,
            endpoint_ready,
            display_enabled,
            keyboard_hook_installed: false,
        };
        monitor.set_native_flyout_replacement_enabled(replace_native_volume_flyout);
        monitor
    }

    pub(super) fn snapshot(&self) -> VolumeSnapshot {
        *self
            .state
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn set_key_handling_enabled(&self, enabled: bool) {
        self.display_enabled.store(enabled, Ordering::Release);
    }

    pub(super) fn can_set_level(&self) -> bool {
        self.endpoint_ready.load(Ordering::Acquire)
    }

    pub(super) fn set_level(&self, level: f32) {
        if self.can_set_level() && level.is_finite() {
            *self
                .state
                .pending_level
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(level.clamp(0.0, 1.0));
        }
    }

    pub(super) fn set_native_flyout_replacement_enabled(&mut self, enabled: bool) {
        if enabled && crate::platform::capabilities().input_hooks {
            if !self.keyboard_hook_installed {
                let sender = self.command_sender.clone();
                let endpoint_ready = self.endpoint_ready.clone();
                let display_enabled = self.display_enabled.clone();
                let callback = Box::new(move |event: VolumeKeyEvent| {
                    let command = match event.key {
                        VolumeKey::Up => VolumeCommand::StepUp,
                        VolumeKey::Down => VolumeCommand::StepDown,
                        VolumeKey::Mute => VolumeCommand::ToggleMute,
                    };
                    endpoint_ready.load(Ordering::Acquire)
                        && display_enabled.load(Ordering::Acquire)
                        && (!event.is_down || sender.try_send(command).is_ok())
                });
                match crate::platform::input().install_volume_keys(callback) {
                    Ok(()) => self.keyboard_hook_installed = true,
                    Err(error) => {
                        crate::platform::update_capabilities(|caps| caps.input_hooks = false);
                        log::warn!("Volume keyboard hook could not be installed: {error}")
                    }
                }
            }
        } else {
            self.remove_keyboard_hook();
        }
    }

    fn remove_keyboard_hook(&mut self) {
        self.display_enabled.store(false, Ordering::Release);
        if self.keyboard_hook_installed {
            if let Err(error) = crate::platform::input().uninstall_volume_keys() {
                log::warn!("Volume keyboard hook could not be removed: {error}");
            }
            self.keyboard_hook_installed = false;
        }
    }
}

impl Drop for VolumeMonitor {
    fn drop(&mut self) {
        self.remove_keyboard_hook();
        self.cancellation.cancel();
    }
}

fn spawn_volume_monitor(
    state: Arc<SharedVolumeState>,
    cancellation: CancellationToken,
    command_receiver: Receiver<VolumeCommand>,
    endpoint_ready: Arc<AtomicBool>,
) {
    tokio::task::spawn_blocking(move || {
        let mut device = match crate::platform::audio().open_volume_endpoint() {
            Ok(device) => device,
            Err(error) => {
                crate::platform::update_capabilities(|caps| caps.volume_control = false);
                log::warn!("Volume monitor could not initialize COM: {error}");
                return;
            }
        };
        let mut next_endpoint_retry = Instant::now();
        let mut next_notifier_retry = Instant::now() + NOTIFIER_RETRY_INTERVAL;
        let mut previous: Option<VolumeSnapshot> = None;

        while !cancellation.is_cancelled() {
            let now = Instant::now();
            if device.take_device_change() {
                device.invalidate_endpoint();
                endpoint_ready.store(false, Ordering::Release);
                previous = None;
                next_endpoint_retry = now;
            }

            if !device.has_enumerator() && now >= next_endpoint_retry {
                next_endpoint_retry = if device.ensure_enumerator() {
                    now
                } else {
                    now + ENDPOINT_RETRY_INTERVAL
                };
            }

            if !device.has_notifier() && now >= next_notifier_retry {
                device.ensure_notifier();
                next_notifier_retry = now + NOTIFIER_RETRY_INTERVAL;
            }

            if !device.has_endpoint() && now >= next_endpoint_retry {
                let available = device.ensure_endpoint();
                endpoint_ready.store(available, Ordering::Release);
                crate::platform::update_capabilities(|caps| caps.volume_control = available);
                previous = None;
                next_endpoint_retry = now + ENDPOINT_RETRY_INTERVAL;
            }

            let mut command_handled = false;
            let pending_level = state
                .pending_level
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            for command in command_receiver
                .try_iter()
                .chain(pending_level.map(VolumeCommand::SetLevel))
            {
                if device.apply(command) {
                    command_handled = true;
                } else {
                    device.invalidate_endpoint();
                    endpoint_ready.store(false, Ordering::Release);
                }
            }

            if let Some(current) = device.read() {
                let current = VolumeSnapshot {
                    level: current.level,
                    muted: current.muted,
                    revision: 0,
                };
                publish_volume_snapshot(&state, current, previous, command_handled);
                previous = Some(current);
            } else {
                device.invalidate_endpoint();
                endpoint_ready.store(false, Ordering::Release);
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        drop(device);
        endpoint_ready.store(false, Ordering::Release);
    });
}
fn publish_volume_snapshot(
    state: &SharedVolumeState,
    current: VolumeSnapshot,
    previous: Option<VolumeSnapshot>,
    force_changed: bool,
) {
    let changed = force_changed
        || previous.is_some_and(|last| {
            (last.level - current.level).abs() > VOLUME_CHANGE_THRESHOLD
                || last.muted != current.muted
        });
    let mut snapshot = state
        .snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let revision = if changed {
        snapshot.revision.wrapping_add(1)
    } else {
        snapshot.revision
    };
    *snapshot = VolumeSnapshot {
        revision,
        ..current
    };
    drop(snapshot);
    if changed {
        crate::platform::wake();
    }
}

pub(super) struct VolumeIndicator {
    snapshot: VolumeSnapshot,
    label: String,
    brightness: bool,
    seen_revision: u64,
    pending: bool,
    display_until: Option<Instant>,
    dragging: bool,
    preview: Option<(f32, Instant)>,
}

impl Default for VolumeIndicator {
    fn default() -> Self {
        Self {
            snapshot: VolumeSnapshot {
                level: 0.0,
                muted: false,
                revision: 0,
            },
            label: tr("volume"),
            brightness: false,
            seen_revision: 0,
            pending: false,
            display_until: None,
            dragging: false,
            preview: None,
        }
    }
}

impl VolumeIndicator {
    pub(super) fn new_brightness() -> Self {
        Self {
            label: tr("brightness"),
            brightness: true,
            ..Self::default()
        }
    }

    pub(super) fn update_brightness(
        &mut self,
        level: f32,
        revision: u64,
        state: CompactOverlayState,
    ) -> bool {
        self.update(
            VolumeSnapshot {
                level,
                muted: false,
                revision,
            },
            state,
        )
    }

    pub(super) fn update(&mut self, snapshot: VolumeSnapshot, state: CompactOverlayState) -> bool {
        let changed = snapshot.revision != self.seen_revision;
        if changed {
            self.seen_revision = snapshot.revision;
            self.snapshot = snapshot;
        }
        if !self.dragging
            && self.preview.is_some_and(|(level, until)| {
                Instant::now() >= until
                    || ((self.snapshot.level - level).abs() <= VOLUME_CHANGE_THRESHOLD
                        && self.snapshot.muted == (level <= 0.0))
            })
        {
            self.preview = None;
        }

        if !matches!(state, CompactOverlayState::Present) {
            self.dragging = false;
            self.preview = None;
            if matches!(state, CompactOverlayState::Defer) && changed {
                self.pending = true;
            } else if matches!(state, CompactOverlayState::Discard) {
                self.pending = false;
            }
            self.display_until = None;
            return changed;
        }

        if !changed && !self.pending {
            return false;
        }

        self.pending = false;
        self.label = tr(if self.brightness {
            "brightness"
        } else {
            "volume"
        });
        self.display_until = Some(Instant::now() + DISPLAY_DURATION);
        changed
    }

    pub(super) fn is_visible(&self) -> bool {
        self.dragging
            || self
                .display_until
                .is_some_and(|until| until + FADE_DURATION > Instant::now())
    }

    pub(super) fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub(super) fn begin_drag(&mut self, x: f32, y: f32, rect: Rect, scale: f32) -> bool {
        let track = self.track_rect(rect, scale);
        if !self.is_visible()
            || x < track.left - 4.0 * scale
            || x > track.right + 4.0 * scale
            || (y - track.center_y()).abs() > 12.0 * scale
        {
            return false;
        }
        self.dragging = true;
        true
    }

    pub(super) fn drag_to(&mut self, x: f32, rect: Rect, scale: f32) -> Option<f32> {
        if !self.dragging {
            return None;
        }
        let track = self.track_rect(rect, scale);
        let level = ((x - track.left) / track.width()).clamp(0.0, 1.0);
        let changed = self
            .preview
            .is_none_or(|(last, _)| (last - level).abs() > f32::EPSILON);
        self.preview = Some((level, Instant::now() + PREVIEW_TIMEOUT));
        self.display_until = Some(Instant::now() + DISPLAY_DURATION);
        changed.then_some(level)
    }

    pub(super) fn finish_drag(&mut self) -> bool {
        if !self.dragging {
            return false;
        }
        self.dragging = false;
        self.display_until = Some(Instant::now() + DISPLAY_DURATION);
        true
    }

    fn track_rect(&self, rect: Rect, scale: f32) -> Rect {
        let label_width = FontManager::global().measure_text_cached(
            &self.label,
            12.0 * scale,
            FontStyle::normal(),
        );
        let left = rect.left + (37.0 + 11.0) * scale + label_width;
        Rect::from_xywh(
            left,
            rect.center_y() - 2.0 * scale,
            (rect.right - 14.0 * scale - left).max(1.0),
            4.0 * scale,
        )
    }

    pub(super) fn target_size(base_width: f32, base_height: f32, scale: f32) -> CompactSize {
        CompactSize {
            width: (base_width + 72.0) * scale,
            height: (base_height + 10.0) * scale,
        }
    }

    pub(super) fn draw(&self, painter: Painter<'_>, rect: Rect, scale: f32, alpha: f32) {
        self.draw_level(painter, rect, scale, alpha, false);
    }

    pub(super) fn draw_brightness(&self, painter: Painter<'_>, rect: Rect, scale: f32, alpha: f32) {
        self.draw_level(painter, rect, scale, alpha, true);
    }

    fn draw_level(
        &self,
        painter: Painter<'_>,
        rect: Rect,
        scale: f32,
        alpha: f32,
        brightness: bool,
    ) {
        let alpha = (alpha * self.opacity() * 255.0).round().clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            return;
        }

        let center_y = rect.center_y();
        let icon_size = 20.0 * scale;
        let icon_center = Point::new(rect.left + 21.0 * scale, center_y);
        let level = self.preview.map_or(self.snapshot.level, |(level, _)| level);
        let muted = self
            .preview
            .map_or(self.snapshot.muted, |(level, _)| level <= 0.0)
            || level <= VOLUME_CHANGE_THRESHOLD;
        if brightness {
            draw_brightness_icon(painter, icon_center, icon_size, alpha, level);
        } else {
            draw_volume_icon(
                painter,
                icon_center,
                icon_size,
                alpha,
                if muted { 0.0 } else { level },
                Rgba::WHITE,
            );
        }

        let label_size = 12.0 * scale;
        let label_x = rect.left + 37.0 * scale;
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            painter,
            text: &self.label,
            x: label_x,
            y: center_y + 4.0 * scale,
            size: label_size,
            bold: false,
            color: Rgba::WHITE.with_alpha((alpha as f32 * 0.9) as u8),
            blur: None,
        });

        let track = self.track_rect(rect, scale);
        let track_left = track.left;
        let track_width = track.width();
        let track_height = track.height();
        let track_top = track.top;
        let thumb_x = track_left + track_width * level;

        painter.fill_round_rect(
            Rect::from_xywh(track_left, track_top, track_width, track_height),
            Radius::uniform(track_height / 2.0),
            Rgba::WHITE.with_alpha((alpha as f32 * 0.28) as u8),
        );

        if thumb_x > track_left {
            painter.fill_round_rect(
                Rect::from_xywh(track_left, track_top, thumb_x - track_left, track_height),
                Radius::uniform(track_height / 2.0),
                Rgba::WHITE.with_alpha(alpha),
            );
        }
    }

    fn opacity(&self) -> f32 {
        if self.dragging {
            return 1.0;
        }
        let Some(until) = self.display_until else {
            return 0.0;
        };
        let elapsed = Instant::now().saturating_duration_since(until);
        if elapsed.is_zero() {
            1.0
        } else {
            (1.0 - elapsed.as_secs_f32() / FADE_DURATION.as_secs_f32()).clamp(0.0, 1.0)
        }
    }
}
