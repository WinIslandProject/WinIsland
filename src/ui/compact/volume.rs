use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use skia_safe::{Canvas, Color, FontStyle, Paint, Rect};
use tokio_util::sync::CancellationToken;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, PROPERTYKEY, WPARAM};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
    IMMNotificationClient_Impl, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};
use windows::core::{PCWSTR, Result};

use crate::core::i18n::tr;
use crate::icons::volume::draw_volume_icon;
use crate::ui::compact::{CompactOverlayState, CompactSize};
use crate::utils::font::{DrawTextCachedParams, FontManager};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const ENDPOINT_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const NOTIFIER_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const DISPLAY_DURATION: Duration = Duration::from_millis(1600);
const FADE_DURATION: Duration = Duration::from_millis(240);
const VOLUME_CHANGE_THRESHOLD: f32 = 0.002;

#[derive(Clone, Copy)]
enum VolumeCommand {
    StepUp,
    StepDown,
    ToggleMute,
}

struct VolumeKeyHandler {
    sender: SyncSender<VolumeCommand>,
    endpoint_ready: Arc<AtomicBool>,
    display_enabled: Arc<AtomicBool>,
}

static VOLUME_KEY_HANDLER: Mutex<Option<VolumeKeyHandler>> = Mutex::new(None);

#[derive(Clone, Copy)]
pub(super) struct VolumeSnapshot {
    level: f32,
    muted: bool,
    revision: u64,
}

struct SharedVolumeState {
    snapshot: Mutex<VolumeSnapshot>,
}

pub(super) struct VolumeMonitor {
    state: Arc<SharedVolumeState>,
    cancellation: CancellationToken,
    command_sender: SyncSender<VolumeCommand>,
    endpoint_ready: Arc<AtomicBool>,
    display_enabled: Arc<AtomicBool>,
    keyboard_hook: Option<HHOOK>,
}

impl VolumeMonitor {
    pub(super) fn new(replace_native_volume_flyout: bool) -> Self {
        let state = Arc::new(SharedVolumeState {
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
            keyboard_hook: None,
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

    pub(super) fn set_native_flyout_replacement_enabled(&mut self, enabled: bool) {
        if enabled {
            let handler = VolumeKeyHandler {
                sender: self.command_sender.clone(),
                endpoint_ready: self.endpoint_ready.clone(),
                display_enabled: self.display_enabled.clone(),
            };
            if self.keyboard_hook.is_some() {
                *VOLUME_KEY_HANDLER
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handler);
            } else {
                self.keyboard_hook = install_volume_keyboard_hook(handler);
            }
        } else {
            self.remove_keyboard_hook();
        }
    }

    fn remove_keyboard_hook(&mut self) {
        self.display_enabled.store(false, Ordering::Release);
        *VOLUME_KEY_HANDLER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        if let Some(hook) = self.keyboard_hook {
            // SAFETY: hook was returned by SetWindowsHookExW and is removed once while the
            // callback function remains valid for the process lifetime.
            if unsafe { UnhookWindowsHookEx(hook) }.is_err() {
                log::warn!("Volume keyboard hook could not be removed");
            } else {
                self.keyboard_hook = None;
                log::info!("Native volume flyout replacement disabled");
            }
        }
    }
}

impl Drop for VolumeMonitor {
    fn drop(&mut self) {
        self.remove_keyboard_hook();
        self.cancellation.cancel();
    }
}

fn install_volume_keyboard_hook(handler: VolumeKeyHandler) -> Option<HHOOK> {
    // SAFETY: the current process module contains the static hook callback, and the hook is
    // installed on the main thread whose winit event loop pumps messages for its lifetime.
    let hook_result = unsafe {
        match GetModuleHandleW(None) {
            Ok(module) => SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(volume_keyboard_hook),
                Some(HINSTANCE(module.0)),
                0,
            ),
            Err(error) => Err(error),
        }
    };
    let hook = match hook_result {
        Ok(hook) => hook,
        Err(error) => {
            log::warn!("Volume keyboard hook could not be installed: {error}");
            return None;
        }
    };
    *VOLUME_KEY_HANDLER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handler);
    log::info!("Volume keys are handled by WinIsland");
    Some(hook)
}

