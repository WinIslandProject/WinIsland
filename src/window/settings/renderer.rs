use crate::core::i18n::tr;
use crate::ui::expanded::widget_view::draw_plugin_widget;
use crate::ui::widget::expanded::draw_mini_card;
use crate::utils::color::SettingsTheme;
use crate::utils::font::FontManager;
use crate::utils::settings_ui::items::{POPUP_ITEM_H, SettingsItem};
use crate::utils::settings_ui::{
    ActiveStepperValue, DrawItemsParams, SettingsPainter, WidgetSource, draw_items, settings_paint,
    widget_grid_geom, widget_source_span,
};
use crate::window::d3d::D3DRenderer;
use skia_safe::{Canvas, Color, Contains, Paint, Point, Rect};

use super::{
    PAGE_NAV_GAP, PAGE_NAV_SIZE, PAGE_NAV_X, PAGE_NAV_Y, PLUGINS_PAGE_INDEX, POPUP_MENU_R,
    POPUP_OPACITY_KEY, SETTINGS_HEADER_H, SIDEBAR_W, SettingsApp, WIDGETS_PAGE_INDEX,
    WINDOW_RADIUS, WidgetEditorMode,
};

impl SettingsApp {
    pub(crate) fn draw(&mut self, renderer: &mut D3DRenderer) {
        let Some(win) = self.window.as_ref() else {
            return;
        };
        let (p_w, p_h, scale) = {
            let size = win.inner_size();
            (
                size.width as i32,
                size.height as i32,
                win.scale_factor() as f32,
            )
        };
        if p_w <= 0 || p_h <= 0 {
            return;
        }

        self.ensure_items_cache();
        let theme = self.theme();
        let win_w = self.win_w / scale;
        let win_h = self.win_h / scale;
        let target = match self.renderer_target {
            Some(target) => target,
            None => return,
        };
        let render_result = renderer.draw(target, |direct_context, sk_surface| {
            let canvas = sk_surface.canvas();
            canvas.reset_matrix();
            canvas.clear(Color::TRANSPARENT);
            canvas.scale((scale, scale));

            let win_rect = Rect::from_xywh(0.0, 0.0, win_w, win_h);
            let win_rrect = skia_safe::RRect::new_rect_xy(win_rect, WINDOW_RADIUS, WINDOW_RADIUS);

            canvas.save();
            canvas.clip_rrect(win_rrect, skia_safe::ClipOp::Intersect, true);

            let bg_paint = settings_paint(theme.win_bg);
            canvas.draw_rect(win_rect, &bg_paint);

            self.draw_sidebar(direct_context, canvas, &theme);
            self.draw_page_navigation(canvas, &theme);
            self.draw_page_header(canvas, &theme, win_w);
            self.draw_widget_mode_control(canvas, &theme);

            let content_w = win_w - SIDEBAR_W;

            let content_start_y = SETTINGS_HEADER_H;

            self.target_scroll_y = self.target_scroll_y.clamp(0.0, self.cached_max_scroll);

            let clip_start_y = SETTINGS_HEADER_H;

            canvas.save();
            canvas.clip_rect(
                Rect::from_xywh(SIDEBAR_W, clip_start_y, content_w, win_h - clip_start_y),
                skia_safe::ClipOp::Intersect,
                true,
            );
            canvas.translate((SIDEBAR_W, -self.scroll_y));
            let active_source_button = self.popup.as_ref().map(|popup| {
                Rect::from_xywh(
                    popup.button_rect.left - SIDEBAR_W,
                    popup.button_rect.top + self.scroll_y,
                    popup.button_rect.width(),
                    popup.button_rect.height(),
                )
            });
            let active_stepper_value = self.number_input.as_ref().map(|input| ActiveStepperValue {
                rect: Rect::from_xywh(
                    input.rect.left - SIDEBAR_W,
                    input.rect.top + self.scroll_y,
                    input.rect.width(),
                    input.rect.height(),
                ),
                text: &input.text,
                show_caret: self.frame_count % 60 < 30,
            });
            draw_items(DrawItemsParams {
                canvas,
                items: &self.cached_items,
                start_y: content_start_y,
                width: content_w,
                anims: &self.switch_anim,
                theme: &theme,
                visible_min_y: self.scroll_y,
                visible_max_y: self.scroll_y + win_h,
                island_style: &self.config.island_style,
                expanded_width: self.config.expanded_width,
                expanded_height: self.config.expanded_height,
                base_width: self.config.base_width,
                base_height: self.config.base_height,
                widget_editor_mode: self.widget_editor_mode,
                widget_layout: &self.config.widget_layout,
                plugin_widget_layout: &self.config.plugin_widget_layout,
                plugin_widgets: &self.plugin_widgets,
                widget_dragging: self.widget_dragging.as_ref(),
                widget_drag_hover_slot: self.widget_drag_hover_slot,
                widget_preview_hover_slot: self.widget_preview_hover_slot,
                compact_widget_layout: &self.config.compact_widget_layout,
                compact_widget_dragging: self.compact_widget_dragging,
                active_source_button,
                active_stepper_value,
                hover_pos: Some((
                    self.logical_mouse_pos.0 - SIDEBAR_W,
                    self.logical_mouse_pos.1 + self.scroll_y,
                )),
            });
            canvas.restore();

            if let Some(scrollbar) = self.scrollbar_geometry() {
                let p = settings_paint(Color::from_argb(60, 255, 255, 255));
                canvas.draw_round_rect(
                    Rect::from_xywh(scrollbar.x, scrollbar.y, scrollbar.width, scrollbar.height),
                    scrollbar.width / 2.0,
                    scrollbar.width / 2.0,
                    &p,
                );
            }

            if self.active_page == PLUGINS_PAGE_INDEX {
                self.draw_plugins_page(direct_context, canvas, &theme, win_w, win_h);
            }

            self.draw_popup(canvas, &theme);
            self.draw_widget_drag_overlay(canvas, &theme, win_w, win_h);
            canvas.restore();

            // Draw a subtle rounded border around the window
            let border_rect = Rect::from_xywh(0.5, 0.5, win_w - 1.0, win_h - 1.0);
            let border_radius = WINDOW_RADIUS - 0.5;
            let border_rrect =
                skia_safe::RRect::new_rect_xy(border_rect, border_radius, border_radius);
            let mut border_paint = settings_paint(theme.separator);
            border_paint.set_style(skia_safe::paint::Style::Stroke);
            border_paint.set_stroke_width(1.0);
            canvas.draw_rrect(border_rrect, &border_paint);
        });
        if let Err(error) = render_result {
            log::error!("D3D12 settings rendering failed: {error}");
            self.close_requested = true;
        }
    }

