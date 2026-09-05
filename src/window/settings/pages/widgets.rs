use skia_safe::{Contains, Point, Rect};

use crate::core::config::{
    WidgetKind, clear_compact_widget_slot, clear_plugin_widget, clear_widget_slot,
    place_builtin_widget, place_compact_widget, place_plugin_widget, plugin_widget_covering_slot,
    widget_covering_slot,
};
use crate::utils::settings_ui::items::SettingsItem;
use crate::utils::settings_ui::{
    CompactWidgetPreviewHit, WidgetEditorSlot, WidgetPreviewHit, WidgetSource,
    compact_widget_grid_geom, compact_widget_preview_hit_test, widget_delete_button_hit,
    widget_grid_geom, widget_library_items, widget_preview_height, widget_preview_hit_test,
};

use super::super::{SETTINGS_HEADER_H, SIDEBAR_W, SettingsApp, WIDGETS_PAGE_INDEX};
use crate::utils::settings_ui::WidgetEditorMode;

const MODE_CONTROL_W: f32 = 154.0;
const MODE_CONTROL_H: f32 = 30.0;
const MODE_CONTROL_RIGHT: f32 = 18.0;
const MODE_CONTROL_Y: f32 = 17.0;

struct WidgetPreviewContext {
    item_y: f32,
    width: f32,
    pointer: (f32, f32),
}

impl SettingsApp {
    pub(crate) fn build_widget_items(&self) -> Vec<SettingsItem> {
        let height = match self.widget_editor_mode {
            WidgetEditorMode::Expanded => {
                let count = widget_library_items(
                    &self.config.widget_layout,
                    &self.config.plugin_widget_layout,
                    &self.plugin_widgets,
                    self.widget_dragging.as_ref(),
                )
                .len();
                widget_preview_height(count)
            }
            WidgetEditorMode::Compact => crate::utils::settings_ui::input::COMPACT_WIDGET_PREVIEW_H,
        };
        vec![SettingsItem::WidgetPreview { height }]
    }

    pub(crate) fn widget_mode_control_rect(&self) -> Rect {
        let width = self.logical_window_size().0;
        Rect::from_xywh(
            width - MODE_CONTROL_RIGHT - MODE_CONTROL_W,
            MODE_CONTROL_Y,
            MODE_CONTROL_W,
            MODE_CONTROL_H,
        )
    }

    pub(crate) fn widget_mode_segment_rect(&self, mode: WidgetEditorMode) -> Rect {
        let control = self.widget_mode_control_rect();
        let segment_width = control.width() / 2.0;
        Rect::from_xywh(
            control.left
                + if mode == WidgetEditorMode::Compact {
                    segment_width
                } else {
                    0.0
                },
            control.top,
            segment_width,
            control.height(),
        )
    }

    pub(crate) fn widget_mode_at(&self, x: f32, y: f32) -> Option<WidgetEditorMode> {
        let point = Point::new(x, y);
        if self.active_page != WIDGETS_PAGE_INDEX
            || !self.widget_mode_control_rect().contains(point)
        {
            return None;
        }
        if self
            .widget_mode_segment_rect(WidgetEditorMode::Expanded)
            .contains(point)
        {
            Some(WidgetEditorMode::Expanded)
        } else {
            Some(WidgetEditorMode::Compact)
        }
    }

    pub(crate) fn handle_widget_mode_click(&mut self, x: f32, y: f32) -> bool {
        let Some(mode) = self.widget_mode_at(x, y) else {
            return false;
        };
        if mode != self.widget_editor_mode {
            self.widget_editor_mode = mode;
            self.widget_dragging = None;
            self.compact_widget_dragging = None;
            self.widget_drag_hover_slot = None;
            self.widget_preview_hover_slot = None;
            self.scroll_y = 0.0;
            self.target_scroll_y = 0.0;
            self.scroll_vel_y = 0.0;
            self.mark_items_dirty();
            self.request_redraw();
        }
        true
    }

