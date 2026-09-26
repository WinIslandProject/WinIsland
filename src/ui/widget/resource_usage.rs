use std::cell::RefCell;
use std::time::{Duration, Instant};

use winisland_platform::{MetricSelection, SystemSample};

use winisland_core::config::{
    ResourceMetricConfig, ResourceMetricKind, default_resource_metrics, normalize_resource_metrics,
};
use winisland_render::Rgba;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const TRANSITION_DURATION: Duration = Duration::from_millis(400);
const METRIC_COUNT: usize = ResourceMetricKind::ALL.len();

#[derive(Default)]
struct AnimatedUsage {
    from: f32,
    target: Option<f32>,
    started: Option<Instant>,
}

impl AnimatedUsage {
    fn value(&self, now: Instant) -> Option<f32> {
        self.target.map(|target| {
            let t = self.started.map_or(1.0, |started| {
                (now.saturating_duration_since(started).as_secs_f32()
                    / TRANSITION_DURATION.as_secs_f32())
                .min(1.0)
            });
            let eased = t * t * (3.0 - 2.0 * t);
            self.from + (target - self.from) * eased
        })
    }

    fn set_target(&mut self, target: Option<f32>, now: Instant) {
        if self.target == target {
            return;
        }
        let current = self.value(now);
        self.from = current.or(target).unwrap_or_default();
        self.target = target;
        self.started = current.map(|_| now);
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.started
            .is_some_and(|started| now.saturating_duration_since(started) < TRANSITION_DURATION)
            && self
                .target
                .is_some_and(|target| (target - self.from).abs() > f32::EPSILON)
    }
}

#[derive(Clone, Copy, Default)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

#[derive(Clone, Copy)]
struct NetworkSample {
    bytes: u64,
    link_bits_per_second: u64,
}

struct ResourceUsageCache {
    sampled_at: Option<Instant>,
    previous_cpu: Option<CpuTimes>,
    previous_network: Option<NetworkSample>,
    values: [Option<f32>; METRIC_COUNT],
    animated: [AnimatedUsage; METRIC_COUNT],
    texts: [String; METRIC_COUNT],
}

impl Default for ResourceUsageCache {
    fn default() -> Self {
        Self {
            sampled_at: None,
            previous_cpu: None,
            previous_network: None,
            values: [None; METRIC_COUNT],
            animated: std::array::from_fn(|_| AnimatedUsage::default()),
            texts: std::array::from_fn(|_| String::new()),
        }
    }
}

