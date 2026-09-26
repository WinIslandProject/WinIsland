use crate::ui::expanded::widget_view::draw_plugin_widget;
use crate::ui::widget::expanded::draw_mini_card;
use crate::utils::color::SettingsTheme;
use crate::utils::settings_ui::items::{POPUP_ITEM_H, SettingsItem};
use crate::utils::settings_ui::{
    ActiveStepperValue, DrawItemsParams, SettingsPainter, WidgetSource, draw_items, ellipsize_text,
    settings_color, widget_grid_geom, widget_source_span,
};
use winisland_core::i18n::tr;
use winisland_render::Renderer;
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{Painter, Path, Point, Radius, Rect, Rgba, StrokeCap, StrokeJoin, Vec2};

use super::{
    PAGE_NAV_GAP, PAGE_NAV_HEIGHT, PAGE_NAV_WIDTH, PAGE_NAV_X, PAGE_NAV_Y, PLUGINS_PAGE_INDEX,
    POPUP_MENU_R, POPUP_OPACITY_KEY, SETTINGS_HEADER_H, SIDEBAR_W, SettingsApp, WIDGETS_PAGE_INDEX,
    WINDOW_RADIUS, WidgetEditorMode,
};

impl SettingsApp {
    fn draw_music_notice(&self, painter: Painter<'_>, width: f32) {
        use super::pages::music::music_notice_button_rect;

        let top = SETTINGS_HEADER_H;
        let card = Rect::from_xywh(24.0, top + 8.0, width - 48.0, 134.0);
        painter.fill_round_rect(
            card,
            Radius::uniform(11.0),
            Rgba::from_argb(42, 44, 132, 245),
        );
        painter.stroke_round_rect(
            card,
            Radius::uniform(11.0),
            1.5,
            Rgba::from_rgb(42, 132, 238),
        );
        let text_color = if self.is_light {
            Rgba::from_rgb(23, 65, 122)
        } else {
            Rgba::WHITE
        };
        let font = FontManager::global();
        font.draw_text_cached(DrawTextCachedParams {
            painter,
            text: "注意",
            x: 38.0,
            y: top + 32.0,
            size: 14.0,
            bold: true,
            color: text_color,
            blur: None,
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
                painter,
                text: line,
                x: 38.0,
                y,
                size: 12.0,
                bold: false,
                color: text_color,
                blur: None,
            });
        }
        let button = music_notice_button_rect(width, top);
        let color = if self.music_notice_pressed {
            Rgba::from_rgb(18, 77, 163)
        } else if self.music_notice_button_hovered() {
            Rgba::from_rgb(33, 113, 218)
        } else {
            Rgba::from_rgb(42, 132, 238)
        };
        painter.fill_round_rect(button, Radius::uniform(7.0), color);
        font.draw_text_cached(DrawTextCachedParams {
            painter,
            text: "我已知晓",
            x: button.center_x()
                - font.measure_text_cached("我已知晓", 12.0, winisland_render::FontStyle::bold())
                    / 2.0,
            y: button.top + 18.0,
            size: 12.0,
            bold: true,
            color: Rgba::WHITE,
            blur: None,
        });
    }

    pub(crate) fn draw(&mut self, renderer: &mut Renderer) {
        let Some(win) = self.window.as_ref() else {
            return;
        };
        if win.is_minimized() == Some(true) {
            return;
        }
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
        let render_result = renderer.frame(target, |drawing_context, painter| {
            painter.scale(Vec2::new(scale, scale));

            let win_rect = Rect::from_xywh(0.0, 0.0, win_w, win_h);
            painter.save();
            painter.clip_round_rect(win_rect, Radius::uniform(WINDOW_RADIUS));
            painter.fill_rect(win_rect, settings_color(theme.win_bg));

            self.draw_sidebar(drawing_context, painter, &theme);
            self.draw_page_navigation(painter, &theme);
            self.draw_page_header(painter, &theme, win_w);
            self.draw_widget_mode_control(painter, &theme);

            let content_w = win_w - SIDEBAR_W;

            let content_start_y = SETTINGS_HEADER_H;

            self.target_scroll_y = self.target_scroll_y.clamp(0.0, self.cached_max_scroll);

            let clip_start_y = SETTINGS_HEADER_H;

            painter.save();
            painter.clip_rect(Rect::from_xywh(
                SIDEBAR_W,
                clip_start_y,
                content_w,
                win_h - clip_start_y,
            ));
            painter.translate(Vec2::new(SIDEBAR_W, -self.scroll_y));
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
                painter,
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
                self.draw_music_notice(painter, content_w);
            }
            painter.restore();

            if let Some(scrollbar) = self.scrollbar_geometry() {
                painter.fill_round_rect(
                    Rect::from_xywh(scrollbar.x, scrollbar.y, scrollbar.width, scrollbar.height),
                    Radius::uniform(scrollbar.width / 2.0),
                    settings_color(theme.scrollbar),
                );
            }

            if self.active_page == PLUGINS_PAGE_INDEX {
                self.draw_plugins_page(drawing_context, painter, &theme, win_w, win_h);
            }

            self.draw_widget_drag_overlay(painter, win_w, win_h);
            self.draw_resource_editor(painter, &theme, win_w, win_h);
            self.draw_popup(painter, &theme);
            painter.restore();

            let border_rect = Rect::from_xywh(0.5, 0.5, win_w - 1.0, win_h - 1.0);
            let border_radius = WINDOW_RADIUS - 0.5;
            painter.stroke_round_rect(
                border_rect,
                Radius::uniform(border_radius),
                1.0,
                settings_color(theme.separator),
            );
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

    fn draw_widget_drag_overlay(&self, painter: Painter<'_>, win_w: f32, win_h: f32) {
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
            painter.fill_round_rect(
                Rect::from_xywh(x, y + 2.0 + 5.0 * lift, width, height),
                Radius::uniform(height / 2.0),
                Rgba::from_argb((55.0 + 65.0 * lift) as u8, 0, 0, 0),
            );
            painter.fill_round_rect(
                Rect::from_xywh(x, y, width, height),
                Radius::uniform(height / 2.0),
                Rgba::from_rgb(10, 10, 10),
            );
            crate::ui::widget::compact::draw_widget(
                painter,
                widget,
                Rect::from_xywh(x, y, width, height),
                1.0,
                255,
            );
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

        let shadow = Rgba::from_argb((55.0 + 65.0 * lift) as u8, 0, 0, 0);
        painter.fill_round_rect(
            Rect::from_xywh(x, y + 2.0 + 5.0 * lift, w, h),
            Radius::uniform(12.0),
            shadow,
        );

        match source {
            WidgetSource::BuiltIn(widget) => draw_mini_card(painter, *widget, x, y, w, h),
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
                    draw_plugin_widget(painter, widget, x, y, w, h, scale, 255);
                }
            }
        }
    }

    fn draw_page_navigation(&self, painter: Painter<'_>, theme: &SettingsTheme) {
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
            let (outer_corner, inner_corner) = (
                Vec2::new(outer_radius, outer_radius),
                Vec2::new(inner_radius, inner_radius),
            );
            let (first, second) = if is_back {
                (outer_corner, inner_corner)
            } else {
                (inner_corner, outer_corner)
            };
            let radius = Radius {
                top_left: first,
                top_right: second,
                bottom_right: second,
                bottom_left: first,
            };
            let hovered = enabled && rect.contains(Point::new(mouse_x, mouse_y));
            let fill = if !enabled {
                theme.control_disabled
            } else if hovered {
                theme.control_hover
            } else {
                theme.control_bg
            };
            painter.fill_round_rect(rect, radius, settings_color(fill));
            painter.stroke_round_rect(rect, radius, 0.75, settings_color(theme.control_border));
        }

        let back_color = if self.can_navigate_back() {
            theme.text_pri
        } else {
            theme.disabled
        };
        if let Some(path) = Path::from_svg(&format!(
            "M {} {} L {} {} L {} {}",
            back_center_x + 2.5,
            center_y - 5.0,
            back_center_x - 2.5,
            center_y,
            back_center_x + 2.5,
            center_y + 5.0,
        )) {
            painter.stroke_path(
                &path,
                1.8,
                settings_color(back_color),
                StrokeCap::Round,
                StrokeJoin::Round,
            );
        }

        let forward_color = if self.can_navigate_forward() {
            theme.text_pri
        } else {
            theme.disabled
        };
        if let Some(path) = Path::from_svg(&format!(
            "M {} {} L {} {} L {} {}",
            forward_center_x - 2.5,
            center_y - 5.0,
            forward_center_x + 2.5,
            center_y,
            forward_center_x - 2.5,
            center_y + 5.0,
        )) {
            painter.stroke_path(
                &path,
                1.8,
                settings_color(forward_color),
                StrokeCap::Round,
                StrokeJoin::Round,
            );
        }
    }

    fn draw_page_header(&self, painter: Painter<'_>, theme: &SettingsTheme, win_w: f32) {
        let title = self.page_title();
        let title_x = PAGE_NAV_X + PAGE_NAV_WIDTH * 2.0 + PAGE_NAV_GAP + 14.0;
        let title = ellipsize_text(
            FontManager::global(),
            &title,
            17.0,
            winisland_render::FontStyle::bold(),
            (win_w - title_x - 20.0).max(0.0),
        );
        let paint = settings_color(theme.separator);
        SettingsPainter::new(painter).text(
            &title,
            (title_x, 39.0),
            17.0,
            true,
            settings_color(theme.text_pri),
        );

        painter.stroke_line(
            Point::new(SIDEBAR_W, SETTINGS_HEADER_H - 0.5),
            Point::new(win_w, SETTINGS_HEADER_H - 0.5),
            0.5,
            paint,
            StrokeCap::Butt,
        );
    }

    fn draw_widget_mode_control(&self, painter: Painter<'_>, theme: &SettingsTheme) {
        if self.active_page != WIDGETS_PAGE_INDEX {
            return;
        }
        let control = self.widget_mode_control_rect();
        painter.fill_round_rect(
            control,
            Radius::uniform(8.0),
            settings_color(theme.control_bg),
        );
        painter.stroke_round_rect(
            control,
            Radius::uniform(8.0),
            0.75,
            settings_color(theme.control_border),
        );

        let selected = self.widget_mode_segment_rect(self.widget_editor_mode);
        painter.fill_round_rect(
            Rect::from_xywh(
                selected.left + 2.0,
                selected.top + 2.0,
                selected.width() - 4.0,
                selected.height() - 4.0,
            ),
            Radius::uniform(6.0),
            settings_color(theme.card_highlight),
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
            let size = 11.5;
            let width = FontManager::global().measure_text_cached(
                &label,
                size,
                winisland_render::FontStyle::normal(),
            );
            SettingsPainter::new(painter).text(
                &label,
                (rect.center_x() - width / 2.0, rect.center_y() + size * 0.35),
                size,
                mode == self.widget_editor_mode,
                settings_color(color),
            );
        }
    }

    pub(crate) fn draw_popup(&self, painter: Painter<'_>, theme: &SettingsTheme) {
        let popup = match &self.popup {
            Some(p) => p,
            None => return,
        };
        let opacity = self.anim.get(POPUP_OPACITY_KEY);
        if opacity < 0.005 {
            return;
        }
        let menu = popup.menu_rect();
        let menu_rect = menu;

        painter.fill_round_rect(
            Rect::from_xywh(
                menu.left - 1.0,
                menu.top + 2.0,
                menu.width() + 2.0,
                menu.height() + 2.0,
            ),
            Radius::uniform(POPUP_MENU_R),
            settings_color(Rgba::from_argb(
                (theme.popup_shadow.a() as f32 * opacity) as u8,
                theme.popup_shadow.r(),
                theme.popup_shadow.g(),
                theme.popup_shadow.b(),
            )),
        );

        painter.fill_round_rect(
            menu_rect,
            Radius::uniform(POPUP_MENU_R),
            settings_color(Rgba::from_argb(
                (255.0 * opacity) as u8,
                theme.popup_bg.r(),
                theme.popup_bg.g(),
                theme.popup_bg.b(),
            )),
        );

        painter.stroke_round_rect(
            menu_rect,
            Radius::uniform(POPUP_MENU_R),
            0.5,
            settings_color(Rgba::from_argb(
                (theme.popup_border.a() as f32 * opacity) as u8,
                theme.popup_border.r(),
                theme.popup_border.g(),
                theme.popup_border.b(),
            )),
        );

        let text_alpha = (255.0 * opacity) as u8;
        for (i, opt_label) in popup.options.iter().enumerate() {
            let item_rect = popup.item_rect(i);

            if popup.hover_idx == Some(i) {
                let a = theme.selection_bg.a() as f32 * opacity;
                painter.fill_round_rect(
                    item_rect,
                    Radius::uniform(4.0),
                    settings_color(Rgba::from_argb(
                        a as u8,
                        theme.selection_bg.r(),
                        theme.selection_bg.g(),
                        theme.selection_bg.b(),
                    )),
                );
            }

            let text_base = if popup.hover_idx == Some(i) {
                theme.selection_text
            } else {
                theme.text_pri
            };
            let text_color =
                Rgba::from_argb(text_alpha, text_base.r(), text_base.g(), text_base.b());
            SettingsPainter::new(painter).text(
                opt_label,
                (item_rect.left + 8.0, item_rect.top + 19.0),
                12.0,
                false,
                settings_color(text_color),
            );

            if i == popup.selected_idx {
                let check_base = if popup.hover_idx == Some(i) {
                    theme.selection_text
                } else {
                    theme.accent
                };
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
                if let Some(path) = winisland_render::Path::from_svg(&svg) {
                    painter.stroke_path(
                        &path,
                        2.0,
                        settings_color(Rgba::from_argb(
                            text_alpha,
                            check_base.r(),
                            check_base.g(),
                            check_base.b(),
                        )),
                        StrokeCap::Butt,
                        StrokeJoin::Miter,
                    );
                }
            }

            if i < popup.options.len() - 1 {
                painter.stroke_line(
                    Point::new(item_rect.left, item_rect.bottom),
                    Point::new(item_rect.right, item_rect.bottom),
                    0.5,
                    settings_color(Rgba::from_argb(
                        (theme.popup_separator.a() as f32 * opacity) as u8,
                        theme.popup_separator.r(),
                        theme.popup_separator.g(),
                        theme.popup_separator.b(),
                    )),
                    StrokeCap::Butt,
                );
            }
        }
    }
}