    fn widget_preview_item_y(&mut self) -> Option<f32> {
        if self.active_page != WIDGETS_PAGE_INDEX {
            return None;
        }
        self.ensure_items_cache();
        let mut y = SETTINGS_HEADER_H;
        for item in &self.cached_items {
            if matches!(item, SettingsItem::WidgetPreview { .. }) {
                return Some(y);
            }
            y += item.height();
        }
        None
    }

    fn widget_preview_context(&mut self) -> Option<WidgetPreviewContext> {
        let item_y = self.widget_preview_item_y()?;
        let (mouse_x, mouse_y) = self.logical_mouse_pos;
        (mouse_x >= SIDEBAR_W).then(|| WidgetPreviewContext {
            item_y,
            width: self.content_width(),
            pointer: (mouse_x - SIDEBAR_W, mouse_y + self.scroll_y),
        })
    }

    fn expanded_widget_preview_hit_at_mouse(&mut self) -> Option<WidgetPreviewHit> {
        let context = self.widget_preview_context()?;
        Some(widget_preview_hit_test(
            context.pointer,
            (context.item_y, context.width),
            (self.config.expanded_width, self.config.expanded_height),
            (
                &self.config.widget_layout,
                &self.config.plugin_widget_layout,
                &self.plugin_widgets,
            ),
            self.widget_dragging.as_ref(),
        ))
    }

    fn compact_widget_preview_hit_at_mouse(&mut self) -> Option<CompactWidgetPreviewHit> {
        let context = self.widget_preview_context()?;
        Some(compact_widget_preview_hit_test(
            context.pointer,
            (context.item_y, context.width),
            (self.config.base_width, self.config.base_height),
            &self.config.compact_widget_layout,
            self.compact_widget_dragging,
        ))
    }

    pub(crate) fn widget_preview_hovered_at_mouse(&mut self) -> bool {
        match self.widget_editor_mode {
            WidgetEditorMode::Expanded => self
                .expanded_widget_preview_hit_at_mouse()
                .is_some_and(|hit| hit != WidgetPreviewHit::None),
            WidgetEditorMode::Compact => self
                .compact_widget_preview_hit_at_mouse()
                .is_some_and(|hit| hit != CompactWidgetPreviewHit::None),
        }
    }

    pub(crate) fn widget_preview_slot_at_mouse(&mut self) -> Option<WidgetEditorSlot> {
        match self.widget_editor_mode {
            WidgetEditorMode::Expanded => {
                self.expanded_widget_preview_hit_at_mouse()
                    .and_then(|hit| match hit {
                        WidgetPreviewHit::Slot(slot) => Some(WidgetEditorSlot::Expanded(slot)),
                        _ => None,
                    })
            }
            WidgetEditorMode::Compact => {
                self.compact_widget_preview_hit_at_mouse()
                    .and_then(|hit| match hit {
                        CompactWidgetPreviewHit::Slot(position) => {
                            Some(WidgetEditorSlot::Compact(position))
                        }
                        _ => None,
                    })
            }
        }
    }

    pub(crate) fn widget_drag_active(&self) -> bool {
        match self.widget_editor_mode {
            WidgetEditorMode::Expanded => self.widget_dragging.is_some(),
            WidgetEditorMode::Compact => self.compact_widget_dragging.is_some(),
        }
    }

    pub(crate) fn active_widget_drag_hover_slot(&self) -> Option<WidgetEditorSlot> {
        self.widget_drag_hover_slot
    }

    pub(crate) fn set_active_widget_drag_hover_slot(&mut self, slot: Option<WidgetEditorSlot>) {
        self.widget_drag_hover_slot = slot.filter(|slot| slot.mode() == self.widget_editor_mode);
    }

    pub(crate) fn active_widget_preview_hover_slot(&self) -> Option<WidgetEditorSlot> {
        self.widget_preview_hover_slot
    }

