use super::items::{
    CONTENT_PADDING, GROUP_INNER_PAD, POPUP_BTN_H, POPUP_BTN_W, ROW_HEIGHT, STEPPER_BTN_SIZE,
    STEPPER_GAP, STEPPER_VALUE_W, SettingsItem, TOGGLE_H, TOGGLE_W,
};
use crate::core::config::{
    AVAILABLE_COMPACT_WIDGETS, AVAILABLE_WIDGETS, CompactWidgetAlignment, CompactWidgetKind,
    CompactWidgetPosition, CompactWidgetSlot, PluginWidgetId, PluginWidgetSlot, WidgetKind,
    WidgetSlot,
};
use crate::core::plugin_widget::PluginWidget;
use crate::ui::widget::compact::widget_width;
use crate::ui::widget::expanded::{WidgetGridLayout, widget_corner_radius, widget_grid_layout};

pub const WIDGET_PREVIEW_BASE_H: f32 = 480.0;
pub const WIDGET_ISLAND_PANEL_H: f32 = 308.0;
pub const WIDGET_PANEL_GAP: f32 = 12.0;
pub const WIDGET_EDITOR_HEADER_H: f32 = 56.0;
pub const WIDGET_LIBRARY_HEADER_H: f32 = 52.0;
pub const WIDGET_LIBRARY_TILE_W: f32 = 112.0;
pub const WIDGET_LIBRARY_TILE_H: f32 = 72.0;
pub const WIDGET_LIBRARY_TILE_GAP: f32 = 10.0;
pub const COMPACT_WIDGET_PREVIEW_H: f32 = 388.0;
pub const COMPACT_WIDGET_ISLAND_PANEL_H: f32 = 226.0;
const COMPACT_WIDGET_PREVIEW_PREFERRED_W: f32 = 260.0;
const COMPACT_WIDGET_PREVIEW_WIDTH_GROWTH: f32 = 0.75;
const COMPACT_WIDGET_PREVIEW_MAX_SCALE: f32 = 1.8;

