use std::sync::mpsc;
use std::time::{Duration, Instant};

use winit::dpi::PhysicalPosition;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::Window;

use crate::core::config::{MIN_HIDDEN_WIDTH, WidgetKind};
use crate::ui::compact::CompactOverlayState;
use crate::ui::expanded::music_view::{
    get_progress_bar_rect, set_progress_dragging, set_progress_hover,
};
use crate::utils::mouse::{
    double_click_interval, get_global_cursor_pos, is_cursor_hidden, is_foreground_fullscreen,
    is_left_button_pressed, is_point_in_continuous_rounded_rect, is_point_in_rect,
};

use super::{App, DragAxis, RIGHT_DRAG_THRESHOLD};

const INTERACTIVE_FRAME_INTERVAL: Duration = Duration::from_millis(20);
const PLAYBACK_FRAME_INTERVAL: Duration = Duration::from_millis(20);
const IDLE_FRAME_INTERVAL: Duration = Duration::from_millis(50);
const HIDDEN_FRAME_INTERVAL: Duration = Duration::from_millis(100);
const WORKING_SET_TRIM_INTERVAL: Duration = Duration::from_secs(20);
const RENDERER_RECOVERY_INTERVAL: Duration = Duration::from_secs(1);
const LYRIC_TRANSITION_STEP: f32 = 0.05;
const LYRIC_TRANSITION_LEAD_MS: u64 = (1000.0 / (60.0 * LYRIC_TRANSITION_STEP as f64)) as u64;