    pub(crate) fn set_active_widget_preview_hover_slot(&mut self, slot: Option<WidgetEditorSlot>) {
        self.widget_preview_hover_slot = slot.filter(|slot| slot.mode() == self.widget_editor_mode);
    }

    pub(crate) fn handle_widget_drag_press(&mut self) -> bool {
        if self.widget_editor_mode == WidgetEditorMode::Compact {
            return self.handle_compact_widget_drag_press();
        }
        let Some(hit) = self.expanded_widget_preview_hit_at_mouse() else {
            return false;
        };
        let source = match hit {
            WidgetPreviewHit::Source(source) => source,
            WidgetPreviewHit::Slot(slot) => {
                let (source, anchor, span, removable) = if let Some((anchor, widget)) =
                    widget_covering_slot(&self.config.widget_layout, slot)
                {
                    (
                        WidgetSource::BuiltIn(widget),
                        anchor,
                        widget.span(),
                        widget != WidgetKind::Settings,
                    )
                } else if let Some((entry, widget)) = plugin_widget_covering_slot(
                    &self.config.plugin_widget_layout,
                    &self.plugin_widgets,
                    slot,
                ) {
                    (
                        WidgetSource::Plugin(entry.id()),
                        entry.slot,
                        widget.span(),
                        true,
                    )
                } else {
                    return false;
                };
                let Some(context) = self.widget_preview_context() else {
                    return false;
                };
                let geometry = widget_grid_geom(
                    context.item_y,
                    context.width,
                    self.config.expanded_width,
                    self.config.expanded_height,
                );
                let (x, y, width, height) = geometry.footprint_rect(span, anchor);
                if removable
                    && widget_delete_button_hit(
                        context.pointer,
                        (x, y, width, height),
                        geometry.cap_scale,
                    )
                {
                    return false;
                }
                source
            }
            WidgetPreviewHit::None => return false,
        };
        self.widget_dragging = Some(source);
        self.widget_drag_hover_slot = None;
        self.mark_items_dirty();
        true
    }

    fn handle_compact_widget_drag_press(&mut self) -> bool {
        let Some(hit) = self.compact_widget_preview_hit_at_mouse() else {
            return false;
        };
        let widget = match hit {
            CompactWidgetPreviewHit::Source(widget) => widget,
            CompactWidgetPreviewHit::Slot(position) => {
                let Some(widget) = self
                    .config
                    .compact_widget_layout
                    .iter()
                    .find(|entry| entry.position() == position)
                    .and_then(|entry| entry.widget)
                else {
                    return false;
                };
                let Some(context) = self.widget_preview_context() else {
                    return false;
                };
                let geometry = compact_widget_grid_geom(
                    context.item_y,
                    context.width,
                    self.config.base_width,
                    self.config.base_height,
                    &self.config.compact_widget_layout,
                    self.compact_widget_dragging,
                );
                let Some((x, y, width, height)) = geometry.slot_rect(position) else {
                    return false;
                };
                if widget_delete_button_hit(
                    context.pointer,
                    (x, y, width, height),
                    geometry.cap_scale,
                ) {
                    return false;
                }
                widget
            }
            CompactWidgetPreviewHit::None => return false,
        };
        self.compact_widget_dragging = Some(widget);
        self.widget_drag_hover_slot = None;
        self.mark_items_dirty();
        true
    }