    fn widget_preview_item_y_cached(&self) -> Option<f32> {
        if self.active_page != WIDGETS_PAGE_INDEX {
            return None;
        }
        let mut y = SETTINGS_HEADER_H;
        for item in &self.cached_items {
            if matches!(item, SettingsItem::WidgetPreview { .. }) {
                return Some(y);
            }
            y += item.height();
        }
        None
    }

    fn draw_widget_drag_overlay(
        &self,
        canvas: &Canvas,
        _theme: &SettingsTheme,
        win_w: f32,
        win_h: f32,
    ) {
        if self.widget_editor_mode == WidgetEditorMode::Compact {
            let Some(widget) = self.compact_widget_dragging else {
                return;
            };
            let width = 92.0;
            let height = 32.0;
            let (mouse_x, mouse_y) = self.logical_mouse_pos;
            let x = (mouse_x - width / 2.0).clamp(8.0, win_w - width - 8.0);
            let y = (mouse_y - height / 2.0).clamp(8.0, win_h - height - 8.0);
            let rect = Rect::from_xywh(x, y, width, height);
            let mut paint = settings_paint(Color::from_argb(90, 0, 0, 0));
            canvas.draw_round_rect(
                Rect::from_xywh(x, y + 4.0, width, height),
                height / 2.0,
                height / 2.0,
                &paint,
            );
            paint.set_color(Color::from_rgb(10, 10, 10));
            canvas.draw_round_rect(rect, height / 2.0, height / 2.0, &paint);
            crate::ui::widget::compact::draw_widget(canvas, widget, rect, 1.0, 255);
            return;
        }
        let Some(source) = self.widget_dragging.as_ref() else {
            return;
        };

        let (w, h) = self
            .widget_preview_item_y_cached()
            .map(|item_y| {
                let width = self.content_width();
                let geom = widget_grid_geom(
                    item_y,
                    width,
                    self.config.expanded_width,
                    self.config.expanded_height,
                );
                widget_source_span(source, &self.plugin_widgets)
                    .map(|span| {
                        let (_, _, w, h) = geom.footprint_rect(span, 0);
                        (w.max(60.0), h.max(48.0))
                    })
                    .unwrap_or((96.0, 72.0))
            })
            .unwrap_or((96.0, 96.0));

        let (mx, my) = self.logical_mouse_pos;
        let x = (mx - w / 2.0).clamp(8.0, win_w - w - 8.0);
        let y = (my - h / 2.0).clamp(8.0, win_h - h - 8.0);

        let shadow = settings_paint(Color::from_argb(90, 0, 0, 0));
        canvas.draw_round_rect(Rect::from_xywh(x, y + 4.0, w, h), 12.0, 12.0, &shadow);

        match source {
            WidgetSource::BuiltIn(widget) => draw_mini_card(canvas, *widget, x, y, w, h),
            WidgetSource::Plugin(id) => {
                if let Some(widget) = self
                    .plugin_widgets
                    .iter()
                    .find(|widget| widget.layout_id().as_ref() == Some(id))
                {
                    let span = widget.span();
                    let logical_width = (span.0 as f32 * 60.0).max(1.0);
                    let logical_height = (span.1 as f32 * 48.0).max(1.0);
                    let scale = (w / logical_width).min(h / logical_height).min(1.0);
                    draw_plugin_widget(canvas, widget, x, y, w, h, scale, 255);
                }
            }
        }
    }