impl ResourceUsageCache {
    fn refresh_if_due(&mut self, metrics: &[ResourceMetricConfig]) {
        if self
            .sampled_at
            .is_some_and(|sampled_at| sampled_at.elapsed() < SAMPLE_INTERVAL)
        {
            return;
        }
        let now = Instant::now();
        let elapsed = self
            .sampled_at
            .map(|sampled_at| now.saturating_duration_since(sampled_at).as_secs_f32())
            .unwrap_or_default();
        self.sampled_at = Some(now);

        let enabled = |kind: ResourceMetricKind| {
            metrics
                .iter()
                .any(|metric| metric.enabled && metric.kind == kind)
        };
        let selection = MetricSelection {
            cpu: enabled(ResourceMetricKind::Cpu),
            memory: enabled(ResourceMetricKind::Ram),
            network: enabled(ResourceMetricKind::Network),
            disk: enabled(ResourceMetricKind::Disk),
            gpu: enabled(ResourceMetricKind::Gpu),
        };
        let sample = match crate::platform::metrics().sample(selection) {
            Ok(sample) => sample,
            Err(error) => {
                log::warn!("Resource usage sample failed: {error}");
                SystemSample::default()
            }
        };
        if enabled(ResourceMetricKind::Cpu)
            && let (Some(idle), Some(total)) = (sample.cpu_idle_ticks, sample.cpu_total_ticks)
        {
            let current = CpuTimes { idle, total };
            if let Some(previous) = self.previous_cpu {
                let total = current.total.saturating_sub(previous.total);
                let idle = current.idle.saturating_sub(previous.idle);
                if total > 0 {
                    self.values[ResourceMetricKind::Cpu.index()] =
                        Some((1.0 - idle as f32 / total as f32).clamp(0.0, 1.0));
                }
            }
            self.previous_cpu = Some(current);
        }
        if enabled(ResourceMetricKind::Ram) {
            self.values[ResourceMetricKind::Ram.index()] = sample
                .memory_load_percent
                .map(|load| (load as f32 / 100.0).clamp(0.0, 1.0));
        }
        if enabled(ResourceMetricKind::Gpu) {
            self.values[ResourceMetricKind::Gpu.index()] = sample
                .gpu_memory_used_bytes
                .zip(sample.gpu_memory_budget_bytes)
                .filter(|(_, budget)| *budget > 0)
                .map(|(used, budget)| (used as f32 / budget as f32).clamp(0.0, 1.0));
        }
        if enabled(ResourceMetricKind::Disk) {
            self.values[ResourceMetricKind::Disk.index()] = sample
                .disk_free_bytes
                .zip(sample.disk_total_bytes)
                .filter(|(_, total)| *total > 0)
                .map(|(free, total)| (1.0 - free as f32 / total as f32).clamp(0.0, 1.0));
        }

        if enabled(ResourceMetricKind::Network)
            && let (Some(bytes), Some(link_bits_per_second)) =
                (sample.network_bytes, sample.network_link_bits_per_second)
        {
            let current = NetworkSample {
                bytes,
                link_bits_per_second,
            };
            if let Some(previous) = self.previous_network
                && elapsed > 0.0
            {
                let bytes_per_second =
                    current.bytes.saturating_sub(previous.bytes) as f32 / elapsed;
                let link_bytes_per_second = current.link_bits_per_second as f32 / 8.0;
                self.values[ResourceMetricKind::Network.index()] = (link_bytes_per_second > 0.0)
                    .then_some((bytes_per_second / link_bytes_per_second).clamp(0.0, 1.0));
                self.texts[ResourceMetricKind::Network.index()] =
                    format_network_rate(bytes_per_second);
            }
            self.previous_network = Some(current);
        }

        for kind in ResourceMetricKind::ALL {
            if !enabled(kind) {
                continue;
            }
            let index = kind.index();
            if kind != ResourceMetricKind::Network {
                update_percent_text(&mut self.texts[index], self.values[index]);
            } else if self.texts[index].is_empty() {
                self.texts[index].push('—');
            }
            self.animated[index].set_target(self.values[index], now);
        }
    }

    fn next_refresh_delay(&self) -> Duration {
        self.sampled_at
            .map(|sampled_at| SAMPLE_INTERVAL.saturating_sub(sampled_at.elapsed()))
            .unwrap_or_default()
    }
}

pub(crate) struct MetricUsage<'a> {
    pub(crate) value: Option<f32>,
    pub(crate) text: &'a str,
}

pub(crate) struct ResourceUsage<'a> {
    values: [Option<f32>; METRIC_COUNT],
    texts: &'a [String; METRIC_COUNT],
}

impl<'a> ResourceUsage<'a> {
    pub(crate) fn metric(&self, kind: ResourceMetricKind) -> MetricUsage<'a> {
        let index = kind.index();
        MetricUsage {
            value: self.values[index],
            text: &self.texts[index],
        }
    }
}

thread_local! {
    static RESOURCE_USAGE: RefCell<ResourceUsageCache> = RefCell::new(ResourceUsageCache::default());
    static EXPANDED_RESOURCE_CONFIG: RefCell<Vec<ResourceMetricConfig>> = RefCell::new(default_resource_metrics());
    static COMPACT_RESOURCE_CONFIG: RefCell<Vec<ResourceMetricConfig>> = RefCell::new(default_resource_metrics());
}