unsafe extern "system" fn volume_keyboard_hook(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32
        && matches!(
            wparam.0 as u32,
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
        )
    {
        // SAFETY: for HC_ACTION, Windows supplies lparam as a valid KBDLLHOOKSTRUCT pointer for
        // the duration of this callback.
        let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let command = match key.vkCode {
            value if value == VK_VOLUME_UP.0 as u32 => Some(VolumeCommand::StepUp),
            value if value == VK_VOLUME_DOWN.0 as u32 => Some(VolumeCommand::StepDown),
            value if value == VK_VOLUME_MUTE.0 as u32 => Some(VolumeCommand::ToggleMute),
            _ => None,
        };
        if let Some(command) = command {
            let is_key_down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let handled = VOLUME_KEY_HANDLER
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|handler| {
                    handler.endpoint_ready.load(Ordering::Acquire)
                        && handler.display_enabled.load(Ordering::Acquire)
                        && (!is_key_down || handler.sender.try_send(command).is_ok())
                });
            if handled {
                return LRESULT(1);
            }
        }
    }

    // SAFETY: unhandled input is forwarded with the original hook parameters, as required by
    // the low-level keyboard hook contract.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn spawn_volume_monitor(
    state: Arc<SharedVolumeState>,
    cancellation: CancellationToken,
    command_receiver: Receiver<VolumeCommand>,
    endpoint_ready: Arc<AtomicBool>,
) {
    tokio::task::spawn_blocking(move || {
        // SAFETY: This worker owns its COM apartment and releases every COM interface before
        // uninitializing it when the monitor stops.
        let com_initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };
        if !com_initialized {
            log::warn!("Volume monitor could not initialize COM");
            return;
        }

        let mut enumerator = create_endpoint_enumerator();
        let mut notifier = enumerator.as_ref().and_then(register_endpoint_notifier);
        let mut endpoint = None;
        let mut next_endpoint_retry = Instant::now();
        let mut next_notifier_retry = Instant::now() + NOTIFIER_RETRY_INTERVAL;
        let mut previous: Option<VolumeSnapshot> = None;

        while !cancellation.is_cancelled() {
            let now = Instant::now();
            if notifier
                .as_ref()
                .is_some_and(DefaultEndpointNotifier::take_change)
            {
                endpoint = None;
                endpoint_ready.store(false, Ordering::Release);
                previous = None;
                next_endpoint_retry = now;
            }

            if enumerator.is_none() && now >= next_endpoint_retry {
                enumerator = create_endpoint_enumerator();
                next_endpoint_retry = if enumerator.is_some() {
                    now
                } else {
                    now + ENDPOINT_RETRY_INTERVAL
                };
            }

            if notifier.is_none() && now >= next_notifier_retry {
                notifier = enumerator.as_ref().and_then(register_endpoint_notifier);
                next_notifier_retry = now + NOTIFIER_RETRY_INTERVAL;
            }

            if endpoint.is_none() && now >= next_endpoint_retry {
                endpoint = enumerator.as_ref().and_then(create_default_endpoint);
                endpoint_ready.store(endpoint.is_some(), Ordering::Release);
                previous = None;
                next_endpoint_retry = now + ENDPOINT_RETRY_INTERVAL;
            }

            let mut command_handled = false;
            while let Ok(command) = command_receiver.try_recv() {
                if endpoint
                    .as_ref()
                    .is_some_and(|endpoint| apply_volume_command(endpoint, command))
                {
                    command_handled = true;
                } else {
                    endpoint = None;
                    endpoint_ready.store(false, Ordering::Release);
                }
            }

            if let Some(current) = endpoint.as_ref().and_then(read_volume) {
                publish_volume_snapshot(&state, current, previous, command_handled);

                previous = Some(current);
            } else {
                endpoint = None;
                endpoint_ready.store(false, Ordering::Release);
            }

            std::thread::sleep(POLL_INTERVAL);
        }

        drop(endpoint);
        endpoint_ready.store(false, Ordering::Release);
        drop(notifier);
        drop(enumerator);
        // SAFETY: COM was initialized successfully on this worker and all COM interfaces have
        // been dropped before the apartment is uninitialized.
        unsafe {
            CoUninitialize();
        }
    });
}

