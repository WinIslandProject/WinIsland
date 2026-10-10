use std::ffi::c_void;

use windows::Win32::Foundation::FILETIME;
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetSystemTimes, SetProcessWorkingSetSize,
};
use windows::core::HSTRING;
use winisland_platform::{MetricSelection, PlatformError, SystemMetrics, SystemSample};

mod gpu;

const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const HARDWARE_INTERFACE_FLAG: u8 = 0b01;
const FILTER_INTERFACE_FLAG: u8 = 0b10;

pub struct WindowsMetrics;

impl SystemMetrics for WindowsMetrics {
    fn sample(&self, selection: MetricSelection) -> Result<SystemSample, PlatformError> {
        let mut sample = SystemSample::default();
        if selection.cpu
            && let Some((idle, total)) = cpu_times()
        {
            sample.cpu_idle_ticks = Some(idle);
            sample.cpu_total_ticks = Some(total);
        }
        if selection.memory
            && let Some((used, total, load)) = memory()
        {
            sample.memory_used_bytes = Some(used);
            sample.memory_total_bytes = Some(total);
            sample.memory_load_percent = Some(load);
        }
        if selection.network
            && let Some((received, sent)) = network()
        {
            sample.network_received_bytes = Some(received);
            sample.network_sent_bytes = Some(sent);
        }
        if selection.disk
            && let Some((free, total)) = disk()
        {
            sample.disk_free_bytes = Some(free);
            sample.disk_total_bytes = Some(total);
        }
        if selection.gpu {
            sample.gpu_usage = gpu::usage(&gpu::adapters());
        }
        Ok(sample)
    }

    fn gpu_adapters(&self) -> Vec<String> {
        gpu::adapters()
            .iter()
            .map(|adapter| adapter.name.clone())
            .collect()
    }

    fn trim_working_set(&self) -> Result<(), PlatformError> {
        // SAFETY: GetCurrentProcess returns a valid pseudo-handle; maximum limits request a trim.
        unsafe { SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX) }
            .map_err(PlatformError::backend)
    }
}

fn filetime_ticks(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

fn cpu_times() -> Option<(u64, u64)> {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: All three outputs are initialized writable FILETIME values.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    Some((
        filetime_ticks(idle),
        filetime_ticks(kernel).saturating_add(filetime_ticks(user)),
    ))
}

fn memory() -> Option<(u64, u64, u32)> {
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: status declares its size and is writable for the duration of the call.
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    Some((
        status.ullTotalPhys.saturating_sub(status.ullAvailPhys),
        status.ullTotalPhys,
        status.dwMemoryLoad,
    ))
}

fn network() -> Option<(u64, u64)> {
    let mut table = std::ptr::null_mut::<MIB_IF_TABLE2>();
    // SAFETY: table is an initialized output pointer.
    if unsafe { GetIfTable2(&mut table) }.0 != 0 || table.is_null() {
        return None;
    }
    // SAFETY: A successful GetIfTable2 allocates a table with NumEntries rows.
    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let mut all = (0u64, 0u64);
    let mut hardware = None::<(u64, u64)>;
    for row in rows {
        let flags = row.InterfaceAndOperStatusFlags._bitfield;
        if row.OperStatus != IfOperStatusUp
            || row.Type == IF_TYPE_SOFTWARE_LOOPBACK
            || flags & FILTER_INTERFACE_FLAG != 0
        {
            continue;
        }
        all = (
            all.0.saturating_add(row.InOctets),
            all.1.saturating_add(row.OutOctets),
        );
        if flags & HARDWARE_INTERFACE_FLAG != 0 {
            let (received, sent) = hardware.unwrap_or_default();
            hardware = Some((
                received.saturating_add(row.InOctets),
                sent.saturating_add(row.OutOctets),
            ));
        }
    }
    // SAFETY: table is the allocation returned by GetIfTable2 and is released once.
    unsafe { FreeMibTable(table.cast::<c_void>()) };
    Some(hardware.unwrap_or(all))
}

fn disk() -> Option<(u64, u64)> {
    let root = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string()) + "\\";
    let mut total = 0u64;
    let mut free = 0u64;
    // SAFETY: The root is NUL-terminated and total and free are writable outputs.
    unsafe {
        GetDiskFreeSpaceExW(
            &HSTRING::from(root),
            None,
            Some(&mut total),
            Some(&mut free),
        )
    }
    .ok()?;
    (total > 0).then_some((free, total))
}
