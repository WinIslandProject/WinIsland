use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, StreamConfig};
use realfft::RealFftPlanner;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat};
use windows::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, S_OK};
use windows::Win32::Media::Audio::{
    Endpoints::IAudioMeterInformation, IAudioSessionControl2, IAudioSessionManager2,
    IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::core::{Interface, PWSTR};

const FFT_LEN: usize = 1024;
const SPECTRUM_BAND_COUNT: usize = 6;
const FFT_BIN_RANGES: [(usize, usize); SPECTRUM_BAND_COUNT] =
    [(2, 8), (8, 20), (20, 50), (50, 120), (120, 280), (280, 511)];
const SPECTRUM_OUTPUT_MAPPING: [(usize, f32); SPECTRUM_BAND_COUNT] =
    [(5, 0.8), (3, 0.9), (0, 1.0), (1, 1.0), (2, 0.9), (4, 0.8)];
const PROCESS_CAPTURE_BYTES_PER_FRAME: usize = 8;
const PROCESS_CAPTURE_BUFFER_LIMIT: usize = LOOPBACK_SAMPLE_RATE * PROCESS_CAPTURE_BYTES_PER_FRAME;
const DEVICE_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const AUDIO_POLL_INTERVAL: Duration = Duration::from_millis(100);
const AUDIO_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const PROCESS_CAPTURE_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const ACTIVE_AUDIO_PEAK_THRESHOLD: f32 = 0.002;
const ADAPTIVE_LEVEL_INITIAL: f32 = 0.1;
const ADAPTIVE_LEVEL_FLOOR: f32 = 0.01;
const ADAPTIVE_LEVEL_DECAY: f32 = 0.995;
const ADAPTIVE_LEVEL_LEARNING_RATE: f32 = 0.005;
const SPECTRUM_NORMALIZATION_GAIN: f32 = 2.3;
const LOOPBACK_SAMPLE_RATE: usize = 48_000;
const LOOPBACK_CHANNELS: usize = 2;
const LOOPBACK_SAMPLE_BITS: usize = 32;

struct AtomicF32(AtomicU32);

impl AtomicF32 {
    const fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }

    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }
}

struct SpectrumAnalyzer {
    fft: Arc<dyn realfft::RealToComplex<f32>>,
    output: Vec<realfft::num_complex::Complex32>,
    input: Vec<f32>,
    input_len: usize,
    adaptive_max: [f32; SPECTRUM_BAND_COUNT],
}

impl SpectrumAnalyzer {
    fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_LEN);
        let output = fft.make_output_vec();
        Self {
            fft,
            output,
            input: vec![0.0; FFT_LEN],
            input_len: 0,
            adaptive_max: [ADAPTIVE_LEVEL_INITIAL; SPECTRUM_BAND_COUNT],
        }
    }

    fn push_sample(
        &mut self,
        sample: f32,
        spectrum: &Arc<Mutex<[f32; SPECTRUM_BAND_COUNT]>>,
        gate: &Arc<AtomicF32>,
        gate_override: &Arc<AtomicF32>,
    ) {
        self.input[self.input_len] = sample;
        self.input_len += 1;
        if self.input_len == FFT_LEN {
            update_spectrum(
                &mut self.input,
                &self.fft,
                &mut self.output,
                &mut self.adaptive_max,
                spectrum,
                gate,
                gate_override,
            );
            self.input_len = 0;
        }
    }
}

struct ProcessCaptureContext {
    cancel: CancellationToken,
    generation: u32,
    worker_generation: Arc<AtomicU32>,
    target_process_id: Arc<AtomicU32>,
    process_capture_active: Arc<AtomicBool>,
    spectrum: Arc<Mutex<[f32; SPECTRUM_BAND_COUNT]>>,
    gate: Arc<AtomicF32>,
    gate_override: Arc<AtomicF32>,
}

impl ProcessCaptureContext {
    fn is_current(&self) -> bool {
        self.worker_generation.load(Ordering::Acquire) == self.generation
    }

    fn set_process_capture_active(&self, active: bool) {
        if self.is_current() {
            self.process_capture_active.store(active, Ordering::Release);
        }
    }
}

#[derive(Clone)]
struct FallbackCaptureContext {
    spectrum: Arc<Mutex<[f32; SPECTRUM_BAND_COUNT]>>,
    gate: Arc<AtomicF32>,
    gate_override: Arc<AtomicF32>,
    process_capture_active: Arc<AtomicBool>,
    worker_generation: Arc<AtomicU32>,
    generation: u32,
}

