use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use winisland_platform::{MetricSelection, SystemSample};

use winisland_core::config::{
    ResourceMetricConfig, ResourceMetricKind, ResourceMetricStyle, add_detected_gpu_metrics,
    default_resource_metrics, normalize_resource_metrics,
};
use winisland_render::Rgba;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const TRANSITION_DURATION: Duration = Duration::from_millis(400);
const GPU_LABELS: [&str; 8] = [
    "GPU 0", "GPU 1", "GPU 2", "GPU 3", "GPU 4", "GPU 5", "GPU 6", "GPU 7",
];
const EMPTY_TEXT: &str = "—";

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

#[derive(Default)]
struct PercentMetric {
    value: Option<f32>,
    animated: AnimatedUsage,
    text: String,
}

impl PercentMetric {
    fn set(&mut self, value: Option<f32>, now: Instant) {
        self.value = value;
        if let Some(value) = value {
            self.text = format!("{:.0}%", value * 100.0);
        } else if self.text.is_empty() {
            self.text.push_str(EMPTY_TEXT);
        }
        self.animated.set_target(value, now);
    }

    fn usage(&self, now: Instant) -> MetricUsage<'_> {
        MetricUsage::Percent {
            value: self.animated.value(now),
            text: if self.text.is_empty() {
                EMPTY_TEXT
            } else {
                &self.text
            },
        }
    }
}

struct RateText {
    value: String,
    unit: &'static str,
}

impl Default for RateText {
    fn default() -> Self {
        Self {
            value: EMPTY_TEXT.to_string(),
            unit: "",
        }
    }
}

impl RateText {
    fn set(&mut self, bytes_per_second: f64) {
        let (value, unit) = format_rate(bytes_per_second);
        self.value = value;
        self.unit = unit;
    }

    fn usage(&self) -> RateUsage<'_> {
        RateUsage {
            value: &self.value,
            unit: self.unit,
        }
    }
}

#[derive(Default)]
struct NetworkMetric {
    previous: Option<(u64, u64)>,
    upload: RateText,
    download: RateText,
}

#[derive(Clone, Copy, Default)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

#[derive(Default)]
struct ResourceUsageCache {
    sampled_at: Option<Instant>,
    previous_cpu: Option<CpuTimes>,
    cpu: PercentMetric,
    ram: PercentMetric,
    disk: PercentMetric,
    gpus: Vec<PercentMetric>,
    network: NetworkMetric,
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
            .map(|sampled_at| now.saturating_duration_since(sampled_at).as_secs_f64())
            .unwrap_or_default();
        self.sampled_at = Some(now);

        let gpu_count = gpu_count();
        let enabled = |kind: ResourceMetricKind| {
            metrics.iter().any(|metric| {
                metric.enabled && metric.kind == kind && metric_visible(metric, gpu_count)
            })
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
        if selection.cpu {
            let mut value = self.cpu.value;
            if let (Some(idle), Some(total)) = (sample.cpu_idle_ticks, sample.cpu_total_ticks) {
                let current = CpuTimes { idle, total };
                if let Some(previous) = self.previous_cpu {
                    let total = current.total.saturating_sub(previous.total);
                    let idle = current.idle.saturating_sub(previous.idle);
                    if total > 0 {
                        value = Some((1.0 - idle as f32 / total as f32).clamp(0.0, 1.0));
                    }
                }
                self.previous_cpu = Some(current);
            }
            self.cpu.set(value, now);
        }
        if selection.memory {
            self.ram.set(
                sample
                    .memory_load_percent
                    .map(|load| (load as f32 / 100.0).clamp(0.0, 1.0)),
                now,
            );
        }
        if selection.disk {
            self.disk.set(
                sample
                    .disk_free_bytes
                    .zip(sample.disk_total_bytes)
                    .filter(|(_, total)| *total > 0)
                    .map(|(free, total)| (1.0 - free as f32 / total as f32).clamp(0.0, 1.0)),
                now,
            );
        }
        if selection.gpu {
            set_gpu_count(sample.gpu_usage.len());
            self.gpus
                .resize_with(sample.gpu_usage.len(), PercentMetric::default);
            for (metric, usage) in self.gpus.iter_mut().zip(&sample.gpu_usage) {
                metric.set(*usage, now);
            }
        }
        if selection.network
            && let (Some(received), Some(sent)) =
                (sample.network_received_bytes, sample.network_sent_bytes)
        {
            if let Some((previous_received, previous_sent)) = self.network.previous
                && elapsed > 0.0
            {
                self.network
                    .download
                    .set(received.saturating_sub(previous_received) as f64 / elapsed);
                self.network
                    .upload
                    .set(sent.saturating_sub(previous_sent) as f64 / elapsed);
            }
            self.network.previous = Some((received, sent));
        }
    }

