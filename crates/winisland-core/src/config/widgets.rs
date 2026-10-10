use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpandedPageKind {
    Music,
    Widgets,
    Calendar,
    Timer,
}

impl ExpandedPageKind {
    pub const ALL: [Self; 4] = [Self::Music, Self::Widgets, Self::Calendar, Self::Timer];
}

pub fn default_expanded_page_order() -> Vec<ExpandedPageKind> {
    ExpandedPageKind::ALL.to_vec()
}

pub fn normalize_expanded_pages(
    order: &mut Vec<ExpandedPageKind>,
    hidden: &mut Vec<ExpandedPageKind>,
) -> bool {
    let mut normalized_order = Vec::with_capacity(ExpandedPageKind::ALL.len());
    let mut normalized_hidden = Vec::with_capacity(ExpandedPageKind::ALL.len());
    for page in order.iter() {
        if !normalized_order.contains(page) {
            normalized_order.push(*page);
        }
    }
    for page in ExpandedPageKind::ALL {
        if !normalized_order.contains(&page) {
            normalized_order.push(page);
            if page != ExpandedPageKind::Widgets {
                normalized_hidden.push(page);
            }
        }
    }
    for page in hidden.iter() {
        if !normalized_hidden.contains(page) {
            normalized_hidden.push(*page);
        }
    }
    if normalized_order
        .iter()
        .all(|page| normalized_hidden.contains(page))
    {
        normalized_hidden.retain(|page| *page != ExpandedPageKind::Widgets);
    }
    let changed = normalized_order != *order || normalized_hidden != *hidden;
    *order = normalized_order;
    *hidden = normalized_hidden;
    changed
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    Clock,
    Calendar,
    ResourceUsage,
    Settings,
}

