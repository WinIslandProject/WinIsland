use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};

use parking_lot::Mutex;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Variant::{VARIANT, VT_I4, VT_UI1, VT_UI4};
use windows::Win32::System::Wmi::{ISWbemLocator, ISWbemObject, ISWbemServices, SWbemLocator};
use windows::core::BSTR;
use winisland_platform::{BrightnessFeed, BrightnessSnapshot};

use crate::com::ComGuard;

const BRIGHTNESS_QUERY: &str =
    "SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE Active = TRUE";
const METHODS_QUERY: &str = "SELECT * FROM WmiMonitorBrightnessMethods WHERE Active = TRUE";

pub(super) struct WindowsBrightnessFeed {
    snapshot: Arc<Mutex<BrightnessSnapshot>>,
    sender: SyncSender<f32>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl WindowsBrightnessFeed {
    pub(super) fn new() -> Self {
        let snapshot = Arc::new(Mutex::new(BrightnessSnapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::sync_channel(8);
        let worker_snapshot = snapshot.clone();
        let worker_stop = stop.clone();
        let worker = std::thread::spawn(move || {
            let Ok(_com) = ComGuard::mta() else {
                log::debug!("Windows brightness monitor could not initialize COM");
                return;
            };
            if let Err(error) = run_monitor(&worker_snapshot, &receiver, &worker_stop) {
                log::debug!("Windows brightness monitor unavailable: {error}");
            }
        });
        Self {
            snapshot,
            sender,
            stop,
            worker: Some(worker),
        }
    }
}

impl BrightnessFeed for WindowsBrightnessFeed {
    fn snapshot(&self) -> BrightnessSnapshot {
        *self.snapshot.lock()
    }

    fn set_level(&self, level: f32) {
        if level.is_finite() && self.snapshot().available {
            let _ = self.sender.try_send(level.clamp(0.0, 1.0));
        }
    }
}

impl Drop for WindowsBrightnessFeed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(super) fn available() -> bool {
    let Ok(_com) = ComGuard::mta() else {
        return false;
    };
    // SAFETY: The WMI service and queried objects remain on this COM thread.
    unsafe {
        connect_wmi().is_ok_and(|services| {
            query_first(&services, BRIGHTNESS_QUERY).is_ok()
                && query_first(&services, METHODS_QUERY).is_ok()
        })
    }
}

fn run_monitor(
    snapshot: &Mutex<BrightnessSnapshot>,
    receiver: &mpsc::Receiver<f32>,
    stop: &AtomicBool,
) -> windows::core::Result<()> {
    // SAFETY: All WMI objects stay on this COM-initialized worker thread.
    unsafe {
        let services = connect_wmi()?;
        let initial = query_first(&services, BRIGHTNESS_QUERY)?;
        let level = property_u32(&initial, "CurrentBrightness")?.min(100) as f32 / 100.0;
        publish(snapshot, level, false);
        let setter = query_first(&services, METHODS_QUERY)?;
        let events = services.ExecNotificationQuery(
            &BSTR::from("SELECT * FROM WmiMonitorBrightnessEvent WHERE Active = TRUE"),
            &BSTR::from("WQL"),
            0,
            None,
        )?;
        while !stop.load(Ordering::Acquire) {
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

unsafe fn connect_wmi() -> windows::core::Result<ISWbemServices> {
    // SAFETY: Caller runs on a COM-initialized thread and keeps the service on it.
    unsafe {
        let locator: ISWbemLocator = CoCreateInstance(&SWbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let empty = BSTR::new();
        locator.ConnectServer(
            &empty,
            &BSTR::from("ROOT\\WMI"),
            &empty,
            &empty,
            &empty,
            &empty,
            0,
            None,
        )
    }
}

unsafe fn query_first(
    services: &ISWbemServices,
    query: &str,
) -> windows::core::Result<ISWbemObject> {
    // SAFETY: Caller keeps the WMI service on its COM-initialized thread.
    let objects = unsafe { services.ExecQuery(&BSTR::from(query), &BSTR::from("WQL"), 0, None)? };
    // SAFETY: WMI returns an error if no instance matches.
    unsafe { objects.ItemIndex(0) }
}

unsafe fn property_u32(object: &ISWbemObject, name: &str) -> windows::core::Result<u32> {
    // SAFETY: The property is read from a live object on its COM thread.
    let value = unsafe { object.Properties_()?.Item(&BSTR::from(name), 0)?.Value()? };
    // SAFETY: The variant members are read only after checking their discriminant.
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
    // SAFETY: The WMI method and input object remain on their creating COM thread.
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
    let mut current = snapshot.lock();
    if (current.level - level).abs() > 0.001 && notify {
        current.revision = current.revision.wrapping_add(1);
    }
    current.level = level;
    current.available = true;
}
