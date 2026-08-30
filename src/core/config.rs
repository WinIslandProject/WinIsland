use serde::{Deserialize, Serialize};
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_AUTHOR: &str = "Eatgrapes";
pub const APP_HOMEPAGE: &str = "https://github.com/WinIslandProject/WinIsland";
pub const WINDOW_TITLE: &str = "WinIsland";
pub const TOP_OFFSET: i32 = 10;
pub const PADDING: f32 = 80.0;
pub const MIN_HIDDEN_WIDTH: f32 = 0.0;
pub const MAX_HIDDEN_WIDTH: f32 = 400.0;
pub const MAX_LYRIC_WIDTH: f32 = 700.0;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(from = "String", into = "String")]
#[derive(Default)]
pub enum DockPosition {
    #[default]
    TopCenter,
    TopLeft,
    TopRight,
    BottomCenter,
    BottomLeft,
    BottomRight,
}

impl DockPosition {
    pub fn is_bottom(&self) -> bool {
        matches!(
            self,
            Self::BottomCenter | Self::BottomLeft | Self::BottomRight
        )
    }

    pub fn is_left(&self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }

    pub fn is_right(&self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::TopCenter => "top_center",
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomCenter => "bottom_center",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        }
    }
}

impl std::fmt::Display for DockPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for DockPosition {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "top_center" => Ok(Self::TopCenter),
            "top_left" => Ok(Self::TopLeft),
            "top_right" => Ok(Self::TopRight),
            "bottom_center" => Ok(Self::BottomCenter),
            "bottom_left" => Ok(Self::BottomLeft),
            "bottom_right" => Ok(Self::BottomRight),
            _ => Err(()),
        }
    }
}

impl From<String> for DockPosition {
    fn from(value: String) -> Self {
        value.parse().unwrap_or_default()
    }
}

