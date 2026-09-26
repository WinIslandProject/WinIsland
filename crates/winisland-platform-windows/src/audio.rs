mod capture;
mod volume;

use std::cell::RefCell;

use winisland_platform::{
    AudioMeter, AudioProvider, PlatformError, ProcessCapture, VolumeEndpoint, VolumeState,
};

pub struct WindowsAudio;

pub(crate) fn process_loopback_available() -> bool {
    capture::process_loopback_available()
}

thread_local! {
    static DEVICE_EVENTS: RefCell<Option<volume::WindowsVolumeEndpoint>> = const { RefCell::new(None) };
}

impl AudioProvider for WindowsAudio {
    fn open_volume_endpoint(&self) -> Result<Box<dyn VolumeEndpoint>, PlatformError> {
        Ok(Box::new(volume::WindowsVolumeEndpoint::new()?))
    }

    fn open_meter(&self) -> Result<Box<dyn AudioMeter>, PlatformError> {
        Ok(Box::new(capture::WindowsAudioMeter::new()?))
    }

    fn open_process_capture(
        &self,
        process_id: u32,
    ) -> Result<Box<dyn ProcessCapture>, PlatformError> {
        Ok(Box::new(capture::WindowsProcessCapture::new(process_id)?))
    }

    fn volume(&self) -> Result<VolumeState, PlatformError> {
        let mut endpoint = volume::WindowsVolumeEndpoint::new()?;
        if !endpoint.ensure_endpoint() {
            return Err(PlatformError::Unavailable("volume control"));
        }
        endpoint
            .read()
            .ok_or(PlatformError::Unavailable("volume control"))
    }

    fn set_volume(&self, level: f32) -> Result<(), PlatformError> {
        let mut endpoint = volume::WindowsVolumeEndpoint::new()?;
        if endpoint.ensure_endpoint()
            && endpoint.apply(winisland_platform::VolumeCommand::SetLevel(level))
        {
            Ok(())
        } else {
            Err(PlatformError::Unavailable("volume control"))
        }
    }

    fn toggle_mute(&self) -> Result<(), PlatformError> {
        let mut endpoint = volume::WindowsVolumeEndpoint::new()?;
        if endpoint.ensure_endpoint()
            && endpoint.apply(winisland_platform::VolumeCommand::ToggleMute)
        {
            Ok(())
        } else {
            Err(PlatformError::Unavailable("volume control"))
        }
    }

    fn poll_device_events(&self) -> Result<bool, PlatformError> {
        DEVICE_EVENTS.with(|slot| {
            if slot.borrow().is_none() {
                *slot.borrow_mut() = Some(volume::WindowsVolumeEndpoint::new()?);
            }
            Ok(slot
                .borrow()
                .as_ref()
                .is_some_and(VolumeEndpoint::take_device_change))
        })
    }
}