impl FallbackCaptureContext {
    fn is_current(&self) -> bool {
        self.worker_generation.load(Ordering::Acquire) == self.generation
    }
}

pub struct AudioProcessor {
    spectrum: Arc<Mutex<[f32; SPECTRUM_BAND_COUNT]>>,
    gate: Arc<AtomicF32>,
    gate_override: Arc<AtomicF32>,
    target_app_id: Arc<RwLock<String>>,
    target_process_id: Arc<AtomicU32>,
    process_capture_active: Arc<AtomicBool>,
    worker_generation: Arc<AtomicU32>,
    workers: Mutex<Option<CancellationToken>>,
}

impl AudioProcessor {
    pub fn new() -> Self {
        let spectrum = Arc::new(Mutex::new([0.0f32; SPECTRUM_BAND_COUNT]));
        let gate = Arc::new(AtomicF32::new(1.0));
        let gate_override = Arc::new(AtomicF32::new(0.0));
        let target_app_id = Arc::new(RwLock::new(String::new()));
        let target_process_id = Arc::new(AtomicU32::new(0));
        let process_capture_active = Arc::new(AtomicBool::new(false));
        let worker_generation = Arc::new(AtomicU32::new(0));
        let processor = Self {
            spectrum,
            gate,
            gate_override,
            target_app_id,
            target_process_id,
            process_capture_active,
            worker_generation,
            workers: Mutex::new(None),
        };
        log::info!("AudioProcessor created in idle state");
        processor
    }