fn replace_config(cell: &RefCell<Vec<ResourceMetricConfig>>, metrics: &[ResourceMetricConfig]) {
    let mut normalized = metrics.to_vec();
    normalize_resource_metrics(&mut normalized);
    *cell.borrow_mut() = normalized;
}

pub(crate) fn set_configs(expanded: &[ResourceMetricConfig], compact: &[ResourceMetricConfig]) {
    EXPANDED_RESOURCE_CONFIG.with(|cell| replace_config(cell, expanded));
    COMPACT_RESOURCE_CONFIG.with(|cell| replace_config(cell, compact));
}

pub(crate) fn with_expanded_config<R>(read: impl FnOnce(&[ResourceMetricConfig]) -> R) -> R {
    EXPANDED_RESOURCE_CONFIG.with(|cell| read(&cell.borrow()))
}

pub(crate) fn with_compact_config<R>(read: impl FnOnce(&[ResourceMetricConfig]) -> R) -> R {
    COMPACT_RESOURCE_CONFIG.with(|cell| read(&cell.borrow()))
}

pub(crate) fn compact_width() -> f32 {
    with_compact_config(|config| {
        let enabled = config.iter().filter(|metric| metric.enabled).count().max(1);
        (enabled as f32 * 66.0).max(132.0)
    })
}

pub(crate) fn with_resource_usage<R>(
    metrics: &[ResourceMetricConfig],
    draw: impl FnOnce(ResourceUsage<'_>) -> R,
) -> R {
    RESOURCE_USAGE.with(|cell| {
        let mut cache = cell.borrow_mut();
        cache.refresh_if_due(metrics);
        let now = Instant::now();
        let values = std::array::from_fn(|index| cache.animated[index].value(now));
        draw(ResourceUsage {
            values,
            texts: &cache.texts,
        })
    })
}

pub(crate) fn next_refresh_delay() -> Duration {
    RESOURCE_USAGE.with(|cell| cell.borrow().next_refresh_delay())
}

pub(crate) fn is_animating(metrics: &[ResourceMetricConfig]) -> bool {
    RESOURCE_USAGE.with(|cell| {
        let cache = cell.borrow();
        let now = Instant::now();
        metrics
            .iter()
            .any(|metric| metric.enabled && cache.animated[metric.kind.index()].is_animating(now))
    })
}

pub(crate) fn metric_color(value: u32) -> Rgba {
    Rgba::from_rgb(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

pub(crate) fn alpha_color(color: Rgba, alpha: u8) -> Rgba {
    color.with_alpha(alpha)
}

pub(crate) fn usage_color(base: Rgba, usage: f32) -> Rgba {
    const WARNING_COLOR: Rgba = Rgba::from_rgb(255, 159, 10);
    const CRITICAL_COLOR: Rgba = Rgba::from_rgb(255, 69, 58);
    if usage <= 0.75 {
        base
    } else if usage <= 0.9 {
        blend_color(base, WARNING_COLOR, (usage - 0.75) / 0.15)
    } else {
        blend_color(WARNING_COLOR, CRITICAL_COLOR, (usage - 0.9) / 0.1)
    }
}

fn blend_color(from: Rgba, to: Rgba, amount: f32) -> Rgba {
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount) as u8;
    Rgba::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

fn update_percent_text(text: &mut String, value: Option<f32>) {
    if let Some(value) = value {
        *text = format!("{:.0}%", value * 100.0);
    } else if text.is_empty() {
        text.push('—');
    }
}

fn format_network_rate(bytes_per_second: f32) -> String {
    if bytes_per_second >= 1024.0 * 1024.0 {
        format!("{:.1}M/s", bytes_per_second / (1024.0 * 1024.0))
    } else if bytes_per_second >= 1024.0 {
        format!("{:.0}K/s", bytes_per_second / 1024.0)
    } else {
        format!("{:.0}B/s", bytes_per_second)
    }
}
