use crate::core::i18n::tr;
use crate::ui::expanded::widget_view::draw_plugin_widget;
use crate::ui::widget::expanded::draw_mini_card;
use crate::utils::color::SettingsTheme;
use crate::utils::font::{DrawTextCachedParams, FontManager};
use crate::utils::settings_ui::items::{POPUP_ITEM_H, SettingsItem};
use crate::utils::settings_ui::{
    ActiveStepperValue, DrawItemsParams, SettingsPainter, WidgetSource, draw_items, ellipsize_text,
    settings_paint, widget_grid_geom, widget_source_span,
};
use crate::window::renderer::Renderer;
use skia_safe::{Canvas, Color, Contains, Paint, Point, RRect, Rect};

use super::{
    PAGE_NAV_GAP, PAGE_NAV_HEIGHT, PAGE_NAV_WIDTH, PAGE_NAV_X, PAGE_NAV_Y, PLUGINS_PAGE_INDEX,
    POPUP_MENU_R, POPUP_OPACITY_KEY, SETTINGS_HEADER_H, SIDEBAR_W, SettingsApp, WIDGETS_PAGE_INDEX,
    WINDOW_RADIUS, WidgetEditorMode,
};

impl SettingsApp {
    fn draw_music_notice(&self, canvas: &Canvas, width: f32) {
        use super::pages::music::music_notice_button_rect;

        let top = SETTINGS_HEADER_H;
        let card = Rect::from_xywh(24.0, top + 8.0, width - 48.0, 134.0);
        canvas.draw_round_rect(
            card,
            11.0,
            11.0,
            &settings_paint(Color::from_argb(42, 44, 132, 245)),
        );
        let mut outline = settings_paint(Color::from_rgb(42, 132, 238));
        outline.set_style(skia_safe::paint::Style::Stroke);
        outline.set_stroke_width(1.5);
        canvas.draw_round_rect(card, 11.0, 11.0, &outline);
        let text = settings_paint(if self.is_light {
            Color::from_rgb(23, 65, 122)
        } else {
            Color::WHITE
        });
        let font = FontManager::global();
        font.draw_text_cached(DrawTextCachedParams {
            canvas,
            text: "注意",
            x: 38.0,
            y: top + 32.0,
            size: 14.0,
            bold: true,
            paint: &text,
        });
        for (line, y) in [
            (
                "若您使用网易云音乐，请在设置中开启 SMTC 以使用此功能。",
                top + 56.0,
            ),
            ("由于网易云适配问题，可能会出现 BUG。", top + 77.0),
            (
                "若无法忍受，请安装 BetterNCM 及插件以体验完整 SMTC 功能。",
                top + 98.0,
            ),
        ] {
            font.draw_text_cached(DrawTextCachedParams {
                canvas,
                text: line,
                x: 38.0,
                y,
                size: 12.0,
                bold: false,
                paint: &text,
            });
        }
        let button = music_notice_button_rect(width, top);
        let color = if self.music_notice_pressed {
            Color::from_rgb(18, 77, 163)
        } else if self.music_notice_button_hovered() {
            Color::from_rgb(33, 113, 218)
        } else {
            Color::from_rgb(42, 132, 238)
        };
        canvas.draw_round_rect(button, 7.0, 7.0, &settings_paint(color));
        font.draw_text_cached(DrawTextCachedParams {
            canvas,
            text: "我已知晓",
            x: button.center_x()
                - font.measure_text_cached("我已知晓", 12.0, skia_safe::FontStyle::bold()) / 2.0,
            y: button.top + 18.0,
            size: 12.0,
            bold: true,
            paint: &settings_paint(Color::WHITE),
        });
    }