    fn percent(&self, metric: &ResourceMetricConfig) -> Option<&PercentMetric> {
        match metric.kind {
            ResourceMetricKind::Cpu => Some(&self.cpu),
            ResourceMetricKind::Ram => Some(&self.ram),
            ResourceMetricKind::Disk => Some(&self.disk),
            ResourceMetricKind::Gpu => self.gpus.get(usize::from(metric.gpu)),
            ResourceMetricKind::Network => None,
        }
    }

    fn next_refresh_delay(&self) -> Duration {
        self.sampled_at
            .map(|sampled_at| SAMPLE_INTERVAL.saturating_sub(sampled_at.elapsed()))
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RateUsage<'a> {
    pub(crate) value: &'a str,
    pub(crate) unit: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) enum MetricUsage<'a> {
    Percent {
        value: Option<f32>,
        text: &'a str,
    },
    Rates {
        upload: RateUsage<'a>,
        download: RateUsage<'a>,
    },
}

pub(crate) struct ResourceUsage<'a> {
    cache: &'a ResourceUsageCache,
    now: Instant,
}

impl<'a> ResourceUsage<'a> {
    pub(crate) fn metric(&self, metric: &ResourceMetricConfig) -> MetricUsage<'a> {
        if metric.kind == ResourceMetricKind::Network {
            return MetricUsage::Rates {
                upload: self.cache.network.upload.usage(),
                download: self.cache.network.download.usage(),
            };
        }
        self.cache.percent(metric).map_or(
            MetricUsage::Percent {
                value: None,
                text: EMPTY_TEXT,
            },
            |percent| percent.usage(self.now),
        )
    }
}

thread_local! {
    static RESOURCE_USAGE: RefCell<ResourceUsageCache> = RefCell::new(ResourceUsageCache::default());
    static EXPANDED_RESOURCE_CONFIG: RefCell<Vec<ResourceMetricConfig>> = RefCell::new(default_resource_metrics());
    static COMPACT_RESOURCE_CONFIG: RefCell<Vec<ResourceMetricConfig>> = RefCell::new(default_resource_metrics());
    static GPU_COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

pub(crate) fn gpu_names() -> Vec<String> {
    let names = crate::platform::metrics().gpu_adapters();
    set_gpu_count(names.len());
    names
}

pub(crate) fn gpu_count() -> usize {
    GPU_COUNT
        .with(Cell::get)
        .unwrap_or_else(|| gpu_names().len())
}

fn set_gpu_count(count: usize) {
    GPU_COUNT.with(|cell| cell.set(Some(count)));
}

pub(crate) fn metric_visible(metric: &ResourceMetricConfig, gpu_count: usize) -> bool {
    metric.kind != ResourceMetricKind::Gpu || usize::from(metric.gpu) < gpu_count.max(1)
}

pub(crate) fn metric_label(metric: &ResourceMetricConfig, gpu_count: usize) -> Cow<'static, str> {
    if metric.kind != ResourceMetricKind::Gpu || gpu_count < 2 {
        return Cow::Borrowed(metric.kind.label());
    }
    GPU_LABELS.get(usize::from(metric.gpu)).map_or_else(
        || Cow::Owned(format!("GPU {}", metric.gpu)),
        |label| Cow::Borrowed(*label),
    )
}

pub(crate) fn visible_metrics(
    metrics: &[ResourceMetricConfig],
) -> impl Iterator<Item = &ResourceMetricConfig> {
    let gpu_count = gpu_count();
    metrics
        .iter()
        .filter(move |metric| metric.enabled && metric_visible(metric, gpu_count))
}

