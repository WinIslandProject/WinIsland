use crate::{PlatformError, VolumeCommand, VolumeState};

/// Device-scoped meter. Use on an audio worker; calls return empty or zero when unavailable.
pub trait AudioMeter {
    /// Rebinds the default endpoint after a device change.
    fn refresh_device(&mut self);
    /// Resolves a source application to a process, or `None` when absent.
    fn find_target_process_id(&self, app_id: &str) -> Option<u32>;
    /// Reads the current peak, or zero when the process is unavailable.
    fn peak(&self, process_id: u32) -> f32;
}

/// Worker-owned process capture. `Drop` stops capture and releases its COM resources.
pub trait ProcessCapture {
    /// Appends one capture cycle; errors end this capture and `false` means no data yet.
    fn read_cycle(&mut self, samples: &mut Vec<f32>) -> Result<bool, PlatformError>;
}

/// Worker-owned endpoint state. Methods must not run on the rendering thread.
pub trait VolumeEndpoint {
    /// Reports whether the device enumerator is currently available.
    fn has_enumerator(&self) -> bool;
    /// Reopens the enumerator after a failure; returns whether it is ready.
    fn ensure_enumerator(&mut self) -> bool;
    /// Reports whether endpoint change notifications are subscribed.
    fn has_notifier(&self) -> bool;
    /// Reinstalls notifications; returns whether they are active.
    fn ensure_notifier(&mut self) -> bool;
    /// Consumes a pending endpoint change flag.
    fn take_device_change(&self) -> bool;
    /// Reports whether the current endpoint is available.
    fn has_endpoint(&self) -> bool;
    /// Reopens the current endpoint; returns whether it is ready.
    fn ensure_endpoint(&mut self) -> bool;
    /// Drops the endpoint so the next call can rebind it.
    fn invalidate_endpoint(&mut self);
    /// Applies a command; returns false when the endpoint rejects it.
    fn apply(&mut self, command: VolumeCommand) -> bool;
    /// Reads level and mute state, or `None` when the endpoint is unavailable.
    fn read(&self) -> Option<VolumeState>;
}

/// Audio calls block and must run off the rendering thread. A capture stays valid until
/// stopped or dropped; missing devices return errors and concurrent calls are not reentrant.
pub trait AudioProvider {
    /// Opens worker-owned volume state, or returns the endpoint error.
    fn open_volume_endpoint(&self) -> Result<Box<dyn VolumeEndpoint>, PlatformError>;
    /// Opens a worker-owned process meter, or returns a device error.
    fn open_meter(&self) -> Result<Box<dyn AudioMeter>, PlatformError>;
    /// Opens process loopback capture, or returns an unsupported/device error.
    fn open_process_capture(
        &self,
        process_id: u32,
    ) -> Result<Box<dyn ProcessCapture>, PlatformError>;
    /// Reads the current default endpoint state, or returns an endpoint error.
    fn volume(&self) -> Result<VolumeState, PlatformError>;
    /// Sets normalized volume, or returns an endpoint error.
    fn set_volume(&self, level: f32) -> Result<(), PlatformError>;
    /// Toggles mute, or returns an endpoint error.
    fn toggle_mute(&self) -> Result<(), PlatformError>;
    /// Polls for a default device change, or returns a subscription error.
    fn poll_device_events(&self) -> Result<bool, PlatformError>;
}
