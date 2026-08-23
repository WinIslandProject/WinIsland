mod controls;
mod items;
mod widget_preview;

use skia_safe::{Canvas, Rect};

use crate::core::config::{
    CompactWidgetKind, CompactWidgetPosition, CompactWidgetSlot, PluginWidgetSlot, WidgetSlot,
};
use crate::core::plugin_widget::PluginWidget;
use crate::utils::color::SettingsTheme;
use crate::utils::settings_ui::input::{WidgetEditorMode, WidgetSource};

use super::anim::SwitchAnimator;
use super::items::SettingsItem;

pub use items::{content_height, draw_items};

pub struct ActiveStepperValue<'a> {
    pub rect: Rect,
    pub text: &'a str,
    pub show_caret: bool,
}

pub struct DrawItemsParams<'a> {
    pub canvas: &'a Canvas,
    pub items: &'a [SettingsItem],
    pub start_y: f32,
    pub width: f32,
    pub anims: &'a SwitchAnimator,
    pub theme: &'a SettingsTheme,
    pub visible_min_y: f32,
    pub visible_max_y: f32,
    pub island_style: &'a str,
    pub expanded_width: f32,
    pub expanded_height: f32,
    pub base_width: f32,
    pub base_height: f32,
    pub widget_editor_mode: WidgetEditorMode,
    pub widget_layout: &'a [WidgetSlot],
    pub plugin_widget_layout: &'a [PluginWidgetSlot],
    pub plugin_widgets: &'a [PluginWidget],
    pub widget_dragging: Option<&'a WidgetSource>,
    pub widget_drag_hover_slot: Option<usize>,
    pub widget_preview_hover_slot: Option<usize>,
    pub compact_widget_layout: &'a [CompactWidgetSlot],
    pub compact_widget_dragging: Option<CompactWidgetKind>,
    pub compact_widget_drag_hover_slot: Option<CompactWidgetPosition>,
    pub compact_widget_preview_hover_slot: Option<CompactWidgetPosition>,
    pub active_source_button: Option<Rect>,
    pub active_stepper_value: Option<ActiveStepperValue<'a>>,
    pub hover_pos: Option<(f32, f32)>,
}
