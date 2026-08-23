use std::cell::RefCell;
use std::time::{Duration, Instant};

use skia_safe::Color;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
pub(crate) const CPU_COLOR: Color = Color::from_rgb(50, 190, 246);
pub(crate) const RAM_COLOR: Color = Color::from_rgb(175, 82, 222);
const WARNING_COLOR: Color = Color::from_rgb(255, 159, 10);
const CRITICAL_COLOR: Color = Color::from_rgb(255, 69, 58);

#[derive(Clone, Copy, Default)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

#[derive(Default)]
struct ResourceUsageCache {
    sampled_at: Option<Instant>,
    previous_cpu: Option<CpuTimes>,
    cpu: Option<f32>,
    ram: Option<f32>,
    cpu_text: String,
    ram_text: String,
}

impl ResourceUsageCache {
    fn refresh_if_due(&mut self) {
        if self
            .sampled_at
            .is_some_and(|sampled_at| sampled_at.elapsed() < SAMPLE_INTERVAL)
        {
            return;
        }
        self.sampled_at = Some(Instant::now());

        if let Some(current) = read_cpu_times() {
            if let Some(previous) = self.previous_cpu {
                let total = current.total.saturating_sub(previous.total);
                let idle = current.idle.saturating_sub(previous.idle);
                if total > 0 {
                    self.cpu = Some((1.0 - idle as f32 / total as f32).clamp(0.0, 1.0));
                }
            }
            self.previous_cpu = Some(current);
        }
        if let Some(ram) = read_ram_usage() {
            self.ram = Some(ram);
        }
        update_percent_text(&mut self.cpu_text, self.cpu);
        update_percent_text(&mut self.ram_text, self.ram);
    }

    fn next_refresh_delay(&self) -> Duration {
        self.sampled_at
            .map(|sampled_at| SAMPLE_INTERVAL.saturating_sub(sampled_at.elapsed()))
            .unwrap_or_default()
    }
}

pub(crate) struct ResourceUsage<'a> {
    pub(crate) cpu: Option<f32>,
    pub(crate) ram: Option<f32>,
    pub(crate) cpu_text: &'a str,
    pub(crate) ram_text: &'a str,
}

thread_local! {
    static RESOURCE_USAGE: RefCell<ResourceUsageCache> = RefCell::new(ResourceUsageCache::default());
}

pub(crate) fn with_resource_usage<R>(draw: impl FnOnce(ResourceUsage<'_>) -> R) -> R {
    RESOURCE_USAGE.with(|cell| {
        let mut cache = cell.borrow_mut();
        cache.refresh_if_due();
        draw(ResourceUsage {
            cpu: cache.cpu,
            ram: cache.ram,
            cpu_text: &cache.cpu_text,
            ram_text: &cache.ram_text,
        })
    })
}

pub(crate) fn next_refresh_delay() -> Duration {
    RESOURCE_USAGE.with(|cell| cell.borrow().next_refresh_delay())
}

pub(crate) fn alpha_color(color: Color, alpha: u8) -> Color {
    Color::from_argb(alpha, color.r(), color.g(), color.b())
}

pub(crate) fn usage_color(base: Color, usage: f32) -> Color {
    if usage <= 0.75 {
        base
    } else if usage <= 0.9 {
        blend_color(base, WARNING_COLOR, (usage - 0.75) / 0.15)
    } else {
        blend_color(WARNING_COLOR, CRITICAL_COLOR, (usage - 0.9) / 0.1)
    }
}

fn blend_color(from: Color, to: Color, amount: f32) -> Color {
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount) as u8;
    Color::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

fn filetime_ticks(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

fn read_cpu_times() -> Option<CpuTimes> {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: All pointers reference initialized FILETIME values that remain valid for the
    // duration of this synchronous call.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    Some(CpuTimes {
        idle: filetime_ticks(idle),
        total: filetime_ticks(kernel).saturating_add(filetime_ticks(user)),
    })
}

fn read_ram_usage() -> Option<f32> {
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: status has the required dwLength and remains valid for this synchronous call.
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    Some((status.dwMemoryLoad as f32 / 100.0).clamp(0.0, 1.0))
}

fn update_percent_text(text: &mut String, value: Option<f32>) {
    if let Some(value) = value {
        *text = format!("{:.0}%", value * 100.0);
    } else if text.is_empty() {
        text.push('—');
    }
}