impl From<DockPosition> for String {
    fn from(value: DockPosition) -> Self {
        value.as_str().to_string()
    }
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
    pub const fn span(&self) -> (usize, usize) {
        match self {
            WidgetKind::Clock => (2, 1),
            WidgetKind::Calendar => (2, 2),
            WidgetKind::ResourceUsage => (2, 1),
            WidgetKind::Settings => (1, 1),
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompactWidgetAlignment {
    Left,
    #[default]
    Center,
    Right,
}

impl CompactWidgetAlignment {
    pub(crate) const fn order(self) -> usize {
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub global_scale: f32,
    pub base_width: f32,
    pub base_height: f32,
    pub expanded_width: f32,
    pub expanded_height: f32,
    pub motion_blur: bool,
    #[serde(default = "default_island_style")]
    pub island_style: String,
    pub smtc_enabled: bool,
    pub smtc_apps: Vec<String>,
    #[serde(default)]
    pub smtc_known_apps: Vec<String>,
    #[serde(default = "default_show_lyrics")]
    pub show_lyrics: bool,
    #[serde(default = "default_lyrics_mode")]
    pub lyrics_mode: String,
    #[serde(default)]
    pub lyrics_local_dir: Option<String>,
    #[serde(default)]
    pub custom_font_path: Option<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub auto_hide: bool,
    #[serde(default = "default_auto_hide_delay")]
    pub auto_hide_delay: f32,
    #[serde(default = "default_hidden_width")]
    pub hidden_width: f32,
    #[serde(default = "default_check_for_updates")]
    pub check_for_updates: bool,
    #[serde(default = "default_update_check_interval")]
    pub update_check_interval: f32,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_lyrics_source")]
    pub lyrics_source: String,
    #[serde(default)]
    pub lyrics_delay: f64,
    #[serde(default)]
    pub lyrics_scroll: bool,
    #[serde(default = "default_lyrics_scroll_max_width")]
    pub lyrics_scroll_max_width: f32,
    #[serde(default)]
    pub position_x_offset: i32,
    #[serde(default)]
    pub position_y_offset: i32,
    #[serde(
        rename = "dock_position",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub legacy_dock_position: Option<DockPosition>,
    #[serde(default)]
    pub monitor_index: i32,
    #[serde(default)]
    pub font_size: f32,
    #[serde(default = "default_settings_theme")]
    pub settings_theme: String,
    #[serde(default = "default_mini_cover_shape")]
    pub mini_cover_shape: String,
    #[serde(default = "default_expanded_cover_shape")]
    pub expanded_cover_shape: String,
    #[serde(default = "default_cover_rotate")]
    pub cover_rotate: bool,
    #[serde(default = "default_update_channel")]
    pub update_channel: String,
    #[serde(default)]
    pub right_click_drag: bool,
    #[serde(default)]
    pub notification_display: bool,
    #[serde(default = "default_replace_native_volume_flyout")]
    pub replace_native_volume_flyout: bool,
    #[serde(default = "default_widget_layout")]
    pub widget_layout: Vec<WidgetSlot>,
    #[serde(default)]
    pub plugin_widget_layout: Vec<PluginWidgetSlot>,
    #[serde(default)]
    pub compact_widget_layout: Vec<CompactWidgetSlot>,
}

macro_rules! defaults {
    ($($name:ident: $ty:ty = $value:expr),* $(,)?) => {
        $(fn $name() -> $ty { $value })*
    };
}

defaults! {
    default_replace_native_volume_flyout: bool = true,
    default_island_style: String = "default".to_string(),
    default_show_lyrics: bool = true,
    default_lyrics_mode: String = "online".to_string(),
    default_auto_hide_delay: f32 = 5.0,
    default_hidden_width: f32 = 5.0,
    default_check_for_updates: bool = true,
    default_update_check_interval: f32 = 4.0,
    default_language: String = "auto".to_string(),
    default_lyrics_source: String = "163".to_string(),
    default_lyrics_scroll_max_width: f32 = 300.0,
    default_settings_theme: String = "system".to_string(),
    default_mini_cover_shape: String = "square".to_string(),
    default_expanded_cover_shape: String = "square".to_string(),
    default_cover_rotate: bool = false,
    default_update_channel: String = "stable".to_string(),
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

pub fn normalize_compact_widget_layout(layout: &mut Vec<CompactWidgetSlot>) -> bool {
    let original = layout.clone();
    layout.retain(|entry| entry.widget.is_some());
    layout.sort_by_key(|entry| (entry.alignment.order(), entry.slot));
    let mut seen = Vec::new();
    layout.retain(|entry| match entry.widget {
        Some(widget) if !seen.contains(&widget) => {
            seen.push(widget);
            true
        }
        _ => false,
    });
    let mut next_slots = [0; 3];
    for entry in layout.iter_mut() {
        let alignment = entry.alignment.order();
        entry.slot = next_slots[alignment];
        next_slots[alignment] += 1;
    }
    *layout != original
}

pub fn place_compact_widget(
    layout: &mut Vec<CompactWidgetSlot>,
    widget: CompactWidgetKind,
    target: CompactWidgetPosition,
) {
    normalize_compact_widget_layout(layout);
    layout.retain(|entry| entry.widget != Some(widget));
    let target_index = target.index.min(
        layout
            .iter()
            .filter(|entry| entry.alignment == target.alignment)
            .count(),
    );
    let insertion = layout
        .iter()
        .position(|entry| {
            entry.alignment.order() > target.alignment.order()
                || (entry.alignment == target.alignment && entry.slot >= target_index)
        })
        .unwrap_or(layout.len());
    layout.insert(
        insertion,
        CompactWidgetSlot {
            slot: target_index,
            widget: Some(widget),
            alignment: target.alignment,
        },
    );
    normalize_compact_widget_layout(layout);
}

pub fn clear_compact_widget_slot(
    layout: &mut Vec<CompactWidgetSlot>,
    target: CompactWidgetPosition,
) {
    layout.retain(|entry| entry.position() != target);
    normalize_compact_widget_layout(layout);
}

pub fn widget_footprint(widget: WidgetKind, anchor_slot: usize) -> Vec<usize> {
    span_cells(anchor_slot, widget.span())
}

pub fn widget_anchor_slot(widget: WidgetKind, target_slot: usize) -> usize {
    *widget_footprint(widget, target_slot)
        .first()
        .unwrap_or(&target_slot)
}

pub fn widget_covering_slot(
    layout: &[WidgetSlot],
    target_slot: usize,
) -> Option<(usize, WidgetKind)> {
    layout.iter().find_map(|entry| {
        let widget = entry.widget?;
        widget_footprint(widget, entry.slot)
            .contains(&target_slot)
            .then_some((entry.slot, widget))
    })
}

pub fn span_cells(anchor: usize, span: (usize, usize)) -> Vec<usize> {
    let (cols, rows) = span;
    if cols == 0 || rows == 0 || cols > WIDGET_GRID_COLS || rows > WIDGET_GRID_ROWS {
        return Vec::new();
    }
    footprint_cells(anchor, cols, rows)
}

fn footprint_cells(anchor: usize, cols: usize, rows: usize) -> Vec<usize> {
    let anchor_col = (anchor % WIDGET_GRID_COLS).min(WIDGET_GRID_COLS - cols);
    let anchor_row = (anchor / WIDGET_GRID_COLS).min(WIDGET_GRID_ROWS - rows);
    let mut cells = Vec::with_capacity(cols * rows);
    for dr in 0..rows {
        for dc in 0..cols {
            cells.push((anchor_row + dr) * WIDGET_GRID_COLS + (anchor_col + dc));
        }
    }
    cells
}

pub fn first_free_anchor(occupied: &[bool], span: (usize, usize)) -> Option<usize> {
    let (cols, rows) = span;
    if cols == 0 || rows == 0 || cols > WIDGET_GRID_COLS || rows > WIDGET_GRID_ROWS {
        return None;
    }
    (0..WIDGET_GRID_SLOTS).find_map(|anchor| {
        let anchor_col = (anchor % WIDGET_GRID_COLS).min(WIDGET_GRID_COLS - cols);
        let anchor_row = (anchor / WIDGET_GRID_COLS).min(WIDGET_GRID_ROWS - rows);
        let anchor = anchor_row * WIDGET_GRID_COLS + anchor_col;
        let free = (0..rows).all(|dr| {
            (0..cols).all(|dc| !occupied[(anchor_row + dr) * WIDGET_GRID_COLS + (anchor_col + dc)])
        });
        free.then_some(anchor)
    })
}

pub fn default_widget_layout() -> Vec<WidgetSlot> {
    (0..WIDGET_GRID_SLOTS)
        .map(|slot| WidgetSlot {
            slot,
            widget: (slot + 1 == WIDGET_GRID_SLOTS).then_some(WidgetKind::Settings),
        })
        .collect()
}

fn ensure_widget_slots(layout: &mut Vec<WidgetSlot>) {
    for slot in 0..WIDGET_GRID_SLOTS {
        if !layout.iter().any(|entry| entry.slot == slot) {
            layout.push(WidgetSlot { slot, widget: None });
        }
    }
    layout.sort_by_key(|entry| entry.slot);
}

pub fn ensure_settings_widget(layout: &mut Vec<WidgetSlot>) -> bool {
    ensure_widget_slots(layout);
    let settings_slots: Vec<usize> = layout
        .iter()
        .filter(|entry| entry.widget == Some(WidgetKind::Settings))
        .map(|entry| entry.slot)
        .collect();
    if let Some(keep) = settings_slots
        .iter()
        .copied()
        .find(|slot| *slot < WIDGET_GRID_SLOTS)
    {
        let changed = settings_slots.len() != 1;
        for entry in layout.iter_mut() {
            if entry.widget == Some(WidgetKind::Settings) && entry.slot != keep {
                entry.widget = None;
            }
        }
        return changed;
    }
    for entry in layout.iter_mut() {
        if entry.widget == Some(WidgetKind::Settings) {
            entry.widget = None;
        }
    }

    let slot = (0..WIDGET_GRID_SLOTS)
        .rev()
        .find(|slot| widget_covering_slot(layout, *slot).is_none())
        .unwrap_or(WIDGET_GRID_SLOTS - 1);
    if let Some(entry) = layout.iter_mut().find(|entry| entry.slot == slot) {
        entry.widget = Some(WidgetKind::Settings);
    }
    true
}

fn clear_cells(layout: &mut [WidgetSlot], cells: &[usize]) {
    let occupants: Vec<usize> = layout
        .iter()
        .filter_map(|entry| entry.widget.map(|w| (entry.slot, w)))
        .filter(|(anchor, w)| {
            widget_footprint(*w, *anchor)
                .iter()
                .any(|cell| cells.contains(cell))
        })
        .map(|(anchor, _)| anchor)
        .collect();
    for anchor in occupants {
        if let Some(entry) = layout.iter_mut().find(|entry| entry.slot == anchor) {
            entry.widget = None;
        }
    }
}

pub fn place_widget_in_layout(
    layout: &mut Vec<WidgetSlot>,
    widget: WidgetKind,
    target_slot: usize,
) {
    ensure_settings_widget(layout);
    let anchor = widget_anchor_slot(widget, target_slot);
    if widget != WidgetKind::Settings {
        let target_cells = widget_footprint(widget, anchor);
        let settings_slot = layout
            .iter()
            .find(|entry| entry.widget == Some(WidgetKind::Settings))
            .map(|entry| entry.slot);
        if settings_slot.is_some_and(|slot| target_cells.contains(&slot)) {
            return;
        }
    }
    for entry in layout.iter_mut() {
        if entry.widget == Some(widget) {
            entry.widget = None;
        }
    }
    clear_cells(layout, &widget_footprint(widget, anchor));
    if let Some(entry) = layout.iter_mut().find(|entry| entry.slot == anchor) {
        entry.widget = Some(widget);
    }
}

pub fn clear_widget_slot(layout: &mut [WidgetSlot], target_slot: usize) {
    if widget_covering_slot(layout, target_slot)
        .is_some_and(|(_, widget)| widget == WidgetKind::Settings)
    {
        return;
    }
    clear_cells(layout, &[target_slot]);
}

pub fn plugin_widget_slot<'a>(
    layout: &'a [PluginWidgetSlot],
    id: &PluginWidgetId,
) -> Option<&'a PluginWidgetSlot> {
    layout
        .iter()
        .find(|entry| entry.plugin_id == id.plugin_id && entry.widget_key == id.widget_key)
}

pub fn clear_plugin_widget(layout: &mut Vec<PluginWidgetSlot>, id: &PluginWidgetId) {
    layout.retain(|entry| entry.plugin_id != id.plugin_id || entry.widget_key != id.widget_key);
}

pub fn plugin_widget_covering_slot<'a>(
    layout: &'a [PluginWidgetSlot],
    widgets: &'a [crate::core::plugin_widget::PluginWidget],
    target_slot: usize,
) -> Option<(
    &'a PluginWidgetSlot,
    &'a crate::core::plugin_widget::PluginWidget,
)> {
    layout.iter().find_map(|entry| {
        let widget = widgets.iter().find(|widget| {
            widget.plugin_id == entry.plugin_id
                && widget.key.as_deref() == Some(entry.widget_key.as_str())
        })?;
        span_cells(entry.slot, widget.span())
            .contains(&target_slot)
            .then_some((entry, widget))
    })
}

pub fn clear_plugin_widgets_in_cells(
    layout: &mut Vec<PluginWidgetSlot>,
    widgets: &[crate::core::plugin_widget::PluginWidget],
    cells: &[usize],
) {
    layout.retain(|entry| {
        let Some(widget) = widgets.iter().find(|widget| {
            widget.plugin_id == entry.plugin_id
                && widget.key.as_deref() == Some(entry.widget_key.as_str())
        }) else {
            return true;
        };
        !span_cells(entry.slot, widget.span())
            .iter()
            .any(|cell| cells.contains(cell))
    });
}

pub fn normalize_active_plugin_widget_layout(
    widget_layout: &[WidgetSlot],
    plugin_layout: &mut Vec<PluginWidgetSlot>,
    widgets: &[crate::core::plugin_widget::PluginWidget],
) -> bool {
    let original = plugin_layout.clone();
    let mut occupied = [false; WIDGET_GRID_SLOTS];
    for entry in widget_layout {
        if let Some(widget) = entry.widget {
            for cell in widget_footprint(widget, entry.slot) {
                occupied[cell] = true;
            }
        }
    }
    let mut normalized = Vec::with_capacity(plugin_layout.len());
    for entry in plugin_layout.drain(..) {
        let Some(widget) = widgets.iter().find(|widget| {
            widget.plugin_id == entry.plugin_id
                && widget.key.as_deref() == Some(entry.widget_key.as_str())
        }) else {
            normalized.push(entry);
            continue;
        };
        let duplicate = normalized.iter().any(|existing: &PluginWidgetSlot| {
            existing.plugin_id == entry.plugin_id && existing.widget_key == entry.widget_key
        });
        let cells = span_cells(entry.slot, widget.span());
        if duplicate || cells.is_empty() || cells.iter().any(|cell| occupied[*cell]) {
            continue;
        }
        for cell in cells {
            occupied[cell] = true;
        }
        normalized.push(entry);
    }
    let changed = normalized != original;
    *plugin_layout = normalized;
    changed
}

pub fn place_plugin_widget(
    widget_layout: &mut Vec<WidgetSlot>,
    plugin_layout: &mut Vec<PluginWidgetSlot>,
    widgets: &[crate::core::plugin_widget::PluginWidget],
    id: &PluginWidgetId,
    target_slot: usize,
) -> bool {
    let Some(widget) = widgets
        .iter()
        .find(|widget| widget.layout_id().as_ref() == Some(id))
    else {
        return false;
    };
    ensure_settings_widget(widget_layout);
    let cells = span_cells(target_slot, widget.span());
    if cells.is_empty()
        || widget_layout
            .iter()
            .any(|entry| entry.widget == Some(WidgetKind::Settings) && cells.contains(&entry.slot))
    {
        return false;
    }
    clear_plugin_widget(plugin_layout, id);
    clear_cells(widget_layout, &cells);
    clear_plugin_widgets_in_cells(plugin_layout, widgets, &cells);
    plugin_layout.push(PluginWidgetSlot {
        plugin_id: id.plugin_id.clone(),
        widget_key: id.widget_key.clone(),
        slot: cells[0],
    });
    true
}

pub fn place_builtin_widget(
    widget_layout: &mut Vec<WidgetSlot>,
    plugin_layout: &mut Vec<PluginWidgetSlot>,
    widgets: &[crate::core::plugin_widget::PluginWidget],
    widget: WidgetKind,
    target_slot: usize,
) {
    ensure_settings_widget(widget_layout);
    let cells = widget_footprint(widget, target_slot);
    let settings_slot = widget_layout
        .iter()
        .find(|entry| entry.widget == Some(WidgetKind::Settings))
        .map(|entry| entry.slot);
    if widget != WidgetKind::Settings && settings_slot.is_some_and(|slot| cells.contains(&slot)) {
        return;
    }
    place_widget_in_layout(widget_layout, widget, target_slot);
    clear_plugin_widgets_in_cells(plugin_layout, widgets, &cells);
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            global_scale: 1.0,
            base_width: 120.0,
            base_height: 27.0,
            expanded_width: 360.0,
            expanded_height: 200.0,
            motion_blur: true,
            island_style: default_island_style(),
            smtc_enabled: true,
            smtc_apps: Vec::new(),
            smtc_known_apps: Vec::new(),
            show_lyrics: default_show_lyrics(),
            lyrics_mode: default_lyrics_mode(),
            lyrics_local_dir: None,
            custom_font_path: None,
            auto_start: false,
            auto_hide: false,
            auto_hide_delay: default_auto_hide_delay(),
            hidden_width: default_hidden_width(),
            check_for_updates: default_check_for_updates(),
            update_check_interval: default_update_check_interval(),
            language: default_language(),
            lyrics_source: default_lyrics_source(),
            lyrics_delay: 0.0,
            lyrics_scroll: false,
            lyrics_scroll_max_width: default_lyrics_scroll_max_width(),
            position_x_offset: 0,
            position_y_offset: 0,
            legacy_dock_position: None,
            monitor_index: 0,
            font_size: 0.0,
            settings_theme: default_settings_theme(),
            mini_cover_shape: default_mini_cover_shape(),
            expanded_cover_shape: default_expanded_cover_shape(),
            cover_rotate: default_cover_rotate(),
            update_channel: default_update_channel(),
            right_click_drag: false,
            notification_display: false,
            replace_native_volume_flyout: default_replace_native_volume_flyout(),
            widget_layout: default_widget_layout(),
            plugin_widget_layout: Vec::new(),
            compact_widget_layout: Vec::new(),
        }
    }
}
