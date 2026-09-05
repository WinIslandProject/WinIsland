use winit::event::ElementState;
use winit::event_loop::ActiveEventLoop;

use crate::core::config::{MIN_HIDDEN_WIDTH, WidgetKind};
use crate::ui::expanded::music_view::{
    get_next_btn_rect, get_pause_btn_rect, get_prev_btn_rect, get_progress_bar_rect,
    trigger_cover_flip, trigger_next_click, trigger_pause_click, trigger_prev_click,
};
use crate::ui::widget::expanded::widget_grid_layout;
use crate::utils::mouse::{is_point_in_g3_rounded_rect, is_point_in_rect};

use super::{App, IslandLayout, should_show_widget_view};

impl App {
    pub(super) fn handle_input(
        &mut self,
        event_loop: &ActiveEventLoop,
        state: ElementState,
        px: i32,
        py: i32,
    ) {
        if self.is_cursor_suppressed {
            return;
        }
        let rel_x = px - self.geom.win_x;
        let rel_y = py - self.geom.win_y;
        let layout = self.compute_island_layout();

        if state == ElementState::Pressed {
            self.handle_press(event_loop, rel_x, rel_y, &layout);
        } else if state == ElementState::Released {
            self.handle_release(py);
        }
    }

    pub(super) fn handle_right_input(&mut self, state: ElementState, px: i32, py: i32) {
        if !self.config.right_click_drag || self.expanded || self.is_cursor_suppressed {
            return;
        }
        match state {
            ElementState::Pressed => {
                let rel_x = px - self.geom.win_x;
                let rel_y = py - self.geom.win_y;
                let layout = self.compute_island_layout();
                let is_hovering = is_point_in_g3_rounded_rect(
                    rel_x as f64,
                    rel_y as f64,
                    layout.current_island_x,
                    layout.current_island_y,
                    self.springs.w.value as f64,
                    self.springs.h.value as f64,
                    self.springs.r.value as f64,
                );
                if is_hovering {
                    self.right_press_cursor = Some((px, py));
                    self.right_drag_start_offset = Some((
                        self.config.position_x_offset + self.geom.win_x - self.geom.configured_x,
                        self.config.position_y_offset + self.geom.win_y - self.geom.configured_y,
                    ));
                }
            }
            ElementState::Released => {
                if self.is_right_dragging {
                    self.is_right_dragging = false;
                    crate::core::persistence::save_config(&self.config);
                    log::info!(
                        "Right click drag offsets saved: ({}, {})",
                        self.config.position_x_offset,
                        self.config.position_y_offset
                    );
                }
                self.right_press_cursor = None;
                self.right_drag_start_offset = None;
            }
        }
    }

