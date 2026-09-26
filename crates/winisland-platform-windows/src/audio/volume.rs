use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
    IMMNotificationClient_Impl, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{PCWSTR, Result};
use winisland_platform::{PlatformError, VolumeCommand, VolumeEndpoint, VolumeState};

use crate::com::ComGuard;

pub(super) struct WindowsVolumeEndpoint {
    endpoint: Option<IAudioEndpointVolume>,
    notifier: Option<DefaultEndpointNotifier>,
    enumerator: Option<IMMDeviceEnumerator>,
    _com: ComGuard,
}

impl WindowsVolumeEndpoint {
    pub(super) fn new() -> std::result::Result<Self, PlatformError> {
        let com = ComGuard::mta()?;
        let enumerator = create_endpoint_enumerator();
        let notifier = enumerator.as_ref().and_then(register_endpoint_notifier);
        Ok(Self {
            endpoint: None,
            notifier,
            enumerator,
            _com: com,
        })
    }
}

impl VolumeEndpoint for WindowsVolumeEndpoint {
    fn has_enumerator(&self) -> bool {
        self.enumerator.is_some()
    }

    fn ensure_enumerator(&mut self) -> bool {
        if self.enumerator.is_none() {
            self.enumerator = create_endpoint_enumerator();
        }
        self.enumerator.is_some()
    }

    fn has_notifier(&self) -> bool {
        self.notifier.is_some()
    }

    fn ensure_notifier(&mut self) -> bool {
        if self.notifier.is_none() {
            self.notifier = self
                .enumerator
                .as_ref()
                .and_then(register_endpoint_notifier);
        }
        self.notifier.is_some()
    }

    fn take_device_change(&self) -> bool {
        self.notifier
            .as_ref()
            .is_some_and(DefaultEndpointNotifier::take_change)
    }

    fn has_endpoint(&self) -> bool {
        self.endpoint.is_some()
    }

    fn ensure_endpoint(&mut self) -> bool {
        if self.endpoint.is_none() {
            self.endpoint = self.enumerator.as_ref().and_then(create_default_endpoint);
        }
        self.endpoint.is_some()
    }

    fn invalidate_endpoint(&mut self) {
        self.endpoint = None;
    }

    fn apply(&mut self, command: VolumeCommand) -> bool {
        self.endpoint
            .as_ref()
            .is_some_and(|endpoint| apply_command(endpoint, command))
    }

    fn read(&self) -> Option<VolumeState> {
        self.endpoint.as_ref().and_then(read_volume)
    }
}

fn apply_command(endpoint: &IAudioEndpointVolume, command: VolumeCommand) -> bool {
    // SAFETY: endpoint belongs to the creating COM worker thread and the event context is optional.
    unsafe {
        match command {
            VolumeCommand::StepUp => endpoint.VolumeStepUp(std::ptr::null()),
            VolumeCommand::StepDown => endpoint.VolumeStepDown(std::ptr::null()),
            VolumeCommand::ToggleMute => endpoint
                .GetMute()
                .and_then(|muted| endpoint.SetMute(!muted.as_bool(), std::ptr::null())),
            VolumeCommand::SetLevel(level) => endpoint
                .SetMasterVolumeLevelScalar(level, std::ptr::null())
                .and_then(|()| endpoint.SetMute(level <= 0.0, std::ptr::null())),
        }
        .is_ok()
    }
}

fn create_endpoint_enumerator() -> Option<IMMDeviceEnumerator> {
    // SAFETY: The caller holds a COM apartment guard on this thread.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok() }
}

fn create_default_endpoint(enumerator: &IMMDeviceEnumerator) -> Option<IAudioEndpointVolume> {
    // SAFETY: enumerator belongs to the initialized COM thread and returns an owned endpoint.
    unsafe {
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        device.Activate(CLSCTX_ALL, None).ok()
    }
}

fn read_volume(endpoint: &IAudioEndpointVolume) -> Option<VolumeState> {
    // SAFETY: endpoint is queried on its initialized COM thread.
    unsafe {
        Some(VolumeState {
            level: endpoint.GetMasterVolumeLevelScalar().ok()?.clamp(0.0, 1.0),
            muted: endpoint.GetMute().ok()?.as_bool(),
        })
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
        // SAFETY: The callback is unregistered before its retained interface is released.
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
    // SAFETY: The callback and enumerator remain live until DefaultEndpointNotifier is dropped.
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