impl WidgetKind {
    pub fn span(self) -> (usize, usize) {
        match self {
            Self::Clock => (2, 1),
            Self::ResourceUsage => resource_widget_span(),
            Self::Calendar => (2, 2),
            Self::Settings => (1, 1),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WidgetSlot {
    pub slot: usize,
    #[serde(default, deserialize_with = "deserialize_widget_kind")]
    pub widget: Option<WidgetKind>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompactWidgetKind {
    Time,
    ResourceUsage,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ResourceMetricKind {
    Cpu,
    Ram,
    Gpu,
    Network,
    Disk,
}

impl ResourceMetricKind {
    pub const ALL: [Self; 5] = [Self::Cpu, Self::Ram, Self::Gpu, Self::Network, Self::Disk];

    pub const fn index(self) -> usize {
        match self {
            Self::Cpu => 0,
            Self::Ram => 1,
            Self::Gpu => 2,
            Self::Network => 3,
            Self::Disk => 4,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Ram => "RAM",
            Self::Gpu => "GPU",
            Self::Network => "NET",
            Self::Disk => "DISK",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceMetricStyle {
    #[default]
    Bar,
    Ring,
}

static RESOURCE_WIDGET_SPAN: AtomicU8 = AtomicU8::new((2 << 4) | 1);

pub fn set_resource_widget_span(columns: usize, rows: usize) -> (usize, usize) {
    let columns = columns.clamp(1, 3);
    let rows = rows.clamp(1, 3).min(6 / columns);
    RESOURCE_WIDGET_SPAN.store(((columns as u8) << 4) | rows as u8, Ordering::Relaxed);
    (columns, rows)
}

pub fn resource_widget_span() -> (usize, usize) {
    let encoded = RESOURCE_WIDGET_SPAN.load(Ordering::Relaxed);
    ((encoded >> 4) as usize, (encoded & 0x0f) as usize)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ResourceMetricConfig {
    pub kind: ResourceMetricKind,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gpu: u8,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub style: ResourceMetricStyle,
    pub color: u32,
}

fn default_true() -> bool {
    true
}

fn is_zero(value: &u8) -> bool {
    *value == 0
}

const GPU_COLORS: [u32; 4] = [0x30d158, 0x64d2ff, 0xbf5af2, 0xffd60a];

impl ResourceMetricConfig {
    pub fn is_same_metric(&self, other: &Self) -> bool {
        self.kind == other.kind && self.gpu == other.gpu
    }
}

pub fn default_resource_metrics() -> Vec<ResourceMetricConfig> {
    [
        (ResourceMetricKind::Cpu, 0x32bef6, true),
        (ResourceMetricKind::Ram, 0xaf52de, true),
        (ResourceMetricKind::Gpu, GPU_COLORS[0], false),
        (ResourceMetricKind::Network, 0x0a84ff, false),
        (ResourceMetricKind::Disk, 0xff9f0a, false),
    ]
    .into_iter()
    .map(|(kind, color, enabled)| ResourceMetricConfig {
        kind,
        gpu: 0,
        enabled,
        style: ResourceMetricStyle::Bar,
        color,
    })
    .collect()
}

pub fn normalize_resource_metrics(metrics: &mut Vec<ResourceMetricConfig>) -> bool {
    let original = metrics.clone();
    let defaults = default_resource_metrics();
    let mut normalized = Vec::with_capacity(ResourceMetricKind::ALL.len());
    for metric in metrics.drain(..) {
        let metric = ResourceMetricConfig {
            gpu: if metric.kind == ResourceMetricKind::Gpu {
                metric.gpu
            } else {
                0
            },
            color: metric.color & 0x00ff_ffff,
            ..metric
        };
        if !normalized
            .iter()
            .any(|entry: &ResourceMetricConfig| entry.is_same_metric(&metric))
        {
            normalized.push(metric);
        }
    }
    for default in defaults {
        if !normalized
            .iter()
            .any(|entry| entry.is_same_metric(&default))
        {
            normalized.push(default);
        }
    }
    *metrics = normalized;
    *metrics != original
}

pub fn add_detected_gpu_metrics(metrics: &mut Vec<ResourceMetricConfig>, gpu_count: usize) -> bool {
    let mut changed = false;
    for gpu in 1..gpu_count.min(usize::from(u8::MAX) + 1) {
        let gpu = gpu as u8;
        if metrics
            .iter()
            .any(|metric| metric.kind == ResourceMetricKind::Gpu && metric.gpu == gpu)
        {
            continue;
        }
        let primary = metrics
            .iter()
            .find(|metric| metric.kind == ResourceMetricKind::Gpu);
        let style = primary.map_or(ResourceMetricStyle::Bar, |metric| metric.style);
        let position = metrics
            .iter()
            .rposition(|metric| metric.kind == ResourceMetricKind::Gpu)
            .map_or(metrics.len(), |index| index + 1);
        let color = GPU_COLORS
            .iter()
            .copied()
            .find(|color| !metrics.iter().any(|metric| metric.color == *color))
            .unwrap_or(GPU_COLORS[usize::from(gpu) % GPU_COLORS.len()]);
        metrics.insert(
            position,
            ResourceMetricConfig {
                kind: ResourceMetricKind::Gpu,
                gpu,
                enabled: false,
                style,
                color,
            },
        );
        changed = true;
    }
    changed
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompactWidgetAlignment {
    Left,
    #[default]
    Center,
    Right,
}

impl CompactWidgetAlignment {
    pub const fn order(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Center => 1,
            Self::Right => 2,
        }
    }

    pub const fn legacy_slot(slot: usize) -> Self {
        match slot {
            0 => Self::Left,
            2 => Self::Right,
            _ => Self::Center,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactWidgetPosition {
    pub alignment: CompactWidgetAlignment,
    pub index: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CompactWidgetSlot {
    pub slot: usize,
    #[serde(default, deserialize_with = "deserialize_compact_widget_kind")]
    pub widget: Option<CompactWidgetKind>,
    #[serde(default)]
    pub alignment: CompactWidgetAlignment,
}

impl CompactWidgetSlot {
    pub const fn position(&self) -> CompactWidgetPosition {
        CompactWidgetPosition {
            alignment: self.alignment,
            index: self.slot,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PluginWidgetId {
    pub plugin_id: String,
    pub widget_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PluginWidgetSlot {
    pub plugin_id: String,
    pub widget_key: String,
    pub slot: usize,
}

impl PluginWidgetSlot {
    pub fn id(&self) -> PluginWidgetId {
        PluginWidgetId {
            plugin_id: self.plugin_id.clone(),
            widget_key: self.widget_key.clone(),
        }
    }
}

fn deserialize_widget_kind<'de, D>(deserializer: D) -> Result<Option<WidgetKind>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.and_then(|s| match s.as_str() {
        "clock" => Some(WidgetKind::Clock),
        "calendar" => Some(WidgetKind::Calendar),
        "resource_usage" => Some(WidgetKind::ResourceUsage),
        "settings" => Some(WidgetKind::Settings),
        _ => None,
    }))
}

fn deserialize_compact_widget_kind<'de, D>(
    deserializer: D,
) -> Result<Option<CompactWidgetKind>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.and_then(|value| match value.as_str() {
        "time" => Some(CompactWidgetKind::Time),
        "resource_usage" => Some(CompactWidgetKind::ResourceUsage),
        _ => None,
    }))
}

pub const WIDGET_GRID_COLS: usize = 6;
pub const WIDGET_GRID_ROWS: usize = 3;
pub const WIDGET_GRID_SLOTS: usize = WIDGET_GRID_COLS * WIDGET_GRID_ROWS;
pub const AVAILABLE_WIDGETS: [WidgetKind; 3] = [
    WidgetKind::Clock,
    WidgetKind::Calendar,
    WidgetKind::ResourceUsage,
];
pub const AVAILABLE_COMPACT_WIDGETS: [CompactWidgetKind; 2] =
    [CompactWidgetKind::Time, CompactWidgetKind::ResourceUsage];