    pub fn get_spectrum(&self) -> [f32; SPECTRUM_BAND_COUNT] {
        *self
            .spectrum
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_gate_override(&self, value: bool) {
        self.gate_override.set(if value { 1.0 } else { 0.0 });
    }

    pub fn set_target_app_id(&self, app_id: &str) {
        let changed = {
            let mut target_app_id = self
                .target_app_id
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if *target_app_id == app_id {
                false
            } else {
                target_app_id.clear();
                target_app_id.push_str(app_id);
                true
            }
        };
        if !changed {
            return;
        }
        if app_id.is_empty() {
            self.stop_workers();
        } else {
            self.start_workers();
        }
    }

    fn start_workers(&self) {
        let cancel = {
            let mut workers = self
                .workers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if workers.is_some() {
                return;
            }
            let cancel = CancellationToken::new();
            *workers = Some(cancel.clone());
            cancel
        };
        let generation = self
            .worker_generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        log::info!("Audio media detected, starting capture workers");
        self.start_capture(cancel.clone(), generation);
        self.start_meter_thread(cancel.clone(), generation);
        self.start_process_capture(cancel, generation);
    }

    fn stop_workers(&self) {
        let cancel = self
            .workers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(cancel) = cancel {
            self.worker_generation.fetch_add(1, Ordering::AcqRel);
            cancel.cancel();
            self.target_process_id.store(0, Ordering::Relaxed);
            self.process_capture_active.store(false, Ordering::Release);
            self.gate.set(0.0);
            *self
                .spectrum
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = [0.0; SPECTRUM_BAND_COUNT];
            log::info!("Audio media ended, stopping capture workers");
        }
    }

    fn start_meter_thread(&self, cancel: CancellationToken, generation: u32) {
        let gate_clone = self.gate.clone();
        let target_app_id = self.target_app_id.clone();
        let target_process_id = self.target_process_id.clone();
        let worker_generation = self.worker_generation.clone();
        tokio::task::spawn_blocking(move || {
            // SAFETY: CoInitializeEx initializes COM for this thread. COINIT_MULTITHREADED
            // is safe as we don't use single-threaded COM apartments.
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            let host = cpal::default_host();
            let mut current_device_name = None;
            let mut session_manager: Option<IAudioSessionManager2> = None;
            let mut current_target_app_id = String::new();
            let mut current_target_process_id = 0;
            let mut next_target_refresh = Instant::now();
            let mut next_device_refresh = Instant::now();

            log::info!("Audio meter thread started (COM: {})", hr.is_ok());

            while !cancel.is_cancelled() && worker_generation.load(Ordering::Acquire) == generation
            {
                let now = Instant::now();
                if now >= next_device_refresh {
                    next_device_refresh = now + DEVICE_REFRESH_INTERVAL;
                    let default_device = host.default_output_device();
                    let default_device_name = default_device
                        .as_ref()
                        .and_then(|d| d.description().map(|desc| desc.name().to_string()).ok());

                    if default_device_name != current_device_name {
                        session_manager = None;
                        current_device_name = None;
                        current_target_process_id = 0;
                        target_process_id.store(0, Ordering::Relaxed);
                        next_target_refresh = Instant::now();

                        if default_device_name.is_some() {
                            session_manager = unsafe {
                                (|| -> Option<IAudioSessionManager2> {
                                    let enumerator: IMMDeviceEnumerator =
                                        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                                            .ok()?;
                                    let device = enumerator
                                        .GetDefaultAudioEndpoint(eRender, eConsole)
                                        .ok()?;
                                    device.Activate(CLSCTX_ALL, None).ok()
                                })()
                            };
                            current_device_name = default_device_name;
                            log::info!(
                                "Audio meter thread: switched to device {current_device_name:?}"
                            );
                        }
                    }
                }

                let requested_app_id = target_app_id
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                if requested_app_id != current_target_app_id || now >= next_target_refresh {
                    current_target_app_id = requested_app_id;
                    current_target_process_id = session_manager
                        .as_ref()
                        .and_then(|manager| find_target_process_id(manager, &current_target_app_id))
                        .unwrap_or(0);
                    if worker_generation.load(Ordering::Acquire) != generation {
                        break;
                    }
                    target_process_id.store(current_target_process_id, Ordering::Relaxed);
                    next_target_refresh = now + DEVICE_REFRESH_INTERVAL;
                }

                if current_target_app_id.is_empty() {
                    gate_clone.set(0.0);
                    std::thread::sleep(AUDIO_POLL_INTERVAL);
                    continue;
                }

                let mut max_peak = 0.0f32;
                if let Some(ref mgr) = session_manager {
                    // SAFETY: GetSessionEnumerator and subsequent COM calls enumerate audio
                    // sessions for peak meter reading. All objects are obtained from the
                    // session_manager which is valid for the lifetime of this thread.
                    unsafe {
                        if let Ok(enumerator) = mgr.GetSessionEnumerator() {
                            let count = enumerator.GetCount().unwrap_or(0);
                            for i in 0..count {
                                if let Ok(session) = enumerator.GetSession(i)
                                    && let Ok(session2) = session.cast::<IAudioSessionControl2>()
                                {
                                    if session2.IsSystemSoundsSession() == S_OK {
                                        continue;
                                    }
                                    if current_target_process_id != 0
                                        && session2.GetProcessId().ok()
                                            != Some(current_target_process_id)
                                    {
                                        continue;
                                    }
                                    if let Ok(meter) = session.cast::<IAudioMeterInformation>()
                                        && let Ok(peak) = meter.GetPeakValue()
                                    {
                                        max_peak = max_peak.max(peak);
                                    }
                                }
                            }
                        }
                    }
                }
                let gate_val = if max_peak > ACTIVE_AUDIO_PEAK_THRESHOLD {
                    1.0f32
                } else {
                    0.0f32
                };
                if worker_generation.load(Ordering::Acquire) != generation {
                    break;
                }
                gate_clone.set(gate_val);
                std::thread::sleep(AUDIO_POLL_INTERVAL);
            }
            // Drop COM objects while COM is still initialized, then clean up.
            drop(session_manager);
            if hr.is_ok() {
                // SAFETY: COM was initialized above, and all COM objects are dropped.
                unsafe {
                    CoUninitialize();
                }
            }
        });
    }

    fn start_process_capture(&self, cancel: CancellationToken, generation: u32) {
        let context = ProcessCaptureContext {
            cancel,
            generation,
            worker_generation: self.worker_generation.clone(),
            target_process_id: self.target_process_id.clone(),
            process_capture_active: self.process_capture_active.clone(),
            spectrum: self.spectrum.clone(),
            gate: self.gate.clone(),
            gate_override: self.gate_override.clone(),
        };
        tokio::task::spawn_blocking(move || {
            let com_initialized = wasapi::initialize_mta().is_ok();
            let mut active_process_id = 0;
            let mut unavailable_process_id = None;
            let mut retry_after = Instant::now();
            let mut analyzer = SpectrumAnalyzer::new();

            while !context.cancel.is_cancelled() && context.is_current() {
                let process_id = context.target_process_id.load(Ordering::Relaxed);
                if process_id == 0 {
                    active_process_id = 0;
                    context.set_process_capture_active(false);
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }

                if process_id != active_process_id {
                    active_process_id = process_id;
                    unavailable_process_id = None;
                    retry_after = Instant::now();
                }

                if Instant::now() < retry_after {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }

                if let Err(error) = capture_process_audio(process_id, &context, &mut analyzer) {
                    context.set_process_capture_active(false);
                    retry_after = Instant::now() + PROCESS_CAPTURE_RETRY_INTERVAL;
                    if unavailable_process_id != Some(process_id) {
                        unavailable_process_id = Some(process_id);
                        log::warn!(
                            "Audio capture: process loopback unavailable for PID {process_id}: {error}"
                        );
                    }
                }
            }

            context.set_process_capture_active(false);
            if com_initialized {
                wasapi::deinitialize();
            }
        });
    }

    fn start_capture(&self, cancel: CancellationToken, generation: u32) {
        let gate_clone = self.gate.clone();
        let gate_override_clone = self.gate_override.clone();
        let process_capture_active = self.process_capture_active.clone();
        let worker_generation = self.worker_generation.clone();
        let capture_context = FallbackCaptureContext {
            spectrum: self.spectrum.clone(),
            gate: gate_clone.clone(),
            gate_override: gate_override_clone.clone(),
            process_capture_active: process_capture_active.clone(),
            worker_generation: worker_generation.clone(),
            generation,
        };
        tokio::task::spawn_blocking(move || {
            let host = cpal::default_host();
            let mut current_device_name = None;
            let mut current_stream: Option<Stream> = None;
            let mut stream_running = false;
            let mut next_device_refresh = Instant::now();

            while !cancel.is_cancelled() && worker_generation.load(Ordering::Acquire) == generation
            {
                let now = Instant::now();
                if now < next_device_refresh {
                    let should_run = analysis_enabled(&gate_clone, &gate_override_clone)
                        && !process_capture_active.load(Ordering::Acquire);
                    if let Some(stream) = current_stream.as_ref() {
                        if should_run && !stream_running {
                            if stream.play().is_ok() {
                                stream_running = true;
                            }
                        } else if !should_run && stream_running && stream.pause().is_ok() {
                            stream_running = false;
                        }
                    }
                    std::thread::sleep(AUDIO_POLL_INTERVAL);
                    continue;
                }
                next_device_refresh = now + DEVICE_REFRESH_INTERVAL;
                let default_device = host.default_output_device();
                let default_device_name = default_device
                    .as_ref()
                    .and_then(|d| d.description().map(|desc| desc.name().to_string()).ok());

                if default_device_name != current_device_name {
                    log::info!(
                        "Audio capture: default device changed from {current_device_name:?} to {default_device_name:?}"
                    );

                    // Releasing old stream and session
                    current_stream = None;
                    stream_running = false;
                    current_device_name = None;

                    if let Some(device) = default_device {
                        let device_name = default_device_name
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string());
                        let config = match device.default_output_config() {
                            Ok(c) => c,
                            Err(e) => {
                                log::warn!(
                                    "Audio capture: no default output config for '{device_name}': {e:?}"
                                );
                                std::thread::sleep(AUDIO_RETRY_INTERVAL);
                                continue;
                            }
                        };

                        log::info!(
                            "Audio capture: device='{}', config={:?} {:?}",
                            device_name,
                            config.sample_format(),
                            config.config()
                        );

                        let stream_config: StreamConfig = config.config();
                        let stream = match config.sample_format() {
                            SampleFormat::F32 => build_capture_stream::<f32>(
                                &device,
                                &stream_config,
                                capture_context.clone(),
                            ),
                            SampleFormat::I16 => build_capture_stream::<i16>(
                                &device,
                                &stream_config,
                                capture_context.clone(),
                            ),
                            SampleFormat::U16 => build_capture_stream::<u16>(
                                &device,
                                &stream_config,
                                capture_context.clone(),
                            ),
                            _ => {
                                std::thread::sleep(AUDIO_RETRY_INTERVAL);
                                continue;
                            }
                        };

                        if let Ok(s) = stream {
                            log::info!("Audio capture stream prepared for '{device_name}'");
                            current_stream = Some(s);
                            current_device_name = Some(device_name);
                        } else if let Err(e) = stream {
                            log::error!("Audio capture: failed to build capture stream: {e:?}");
                        }
                    }
                }

                std::thread::sleep(AUDIO_POLL_INTERVAL);
            }

            // Cleanup when loop ends
        });
    }
}

