pub mod anim;
pub mod input;
pub mod items;
pub mod renderer;

pub use anim::SwitchAnimator;
pub use input::{
    ClickResult, CompactWidgetPreviewHit, StepDirection, WidgetEditorMode, WidgetEditorSlot,
    WidgetPreviewHit, WidgetSource, compact_widget_grid_geom, compact_widget_preview_hit_test,
    hit_test, hover_test, widget_delete_button_hit, widget_grid_geom, widget_library_items,
    widget_preview_height, widget_preview_hit_test, widget_source_span,
};
pub use renderer::{ActiveStepperValue, DrawItemsParams, content_height, draw_items};
pub(crate) use renderer::{SettingsPainter, ellipsize_text, settings_paint};
