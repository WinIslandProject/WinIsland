use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Variant::{VARIANT, VT_I4, VT_UI1, VT_UI4};
use windows::Win32::System::Wmi::{ISWbemLocator, ISWbemObject, ISWbemServices, SWbemLocator};
use windows::core::BSTR;

#[derive(Clone, Copy, Default)]
pub(super) struct BrightnessSnapshot {
    pub level: f32,
    pub revision: u64,
    pub available: bool,
}

pub(super) struct BrightnessMonitor {
    snapshot: Arc<Mutex<BrightnessSnapshot>>,
    command_sender: SyncSender<f32>,
    cancellation: CancellationToken,
}

impl BrightnessMonitor {
    pub(super) fn new() -> Self {
        let snapshot = Arc::new(Mutex::new(BrightnessSnapshot::default()));
        let cancellation = CancellationToken::new();
        let (command_sender, receiver) = mpsc::sync_channel(8);
        let shared = snapshot.clone();
        let stop = cancellation.clone();
        std::thread::spawn(move || {
            // SAFETY: COM is initialized and released on this worker thread only.
            if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() {
                return;
            }
            let result = run_brightness_monitor(&shared, &receiver, &stop);
            if let Err(error) = result {
                log::debug!("Windows brightness monitor unavailable: {error}");
            }
            // SAFETY: Balances the successful CoInitializeEx on this thread.
            unsafe { CoUninitialize() };
        });
        Self {
            snapshot,
            command_sender,
            cancellation,
        }
    }

    pub(super) fn snapshot(&self) -> BrightnessSnapshot {
        *self
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn set_level(&self, level: f32) {
        if level.is_finite() && self.snapshot().available {
            let _ = self.command_sender.try_send(level.clamp(0.0, 1.0));
        }
    }
}

impl Drop for BrightnessMonitor {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

fn run_brightness_monitor(
    snapshot: &Mutex<BrightnessSnapshot>,
    receiver: &mpsc::Receiver<f32>,
    stop: &CancellationToken,
) -> windows::core::Result<()> {
    // SAFETY: All WMI objects stay on this COM-initialized worker thread.
    unsafe {
        let locator: ISWbemLocator = CoCreateInstance(&SWbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let empty = BSTR::new();
        let services = locator.ConnectServer(
            &empty,
            &BSTR::from("ROOT\\WMI"),
            &empty,
            &empty,
            &empty,
            &empty,
            0,
            None,
        )?;
        let initial = query_first(
            &services,
            "SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE Active = TRUE",
        )?;
        let level = property_u32(&initial, "CurrentBrightness")?.min(100) as f32 / 100.0;
        publish(snapshot, level, false);
        let setter = query_first(
            &services,
            "SELECT * FROM WmiMonitorBrightnessMethods WHERE Active = TRUE",
        )?;
        let events = services.ExecNotificationQuery(
            &BSTR::from("SELECT * FROM WmiMonitorBrightnessEvent WHERE Active = TRUE"),
            &BSTR::from("WQL"),
            0,
            None,
        )?;
        while !stop.is_cancelled() {
            if let Ok(level) = receiver.try_recv()
                && set_brightness(&setter, (level * 100.0).round() as u8).is_ok()
            {
                publish(snapshot, level, true);
            }
            if let Ok(event) = events.NextEvent(120)
                && let Ok(value) = property_u32(&event, "Brightness")
            {
                publish(snapshot, value.min(100) as f32 / 100.0, true);
            }
        }
    }
    Ok(())
}

unsafe fn query_first(
    services: &ISWbemServices,
    query: &str,
) -> windows::core::Result<ISWbemObject> {
    // SAFETY: Caller keeps this WMI service on its COM-initialized thread.
    let objects = unsafe { services.ExecQuery(&BSTR::from(query), &BSTR::from("WQL"), 0, None)? };
    // SAFETY: WMI returns an error if the query has no matching instance.
    unsafe { objects.ItemIndex(0) }
}

unsafe fn property_u32(object: &ISWbemObject, name: &str) -> windows::core::Result<u32> {
    // SAFETY: The property value is read from a live WMI object on its COM thread.
    let value = unsafe { object.Properties_()?.Item(&BSTR::from(name), 0)?.Value()? };
    // SAFETY: These fields are valid for the variant types checked below.
    let inner = unsafe { &value.Anonymous.Anonymous };
    let number = match inner.vt {
        VT_UI1 => unsafe { inner.Anonymous.bVal as u32 },
        VT_UI4 => unsafe { inner.Anonymous.ulVal },
        VT_I4 => unsafe { inner.Anonymous.lVal.max(0) as u32 },
        _ => 0,
    };
    Ok(number)
}

unsafe fn set_brightness(object: &ISWbemObject, level: u8) -> windows::core::Result<()> {
    // SAFETY: The WMI method and input object remain on the creating COM thread.
    unsafe {
        let method = object
            .Methods_()?
            .Item(&BSTR::from("WmiSetBrightness"), 0)?;
        let input = method.InParameters()?.SpawnInstance_(0)?;
        let mut brightness = VARIANT::default();
        (*brightness.Anonymous.Anonymous).vt = VT_UI1;
        (*brightness.Anonymous.Anonymous).Anonymous.bVal = level;
        input
            .Properties_()?
            .Item(&BSTR::from("Brightness"), 0)?
            .SetValue(&brightness)?;
        let mut timeout = VARIANT::default();
        (*timeout.Anonymous.Anonymous).vt = VT_UI4;
        (*timeout.Anonymous.Anonymous).Anonymous.ulVal = 0;
        input
            .Properties_()?
            .Item(&BSTR::from("Timeout"), 0)?
            .SetValue(&timeout)?;
        object.ExecMethod_(&BSTR::from("WmiSetBrightness"), &input, 0, None)?;
    }
    Ok(())
}

fn publish(snapshot: &Mutex<BrightnessSnapshot>, level: f32, notify: bool) {
    let mut current = snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if (current.level - level).abs() > 0.001 && notify {
        current.revision = current.revision.wrapping_add(1);
    }
    current.level = level;
    current.available = true;
}
