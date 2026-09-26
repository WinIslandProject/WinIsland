use wasapi::{
    AudioCaptureClient, AudioClient, Direction, Handle, SampleType, StreamMode, WaveFormat,
};
use windows::Win32::Foundation::S_OK;
use windows::Win32::Media::Audio::{
    Endpoints::IAudioMeterInformation, IAudioSessionControl2, IAudioSessionManager2,
    IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::Interface;
use winisland_platform::{AudioMeter, PlatformError, ProcessCapture};

use crate::com::ComGuard;
use crate::process;

const LOOPBACK_SAMPLE_RATE: usize = 48_000;
const LOOPBACK_CHANNELS: usize = 2;
const LOOPBACK_SAMPLE_BITS: usize = 32;
const BYTES_PER_FRAME: usize = 8;
const BUFFER_LIMIT: usize = LOOPBACK_SAMPLE_RATE * BYTES_PER_FRAME;

pub(super) fn process_loopback_available() -> bool {
    let Ok(_com) = ComGuard::mta() else {
        return false;
    };
    AudioClient::new_application_loopback_client(std::process::id(), true).is_ok()
}

pub(super) struct WindowsAudioMeter {
    manager: Option<IAudioSessionManager2>,
    _com: ComGuard,
}

impl WindowsAudioMeter {
    pub(super) fn new() -> Result<Self, PlatformError> {
        Ok(Self {
            manager: None,
            _com: ComGuard::mta()?,
        })
    }
}

impl AudioMeter for WindowsAudioMeter {
    fn refresh_device(&mut self) {
        // SAFETY: The enumerator and activated session manager stay on this COM-initialized thread.
        self.manager = unsafe {
            (|| -> Option<IAudioSessionManager2> {
                let enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
                let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
                device.Activate(CLSCTX_ALL, None).ok()
            })()
        };
    }

    fn find_target_process_id(&self, app_id: &str) -> Option<u32> {
        let manager = self.manager.as_ref()?;
        if app_id.is_empty() {
            return None;
        }
        // SAFETY: Sessions are enumerated and used on the manager's COM thread.
        unsafe {
            let enumerator = manager.GetSessionEnumerator().ok()?;
            let count = enumerator.GetCount().ok()?;
            for index in 0..count {
                let Ok(session) = enumerator.GetSession(index) else {
                    continue;
                };
                let Ok(control) = session.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let Ok(process_id) = control.GetProcessId() else {
                    continue;
                };
                if process_id != 0
                    && process::open(process_id)
                        .and_then(|process| process::app_user_model_id(*process))
                        .is_some_and(|id| id.eq_ignore_ascii_case(app_id))
                {
                    return Some(process_id);
                }
            }
        }
        None
    }

    fn peak(&self, process_id: u32) -> f32 {
        let Some(manager) = &self.manager else {
            return 0.0;
        };
        let mut peak = 0.0f32;
        // SAFETY: Each session and meter remains alive during this synchronous enumeration.
        unsafe {
            if let Ok(enumerator) = manager.GetSessionEnumerator() {
                let count = enumerator.GetCount().unwrap_or(0);
                for index in 0..count {
                    if let Ok(session) = enumerator.GetSession(index)
                        && let Ok(control) = session.cast::<IAudioSessionControl2>()
                    {
                        if control.IsSystemSoundsSession() == S_OK {
                            continue;
                        }
                        if process_id != 0 && control.GetProcessId().ok() != Some(process_id) {
                            continue;
                        }
                        if let Ok(meter) = session.cast::<IAudioMeterInformation>()
                            && let Ok(value) = meter.GetPeakValue()
                        {
                            peak = peak.max(value);
                        }
                    }
                }
            }
        }
        peak
    }
}

pub(super) struct WindowsProcessCapture {
    client: AudioClient,
    capture: AudioCaptureClient,
    event: Handle,
    bytes: Vec<u8>,
    _com: ComGuard,
}

impl WindowsProcessCapture {
    pub(super) fn new(process_id: u32) -> Result<Self, PlatformError> {
        let com = ComGuard::mta()?;
        let format = WaveFormat::new(
            LOOPBACK_SAMPLE_BITS,
            LOOPBACK_SAMPLE_BITS,
            &SampleType::Float,
            LOOPBACK_SAMPLE_RATE,
            LOOPBACK_CHANNELS,
            None,
        );
        let mut client = AudioClient::new_application_loopback_client(process_id, true)
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        client
            .initialize_client(
                &format,
                &Direction::Capture,
                &StreamMode::EventsShared {
                    autoconvert: true,
                    buffer_duration_hns: 0,
                },
            )
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        let event = client
            .set_get_eventhandle()
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        let capture = client
            .get_audiocaptureclient()
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        client
            .start_stream()
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        Ok(Self {
            client,
            capture,
            event,
            bytes: Vec::new(),
            _com: com,
        })
    }
}

impl ProcessCapture for WindowsProcessCapture {
    fn read_cycle(&mut self, samples: &mut Vec<f32>) -> Result<bool, PlatformError> {
        let _ = self.event.wait_for_event(100);
        let mut captured = false;
        samples.clear();
        while let Some(frame_count) = self
            .capture
            .get_next_packet_size()
            .map_err(|error| PlatformError::Backend(error.to_string()))?
            .filter(|count| *count > 0)
        {
            self.bytes.resize(frame_count as usize * BYTES_PER_FRAME, 0);
            let (frames_read, _) = self
                .capture
                .read_from_device(&mut self.bytes)
                .map_err(|error| PlatformError::Backend(error.to_string()))?;
            captured = true;
            let pending = self
                .capture
                .get_next_packet_size()
                .map_err(|error| PlatformError::Backend(error.to_string()))?
                .is_some_and(|count| count > 0);
            if !pending {
                samples.clear();
                for frame in self.bytes[..frames_read as usize * BYTES_PER_FRAME]
                    .as_chunks::<BYTES_PER_FRAME>()
                    .0
                {
                    let left = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
                    let right = f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
                    samples.push((left + right) * 0.5);
                }
            }
        }
        if self.bytes.capacity() > BUFFER_LIMIT {
            self.bytes = Vec::with_capacity(BUFFER_LIMIT);
        }
        Ok(captured)
    }
}

impl Drop for WindowsProcessCapture {
    fn drop(&mut self) {
        let _ = self.client.stop_stream();
    }
}