    fn draw_page_navigation(&self, canvas: &Canvas, theme: &SettingsTheme) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(1.8);
        paint.set_stroke_cap(skia_safe::paint::Cap::Round);
        paint.set_stroke_join(skia_safe::paint::Join::Round);

        let back_center_x = PAGE_NAV_X + PAGE_NAV_SIZE / 2.0;
        let forward_center_x = back_center_x + PAGE_NAV_SIZE + PAGE_NAV_GAP;
        let center_y = PAGE_NAV_Y + PAGE_NAV_SIZE / 2.0;
        let (mouse_x, mouse_y) = self.logical_mouse_pos;

        for (x, enabled) in [
            (PAGE_NAV_X, self.can_navigate_back()),
            (
                PAGE_NAV_X + PAGE_NAV_SIZE + PAGE_NAV_GAP,
                self.can_navigate_forward(),
            ),
        ] {
            if enabled
                && Rect::from_xywh(x, PAGE_NAV_Y, PAGE_NAV_SIZE, PAGE_NAV_SIZE)
                    .contains(Point::new(mouse_x, mouse_y))
            {
                let hover = settings_paint(theme.sidebar_hover);
                canvas.draw_round_rect(
                    Rect::from_xywh(x, PAGE_NAV_Y, PAGE_NAV_SIZE, PAGE_NAV_SIZE),
                    7.0,
                    7.0,
                    &hover,
                );
            }
        }

        paint.set_color(if self.can_navigate_back() {
            theme.text_pri
        } else {
            theme.disabled
        });
        if let Some(path) = skia_safe::Path::from_svg(format!(
            "M {} {} L {} {} L {} {}",
            back_center_x + 2.5,
            center_y - 5.0,
            back_center_x - 2.5,
            center_y,
            back_center_x + 2.5,
            center_y + 5.0,
        )) {
            canvas.draw_path(&path, &paint);
        }