fn apply_volume_command(endpoint: &IAudioEndpointVolume, command: VolumeCommand) -> bool {
    // SAFETY: endpoint belongs to this initialized COM worker thread. A null event-context GUID
    // is explicitly supported by the endpoint volume API.
    unsafe {
        match command {
            VolumeCommand::StepUp => endpoint.VolumeStepUp(std::ptr::null()),
            VolumeCommand::StepDown => endpoint.VolumeStepDown(std::ptr::null()),
            VolumeCommand::ToggleMute => endpoint
                .GetMute()
                .and_then(|muted| endpoint.SetMute(!muted.as_bool(), std::ptr::null())),
        }
        .is_ok()
    }
}

fn create_endpoint_enumerator() -> Option<IMMDeviceEnumerator> {
    // SAFETY: The monitor calls this only from its initialized COM worker thread. The returned
    // interface is retained and used only by that thread.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok() }
}

fn create_default_endpoint(enumerator: &IMMDeviceEnumerator) -> Option<IAudioEndpointVolume> {
    // SAFETY: enumerator was created on the monitor's initialized COM thread and is used only
    // there to obtain the current default render endpoint.
    unsafe {
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        device.Activate(CLSCTX_ALL, None).ok()
    }
}

fn read_volume(endpoint: &IAudioEndpointVolume) -> Option<VolumeSnapshot> {
    // SAFETY: endpoint was created on the monitor's initialized COM thread and is used only
    // there for read-only endpoint volume queries.
    unsafe {
        let level = endpoint.GetMasterVolumeLevelScalar().ok()?.clamp(0.0, 1.0);
        let muted = endpoint.GetMute().ok()?.as_bool();
        Some(VolumeSnapshot {
            level,
            muted,
            revision: 0,
        })
    }
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
        crate::utils::event_loop::wake();
    }
}

struct DefaultEndpointNotifier {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
    changed: Arc<AtomicBool>,
}

impl DefaultEndpointNotifier {
    fn take_change(&self) -> bool {
        self.changed.swap(false, Ordering::Acquire)
    }
}

impl Drop for DefaultEndpointNotifier {
    fn drop(&mut self) {
        // SAFETY: The callback was registered with the default-device enumerator on this COM
        // worker thread. Unregistering it before dropping the callback prevents future calls.
        unsafe {
            let _ = self
                .enumerator
                .UnregisterEndpointNotificationCallback(&self.client);
        }
    }
}

fn register_endpoint_notifier(enumerator: &IMMDeviceEnumerator) -> Option<DefaultEndpointNotifier> {
    let changed = Arc::new(AtomicBool::new(false));
    let client: IMMNotificationClient = DefaultEndpointNotification {
        changed: changed.clone(),
    }
    .into();

    // SAFETY: enumerator and callback are owned by the monitor's initialized COM thread. The
    // notifier retains the callback until it can be unregistered during worker shutdown.
    unsafe {
        enumerator
            .RegisterEndpointNotificationCallback(&client)
            .ok()?;
    }

    Some(DefaultEndpointNotifier {
        enumerator: enumerator.clone(),
        client,
        changed,
    })
}

#[windows::core::implement(IMMNotificationClient)]
struct DefaultEndpointNotification {
    changed: Arc<AtomicBool>,
}

impl IMMNotificationClient_Impl for DefaultEndpointNotification_Impl {
    fn OnDeviceStateChanged(&self, _device_id: &PCWSTR, _state: DEVICE_STATE) -> Result<()> {
        Ok(())
    }

