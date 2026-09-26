use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    COINIT, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize,
};
use winisland_platform::PlatformError;

pub(crate) struct ComGuard {
    initialized: bool,
}

impl ComGuard {
    pub(crate) fn mta() -> Result<Self, PlatformError> {
        Self::new(COINIT_MULTITHREADED)
    }

    pub(crate) fn sta() -> Result<Self, PlatformError> {
        Self::new(COINIT_APARTMENTTHREADED)
    }

    fn new(apartment: COINIT) -> Result<Self, PlatformError> {
        // SAFETY: The guard balances a successful initialization on the same thread.
        let result = unsafe { CoInitializeEx(None, apartment) };
        if result.is_ok() {
            Ok(Self { initialized: true })
        } else if result == RPC_E_CHANGED_MODE {
            Ok(Self { initialized: false })
        } else {
            Err(PlatformError::Backend(format!(
                "COM initialization failed: {result:?}"
            )))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: This guard is dropped on its creating thread after successful initialization.
            unsafe { CoUninitialize() };
        }
    }
}