    pub(super) fn handle_press(
        &mut self,
        event_loop: &ActiveEventLoop,
        rel_x: i32,
        rel_y: i32,
        layout: &IslandLayout,
    ) {
        let island_y = layout.island_y;
        let offset_x = layout.offset_x;
        let current_island_x = layout.current_island_x;
        let current_island_y = layout.current_island_y;
        let is_hovering_visible = is_point_in_g3_rounded_rect(
            rel_x as f64,
            rel_y as f64,
            current_island_x,
            current_island_y,
            self.springs.w.value as f64,
            self.springs.h.value as f64,
            self.springs.r.value as f64,
        );
        let is_on_hidden_reveal = self.is_hidden()
            && self.config.hidden_width <= MIN_HIDDEN_WIDTH
            && self.springs.hide.value >= 0.999
            && is_point_in_rect(
                rel_x as f64,
                rel_y as f64,
                layout.hidden_reveal_x,
                layout.hidden_reveal_y,
                layout.hidden_reveal_w,
                layout.hidden_reveal_h,
            );

        if !self.expanded && self.compact_overlay.is_notification_visible() && is_hovering_visible {
            self.dismissing_notification = true;
            self.is_dragging = true;
            self.drag_start_px = rel_x + self.geom.win_x;
            self.drag_start_py = rel_y + self.geom.win_y;
            self.drag_has_moved = false;
            return;
        }

        if self.expanded {
            let music_page_available = self.music_page_available;
            let view_val = self.springs.view.value as f64;
            let w = self.springs.w.value as f64;
            let h = self.springs.h.value as f64;
            let page_shift = view_val * w;
            let scale = self.config.global_scale as f64;

            if view_val < 0.5 {
                let media = self.current_media_info().clone();
                let music_on = !media.title.is_empty()
                    && (self.plugin_media_source.is_some() || self.config.smtc_enabled);

                let (bx, by, bw, bh) = get_pause_btn_rect(
                    offset_x as f32,
                    island_y as f32,
                    w as f32,
                    h as f32,
                    self.config.global_scale,
                    &self.config.expanded_cover_shape,
                );
                let cx = rel_x as f32 - (page_shift as f32);
                let cy = rel_y as f32;
                if music_on
                    && self.media_control_available(crate::plugin::types::MEDIA_CONTROL_TOGGLE_PLAY)
                    && cx >= bx
                    && cx <= bx + bw
                    && cy >= by
                    && cy <= by + bh
                {
                    trigger_pause_click(media.is_playing);
                    self.dispatch_media_command(crate::plugin::types::MEDIA_COMMAND_TOGGLE_PLAY, 0);
                    return;
                }

                let (px, py, pw, ph) = get_prev_btn_rect(
                    offset_x as f32,
                    island_y as f32,
                    w as f32,
                    h as f32,
                    self.config.global_scale,
                    &self.config.expanded_cover_shape,
                );
                if music_on
                    && self.media_control_available(crate::plugin::types::MEDIA_CONTROL_PREVIOUS)
                    && cx >= px
                    && cx <= px + pw
                    && cy >= py
                    && cy <= py + ph
                {
                    trigger_cover_flip();
                    trigger_prev_click();
                    self.dispatch_media_command(crate::plugin::types::MEDIA_COMMAND_PREVIOUS, 0);
                    return;
                }

                let (nx, ny, nw, nh) = get_next_btn_rect(
                    offset_x as f32,
                    island_y as f32,
                    w as f32,
                    h as f32,
                    self.config.global_scale,
                    &self.config.expanded_cover_shape,
                );
                if music_on
                    && self.media_control_available(crate::plugin::types::MEDIA_CONTROL_NEXT)
                    && cx >= nx
                    && cx <= nx + nw
                    && cy >= ny
                    && cy <= ny + nh
                {
                    trigger_cover_flip();
                    trigger_next_click();
                    self.dispatch_media_command(crate::plugin::types::MEDIA_COMMAND_NEXT, 0);
                    return;
                }

                if let Some((bar_left, bar_right, bar_top, bar_hit_h)) = get_progress_bar_rect(
                    offset_x as f32,
                    island_y as f32,
                    w as f32,
                    &media,
                    music_on,
                    self.config.global_scale,
                    &self.config.expanded_cover_shape,
                ) && self.media_control_available(crate::plugin::types::MEDIA_CONTROL_SEEK)
                    && cx >= bar_left
                    && cx <= bar_right
                    && cy >= bar_top
                    && cy <= bar_top + bar_hit_h
                {
                    let ratio = ((cx - bar_left) / (bar_right - bar_left)).clamp(0.0, 1.0);
                    let duration_ms = media.effective_duration_ms();
                    let seek_ms = (ratio as f64 * duration_ms as f64) as u64;
                    self.seek.begin(
                        bar_left,
                        bar_right,
                        duration_ms,
                        seek_ms,
                        self.plugin_media_source
                            .as_ref()
                            .map(|source| source.resource_id),
                    );
                    return;
                }
            }

            if view_val > 0.5 {
                let settings_hit = self
                    .config
                    .widget_layout
                    .iter()
                    .find(|entry| entry.widget == Some(WidgetKind::Settings))
                    .is_some_and(|entry| {
                        let layout = widget_grid_layout(
                            offset_x as f32,
                            island_y as f32,
                            w as f32,
                            h as f32,
                            self.config.global_scale,
                        );
                        let (x, y, width, height) =
                            layout.footprint_rect(WidgetKind::Settings, entry.slot);
                        is_point_in_rect(
                            rel_x as f64,
                            rel_y as f64,
                            x as f64 + w - page_shift,
                            y as f64,
                            width as f64,
                            height as f64,
                        )
                    });
                if settings_hit {
                    self.open_settings(event_loop);
                    return;
                }

                if music_page_available {
                    let arrow_x = offset_x + 7.5 * scale + w - page_shift;
                    let arrow_y = island_y + h / 2.0;
                    let adx = rel_x as f64 - arrow_x;
                    let ady = rel_y as f64 - arrow_y;
                    if adx * adx + ady * ady <= (12.0 * scale).powi(2) {
                        self.widget_view = false;
                        return;
                    }
                }
            }

            if music_page_available && view_val < 0.5 {
                let arrow_x = offset_x + w - 7.5 * scale;
                let arrow_y = island_y + h / 2.0;
                let adx = rel_x as f64 - arrow_x;
                let ady = rel_y as f64 - arrow_y;
                if adx * adx + ady * ady <= (12.0 * scale).powi(2) {
                    self.widget_view = true;
                    return;
                }
            }

            if (rel_y as f64) < island_y + 40.0 * scale {
                self.expanded = false;
                self.widget_view = false;
            }
        } else if is_hovering_visible || is_on_hidden_reveal {
            if self.is_hidden() {
                self.reveal_island();
                return;
            }
            self.is_dragging = true;
            self.drag_start_px = rel_x + self.geom.win_x;
            self.drag_start_py = rel_y + self.geom.win_y;
            self.drag_start_hide_val = self.springs.hide.value;
            self.drag_has_moved = false;
        }
    }

    pub(super) fn handle_release(&mut self, py: i32) {
        if self.finish_seek() {
            return;
        }
        if self.dismissing_notification {
            self.dismissing_notification = false;
            self.is_dragging = false;
            if self.drag_start_py - py > 20 {
                self.compact_overlay.dismiss_notification();
            } else if !self.compact_overlay.activate_notification() {
                self.expand();
            }
            return;
        }
        if self.is_dragging {
            self.is_dragging = false;
            if !self.drag_has_moved {
                if self.is_hidden() {
                    self.reveal_island();
                } else {
                    self.expand();
                }
            } else if self.springs.hide.value > 0.3 {
                self.hide.manual = true;
                self.hide.auto = false;
                self.hide.fullscreen = false;
            } else {
                self.hide.manual = false;
                self.hide.auto = false;
                self.hide.fullscreen = false;
                if self.is_fullscreen_suppressed {
                    self.hide.fullscreen_reveal_override = true;
                }
            }
        }
    }

    fn expand(&mut self) {
        let widget_view = should_show_widget_view(self.music_page_available);
        let compact_height = self.compact_content_height();
        let interrupts_collapse = self.springs.h.value - compact_height
            > 0.5 * self.config.global_scale
            || self.springs.h.velocity.abs() > 0.001;
        self.widget_view = widget_view;
        if !interrupts_collapse {
            self.springs.view.value = f32::from(widget_view);
            self.springs.view.velocity = 0.0;
        }
        self.expanded = true;
    }
}