impl App {
    fn input_pressed(&self) -> bool {
        self.touch_id.is_some()
            || (self
                .last_touch_at
                .is_none_or(|time| time.elapsed() >= Duration::from_millis(250))
                && is_left_button_pressed())
    }
    pub(super) fn on_about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let window = match self.window.clone() {
            Some(w) => w,
            None => return,
        };
        let now = Instant::now();
        if let Some(error) = self
            .renderer
            .as_mut()
            .and_then(crate::window::renderer::Renderer::take_failure)
        {
            self.invalidate_renderer(&error, now);
        }
        if crate::window::renderer::take_dwm_composition_changed() {
            self.invalidate_renderer("DWM composition changed", now);
        }
        self.recover_renderer(&window, now, RENDERER_RECOVERY_INTERVAL);
        if now < self.next_frame_deadline {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_deadline));
            return;
        }
        if self
            .geom
            .position_restore_after
            .is_some_and(|restore_after| now >= restore_after)
        {
            self.geom.position_restore_after = None;
            self.geom.win_x = self.geom.configured_x;
            self.geom.win_y = self.geom.configured_y;
            window.set_outer_position(PhysicalPosition::new(self.geom.win_x, self.geom.win_y));
        }
        if now.duration_since(self.last_topmost_check) >= Duration::from_secs(1) {
            Self::enforce_overlay_window(&window);
            self.last_topmost_check = now;
        }
        self.handle_tray_events(&window, event_loop);
        self.reload_config_if_changed(&window);
        if self.is_hidden() && !self.can_hide() {
            self.reveal_island();
        }

        self.poll_pending_plugin_install();
        self.poll_pending_plugin_marketplace();
        if let Some(pages) = crate::plugin::manager::drain_settings_page_changes()
            && let Some(settings) = self.settings.as_mut()
        {
            settings.set_plugin_settings_pages(pages);
        }
        if self.ctx_mgr.tick() {
            window.request_redraw();
        }

        let dt = (self.last_update_time.elapsed().as_secs_f32() * 60.0).clamp(0.1, 6.0);
        self.last_update_time = now;

        if !self.visible {
            self.compact_overlay.finish_volume_drag();
            self.compact_overlay.finish_brightness_drag();
            self.audio.set_gate_override(false);
            self.next_frame_deadline = now + HIDDEN_FRAME_INTERVAL;
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_deadline));
            return;
        }
        let (px, py) = if self.touch_id.is_some() {
            (
                (self.touch_pos.x + self.geom.win_x as f64) as i32,
                (self.touch_pos.y + self.geom.win_y as f64) as i32,
            )
        } else {
            get_global_cursor_pos()
        };
        self.update_right_drag(&window, px, py);

        let (music_active, media_is_playing) = self.poll_media_info(&window);

        if now.duration_since(self.last_fullscreen_check) >= Duration::from_millis(100) {
            self.update_fullscreen_suppression(&window, now);
        }

        let rel_x = px - self.geom.win_x;
        let rel_y = py - self.geom.win_y;
        let layout = self.compute_island_layout();
        let island_y = layout.island_y;
        let offset_x = layout.offset_x;
        let current_island_x = layout.current_island_x;
        let current_island_y = layout.current_island_y;
        let is_hovering_island = is_point_in_continuous_rounded_rect(
            rel_x as f64,
            rel_y as f64,
            current_island_x,
            current_island_y,
            self.springs.w.value as f64,
            self.springs.h.value as f64,
            self.springs.r.value as f64,
        );
        let is_over_hidden_reveal = self.is_hidden()
            && (is_hovering_island
                || (self.config.hidden_width <= MIN_HIDDEN_WIDTH
                    && self.springs.hide.value >= 0.999
                    && is_point_in_rect(
                        rel_x as f64,
                        rel_y as f64,
                        layout.hidden_reveal_x,
                        layout.hidden_reveal_y,
                        layout.hidden_reveal_w,
                        layout.hidden_reveal_h,
                    )));
        let cursor_interaction_suppressed = self.is_cursor_suppressed && self.touch_id.is_none();
        let interaction_suppressed = cursor_interaction_suppressed
            || (self.config.fullscreen_auto_hide
                && self.is_fullscreen_suppressed
                && !self.hide.overlay_reveal);
        let is_hovering_visible = !interaction_suppressed && is_hovering_island;
        let is_on_hidden_reveal = !interaction_suppressed
            && is_over_hidden_reveal
            && self.config.hidden_width <= MIN_HIDDEN_WIDTH
            && self.springs.hide.value >= 0.999;

        let passive_reveal_active = self.is_fullscreen_suppressed
            && !self.config.fullscreen_auto_hide
            && is_over_hidden_reveal;
        if self.hidden_reveal_click.update(
            passive_reveal_active,
            self.input_pressed(),
            (px as f32, py as f32),
            now,
            double_click_interval(),
        ) {
            self.reveal_island();
            window.request_redraw();
            log::info!("Island revealed by fullscreen edge double-click");
        }

        if interaction_suppressed {
            let _ = window.set_cursor_hittest(false);
        } else {
            let _ = window.set_cursor_hittest(
                is_hovering_visible
                    || is_on_hidden_reveal
                    || self.compact_overlay.is_volume_dragging()
                    || self.compact_overlay.is_brightness_dragging(),
            );
        }

        let compact_overlay_visible = self.update_compact_and_auto_hide(
            &window,
            is_hovering_visible,
            music_active,
            media_is_playing,
        );

        if self.compact_overlay.is_volume_dragging() {
            if self.input_pressed() && !interaction_suppressed {
                self.update_volume_drag_position(rel_x, &layout);
            } else {
                self.compact_overlay.finish_volume_drag();
            }
            window.request_redraw();
        }
        if self.compact_overlay.is_brightness_dragging() {
            if self.input_pressed() && !interaction_suppressed {
                self.update_brightness_drag_position(rel_x, &layout);
            } else {
                self.compact_overlay.finish_brightness_drag();
            }
            window.request_redraw();
        }

        self.update_seeking_input(&window, rel_x);
        self.update_progress_hover(rel_x, rel_y, offset_x, island_y, music_active);
        self.update_hide_drag(&window, px, py, dt);
        self.update_expand_collapse_click(&window, is_hovering_visible);

        let is_paused = music_active && !media_is_playing;
        self.update_lyrics(&window, music_active, is_paused, dt);
        self.update_spring_targets(&window, music_active, is_paused, dt);
        self.update_compact_widget_refresh(&window, now);

        self.schedule_next_frame(
            event_loop,
            &window,
            now,
            FramePacing {
                media_is_playing,
                is_hovering_visible,
                compact_overlay_visible,
                passive_reveal_active,
            },
        );
    }

    fn poll_pending_plugin_install(&mut self) {
        let Some(rx) = self.pending_install.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok((manifest, staging))) => {
                if let Err(error) = self.plugin_mgr.activate_staged_plugin(&manifest, &staging) {
                    let _ = std::fs::remove_dir_all(staging);
                    Self::show_toast("Plugin Error", &error);
                    log::error!("Failed to activate installed plugin: {error}");
                    self.set_plugin_install_error(crate::core::i18n::tr_args(
                        "plugin_install_failed",
                        &[&error],
                    ));
                    return;
                }
                Self::show_toast(
                    "Plugin Installed",
                    &format!("{} loaded successfully!", manifest.name),
                );
                if let Some(settings) = self.settings.as_mut() {
                    settings.finish_marketplace_install();
                    settings
                        .set_plugin_inventory_receiver(self.plugin_mgr.installed_plugins_async());
                    settings
                        .set_plugin_status(crate::core::i18n::tr("plugin_install_success"), false);
                }
                log::info!("Plugin '{}' installed via drop", manifest.name);
            }
            Ok(Err(e)) => {
                Self::show_toast("Plugin Error", &e);
                self.set_plugin_install_error(crate::core::i18n::tr_args(
                    "plugin_install_failed",
                    &[&e],
                ));
                log::error!("Failed to install plugin from drop: {e}");
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.pending_install = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                Self::show_toast("Plugin Error", "Installation thread crashed");
                log::error!("Plugin installation thread disconnected unexpectedly");
                self.set_plugin_install_error(crate::core::i18n::tr_args(
                    "plugin_install_failed",
                    &["installation thread crashed"],
                ));
            }
        }
    }

    fn poll_pending_plugin_marketplace(&mut self) {
        if let Some(rx) = self.pending_marketplace_catalog.take() {
            match rx.try_recv() {
                Ok(Ok(catalog)) => {
                    self.marketplace_catalog = Some(catalog.clone());
                    if let Some(settings) = self.settings.as_mut() {
                        settings.set_marketplace_catalog(catalog);
                    }
                }
                Ok(Err(error)) => {
                    log::error!("Failed to load plugin marketplace: {error}");
                    if self.marketplace_catalog.is_none()
                        && let Some(settings) = self.settings.as_mut()
                    {
                        settings.set_marketplace_error(error);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.pending_marketplace_catalog = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.marketplace_catalog.is_none()
                        && let Some(settings) = self.settings.as_mut()
                    {
                        settings.set_marketplace_error(
                            "The marketplace task stopped unexpectedly".into(),
                        );
                    }
                }
            }
        }

        let Some(rx) = self.pending_marketplace_download.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(path)) => {
                if let Some(settings) = self.settings.as_mut() {
                    settings.set_plugin_status(crate::core::i18n::tr("plugin_installing"), false);
                }
                self.install_zip_drop(&path);
            }
            Ok(Err(error)) => {
                log::error!("Failed to download marketplace plugin: {error}");
                self.set_plugin_install_error(crate::core::i18n::tr_args(
                    "plugin_install_failed",
                    &[&error],
                ));
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.pending_marketplace_download = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.set_plugin_install_error(crate::core::i18n::tr_args(
                    "plugin_install_failed",
                    &["the marketplace download task stopped unexpectedly"],
                ));
            }
        }
    }

    fn set_plugin_install_error(&mut self, message: String) {
        if let Some(settings) = self.settings.as_mut() {
            settings.finish_marketplace_install();
            settings.set_plugin_status(message, false);
        }
    }

    fn update_right_drag(&mut self, window: &Window, px: i32, py: i32) {
        let (Some((start_cx, start_cy)), Some((start_ox, start_oy))) =
            (self.right_press_cursor, self.right_drag_start_offset)
        else {
            return;
        };
        let dx = px - start_cx;
        let dy = py - start_cy;
        if !self.is_right_dragging
            && (dx.abs() >= RIGHT_DRAG_THRESHOLD || dy.abs() >= RIGHT_DRAG_THRESHOLD)
        {
            self.is_right_dragging = true;
            log::info!("Right click drag started at offsets: ({start_ox}, {start_oy})");
        }
        if !self.is_right_dragging {
            return;
        }
        self.config.position_x_offset = start_ox + dx;
        self.config.position_y_offset = start_oy + dy;
        if let Some(monitor) = Self::get_target_monitor(window, self.config.monitor_index) {
            let mon_size = monitor.size();
            let mon_pos = monitor.position();
            let (new_x, new_y) = self.compute_window_position(mon_pos, mon_size);
            if new_x != self.geom.configured_x || new_y != self.geom.configured_y {
                self.set_configured_window_position(window, new_x, new_y);
            }
        }
        window.request_redraw();
    }

    fn update_fullscreen_suppression(&mut self, window: &Window, now: Instant) {
        self.last_fullscreen_check = now;
        let prev_fullscreen = self.is_fullscreen_suppressed;
        self.is_fullscreen_suppressed = is_foreground_fullscreen(
            self.geom.monitor_pos.0,
            self.geom.monitor_pos.1,
            self.geom.monitor_size.0,
            self.geom.monitor_size.1,
        );
        self.is_cursor_suppressed = is_cursor_hidden();
        let should_hide_for_fullscreen =
            self.config.fullscreen_auto_hide && self.is_fullscreen_suppressed;
        if should_hide_for_fullscreen != self.hide.fullscreen {
            if should_hide_for_fullscreen {
                self.hide.fullscreen = true;
                let hide_started = if self.hide.origin.is_some() {
                    true
                } else {
                    self.prepare_hide(window)
                };
                if hide_started {
                    self.expanded = false;
                    self.widget_view = false;
                    self.hide.notification_reveal = false;
                    self.hide.overlay_reveal = false;
                    self.attention_pulse_started = Some(now);
                } else {
                    self.hide.fullscreen = false;
                }
            } else {
                let was_fullscreen_hidden = self.hide.fullscreen;
                self.hide.fullscreen = false;
                self.attention_pulse_started = None;
                self.idle_timer = Instant::now();
                if was_fullscreen_hidden && !self.is_hidden() {
                    self.springs.hide.velocity = -0.65;
                }
            }
            window.request_redraw();
        }
        if self.is_fullscreen_suppressed != prev_fullscreen {
            log::info!(
                "Fullscreen state: {}",
                if self.is_fullscreen_suppressed {
                    "active"
                } else {
                    "normal"
                }
            );
        }
    }

    fn poll_media_info(&mut self, window: &Window) -> (bool, bool) {
        let smtc_cover_changed = self.smtc.take_info_if_changed().is_some_and(|media| {
            let cover_changed = self.plugin_media_source.is_none()
                && media.thumbnail.is_some()
                && media.thumbnail_hash != self.smtc_media_info.thumbnail_hash;
            self.smtc_media_info = media;
            cover_changed
        });
        self.audio.set_target_app_id(self.audio_target_app_id());
        let music_active = self.media_active();
        let media_is_playing = music_active && self.current_media_info().is_playing;
        let music_became_available = music_active && !self.music_page_available;
        self.music_page_available = music_active;
        if !self.music_page_available && self.expanded {
            self.widget_view = true;
            self.springs.view.value = 1.0;
            self.springs.view.velocity = 0.0;
        } else if music_became_available && self.expanded {
            self.widget_view = false;
        }
        let media = self.current_media_info();
        let title = media.title.clone();
        let artist = media.artist.clone();
        let album = media.album.clone();
        if !music_active {
            self.audio.set_gate_override(false);
            if !self.last_media_title.is_empty() {
                self.last_media_title.clear();
                crate::ui::expanded::music_view::clear_cover_cache();
                crate::utils::backdrop::clear_blurred_cover_cache();
            }
        }
        let track_changed = music_active && title != self.last_media_title;
        if track_changed {
            log::info!("Track changed: {title} - {artist} / {album}");
            self.last_media_title = title;
            crate::ui::expanded::music_view::trigger_cover_flip();
            window.request_redraw();
        } else if smtc_cover_changed {
            log::info!("SMTC: late thumbnail applied");
            crate::ui::expanded::music_view::trigger_cover_flip();
            window.request_redraw();
        }
        (music_active, media_is_playing)
    }

    fn update_compact_and_auto_hide(
        &mut self,
        window: &Window,
        is_hovering_visible: bool,
        music_active: bool,
        media_is_playing: bool,
    ) -> bool {
        let is_paused_idle = music_active && !media_is_playing;
        let overlay_present = !self.expanded && !self.is_hidden();
        let fullscreen_hidden = self.config.fullscreen_auto_hide && self.is_fullscreen_suppressed;
        let volume_state = if overlay_present && (!fullscreen_hidden || self.hide.overlay_reveal) {
            CompactOverlayState::Present
        } else if fullscreen_hidden || (self.hide.auto && !self.hide.manual) {
            CompactOverlayState::Defer
        } else {
            CompactOverlayState::Discard
        };
        let notification_state = if overlay_present && !fullscreen_hidden {
            CompactOverlayState::Present
        } else if !self.expanded && self.hide.has_hidden_reason() {
            CompactOverlayState::Defer
        } else {
            CompactOverlayState::Discard
        };
        let compact_update = self.compact_overlay.update(
            volume_state,
            notification_state,
            self.config.notification_display,
        );
        if compact_update.notification_received
            && !fullscreen_hidden
            && self.hide.has_hidden_reason()
            && !self.hide.notification_reveal
        {
            self.hide.notification_reveal = true;
            self.idle_timer = Instant::now();
            self.springs.hide.velocity = -0.65;
            self.compact_overlay.update(
                CompactOverlayState::Present,
                CompactOverlayState::Present,
                self.config.notification_display,
            );
            log::info!("Island temporarily revealed for notification");
        } else if (compact_update.volume_changed || compact_update.brightness_changed)
            && (fullscreen_hidden || (self.hide.auto && !self.hide.manual))
        {
            if fullscreen_hidden {
                self.hide.overlay_reveal = true;
            } else if self.hide.auto && !self.hide.manual {
                self.hide.auto = false;
            }
            self.idle_timer = Instant::now();
            self.springs.hide.velocity = -0.65;
            self.compact_overlay.update(
                CompactOverlayState::Present,
                CompactOverlayState::Present,
                self.config.notification_display,
            );
            log::info!("Island un-hidden (compact overlay event)");
        }
        if self.hide.notification_reveal && !self.compact_overlay.is_notification_visible() {
            self.hide.notification_reveal = false;
            if self.hide.has_hidden_reason() {
                self.prepare_hide(window);
            }
            window.request_redraw();
        }
        let compact_overlay_visible = self.compact_overlay.is_visible();
        if self.hide.overlay_reveal && !compact_overlay_visible {
            self.hide.overlay_reveal = false;
            if self.hide.has_hidden_reason() {
                self.prepare_hide(window);
            }
            window.request_redraw();
        }
        let has_widgets = !self.components_hidden
            && (self.config.widget_layout.iter().any(|entry| {
                entry
                    .widget
                    .is_some_and(|widget| widget != WidgetKind::Settings)
            }) || !self.config.plugin_widget_layout.is_empty()
                || self
                    .config
                    .compact_widget_layout
                    .iter()
                    .any(|entry| entry.widget.is_some())
                || matches!(
                    self.ctx_mgr.current_mini(),
                    Some(crate::core::context::MiniContent::Plugin(_))
                ));
        let is_idle = (!is_hovering_visible || self.components_hidden)
            && !self.expanded
            && !self.is_dragging
            && !compact_overlay_visible
            && (self.components_hidden || !music_active || is_paused_idle)
            && !has_widgets;
        if !self.config.auto_hide {
            let was_auto_hidden = self.hide.auto;
            self.hide.auto = false;
            self.idle_timer = Instant::now();
            if was_auto_hidden && !self.is_hidden() {
                self.springs.hide.velocity = -0.65;
            }
        } else if media_is_playing && !self.components_hidden && self.hide.auto && !self.hide.manual
        {
            self.hide.auto = false;
            self.idle_timer = Instant::now();
            if !self.is_hidden() {
                self.springs.hide.velocity = -0.65;
            }
            log::info!("Island un-hidden (media playing)");
        } else if !self.is_hidden() && is_idle {
            if self.idle_timer.elapsed().as_secs_f32() > self.config.auto_hide_delay
                && self.prepare_hide(window)
            {
                self.hide.auto = true;
                log::info!(
                    "Island auto-hidden (idle {:.1}s)",
                    self.config.auto_hide_delay
                );
            }
        } else if !self.is_hidden() && !is_idle {
            self.idle_timer = Instant::now();
        }
        compact_overlay_visible
    }

    fn update_seeking_input(&mut self, window: &Window, rel_x: i32) {
        if self.seek.active && self.input_pressed() {
            let page_shift = self.springs.view.value * self.springs.w.value;
            let click_x = rel_x as f32 - page_shift;
            self.seek.preview_at(click_x);
            window.request_redraw();
        } else if self.finish_seek() {
            window.request_redraw();
        }
    }

    fn update_progress_hover(
        &self,
        rel_x: i32,
        rel_y: i32,
        offset_x: f64,
        island_y: f64,
        music_active: bool,
    ) {
        let progress_hover_active = if self.seek.active {
            true
        } else if self.expanded
            && (self.springs.view.value as f64) < 0.5
            && self.media_control_available(crate::plugin::types::MEDIA_CONTROL_SEEK)
        {
            if let Some((bar_left, bar_right, bar_top, bar_hit_h)) = get_progress_bar_rect(
                offset_x as f32,
                island_y as f32,
                self.springs.w.value,
                music_active,
                self.config.expanded_scale,
            ) {
                let page_shift = self.springs.view.value * self.springs.w.value;
                let cx = rel_x as f32 - page_shift;
                let cy = rel_y as f32;
                let margin = 4.0 * self.config.expanded_scale;
                cx >= bar_left - margin
                    && cx <= bar_right + margin
                    && cy >= bar_top - margin
                    && cy <= bar_top + bar_hit_h + margin
            } else {
                false
            }
        } else {
            false
        };
        set_progress_hover(progress_hover_active);
        set_progress_dragging(self.seek.active);
    }

    fn update_hide_drag(&mut self, window: &Window, px: i32, py: i32, dt: f32) {
        if self.is_dragging && !self.dismissing_notification && !self.is_hidden() {
            let upward_distance = self.drag_start_py - py;
            let horizontal_distance = px - self.drag_start_px;
            if upward_distance.abs() > 3 || horizontal_distance.abs() > 3 {
                self.drag_has_moved = true;
            }
            if self.drag_axis.is_none() {
                let horizontal = horizontal_distance.abs();
                let vertical = upward_distance.abs();
                if horizontal >= 10 && horizontal as f32 > vertical as f32 * 1.4 {
                    self.drag_axis = Some(DragAxis::Horizontal);
                } else if vertical >= 24 && vertical as f32 > horizontal as f32 * 1.4 {
                    self.drag_axis = Some(DragAxis::Vertical);
                }
            }
            if self.drag_axis == Some(DragAxis::Vertical)
                && upward_distance > 3
                && self.hide.origin.is_none()
            {
                self.prepare_hide(window);
            }
            if self.drag_axis == Some(DragAxis::Vertical) && self.hide.origin.is_some() {
                let drag_layout = self.compute_island_layout();
                if drag_layout.hide_distance > 0.0 {
                    let mut new_val = self.drag_start_hide_val
                        + (upward_distance as f32 / drag_layout.hide_distance as f32);
                    new_val = new_val.clamp(0.0, 1.0);
                    self.springs.hide.value = new_val;
                    self.springs.hide.velocity = 0.0;
                    window.request_redraw();
                }
            }
        } else {
            let hide_target = if self.is_hidden() { 1.0 } else { 0.0 };
            let (stiffness, damping) = if self.is_hidden() {
                (0.12, 0.70)
            } else {
                (0.08, 0.78)
            };
            self.springs
                .hide
                .update_dt(hide_target, stiffness, damping, dt);
        }
        if !self.is_hidden() {
            self.restore_hide_origin(window);
        }
        if self.springs.hide.velocity.abs() > 0.001
            || (self.springs.hide.value > 0.0 && self.springs.hide.value < 1.0)
        {
            window.request_redraw();
        }
    }

    fn update_expand_collapse_click(&mut self, window: &Window, is_hovering_visible: bool) {
        if self.config.fullscreen_auto_hide && self.is_fullscreen_suppressed {
            return;
        }
        let pressing = self.input_pressed();
        if self.expanded && !is_hovering_visible && pressing {
            self.expanded = false;
            self.widget_view = false;
            window.request_redraw();
        }
        if !self.expanded && is_hovering_visible && pressing {
            self.idle_timer = Instant::now();
        }
    }

    fn update_lyrics(&mut self, window: &Window, music_active: bool, is_paused: bool, dt: f32) {
        let current_lyric = if music_active && self.config.show_lyrics && !is_paused {
            self.current_media_info()
                .current_lyric(
                    (self.config.lyrics_delay * 1000.0) as i64,
                    LYRIC_TRANSITION_LEAD_MS,
                )
                .map(|lyric| {
                    (
                        lyric.text.to_owned(),
                        lyric.secondary_text.unwrap_or_default().to_owned(),
                        lyric.highlight,
                        lyric.started,
                    )
                })
        } else {
            None
        };
        if let Some((lyric, secondary_lyric, highlight, started)) = current_lyric {
            if lyric != self.lyrics.current_text
                || secondary_lyric != self.lyrics.current_secondary_text
            {
                self.lyrics.transition_to(
                    lyric,
                    secondary_lyric,
                    highlight,
                    started,
                    self.config.lyrics_transition_animation,
                );
                window.request_redraw();
            } else if highlight != self.lyrics.highlight {
                self.lyrics.highlight = highlight;
                window.request_redraw();
            }
        } else if !is_paused && !self.lyrics.current_text.is_empty() {
            self.lyrics.transition_to(
                String::new(),
                String::new(),
                None,
                false,
                self.config.lyrics_transition_animation,
            );
        }

        if self.lyrics.transition < 1.0 {
            self.lyrics.transition += LYRIC_TRANSITION_STEP * dt;
            if self.lyrics.transition > 1.0 {
                self.lyrics.transition = 1.0;
            }
            window.request_redraw();
        }
        if self.lyrics.transition >= 1.0 && !self.lyrics.old_text.is_empty() {
            self.lyrics.old_text = String::new();
            self.lyrics.old_secondary_text = String::new();
        }
    }

    fn update_spring_targets(
        &mut self,
        window: &Window,
        music_active: bool,
        is_paused: bool,
        dt: f32,
    ) {
        let was_animating = self.springs.any_animating();
        let compact_components_visible = self.expanded || !self.components_hidden;
        let music_active = music_active && compact_components_visible;
        let lyric_target_w = self.compute_lyric_target_width(window, music_active, is_paused, dt);
        let preserve_compact_widget_width = !self.is_width_hiding()
            || !crate::ui::widget::compact::hide_fade_complete(self.springs.hide.value);
        let compact_widget_target_w = if !self.expanded
            && !self.compact_overlay.is_visible()
            && preserve_compact_widget_width
        {
            let scale = self.config.compact_scale.max(f32::EPSILON);
            let has_mini_content = !self.components_hidden && self.ctx_mgr.current_mini().is_some();
            let center_content_width = has_mini_content.then_some(lyric_target_w / scale);
            crate::ui::widget::compact::target_width(
                if self.components_hidden {
                    &[]
                } else {
                    &self.config.compact_widget_layout
                },
                self.config.base_width,
                center_content_width,
            ) * scale
        } else {
            lyric_target_w
        };
        let compact_content_h = self.compact_content_height();
        let default_target_h = if self.expanded {
            self.config.expanded_height * self.config.expanded_scale
        } else {
            compact_content_h
        };
        let default_target_r = if self.expanded {
            crate::utils::shape::expanded_island_radius(
                self.config.expanded_width * self.config.expanded_scale,
                default_target_h,
                self.config.expanded_scale,
            )
        } else {
            compact_content_h / 2.0
        };
        let (target_w, target_h, target_r) = if let Some(size) = self.compact_overlay.target_size(
            self.config.base_width,
            self.config.base_height,
            self.config.compact_scale,
        ) {
            (size.width, size.height, size.height / 2.0)
        } else {
            (compact_widget_target_w, default_target_h, default_target_r)
        };
        let target_view = if self.widget_view || !self.music_page_available {
            1.0
        } else {
            0.0
        };
        self.springs
            .retarget_expansion(self.expanded, target_w, target_h, target_r, target_view);
        let width_hiding = self.is_width_hiding();
        if self.width_hiding_last_frame && !width_hiding {
            self.restoring_hide_width = true;
        } else if width_hiding {
            self.restoring_hide_width = false;
        }
        self.width_hiding_last_frame = width_hiding;

        if self.restoring_hide_width {
            let progress = 1.0 - 0.78_f32.powf(dt);
            self.springs.w.value += (target_w - self.springs.w.value) * progress;
            self.springs.w.velocity = 0.0;
            if (target_w - self.springs.w.value).abs() <= 0.25 * self.config.compact_scale {
                self.springs.w.value = target_w;
                self.restoring_hide_width = false;
            }
        } else {
            self.springs.w.update_dt(target_w, 0.10, 0.68, dt);
        }
        self.springs.h.update_dt(target_h, 0.10, 0.68, dt);
        self.springs.r.update_dt(target_r, 0.10, 0.68, dt);
        self.springs.view.update_dt(target_view, 0.12, 0.68, dt);
        if was_animating && !self.springs.any_animating() {
            window.request_redraw();
        }
    }

    fn update_compact_widget_refresh(&mut self, window: &Window, now: Instant) {
        if self.expanded
            || self.components_hidden
            || self.is_hidden()
            || self.compact_overlay.is_visible()
        {
            self.compact_widget_refresh_at = now;
            return;
        }
        let Some(delay) =
            crate::ui::widget::compact::next_refresh_delay(&self.config.compact_widget_layout)
        else {
            self.compact_widget_refresh_at = now;
            return;
        };
        if now >= self.compact_widget_refresh_at {
            window.request_redraw();
            self.compact_widget_refresh_at = now + delay;
        }
    }

    fn periodic_effect_redraw_due(&mut self) -> bool {
        let is_dynamic = self.config.island_style == "dynamic";
        let due = !self.is_hidden()
            && self.last_effect_refresh.elapsed().as_millis() >= 1000
            && (is_dynamic || self.expanded);
        if due {
            self.last_effect_refresh = Instant::now();
        }
        due
    }

    fn schedule_next_frame(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        now: Instant,
        pacing: FramePacing,
    ) {
        let should_periodic_redraw = self.periodic_effect_redraw_due();
        let transition_active = self.springs.any_animating()
            || self
                .attention_pulse_started
                .is_some_and(|started| now.duration_since(started) < Duration::from_millis(1600))
            || self.compact_overlay.is_volume_dragging()
            || self.compact_overlay.is_brightness_dragging()
            || self.lyrics.transition < 1.0
            || self.is_dragging
            || self.seek.active
            || self.is_right_dragging;
        let resource_usage_active = self.resource_usage_animating();
        let compact_components_visible = self.expanded || !self.components_hidden;
        let playback_active = !self.is_hidden()
            && compact_components_visible
            && pacing.media_is_playing
            && (!self.expanded || self.springs.view.value < 1.0);
        let dynamic_effect_active = !self.is_hidden()
            && compact_components_visible
            && self.config.island_style == "dynamic"
            && self.current_media_info().thumbnail.is_some();
        let interactive_active = (pacing.is_hovering_visible
            && (!self.expanded || (self.music_page_available && self.springs.view.value < 1.0)))
            || pacing.compact_overlay_visible
            || pacing.passive_reveal_active
            || self.right_press_cursor.is_some();
        if !transition_active
            && !interactive_active
            && !resource_usage_active
            && (!self.expanded || (!playback_active && !dynamic_effect_active))
            && self.last_working_set_trim.elapsed() >= WORKING_SET_TRIM_INTERVAL
        {
            crate::utils::win32::trim_process_working_set();
            self.last_working_set_trim = now;
        }
        let frame_interval = if transition_active {
            self.animation_frame_interval
        } else if self.expanded
            && (playback_active
                || dynamic_effect_active
                || interactive_active
                || resource_usage_active)
        {
            self.aligned_frame_interval(Duration::from_secs_f64(
                1.0 / f64::from(self.config.expanded_idle_fps),
            ))
        } else if playback_active || dynamic_effect_active {
            self.aligned_frame_interval(PLAYBACK_FRAME_INTERVAL)
        } else if interactive_active || resource_usage_active {
            self.aligned_frame_interval(INTERACTIVE_FRAME_INTERVAL)
        } else if self.is_hidden() {
            HIDDEN_FRAME_INTERVAL
        } else {
            IDLE_FRAME_INTERVAL
        };
        self.next_frame_deadline = now + frame_interval;
        if transition_active
            || resource_usage_active
            || playback_active
            || dynamic_effect_active
            || interactive_active
            || should_periodic_redraw
        {
            window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_deadline));
    }

    fn aligned_frame_interval(&self, desired: Duration) -> Duration {
        let refresh = self.display_frame_interval.as_nanos().max(1);
        let refreshes = ((desired.as_nanos() + refresh / 2) / refresh).max(1) as u32;
        self.display_frame_interval.saturating_mul(refreshes)
    }

    fn resource_usage_animating(&self) -> bool {
        use crate::core::config::{CompactWidgetKind, WidgetKind};

        if self.is_hidden() || self.compact_overlay.is_visible() {
            return false;
        }
        let visible = if self.expanded {
            (!self.music_page_available || self.springs.view.value > 0.0)
                && self
                    .config
                    .widget_layout
                    .iter()
                    .any(|slot| slot.widget == Some(WidgetKind::ResourceUsage))
        } else {
            self.config
                .compact_widget_layout
                .iter()
                .any(|slot| slot.widget == Some(CompactWidgetKind::ResourceUsage))
        };
        visible
            && if self.expanded {
                crate::ui::widget::resource_usage::is_animating(&self.config.resource_metrics)
            } else {
                crate::ui::widget::resource_usage::is_animating(
                    &self.config.compact_resource_metrics,
                )
            }
    }
}

struct FramePacing {
    media_is_playing: bool,
    is_hovering_visible: bool,
    compact_overlay_visible: bool,
    passive_reveal_active: bool,
}
