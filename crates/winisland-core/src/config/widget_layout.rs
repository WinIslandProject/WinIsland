use crate::widgets::PluginWidget;

use super::widgets::*;

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
    widgets: &'a [PluginWidget],
    target_slot: usize,
) -> Option<(&'a PluginWidgetSlot, &'a PluginWidget)> {
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
    widgets: &[PluginWidget],
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
    widgets: &[PluginWidget],
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
    widgets: &[PluginWidget],
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
    widgets: &[PluginWidget],
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