    fn OnDeviceAdded(&self, _device_id: &PCWSTR) -> Result<()> {
        Ok(())
    }

    fn OnDeviceRemoved(&self, _device_id: &PCWSTR) -> Result<()> {
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _device_id: &PCWSTR,
    ) -> Result<()> {
        if flow == eRender && role == eConsole {
            self.changed.store(true, Ordering::Release);
        }
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _device_id: &PCWSTR, _key: &PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

pub(super) struct VolumeIndicator {
    snapshot: VolumeSnapshot,
    label: String,
    seen_revision: u64,
    pending: bool,
    display_until: Option<Instant>,
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
            seen_revision: 0,
            pending: false,
            display_until: None,
        }
    }
}

impl VolumeIndicator {
    pub(super) fn update(&mut self, snapshot: VolumeSnapshot, state: CompactOverlayState) -> bool {
        let changed = snapshot.revision != self.seen_revision;
        if changed {
            self.seen_revision = snapshot.revision;
            self.snapshot = snapshot;
        }

        if !matches!(state, CompactOverlayState::Present) {
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
        self.label = tr("volume");
        self.display_until = Some(Instant::now() + DISPLAY_DURATION);
        changed
    }

    pub(super) fn is_visible(&self) -> bool {
        self.display_until
            .is_some_and(|until| until + FADE_DURATION > Instant::now())
    }

    pub(super) fn target_size(base_width: f32, base_height: f32, scale: f32) -> CompactSize {
        CompactSize {
            width: (base_width + 72.0) * scale,
            height: (base_height + 10.0) * scale,
        }
    }

    pub(super) fn draw(&self, canvas: &Canvas, rect: Rect, scale: f32, alpha: f32) {
        let alpha = (alpha * self.opacity() * 255.0).round().clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            return;
        }

        let center_y = rect.center_y();
        let icon_size = 20.0 * scale;
        let icon_center = skia_safe::Point::new(rect.left() + 21.0 * scale, center_y);
        let muted = self.snapshot.muted || self.snapshot.level <= VOLUME_CHANGE_THRESHOLD;
        draw_volume_icon(canvas, icon_center, icon_size, alpha, muted, Color::WHITE);

        let label_size = 12.0 * scale;
        let label_x = rect.left() + 37.0 * scale;
        let label_width =
            FontManager::global().measure_text_cached(&self.label, label_size, FontStyle::normal());
        let mut label_paint = Paint::default();
        label_paint.set_anti_alias(true);
        label_paint.set_color(Color::from_argb((alpha as f32 * 0.9) as u8, 255, 255, 255));
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            canvas,
            text: &self.label,
            x: label_x,
            y: center_y + 4.0 * scale,
            size: label_size,
            bold: false,
            paint: &label_paint,
        });

        let track_left = label_x + label_width + 26.0 * scale;
        let track_right = rect.right() - 14.0 * scale;
        let track_width = (track_right - track_left).max(1.0);
        let track_height = 4.0 * scale;
        let track_top = center_y - track_height / 2.0;
        let thumb_x = track_left + track_width * self.snapshot.level;

        let mut track_paint = Paint::default();
        track_paint.set_anti_alias(true);
        track_paint.set_color(Color::from_argb((alpha as f32 * 0.28) as u8, 255, 255, 255));
        canvas.draw_round_rect(
            Rect::from_xywh(track_left, track_top, track_width, track_height),
            track_height / 2.0,
            track_height / 2.0,
            &track_paint,
        );

        if thumb_x > track_left {
            let mut fill_paint = Paint::default();
            fill_paint.set_anti_alias(true);
            fill_paint.set_color(Color::from_argb(alpha, 255, 255, 255));
            canvas.draw_round_rect(
                Rect::from_xywh(track_left, track_top, thumb_x - track_left, track_height),
                track_height / 2.0,
                track_height / 2.0,
                &fill_paint,
            );
        }
    }

    fn opacity(&self) -> f32 {
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