    pub(crate) fn draw(&mut self, renderer: &mut Renderer) {
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
        let render_result = renderer.draw(target, |drawing_context, sk_surface| {
            let canvas = sk_surface.canvas();
            canvas.scale((scale, scale));

            let win_rect = Rect::from_xywh(0.0, 0.0, win_w, win_h);
            let win_rrect = skia_safe::RRect::new_rect_xy(win_rect, WINDOW_RADIUS, WINDOW_RADIUS);

            canvas.save();
            canvas.clip_rrect(win_rrect, skia_safe::ClipOp::Intersect, true);

            let bg_paint = settings_paint(theme.win_bg);
            canvas.draw_rect(win_rect, &bg_paint);

            self.draw_sidebar(drawing_context, canvas, &theme);
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
                widget_hover: self.widget_hover_visual.as_ref(),
                widget_hover_progress: self.widget_hover_progress,
                widget_drop_animation: self.widget_drop_animation.as_ref(),
                active_source_button,
                active_stepper_value,
                hover_pos: Some((
                    self.logical_mouse_pos.0 - SIDEBAR_W,
                    self.logical_mouse_pos.1 + self.scroll_y,
                )),
            });
            if self.active_page == 1 && self.show_music_notice() {
                self.draw_music_notice(canvas, content_w);
            }
            canvas.restore();

            if let Some(scrollbar) = self.scrollbar_geometry() {
                let p = settings_paint(theme.scrollbar);
                canvas.draw_round_rect(
                    Rect::from_xywh(scrollbar.x, scrollbar.y, scrollbar.width, scrollbar.height),
                    scrollbar.width / 2.0,
                    scrollbar.width / 2.0,
                    &p,
                );
            }

            if self.active_page == PLUGINS_PAGE_INDEX {
                self.draw_plugins_page(drawing_context, canvas, &theme, win_w, win_h);
            }

            self.draw_widget_drag_overlay(canvas, win_w, win_h);
            self.draw_resource_editor(canvas, &theme, win_w, win_h);
            self.draw_popup(canvas, &theme);
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
            log::error!("Settings rendering failed: {error}");
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

    fn draw_widget_drag_overlay(&self, canvas: &Canvas, win_w: f32, win_h: f32) {
        let lift = self.widget_drag_lift_progress.clamp(0.0, 1.0);
        let lift_scale = 0.94 + 0.06 * (1.0 - (1.0 - lift).powi(3));
        if self.widget_editor_mode == WidgetEditorMode::Compact {
            let Some(widget) = self.compact_widget_dragging else {
                return;
            };
            let width = 92.0 * lift_scale;
            let height = 32.0 * lift_scale;
            let (mouse_x, mouse_y) = self.logical_mouse_pos;
            let x = (mouse_x - width / 2.0).clamp(8.0, win_w - width - 8.0);
            let y = (mouse_y - height / 2.0 - 5.0 * lift).clamp(8.0, win_h - height - 8.0);
            let rect = Rect::from_xywh(x, y, width, height);
            let mut paint = settings_paint(Color::from_argb((55.0 + 65.0 * lift) as u8, 0, 0, 0));
            canvas.draw_round_rect(
                Rect::from_xywh(x, y + 2.0 + 5.0 * lift, width, height),
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

        let (base_w, base_h) = self
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
        let w = base_w * lift_scale;
        let h = base_h * lift_scale;

        let (mx, my) = self.logical_mouse_pos;
        let x = (mx - w / 2.0).clamp(8.0, win_w - w - 8.0);
        let y = (my - h / 2.0 - 6.0 * lift).clamp(8.0, win_h - h - 8.0);

        let shadow = settings_paint(Color::from_argb((55.0 + 65.0 * lift) as u8, 0, 0, 0));
        canvas.draw_round_rect(
            Rect::from_xywh(x, y + 2.0 + 5.0 * lift, w, h),
            12.0,
            12.0,
            &shadow,
        );

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

        let back_center_x = PAGE_NAV_X + PAGE_NAV_WIDTH / 2.0;
        let forward_center_x = back_center_x + PAGE_NAV_WIDTH + PAGE_NAV_GAP;
        let center_y = PAGE_NAV_Y + PAGE_NAV_HEIGHT / 2.0;
        let (mouse_x, mouse_y) = self.logical_mouse_pos;

        for (x, enabled, is_back) in [
            (PAGE_NAV_X, self.can_navigate_back(), true),
            (
                PAGE_NAV_X + PAGE_NAV_WIDTH + PAGE_NAV_GAP,
                self.can_navigate_forward(),
                false,
            ),
        ] {
            let rect = Rect::from_xywh(x, PAGE_NAV_Y, PAGE_NAV_WIDTH, PAGE_NAV_HEIGHT);
            let outer_radius = PAGE_NAV_HEIGHT / 2.0;
            let inner_radius = 3.0;
            let radii = if is_back {
                [
                    Point::new(outer_radius, outer_radius),
                    Point::new(inner_radius, inner_radius),
                    Point::new(inner_radius, inner_radius),
                    Point::new(outer_radius, outer_radius),
                ]
            } else {
                [
                    Point::new(inner_radius, inner_radius),
                    Point::new(outer_radius, outer_radius),
                    Point::new(outer_radius, outer_radius),
                    Point::new(inner_radius, inner_radius),
                ]
            };
            let shape = RRect::new_rect_radii(rect, &radii);
            let hovered = enabled && rect.contains(Point::new(mouse_x, mouse_y));
            let fill = if !enabled {
                theme.control_disabled
            } else if hovered {
                theme.control_hover
            } else {
                theme.control_bg
            };
            canvas.draw_rrect(shape, &settings_paint(fill));

            let mut border = settings_paint(theme.control_border);
            border.set_style(skia_safe::paint::Style::Stroke);
            border.set_stroke_width(0.75);
            canvas.draw_rrect(shape, &border);
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
        let title = self.page_title();
        let title_x = PAGE_NAV_X + PAGE_NAV_WIDTH * 2.0 + PAGE_NAV_GAP + 14.0;
        let title = ellipsize_text(
            FontManager::global(),
            &title,
            17.0,
            skia_safe::FontStyle::bold(),
            (win_w - title_x - 20.0).max(0.0),
        );
        let mut paint = settings_paint(theme.separator);
        SettingsPainter::new(canvas).text(&title, (title_x, 39.0), 17.0, true, theme.text_pri);

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

        let shadow = settings_paint(Color::from_argb(
            (theme.popup_shadow.a() as f32 * opacity) as u8,
            theme.popup_shadow.r(),
            theme.popup_shadow.g(),
            theme.popup_shadow.b(),
        ));
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
            (theme.popup_border.a() as f32 * opacity) as u8,
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
                let a = theme.selection_bg.a() as f32 * opacity;
                paint.set_color(Color::from_argb(
                    a as u8,
                    theme.selection_bg.r(),
                    theme.selection_bg.g(),
                    theme.selection_bg.b(),
                ));
                paint.set_style(skia_safe::paint::Style::Fill);
                canvas.draw_round_rect(item_rect, 4.0, 4.0, &paint);
            }

            let text_base = if popup.hover_idx == Some(i) {
                theme.selection_text
            } else {
                theme.text_pri
            };
            let text_color =
                Color::from_argb(text_alpha, text_base.r(), text_base.g(), text_base.b());
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
                    theme.selection_text
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
                    (theme.popup_separator.a() as f32 * opacity) as u8,
                    theme.popup_separator.r(),
                    theme.popup_separator.g(),
                    theme.popup_separator.b(),
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