        paint.set_color(if self.can_navigate_forward() {
            theme.text_pri
        } else {
            theme.disabled
        });
        if let Some(path) = skia_safe::Path::from_svg(format!(
            "M {} {} L {} {} L {} {}",
            forward_center_x - 2.5,
            center_y - 5.0,
            forward_center_x + 2.5,
            center_y,
            forward_center_x - 2.5,
            center_y + 5.0,
        )) {
            canvas.draw_path(&path, &paint);
        }
    }

    fn draw_page_header(&self, canvas: &Canvas, theme: &SettingsTheme, win_w: f32) {
        let title = match self.active_page {
            0 => tr("tab_general"),
            1 => tr("tab_music"),
            2 => tr("tab_widgets"),
            3 => tr("tab_plugins"),
            _ => tr("tab_about"),
        };
        let mut paint = settings_paint(theme.separator);
        SettingsPainter::new(canvas).text(
            &title,
            (PAGE_NAV_X + PAGE_NAV_SIZE * 2.0 + PAGE_NAV_GAP + 14.0, 39.0),
            17.0,
            true,
            theme.text_pri,
        );

        paint.set_stroke_width(0.5);
        canvas.draw_line(
            (SIDEBAR_W, SETTINGS_HEADER_H - 0.5),
            (win_w, SETTINGS_HEADER_H - 0.5),
            &paint,
        );
    }

    fn draw_widget_mode_control(&self, canvas: &Canvas, theme: &SettingsTheme) {
        if self.active_page != WIDGETS_PAGE_INDEX {
            return;
        }
        let control = self.widget_mode_control_rect();
        let mut paint = settings_paint(theme.control_bg);
        canvas.draw_round_rect(control, 8.0, 8.0, &paint);
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(0.75);
        paint.set_color(theme.control_border);
        canvas.draw_round_rect(control, 8.0, 8.0, &paint);

        let selected = self.widget_mode_segment_rect(self.widget_editor_mode);
        paint.set_style(skia_safe::paint::Style::Fill);
        paint.set_color(theme.card_highlight);
        canvas.draw_round_rect(
            Rect::from_xywh(
                selected.left + 2.0,
                selected.top + 2.0,
                selected.width() - 4.0,
                selected.height() - 4.0,
            ),
            6.0,
            6.0,
            &paint,
        );

        for (mode, label) in [
            (WidgetEditorMode::Expanded, tr("widget_mode_expanded")),
            (WidgetEditorMode::Compact, tr("widget_mode_compact")),
        ] {
            let rect = self.widget_mode_segment_rect(mode);
            let hovered = self.focused
                && rect.contains(Point::new(
                    self.logical_mouse_pos.0,
                    self.logical_mouse_pos.1,
                ));
            let color = if mode == self.widget_editor_mode || hovered {
                theme.text_pri
            } else {
                theme.text_sec
            };
            paint.set_color(color);
            let size = 11.5;
            let width = FontManager::global().measure_text_cached(
                &label,
                size,
                skia_safe::FontStyle::normal(),
            );
            SettingsPainter::new(canvas).text(
                &label,
                (rect.center_x() - width / 2.0, rect.center_y() + size * 0.35),
                size,
                mode == self.widget_editor_mode,
                color,
            );
        }
    }

    pub(crate) fn draw_popup(&self, canvas: &Canvas, theme: &SettingsTheme) {
        let popup = match &self.popup {
            Some(p) => p,
            None => return,
        };
        let opacity = self.anim.get(POPUP_OPACITY_KEY);
        if opacity < 0.005 {
            return;
        }
        let menu = popup.menu_rect();

        let shadow = settings_paint(Color::from_argb((60.0 * opacity) as u8, 0, 0, 0));
        canvas.draw_round_rect(
            Rect::from_xywh(
                menu.left - 1.0,
                menu.top + 2.0,
                menu.width() + 2.0,
                menu.height() + 2.0,
            ),
            POPUP_MENU_R,
            POPUP_MENU_R,
            &shadow,
        );

        let mut paint = settings_paint(Color::from_argb(
            (255.0 * opacity) as u8,
            theme.popup_bg.r(),
            theme.popup_bg.g(),
            theme.popup_bg.b(),
        ));
        canvas.draw_round_rect(menu, POPUP_MENU_R, POPUP_MENU_R, &paint);

        let mut border = settings_paint(Color::from_argb(
            (40.0 * opacity) as u8,
            theme.popup_border.r(),
            theme.popup_border.g(),
            theme.popup_border.b(),
        ));
        border.set_style(skia_safe::paint::Style::Stroke);
        border.set_stroke_width(0.5);
        canvas.draw_round_rect(menu, POPUP_MENU_R, POPUP_MENU_R, &border);

        let text_alpha = (255.0 * opacity) as u8;
        for (i, opt_label) in popup.options.iter().enumerate() {
            let item_rect = popup.item_rect(i);

            if popup.hover_idx == Some(i) {
                let a = theme.accent.a() as f32 * opacity;
                paint.set_color(Color::from_argb(
                    a as u8,
                    theme.accent.r(),
                    theme.accent.g(),
                    theme.accent.b(),
                ));
                paint.set_style(skia_safe::paint::Style::Fill);
                canvas.draw_round_rect(item_rect, 4.0, 4.0, &paint);
            }

            let text_color = Color::from_argb(
                text_alpha,
                theme.text_pri.r(),
                theme.text_pri.g(),
                theme.text_pri.b(),
            );
            paint.set_style(skia_safe::paint::Style::Fill);
            SettingsPainter::new(canvas).text(
                opt_label,
                (item_rect.left + 8.0, item_rect.top + 19.0),
                12.0,
                false,
                text_color,
            );

            if i == popup.selected_idx {
                let check_base = if popup.hover_idx == Some(i) {
                    theme.text_pri
                } else {
                    theme.accent
                };
                paint.set_color(Color::from_argb(
                    text_alpha,
                    check_base.r(),
                    check_base.g(),
                    check_base.b(),
                ));
                paint.set_style(skia_safe::paint::Style::Stroke);
                paint.set_stroke_width(2.0);
                let cx = item_rect.right - 14.0;
                let cy = item_rect.top + POPUP_ITEM_H / 2.0;
                let svg = format!(
                    "M {} {} L {} {} L {} {}",
                    cx - 4.0,
                    cy,
                    cx - 1.0,
                    cy + 3.0,
                    cx + 4.0,
                    cy - 3.0,
                );
                if let Some(path) = skia_safe::Path::from_svg(&svg) {
                    canvas.draw_path(&path, &paint);
                }
                paint.set_style(skia_safe::paint::Style::Fill);
            }

            if i < popup.options.len() - 1 {
                let mut sep = settings_paint(Color::from_argb(
                    (30.0 * opacity) as u8,
                    theme.separator.r(),
                    theme.separator.g(),
                    theme.separator.b(),
                ));
                sep.set_stroke_width(0.5);
                sep.set_style(skia_safe::paint::Style::Stroke);
                canvas.draw_line(
                    (item_rect.left, item_rect.bottom),
                    (item_rect.right, item_rect.bottom),
                    &sep,
                );
            }
        }
    }
}