fn replace_config(cell: &RefCell<Vec<ResourceMetricConfig>>, metrics: &[ResourceMetricConfig]) {
    let mut normalized = metrics.to_vec();
    normalize_resource_metrics(&mut normalized);
    add_detected_gpu_metrics(&mut normalized, gpu_count());
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

pub(crate) const COMPACT_METRIC_GAP: f32 = 4.0;
const COMPACT_NETWORK_WIDTH: f32 = 50.0;
const COMPACT_GPU_INDEX_WIDTH: f32 = 7.0;

pub(crate) fn compact_metric_width(metric: &ResourceMetricConfig, gpu_count: usize) -> f32 {
    if metric.kind == ResourceMetricKind::Network {
        return COMPACT_NETWORK_WIDTH;
    }
    let base = match metric.style {
        ResourceMetricStyle::Bar => 44.0,
        ResourceMetricStyle::Ring => 38.0,
    };
    if metric.kind == ResourceMetricKind::Gpu && gpu_count > 1 {
        base + COMPACT_GPU_INDEX_WIDTH
    } else {
        base
    }
}

pub(crate) fn compact_width() -> f32 {
    with_compact_config(|config| {
        let gpu_count = gpu_count();
        let (count, width) = visible_metrics(config).fold((0, 0.0), |(count, width), metric| {
            (count + 1, width + compact_metric_width(metric, gpu_count))
        });
        if count == 0 {
            44.0
        } else {
            width + COMPACT_METRIC_GAP * (count - 1) as f32
        }
    })
}

pub(crate) fn with_resource_usage<R>(
    metrics: &[ResourceMetricConfig],
    draw: impl FnOnce(ResourceUsage<'_>) -> R,
) -> R {
    RESOURCE_USAGE.with(|cell| {
        cell.borrow_mut().refresh_if_due(metrics);
        let cache = cell.borrow();
        draw(ResourceUsage {
            cache: &cache,
            now: Instant::now(),
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
        visible_metrics(metrics).any(|metric| {
            cache
                .percent(metric)
                .is_some_and(|percent| percent.animated.is_animating(now))
        })
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

fn format_rate(bytes_per_second: f64) -> (String, &'static str) {
    const UNITS: [&str; 4] = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut value = bytes_per_second.max(0.0);
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    let text = if unit > 0 && value < 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    };
    (text, UNITS[unit])
}

pub(crate) fn draw_rate_arrow(
    painter: winisland_render::Painter<'_>,
    center: winisland_render::Point,
    size: f32,
    upward: bool,
    color: Rgba,
) {
    use winisland_render::{Point, StrokeCap};
    let direction = if upward { -1.0 } else { 1.0 };
    let tip = Point::new(center.x, center.y + direction * size / 2.0);
    let tail = Point::new(center.x, center.y - direction * size / 2.0);
    let width = (size * 0.19).max(1.0);
    painter.stroke_line(tail, tip, width, color, StrokeCap::Round);
    for side in [-1.0, 1.0] {
        painter.stroke_line(
            tip,
            Point::new(
                center.x + side * size * 0.36,
                tip.y - direction * size * 0.36,
            ),
            width,
            color,
            StrokeCap::Round,
        );
    }
}

pub(crate) fn preview_usage(metric: &ResourceMetricConfig) -> MetricUsage<'static> {
    const GPU_PREVIEW: [(f32, &str); 4] =
        [(0.48, "48%"), (0.23, "23%"), (0.12, "12%"), (0.07, "7%")];
    let (value, text) = match metric.kind {
        ResourceMetricKind::Cpu => (0.37, "37%"),
        ResourceMetricKind::Ram => (0.62, "62%"),
        ResourceMetricKind::Disk => (0.71, "71%"),
        ResourceMetricKind::Gpu => GPU_PREVIEW[usize::from(metric.gpu) % GPU_PREVIEW.len()],
        ResourceMetricKind::Network => {
            return MetricUsage::Rates {
                upload: RateUsage {
                    value: "1.2",
                    unit: "MB/s",
                },
                download: RateUsage {
                    value: "18",
                    unit: "MB/s",
                },
            };
        }
    };
    MetricUsage::Percent {
        value: Some(value),
        text,
    }
}