    pub(crate) fn handle_widget_drag_release(&mut self) -> bool {
        if self.widget_editor_mode == WidgetEditorMode::Compact {
            return self.handle_compact_widget_drag_release();
        }
        let Some(source) = self.widget_dragging.take() else {
            return false;
        };
        let old_widget_layout = self.config.widget_layout.clone();
        let old_plugin_layout = self.config.plugin_widget_layout.clone();
        if let Some(slot) = self
            .widget_drag_hover_slot
            .take()
            .and_then(WidgetEditorSlot::expanded)
        {
            match source {
                WidgetSource::BuiltIn(widget) => place_builtin_widget(
                    &mut self.config.widget_layout,
                    &mut self.config.plugin_widget_layout,
                    &self.plugin_widgets,
                    widget,
                    slot,
                ),
                WidgetSource::Plugin(id) => {
                    place_plugin_widget(
                        &mut self.config.widget_layout,
                        &mut self.config.plugin_widget_layout,
                        &self.plugin_widgets,
                        &id,
                        slot,
                    );
                }
            }
        }
        self.mark_items_dirty();
        if old_widget_layout != self.config.widget_layout
            || old_plugin_layout != self.config.plugin_widget_layout
        {
            crate::core::persistence::save_config(&self.config);
        }
        true
    }

    fn handle_compact_widget_drag_release(&mut self) -> bool {
        let Some(widget) = self.compact_widget_dragging.take() else {
            return false;
        };
        let old_layout = self.config.compact_widget_layout.clone();
        if let Some(position) = self
            .widget_drag_hover_slot
            .take()
            .and_then(WidgetEditorSlot::compact)
        {
            place_compact_widget(&mut self.config.compact_widget_layout, widget, position);
        }
        self.mark_items_dirty();
        if old_layout != self.config.compact_widget_layout {
            crate::core::persistence::save_config(&self.config);
        }
        true
    }

    pub(crate) fn handle_widget_click(&mut self) -> bool {
        if self.widget_editor_mode == WidgetEditorMode::Compact {
            return self.handle_compact_widget_click();
        }
        let Some(context) = self.widget_preview_context() else {
            return false;
        };
        let geometry = widget_grid_geom(
            context.item_y,
            context.width,
            self.config.expanded_width,
            self.config.expanded_height,
        );

        let built_in_anchor = self.config.widget_layout.iter().find_map(|entry| {
            let widget = entry.widget?;
            if widget == WidgetKind::Settings {
                return None;
            }
            let (x, y, width, height) = geometry.footprint_rect(widget.span(), entry.slot);
            widget_delete_button_hit(context.pointer, (x, y, width, height), geometry.cap_scale)
                .then_some(entry.slot)
        });
        if let Some(anchor) = built_in_anchor {
            clear_widget_slot(&mut self.config.widget_layout, anchor);
            crate::core::persistence::save_config(&self.config);
            self.mark_items_dirty();
            return true;
        }

        let plugin_id = self.config.plugin_widget_layout.iter().find_map(|entry| {
            let widget = self
                .plugin_widgets
                .iter()
                .find(|widget| widget.layout_id().as_ref() == Some(&entry.id()))?;
            let (x, y, width, height) = geometry.footprint_rect(widget.span(), entry.slot);
            widget_delete_button_hit(context.pointer, (x, y, width, height), geometry.cap_scale)
                .then(|| entry.id())
        });
        let Some(plugin_id) = plugin_id else {
            return false;
        };
        clear_plugin_widget(&mut self.config.plugin_widget_layout, &plugin_id);
        crate::core::persistence::save_config(&self.config);
        self.mark_items_dirty();
        true
    }

    fn handle_compact_widget_click(&mut self) -> bool {
        let Some(context) = self.widget_preview_context() else {
            return false;
        };
        let geometry = compact_widget_grid_geom(
            context.item_y,
            context.width,
            self.config.base_width,
            self.config.base_height,
            &self.config.compact_widget_layout,
            self.compact_widget_dragging,
        );
        let position = self.config.compact_widget_layout.iter().find_map(|entry| {
            entry.widget?;
            let (x, y, width, height) = geometry.slot_rect(entry.position())?;
            widget_delete_button_hit(context.pointer, (x, y, width, height), geometry.cap_scale)
                .then_some(entry.position())
        });
        let Some(position) = position else {
            return false;
        };
        clear_compact_widget_slot(&mut self.config.compact_widget_layout, position);
        crate::core::persistence::save_config(&self.config);
        self.mark_items_dirty();
        true
    }
}
