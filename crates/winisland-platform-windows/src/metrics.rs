use std::cell::RefCell;
use std::ffi::c_void;

use windows::Win32::Foundation::FILETIME;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
    IDXGIAdapter3, IDXGIFactory1,
};
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetSystemTimes, SetProcessWorkingSetSize,
};
use windows::core::{HSTRING, Interface};
use winisland_platform::{MetricSelection, PlatformError, SystemMetrics, SystemSample};

const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;

thread_local! {
    static GPU_ADAPTERS: RefCell<Option<Vec<IDXGIAdapter3>>> = const { RefCell::new(None) };
}

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
            && let Some((bytes, link)) = network()
        {
            sample.network_bytes = Some(bytes);
            sample.network_link_bits_per_second = Some(link);
        }
        if selection.disk
            && let Some((free, total)) = disk()
        {
            sample.disk_free_bytes = Some(free);
            sample.disk_total_bytes = Some(total);
        }
        if selection.gpu
            && let Some((used, budget)) = GPU_ADAPTERS.with(|cell| gpu(&mut cell.borrow_mut()))
        {
            sample.gpu_memory_used_bytes = Some(used);
            sample.gpu_memory_budget_bytes = Some(budget);
        }
        Ok(sample)
    }

    fn trim_working_set(&self) -> Result<(), PlatformError> {
        // SAFETY: GetCurrentProcess returns a valid pseudo-handle; maximum limits request a trim.
        unsafe { SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX) }
            .map_err(|error| PlatformError::Backend(error.to_string()))
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
    let mut bytes = 0u64;
    let mut link = 0u64;
    for row in rows {
        if row.OperStatus == IfOperStatusUp && row.Type != IF_TYPE_SOFTWARE_LOOPBACK {
            bytes = bytes.saturating_add(row.InOctets.saturating_add(row.OutOctets));
            link = link.saturating_add(row.ReceiveLinkSpeed.saturating_add(row.TransmitLinkSpeed));
        }
    }
    // SAFETY: table is the allocation returned by GetIfTable2 and is released once.
    unsafe { FreeMibTable(table.cast::<c_void>()) };
    Some((bytes, link))
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

fn gpu(adapters: &mut Option<Vec<IDXGIAdapter3>>) -> Option<(u64, u64)> {
    if adapters.is_none() {
        // SAFETY: Factory creation and adapter enumeration retain no caller-owned pointers.
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.ok()?;
        let mut available = Vec::new();
        for index in 0..16 {
            // SAFETY: The factory owns the adapter and returns an owned interface.
            let Ok(adapter) = (unsafe { factory.EnumAdapters1(index) }) else {
                break;
            };
            if let Ok(adapter) = adapter.cast::<IDXGIAdapter3>() {
                available.push(adapter);
            }
        }
        *adapters = Some(available);
    }
    let mut used = 0u64;
    let mut budget = 0u64;
    for adapter in adapters.as_ref()? {
        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        // SAFETY: info is writable and adapter is retained by this thread-local cache.
        if unsafe { adapter.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info) }
            .is_ok()
            && info.Budget > 0
        {
            used = used.saturating_add(info.CurrentUsage);
            budget = budget.saturating_add(info.Budget);
        }
    }
    (budget > 0).then_some((used, budget))
}
