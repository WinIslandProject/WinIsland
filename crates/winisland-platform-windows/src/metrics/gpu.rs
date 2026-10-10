use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
};
use windows::Win32::System::Performance::{
    PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
    PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA, PdhAddEnglishCounterW, PdhCloseQuery,
    PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows::core::{PCWSTR, w};

const ADAPTER_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const IDLE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct GpuAdapter {
    pub(super) name: String,
    luid: u64,
}

struct AdapterCache {
    refreshed: Instant,
    adapters: Arc<[GpuAdapter]>,
}

struct UsageMonitor {
    usage: Mutex<HashMap<u64, f32>>,
    requested: Mutex<Instant>,
    running: AtomicBool,
}

static ADAPTERS: Mutex<Option<AdapterCache>> = Mutex::new(None);
static REFRESHING: AtomicBool = AtomicBool::new(false);
static MONITOR: LazyLock<UsageMonitor> = LazyLock::new(|| UsageMonitor {
    usage: Mutex::new(HashMap::new()),
    requested: Mutex::new(Instant::now()),
    running: AtomicBool::new(false),
});

pub(super) fn adapters() -> Arc<[GpuAdapter]> {
    let mut cache = ADAPTERS.lock();
    if let Some(current) = cache.as_ref() {
        if current.refreshed.elapsed() >= ADAPTER_REFRESH_INTERVAL
            && !REFRESHING.swap(true, Ordering::AcqRel)
            && std::thread::Builder::new()
                .name("winisland-gpu-adapters".to_string())
                .spawn(|| {
                    let adapters = enumerate_adapters().into();
                    *ADAPTERS.lock() = Some(AdapterCache {
                        refreshed: Instant::now(),
                        adapters,
                    });
                    REFRESHING.store(false, Ordering::Release);
                })
                .is_err()
        {
            REFRESHING.store(false, Ordering::Release);
        }
        return current.adapters.clone();
    }
    let adapters: Arc<[GpuAdapter]> = enumerate_adapters().into();
    *cache = Some(AdapterCache {
        refreshed: Instant::now(),
        adapters: adapters.clone(),
    });
    adapters
}

pub(super) fn usage(adapters: &[GpuAdapter]) -> Vec<Option<f32>> {
    *MONITOR.requested.lock() = Instant::now();
    if !MONITOR.running.swap(true, Ordering::AcqRel)
        && std::thread::Builder::new()
            .name("winisland-gpu-usage".to_string())
            .spawn(|| run_monitor(&MONITOR))
            .is_err()
    {
        MONITOR.running.store(false, Ordering::Release);
    }
    let usage = MONITOR.usage.lock();
    adapters
        .iter()
        .map(|adapter| usage.get(&adapter.luid).copied())
        .collect()
}

fn luid_key(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

fn instance_luid(name: &str) -> Option<(u64, &str)> {
    let luid = &name[name.find("luid_0x")? + "luid_0x".len()..];
    let (high, rest) = luid.split_once("_0x")?;
    let (low, rest) = rest.split_once('_')?;
    Some((
        luid_key(
            u32::from_str_radix(high, 16).ok()?,
            u32::from_str_radix(low, 16).ok()?,
        ),
        rest,
    ))
}

fn engine_key(name: &str) -> Option<(u64, u32, u32)> {
    let (luid, rest) = instance_luid(name)?;
    let (physical, rest) = rest.strip_prefix("phys_")?.split_once("_eng_")?;
    let engine = rest.split('_').next()?;
    Some((luid, physical.parse().ok()?, engine.parse().ok()?))
}

fn physical_adapter_luids() -> Option<HashSet<u64>> {
    let query = CounterQuery::open(w!("\\GPU Adapter Memory(*)\\Dedicated Usage"))?;
    let luids: HashSet<u64> = query
        .items()?
        .iter()
        .filter_map(|(name, _)| instance_luid(name).map(|(luid, _)| luid))
        .collect();
    (!luids.is_empty()).then_some(luids)
}

fn enumerate_adapters() -> Vec<GpuAdapter> {
    // SAFETY: Factory creation retains no caller-owned pointers.
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return Vec::new();
    };
    let physical = physical_adapter_luids();
    let mut adapters: Vec<GpuAdapter> = Vec::new();
    for index in 0.. {
        // SAFETY: The factory owns the adapter and returns an owned interface.
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(index) }) else {
            break;
        };
        // SAFETY: This only queries the live adapter returned by DXGI.
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
            continue;
        };
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let luid = luid_key(desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart);
        if adapters.iter().any(|adapter| adapter.luid == luid)
            || physical
                .as_ref()
                .is_some_and(|known| !known.contains(&luid))
        {
            continue;
        }
        let end = desc
            .Description
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(desc.Description.len());
        adapters.push(GpuAdapter {
            name: String::from_utf16_lossy(&desc.Description[..end])
                .trim()
                .to_string(),
            luid,
        });
    }
    adapters
}

