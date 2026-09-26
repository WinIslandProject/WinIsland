use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

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
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub style: ResourceMetricStyle,
    pub color: u32,
}

fn default_true() -> bool {
    true
}

pub fn default_resource_metrics() -> Vec<ResourceMetricConfig> {
    [
        (ResourceMetricKind::Cpu, 0x32bef6, true),
        (ResourceMetricKind::Ram, 0xaf52de, true),
        (ResourceMetricKind::Gpu, 0x30d158, false),
        (ResourceMetricKind::Network, 0x0a84ff, false),
        (ResourceMetricKind::Disk, 0xff9f0a, false),
    ]
    .into_iter()
    .map(|(kind, color, enabled)| ResourceMetricConfig {
        kind,
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
        if !normalized
            .iter()
            .any(|entry: &ResourceMetricConfig| entry.kind == metric.kind)
        {
            normalized.push(ResourceMetricConfig {
                color: metric.color & 0x00ff_ffff,
                ..metric
            });
        }
    }
    for default in defaults {
        if !normalized.iter().any(|entry| entry.kind == default.kind) {
            normalized.push(default);
        }
    }
    *metrics = normalized;
    *metrics != original
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