fn capture_process_audio(
    process_id: u32,
    context: &ProcessCaptureContext,
    analyzer: &mut SpectrumAnalyzer,
) -> Result<(), wasapi::WasapiError> {
    let format = WaveFormat::new(
        LOOPBACK_SAMPLE_BITS,
        LOOPBACK_SAMPLE_BITS,
        &SampleType::Float,
        LOOPBACK_SAMPLE_RATE,
        LOOPBACK_CHANNELS,
        None,
    );
    let mut audio_client = AudioClient::new_application_loopback_client(process_id, true)?;
    audio_client.initialize_client(
        &format,
        &Direction::Capture,
        &StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        },
    )?;
    let event = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;
    audio_client.start_stream()?;
    context.set_process_capture_active(true);

    let mut bytes = Vec::new();
    let result = (|| {
        while !context.cancel.is_cancelled()
            && context.is_current()
            && context.target_process_id.load(Ordering::Relaxed) == process_id
        {
            let _ = event.wait_for_event(100);
            let mut captured = false;
            while let Some(frame_count) = capture_client
                .get_next_packet_size()?
                .filter(|frame_count| *frame_count > 0)
            {
                bytes.resize(frame_count as usize * PROCESS_CAPTURE_BYTES_PER_FRAME, 0);
                let (frames_read, _) = capture_client.read_from_device(&mut bytes)?;
                let bytes_read = frames_read as usize * PROCESS_CAPTURE_BYTES_PER_FRAME;
                captured = true;
                let newer_packet_pending = capture_client
                    .get_next_packet_size()?
                    .is_some_and(|frame_count| frame_count > 0);
                if !newer_packet_pending && analysis_enabled(&context.gate, &context.gate_override)
                {
                    for sample in bytes[..bytes_read].chunks_exact(4) {
                        analyzer.push_sample(
                            f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]),
                            &context.spectrum,
                            &context.gate,
                            &context.gate_override,
                        );
                    }
                } else if !newer_packet_pending {
                    reset_spectrum(analyzer, &context.spectrum);
                }
            }
            if bytes.capacity() > PROCESS_CAPTURE_BUFFER_LIMIT {
                bytes = Vec::with_capacity(PROCESS_CAPTURE_BUFFER_LIMIT);
            }
            if !captured && analysis_enabled(&context.gate, &context.gate_override) {
                for _ in 0..FFT_LEN {
                    analyzer.push_sample(
                        0.0,
                        &context.spectrum,
                        &context.gate,
                        &context.gate_override,
                    );
                }
            } else if !captured {
                reset_spectrum(analyzer, &context.spectrum);
            }
        }
        Ok(())
    })();

    context.set_process_capture_active(false);
    let _ = audio_client.stop_stream();
    result
}