fn run_monitor(monitor: &UsageMonitor) {
    if let Some(query) = CounterQuery::open(w!("\\GPU Engine(*)\\Utilization Percentage")) {
        while monitor.requested.lock().elapsed() < IDLE_TIMEOUT {
            if let Some(items) = query.items() {
                *monitor.usage.lock() = adapter_usage(&items);
            }
            std::thread::sleep(SAMPLE_INTERVAL);
        }
    }
    monitor.usage.lock().clear();
    monitor.running.store(false, Ordering::Release);
}

fn adapter_usage(items: &[(String, f64)]) -> HashMap<u64, f32> {
    let mut engines: HashMap<(u64, u32, u32), f64> = HashMap::new();
    for (name, value) in items {
        if let Some(key) = engine_key(name) {
            *engines.entry(key).or_default() += value;
        }
    }
    let mut usage = HashMap::new();
    for ((luid, _, _), value) in engines {
        let adapter = usage.entry(luid).or_insert(0.0f32);
        *adapter = adapter.max((value / 100.0).clamp(0.0, 1.0) as f32);
    }
    usage
}

struct CounterQuery {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}

impl CounterQuery {
    fn open(path: PCWSTR) -> Option<Self> {
        let mut query = PDH_HQUERY(std::ptr::null_mut());
        // SAFETY: query is a writable handle output for a local, real-time query.
        if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
            return None;
        }
        let mut counter = PDH_HCOUNTER(std::ptr::null_mut());
        // SAFETY: query is open, path is a NUL-terminated wide string, and counter is writable.
        let added = unsafe { PdhAddEnglishCounterW(query, path, 0, &mut counter) };
        let counter_query = Self { query, counter };
        if added != 0 {
            return None;
        }
        // SAFETY: The query is open; the first collection primes rate counters.
        unsafe { PdhCollectQueryData(counter_query.query) };
        Some(counter_query)
    }

    fn items(&self) -> Option<Vec<(String, f64)>> {
        // SAFETY: The query stays open for the lifetime of self.
        if unsafe { PdhCollectQueryData(self.query) } != 0 {
            return None;
        }
        let mut size = 0u32;
        let mut count = 0u32;
        // SAFETY: A null buffer asks PDH for the required size.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut size, &mut count, None)
        };
        if status != PDH_MORE_DATA || size == 0 {
            return None;
        }
        let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
        let items = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        // SAFETY: buffer holds size bytes aligned for the item array PDH writes into it.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                self.counter,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                Some(items),
            )
        };
        if status != 0 {
            return None;
        }
        // SAFETY: PDH wrote count items at the start of buffer, which outlives this slice.
        let items = unsafe { std::slice::from_raw_parts(items, count as usize) };
        Some(
            items
                .iter()
                .filter(|item| {
                    matches!(
                        item.FmtValue.CStatus,
                        PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
                    )
                })
                .filter_map(|item| {
                    // SAFETY: szName points to a NUL-terminated instance name inside buffer, and
                    // PDH_FMT_DOUBLE fills the double member of the value union.
                    unsafe {
                        item.szName
                            .to_string()
                            .ok()
                            .map(|name| (name, item.FmtValue.Anonymous.doubleValue))
                    }
                })
                .collect(),
        )
    }
}

impl Drop for CounterQuery {
    fn drop(&mut self) {
        // SAFETY: The query was opened by this value and is closed exactly once.
        unsafe { PdhCloseQuery(self.query) };
    }
}