#[derive(Debug, Clone, PartialEq)]
pub enum ClickResult {
    None,
    Switch(usize),
    StepperDec(usize),
    StepperInc(usize),
    StepperValue(usize),
    FontSelect(usize),
    FontReset(usize),
    CenterLink(usize),
    SourceButton(usize),
    RowButton(usize),
    AppItem(usize),
    FolderSelect(usize),
    FolderClear(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepDirection {
    Decrement,
    Increment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetEditorMode {
    Expanded,
    Compact,
}

impl ClickResult {
    pub fn item_index(&self) -> Option<usize> {
        match self {
            ClickResult::None => None,
            ClickResult::Switch(index)
            | ClickResult::StepperDec(index)
            | ClickResult::StepperInc(index)
            | ClickResult::StepperValue(index)
            | ClickResult::FontSelect(index)
            | ClickResult::FontReset(index)
            | ClickResult::CenterLink(index)
            | ClickResult::SourceButton(index)
            | ClickResult::RowButton(index)
            | ClickResult::AppItem(index)
            | ClickResult::FolderSelect(index)
            | ClickResult::FolderClear(index) => Some(*index),
        }
    }

    pub fn step_direction(&self) -> Option<StepDirection> {
        match self {
            ClickResult::StepperDec(_) => Some(StepDirection::Decrement),
            ClickResult::StepperInc(_) => Some(StepDirection::Increment),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetSource {
    BuiltIn(WidgetKind),
    Plugin(PluginWidgetId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetPreviewHit {
    None,
    Source(WidgetSource),
    Slot(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetEditorSlot {
    Expanded(usize),
    Compact(CompactWidgetPosition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactWidgetPreviewHit {
    None,
    Source(CompactWidgetKind),
    Slot(CompactWidgetPosition),
}

fn in_rect(mx: f32, my: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    mx >= x && mx <= x + w && my >= y && my <= y + h
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetGridGeom {
    pub cap_x: f32,
    pub cap_y: f32,
    pub cap_w: f32,
    pub cap_h: f32,
    pub cap_scale: f32,
    layout: WidgetGridLayout,
}

impl WidgetGridGeom {
    pub fn slot_rect(&self, slot: usize) -> (f32, f32, f32, f32) {
        self.layout.slot_rect(slot)
    }

    pub fn footprint_rect(&self, span: (usize, usize), slot: usize) -> (f32, f32, f32, f32) {
        let cells = crate::core::config::span_cells(slot, span);
        self.layout.footprint_rect_span(cells[0], span)
    }

    pub fn slot_at_point(&self, x: f32, y: f32, include_gaps: bool) -> Option<usize> {
        self.layout.slot_at_point(x, y, include_gaps)
    }
}

#[derive(Debug, Clone)]
pub struct CompactWidgetGridGeom {
    pub cap_x: f32,
    pub cap_y: f32,
    pub cap_w: f32,
    pub cap_h: f32,
    pub cap_scale: f32,
    slot_y: f32,
    slot_h: f32,
    gap: f32,
    slots: Vec<(CompactWidgetPosition, f32, f32)>,
}

impl CompactWidgetGridGeom {
    pub fn slot_rect(&self, position: CompactWidgetPosition) -> Option<(f32, f32, f32, f32)> {
        self.slots
            .iter()
            .find(|(candidate, _, _)| *candidate == position)
            .map(|(_, x, width)| (*x, self.slot_y, *width, self.slot_h))
    }

    pub fn drop_indicator_x(&self, position: CompactWidgetPosition) -> f32 {
        let mut aligned = self
            .slots
            .iter()
            .filter(|(candidate, _, _)| candidate.alignment == position.alignment);
        if let Some((_, x, _)) = aligned.nth(position.index) {
            return *x - self.gap / 2.0;
        }
        if let Some((_, x, width)) = self
            .slots
            .iter()
            .rev()
            .find(|(candidate, _, _)| candidate.alignment == position.alignment)
        {
            return *x + *width + self.gap / 2.0;
        }
        match position.alignment {
            CompactWidgetAlignment::Left => self.cap_x + 9.0 * self.cap_scale,
            CompactWidgetAlignment::Center => self.cap_x + self.cap_w / 2.0,
            CompactWidgetAlignment::Right => self.cap_x + self.cap_w - 9.0 * self.cap_scale,
        }
    }

    pub fn slot_at_point(
        &self,
        x: f32,
        y: f32,
        include_drop_zones: bool,
    ) -> Option<CompactWidgetPosition> {
        if y < self.slot_y || y > self.slot_y + self.slot_h {
            return None;
        }
        if include_drop_zones && x >= self.cap_x && x <= self.cap_x + self.cap_w {
            let relative_x = (x - self.cap_x) / self.cap_w;
            let alignment = [
                CompactWidgetAlignment::Left,
                CompactWidgetAlignment::Center,
                CompactWidgetAlignment::Right,
            ]
            .into_iter()
            .find(|alignment| {
                let bounds = self
                    .slots
                    .iter()
                    .filter(|(position, _, _)| position.alignment == *alignment)
                    .fold(None::<(f32, f32)>, |bounds, (_, slot_x, width)| {
                        Some(match bounds {
                            Some((left, right)) => (left.min(*slot_x), right.max(*slot_x + *width)),
                            None => (*slot_x, *slot_x + *width),
                        })
                    });
                bounds.is_some_and(|(left, right)| x >= left - self.gap && x <= right + self.gap)
            })
            .unwrap_or_else(|| {
                if relative_x < 1.0 / 3.0 {
                    CompactWidgetAlignment::Left
                } else if relative_x < 2.0 / 3.0 {
                    CompactWidgetAlignment::Center
                } else {
                    CompactWidgetAlignment::Right
                }
            });
            let index = self
                .slots
                .iter()
                .filter(|(position, slot_x, width)| {
                    position.alignment == alignment && x > *slot_x + *width / 2.0
                })
                .count();
            return Some(CompactWidgetPosition { alignment, index });
        }
        self.slots.iter().find_map(|(position, slot_x, width)| {
            in_rect(x, y, *slot_x, self.slot_y, *width, self.slot_h).then_some(*position)
        })
    }
}

pub fn widget_delete_button_center(x: f32, y: f32, w: f32, h: f32, scale: f32) -> (f32, f32) {
    let corner_inset = widget_corner_radius(w, h, scale) * (1.0 - std::f32::consts::FRAC_1_SQRT_2);
    (x + w - corner_inset, y + corner_inset)
}

pub fn widget_delete_button_hit(mouse: (f32, f32), rect: (f32, f32, f32, f32), scale: f32) -> bool {
    let (mx, my) = mouse;
    let (x, y, w, h) = rect;
    let (cx, cy) = widget_delete_button_center(x, y, w, h, scale);
    let radius = (7.0 * scale).max(6.0);
    (mx - cx).powi(2) + (my - cy).powi(2) <= radius.powi(2)
}

pub fn widget_source_rect(row_x: f32, source_y: f32, index: usize) -> (f32, f32, f32, f32) {
    let column = index % 4;
    let row = index / 4;
    let source_x = row_x + 12.0 + column as f32 * (WIDGET_LIBRARY_TILE_W + WIDGET_LIBRARY_TILE_GAP);
    let source_y = source_y + row as f32 * (WIDGET_LIBRARY_TILE_H + WIDGET_LIBRARY_TILE_GAP);
    (
        source_x,
        source_y,
        WIDGET_LIBRARY_TILE_W,
        WIDGET_LIBRARY_TILE_H,
    )
}

pub fn widget_library_items(
    widget_layout: &[WidgetSlot],
    plugin_widget_layout: &[PluginWidgetSlot],
    plugin_widgets: &[PluginWidget],
    dragging: Option<&WidgetSource>,
) -> Vec<WidgetSource> {
    let mut items = AVAILABLE_WIDGETS
        .iter()
        .copied()
        .filter(|kind| {
            dragging != Some(&WidgetSource::BuiltIn(*kind))
                && !widget_layout
                    .iter()
                    .any(|entry| entry.widget == Some(*kind))
        })
        .map(WidgetSource::BuiltIn)
        .collect::<Vec<_>>();
    items.extend(plugin_widgets.iter().filter_map(|widget| {
        let id = widget.layout_id()?;
        (dragging != Some(&WidgetSource::Plugin(id.clone()))
            && !plugin_widget_layout.iter().any(|entry| entry.id() == id))
        .then_some(WidgetSource::Plugin(id))
    }));
    items
}

pub fn compact_widget_library_items(
    layout: &[CompactWidgetSlot],
    dragging: Option<CompactWidgetKind>,
) -> Vec<CompactWidgetKind> {
    AVAILABLE_COMPACT_WIDGETS
        .iter()
        .copied()
        .filter(|widget| {
            dragging != Some(*widget) && !layout.iter().any(|entry| entry.widget == Some(*widget))
        })
        .collect()
}

pub fn widget_preview_height(item_count: usize) -> f32 {
    let rows = item_count.max(1).div_ceil(4);
    WIDGET_PREVIEW_BASE_H
        + (rows.saturating_sub(1) as f32) * (WIDGET_LIBRARY_TILE_H + WIDGET_LIBRARY_TILE_GAP)
}

pub fn widget_source_span(
    source: &WidgetSource,
    plugin_widgets: &[PluginWidget],
) -> Option<(usize, usize)> {
    match source {
        WidgetSource::BuiltIn(kind) => Some(kind.span()),
        WidgetSource::Plugin(id) => plugin_widgets
            .iter()
            .find(|widget| widget.layout_id().as_ref() == Some(id))
            .map(PluginWidget::span),
    }
}

pub fn widget_grid_geom(
    item_y: f32,
    width: f32,
    expanded_width: f32,
    expanded_height: f32,
) -> WidgetGridGeom {
    let content_w = width - CONTENT_PADDING * 2.0;
    let row_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let preview_w = content_w - GROUP_INNER_PAD * 2.0;
    let py = item_y + 10.0;

    let mut cap_w = expanded_width;
    let mut cap_h = expanded_height;
    let max_w = preview_w - 32.0;
    let editor_content_h = WIDGET_ISLAND_PANEL_H - WIDGET_EDITOR_HEADER_H - 16.0;
    let max_h = editor_content_h;
    let mut cap_scale = 1.0;
    if cap_w > max_w || cap_h > max_h {
        let scale_w = max_w / cap_w;
        let scale_h = max_h / cap_h;
        cap_scale = scale_w.min(scale_h);
        cap_w *= cap_scale;
        cap_h *= cap_scale;
    }

    let cap_x = row_x + (preview_w - cap_w) / 2.0;
    let cap_y = py + WIDGET_EDITOR_HEADER_H + (editor_content_h - cap_h) / 2.0;

    let layout = widget_grid_layout(cap_x, cap_y, cap_w, cap_h, cap_scale);

    WidgetGridGeom {
        cap_x,
        cap_y,
        cap_w,
        cap_h,
        cap_scale,
        layout,
    }
}

pub fn compact_widget_grid_geom(
    item_y: f32,
    width: f32,
    base_width: f32,
    base_height: f32,
    layout: &[CompactWidgetSlot],
    dragging: Option<CompactWidgetKind>,
) -> CompactWidgetGridGeom {
    let content_w = width - CONTENT_PADDING * 2.0;
    let row_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let preview_w = content_w - GROUP_INNER_PAD * 2.0;
    let panel_y = item_y + 10.0;
    let editor_content_h = COMPACT_WIDGET_ISLAND_PANEL_H - WIDGET_EDITOR_HEADER_H - 16.0;
    let max_w = preview_w - 48.0;
    let max_h = editor_content_h - 12.0;
    let preview_width = crate::ui::widget::compact::preview_width(layout, base_width, dragging);
    let extra_width = (preview_width - base_width).max(0.0);
    let preferred_w = (COMPACT_WIDGET_PREVIEW_PREFERRED_W
        + extra_width * COMPACT_WIDGET_PREVIEW_WIDTH_GROWTH)
        .min(max_w);
    let cap_scale = (preferred_w / preview_width.max(1.0))
        .min(max_h / base_height.max(1.0))
        .clamp(0.25, COMPACT_WIDGET_PREVIEW_MAX_SCALE);
    let cap_w = preview_width * cap_scale;
    let cap_h = base_height * cap_scale;
    let cap_x = row_x + (preview_w - cap_w) / 2.0;
    let cap_y = panel_y + WIDGET_EDITOR_HEADER_H + (editor_content_h - cap_h) / 2.0;
    let mut entries = layout
        .iter()
        .filter_map(|entry| {
            entry
                .widget
                .map(|widget| (entry.position(), widget_width(widget) * cap_scale))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(position, _)| (position.alignment.order(), position.index));
    let gap = 7.0 * cap_scale;
    let mut slots = Vec::with_capacity(entries.len());
    for alignment in [
        CompactWidgetAlignment::Left,
        CompactWidgetAlignment::Center,
        CompactWidgetAlignment::Right,
    ] {
        let mut next_x = cap_x
            + crate::ui::widget::compact::alignment_offset(layout, preview_width, alignment)
                * cap_scale;
        for (position, width) in entries
            .iter()
            .filter(|(position, _)| position.alignment == alignment)
        {
            slots.push((*position, next_x, *width));
            next_x += *width + gap;
        }
    }

    CompactWidgetGridGeom {
        cap_x,
        cap_y,
        cap_w,
        cap_h,
        cap_scale,
        slot_y: cap_y + 3.0 * cap_scale,
        slot_h: (cap_h - 6.0 * cap_scale).max(1.0),
        gap,
        slots,
    }
}

pub fn compact_widget_preview_hit_test(
    mouse: (f32, f32),
    panel: (f32, f32),
    base_size: (f32, f32),
    layout: &[CompactWidgetSlot],
    dragging: Option<CompactWidgetKind>,
) -> CompactWidgetPreviewHit {
    let (mx, my) = mouse;
    let (item_y, width) = panel;
    let (base_width, base_height) = base_size;
    let row_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let panel_y = item_y + 10.0;
    let library_y = panel_y + COMPACT_WIDGET_ISLAND_PANEL_H + WIDGET_PANEL_GAP;
    let source_y = library_y + WIDGET_LIBRARY_HEADER_H;
    for (index, widget) in compact_widget_library_items(layout, dragging)
        .into_iter()
        .enumerate()
    {
        let (source_x, source_y, source_w, source_h) = widget_source_rect(row_x, source_y, index);
        if in_rect(mx, my, source_x, source_y, source_w, source_h) {
            return CompactWidgetPreviewHit::Source(widget);
        }
    }

    let geometry =
        compact_widget_grid_geom(item_y, width, base_width, base_height, layout, dragging);
    if let Some(slot) = geometry.slot_at_point(mx, my, dragging.is_some()) {
        return CompactWidgetPreviewHit::Slot(slot);
    }
    CompactWidgetPreviewHit::None
}

pub fn widget_preview_hit_test(
    mouse: (f32, f32),
    panel: (f32, f32),
    expanded_size: (f32, f32),
    layouts: (&[WidgetSlot], &[PluginWidgetSlot], &[PluginWidget]),
    dragging: Option<&WidgetSource>,
) -> WidgetPreviewHit {
    let (mx, my) = mouse;
    let (item_y, width) = panel;
    let (expanded_width, expanded_height) = expanded_size;
    let (widget_layout, plugin_widget_layout, plugin_widgets) = layouts;
    let row_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let py = item_y + 10.0;
    let library_panel_y = py + WIDGET_ISLAND_PANEL_H + WIDGET_PANEL_GAP;

    let source_y = library_panel_y + WIDGET_LIBRARY_HEADER_H;
    for (idx, source) in widget_library_items(
        widget_layout,
        plugin_widget_layout,
        plugin_widgets,
        dragging,
    )
    .iter()
    .enumerate()
    {
        let (source_x, source_y, source_w, source_h) = widget_source_rect(row_x, source_y, idx);
        if in_rect(mx, my, source_x, source_y, source_w, source_h) {
            return WidgetPreviewHit::Source(source.clone());
        }
    }

    let geom = widget_grid_geom(item_y, width, expanded_width, expanded_height);
    if let Some(slot) = geom.slot_at_point(mx, my, dragging.is_some()) {
        return WidgetPreviewHit::Slot(slot);
    }

    WidgetPreviewHit::None
}

pub fn hit_test(items: &[SettingsItem], mx: f32, my: f32, start_y: f32, width: f32) -> ClickResult {
    let mut y = start_y;
    let content_w = width - CONTENT_PADDING * 2.0;

    for (idx, item) in items.iter().enumerate() {
        match item {
            SettingsItem::RowStepper { enabled, .. } if *enabled => {
                let cy = y + ROW_HEIGHT / 2.0;
                let btn_inc_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - STEPPER_BTN_SIZE;
                let value_x = btn_inc_x - STEPPER_GAP - STEPPER_VALUE_W;
                let btn_dec_x = value_x - STEPPER_GAP - STEPPER_BTN_SIZE;
                let btn_y = cy - STEPPER_BTN_SIZE / 2.0;
                if in_rect(mx, my, btn_dec_x, btn_y, STEPPER_BTN_SIZE, STEPPER_BTN_SIZE) {
                    return ClickResult::StepperDec(idx);
                }
                if in_rect(mx, my, btn_inc_x, btn_y, STEPPER_BTN_SIZE, STEPPER_BTN_SIZE) {
                    return ClickResult::StepperInc(idx);
                }
                if in_rect(mx, my, value_x, btn_y, STEPPER_VALUE_W, STEPPER_BTN_SIZE) {
                    return ClickResult::StepperValue(idx);
                }
            }
            SettingsItem::RowSwitch { enabled, .. } if *enabled => {
                let cy = y + ROW_HEIGHT / 2.0;
                let toggle_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - TOGGLE_W;
                let toggle_y = cy - TOGGLE_H / 2.0;
                if in_rect(mx, my, toggle_x, toggle_y, TOGGLE_W, TOGGLE_H) {
                    return ClickResult::Switch(idx);
                }
            }
            SettingsItem::RowFontPicker { reset_label, .. } => {
                let cy = y + ROW_HEIGHT / 2.0;
                let sel_w: f32 = 72.0;
                let sel_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - sel_w;
                let btn_y = cy - POPUP_BTN_H / 2.0;
                if in_rect(mx, my, sel_x, btn_y, sel_w, POPUP_BTN_H) {
                    return ClickResult::FontSelect(idx);
                }
                if reset_label.is_some() {
                    let rst_w: f32 = 72.0;
                    let rst_x = sel_x - rst_w - 6.0;
                    if in_rect(mx, my, rst_x, btn_y, rst_w, POPUP_BTN_H) {
                        return ClickResult::FontReset(idx);
                    }
                }
            }
            SettingsItem::RowFolderPicker {
                clear_label,
                current_path,
                enabled,
                ..
            } if *enabled => {
                let has_path = current_path.as_ref().is_some_and(|p| !p.is_empty());
                let row_h = if has_path { 64.0 } else { ROW_HEIGHT };
                let cy = y + row_h / 2.0;
                let sel_w: f32 = 72.0;
                let sel_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - sel_w;
                let btn_y = cy - POPUP_BTN_H / 2.0;
                if in_rect(mx, my, sel_x, btn_y, sel_w, POPUP_BTN_H) {
                    return ClickResult::FolderSelect(idx);
                }
                if clear_label.is_some() {
                    let clr_w: f32 = 72.0;
                    let clr_x = sel_x - clr_w - 6.0;
                    if in_rect(mx, my, clr_x, btn_y, clr_w, POPUP_BTN_H) {
                        return ClickResult::FolderClear(idx);
                    }
                }
            }
            SettingsItem::RowSourceSelect { enabled, .. } if *enabled => {
                let cy = y + ROW_HEIGHT / 2.0;
                let btn_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - POPUP_BTN_W;
                let btn_y = cy - POPUP_BTN_H / 2.0;
                if in_rect(mx, my, btn_x, btn_y, POPUP_BTN_W, POPUP_BTN_H) {
                    return ClickResult::SourceButton(idx);
                }
            }
            SettingsItem::RowButton { enabled, .. } if *enabled => {
                let cy = y + ROW_HEIGHT / 2.0;
                let btn_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - POPUP_BTN_W;
                let btn_y = cy - POPUP_BTN_H / 2.0;
                if in_rect(mx, my, btn_x, btn_y, POPUP_BTN_W, POPUP_BTN_H) {
                    return ClickResult::RowButton(idx);
                }
            }
            SettingsItem::RowAppItem { enabled, .. }
                if *enabled && in_rect(mx, my, CONTENT_PADDING, y, content_w, ROW_HEIGHT) =>
            {
                return ClickResult::AppItem(idx);
            }
            SettingsItem::RowLabel { .. } => {}
            SettingsItem::CenterLink { .. }
                if mx >= width / 2.0 - 100.0
                    && mx <= width / 2.0 + 100.0
                    && my >= y
                    && my <= y + 40.0 =>
            {
                return ClickResult::CenterLink(idx);
            }
            _ => {}
        }
        y += item.height();
    }
    ClickResult::None
}

pub fn hover_test(items: &[SettingsItem], mx: f32, my: f32, start_y: f32, width: f32) -> bool {
    hit_test(items, mx, my, start_y, width) != ClickResult::None
}