fn find_target_process_id(manager: &IAudioSessionManager2, target_app_id: &str) -> Option<u32> {
    if target_app_id.is_empty() {
        return None;
    }

    // SAFETY: The session manager is owned by the meter thread's COM apartment. Each audio
    // session interface is used only while its enumerator and manager remain alive.
    unsafe {
        let Ok(enumerator) = manager.GetSessionEnumerator() else {
            return None;
        };
        let Ok(count) = enumerator.GetCount() else {
            return None;
        };
        for index in 0..count {
            let Ok(session) = enumerator.GetSession(index) else {
                continue;
            };
            let Ok(session_control) = session.cast::<IAudioSessionControl2>() else {
                continue;
            };
            let Ok(process_id) = session_control.GetProcessId() else {
                continue;
            };
            if process_id != 0
                && process_app_user_model_id(process_id)
                    .is_some_and(|app_id| app_id.eq_ignore_ascii_case(target_app_id))
            {
                return Some(process_id);
            }
        }
    }
    None
}

fn process_app_user_model_id(process_id: u32) -> Option<String> {
    // SAFETY: The process ID comes from an active audio session. The requested access only reads
    // the target process's application identity and does not modify its state.
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()? };
    let mut length = 0;
    // SAFETY: The process handle is valid while this function runs. Passing a null output buffer
    // requests the required UTF-16 buffer length without writing through a dangling pointer.
    let first_result = unsafe { GetApplicationUserModelId(process, &mut length, None) };
    if first_result != ERROR_INSUFFICIENT_BUFFER || length == 0 {
        // SAFETY: `process` was opened above and has not been closed yet.
        unsafe {
            let _ = CloseHandle(process);
        }
        return None;
    }

    let mut app_id = vec![0u16; length as usize];
    // SAFETY: `app_id` has the length requested by the previous call and remains allocated for
    // the duration of this call. The process handle remains valid until it is closed below.
    let result = unsafe {
        GetApplicationUserModelId(process, &mut length, Some(PWSTR(app_id.as_mut_ptr())))
    };
    // SAFETY: `process` was opened above and is no longer used after this point.
    unsafe {
        let _ = CloseHandle(process);
    }
    if result.0 != 0 {
        return None;
    }

    String::from_utf16(&app_id)
        .ok()
        .map(|app_id| app_id.trim_end_matches('\0').to_string())
}

fn build_capture_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    context: FallbackCaptureContext,
) -> Result<Stream, cpal::Error>
where
    T: cpal::SizedSample + Copy,
    f32: FromSample<T>,
{
    let mut analyzer = SpectrumAnalyzer::new();

    device.build_input_stream(
        *config,
        move |data: &[T], _: &_| {
            if !context.is_current() {
                return;
            }
            if context.process_capture_active.load(Ordering::Acquire) {
                return;
            }
            if !analysis_enabled(&context.gate, &context.gate_override) {
                reset_spectrum(&mut analyzer, &context.spectrum);
                return;
            }
            for &sample in data {
                analyzer.push_sample(
                    f32::from_sample(sample),
                    &context.spectrum,
                    &context.gate,
                    &context.gate_override,
                );
            }
        },
        |err| log::error!("Audio error: {err}"),
        None,
    )
}

fn update_spectrum(
    input: &mut [f32],
    fft: &Arc<dyn realfft::RealToComplex<f32>>,
    output: &mut [realfft::num_complex::Complex32],
    adaptive_max: &mut [f32; SPECTRUM_BAND_COUNT],
    spectrum_arc: &Arc<Mutex<[f32; SPECTRUM_BAND_COUNT]>>,
    gate_clone: &Arc<AtomicF32>,
    gate_override_clone: &Arc<AtomicF32>,
) {
    if !analysis_enabled(gate_clone, gate_override_clone) {
        if let Ok(mut spectrum) = spectrum_arc.try_lock() {
            *spectrum = [0.0; SPECTRUM_BAND_COUNT];
        }
        return;
    }
    if let Err(e) = fft.process(input, output) {
        log::warn!("FFT processing failed: {e:?}");
        // Feed the floor value into adaptive_max to prevent slow baseline decay
        // when FFT frames are intermittently dropped.
        for v in adaptive_max.iter_mut() {
            *v = *v * ADAPTIVE_LEVEL_DECAY + ADAPTIVE_LEVEL_FLOOR * ADAPTIVE_LEVEL_LEARNING_RATE;
        }
        return;
    }
    let effective_gate = gate_clone.get() * gate_override_clone.get();
    let mut raw_bins = [0.0f32; SPECTRUM_BAND_COUNT];
    for (band, (start, end)) in FFT_BIN_RANGES.iter().enumerate() {
        let sum = output[*start..*end]
            .iter()
            .map(|value| value.norm())
            .sum::<f32>();
        let avg = sum / (*end - *start) as f32;
        adaptive_max[band] = adaptive_max[band] * ADAPTIVE_LEVEL_DECAY
            + avg.max(ADAPTIVE_LEVEL_FLOOR) * ADAPTIVE_LEVEL_LEARNING_RATE;
        raw_bins[band] = (avg / (adaptive_max[band] * SPECTRUM_NORMALIZATION_GAIN)
            * effective_gate)
            .clamp(0.0, 1.0);
    }
    let mut final_bins = [0.0f32; SPECTRUM_BAND_COUNT];
    for (output_band, (input_band, gain)) in SPECTRUM_OUTPUT_MAPPING.iter().enumerate() {
        final_bins[output_band] = raw_bins[*input_band] * gain;
    }
    if let Ok(mut s) = spectrum_arc.try_lock() {
        *s = final_bins;
    }
}

fn analysis_enabled(gate: &AtomicF32, gate_override: &AtomicF32) -> bool {
    gate.get() > 0.0 && gate_override.get() > 0.0
}

fn reset_spectrum(analyzer: &mut SpectrumAnalyzer, spectrum: &Mutex<[f32; SPECTRUM_BAND_COUNT]>) {
    analyzer.input_len = 0;
    if let Ok(mut spectrum) = spectrum.try_lock() {
        *spectrum = [0.0; SPECTRUM_BAND_COUNT];
    }
}

impl Drop for AudioProcessor {
    fn drop(&mut self) {
        log::info!("AudioProcessor dropped");
        self.stop_workers();
    }
}
