use std::time::Instant;

use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::core::render::draw_island;
use crate::utils::blur::calculate_blur_sigmas;
use crate::utils::mouse::get_global_cursor_pos;
use crate::window::d3d::MAIN_D3D_TARGET;

use super::App;

impl App {
    pub(super) fn on_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(win) = &self.window
            && win.id() == id
        {
            match event {
                WindowEvent::ThemeChanged(theme) => {
                    let is_light = theme == winit::window::Theme::Light;
                    self.is_light_theme = is_light;
                    crate::plugin::manager::update_host_theme(is_light);
                    win.request_redraw();
                    log::info!("Window theme changed to {theme:?}");
                    if let Some(tray) = self.tray.as_mut() {
                        tray.update_theme(is_light);
                    }
                }
                WindowEvent::Resized(_) if win.is_maximized() => {
                    win.set_maximized(false);
                }
                WindowEvent::Moved(position) => {
                    self.geom.win_x = position.x;
                    self.geom.win_y = position.y;
                    if !self.is_dragging
                        && !self.is_right_dragging
                        && self.geom.position_restore_after.is_none()
                        && (position.x != self.geom.configured_x
                            || position.y != self.geom.configured_y)
                    {
                        self.geom.win_x = self.geom.configured_x;
                        self.geom.win_y = self.geom.configured_y;
                        win.set_outer_position(winit::dpi::PhysicalPosition::new(
                            self.geom.configured_x,
                            self.geom.configured_y,
                        ));
                    }
                }
                WindowEvent::DroppedFile(path)
                    if path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("zip")) =>
                {
                    log::info!("File dropped: {}", path.display());
                    self.install_zip_drop(&path);
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let (px, py) = get_global_cursor_pos();
                    if button == MouseButton::Left {
                        self.handle_input(event_loop, state, px, py);
                    } else if button == MouseButton::Right {
                        self.handle_right_input(state, px, py);
                    }
                }
                WindowEvent::Touch(touch) => {
                    let (px, py) = (
                        (touch.location.x + self.geom.win_x as f64) as i32,
                        (touch.location.y + self.geom.win_y as f64) as i32,
                    );
                    self.touch_pos = touch.location;
                    match touch.phase {
                        TouchPhase::Started => {
                            self.touch_id = Some(touch.id);
                            self.handle_input(event_loop, ElementState::Pressed, px, py);
                        }
                        TouchPhase::Moved => {
                            self.touch_id = Some(touch.id);
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            self.handle_input(event_loop, ElementState::Released, px, py);
                            self.touch_id = None;
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let island_layout = self.compute_island_layout();
                    let is_hidden = self.is_hidden();
                    if let Some(mut renderer) = self.renderer.take() {
                        let dt =
                            (self.last_render_time.elapsed().as_secs_f32() * 60.0).clamp(0.1, 6.0);
                        self.last_render_time = Instant::now();
                        let sigmas = if self.config.motion_blur {
                            calculate_blur_sigmas(
                                self.springs.w.velocity,
                                self.springs.h.velocity,
                                self.springs.view.velocity,
                                self.springs.w.value,
                            )
                        } else {
                            (0.0, 0.0)
                        };
                        let compact_target_h = self.compact_content_height();
                        let compact_content_h = compact_target_h.min(self.springs.h.value).max(0.0);
                        let total_h = (self.config.expanded_height * self.config.global_scale
                            - compact_target_h)
                            .abs()
                            .max(1.0);
                        let dist_h = (self.springs.h.value - compact_target_h).abs();
                        let progress = (dist_h / total_h).clamp(0.0, 1.0);
                        if let Some(event) = crate::plugin::manager::drain_media_source_event() {
                            match event {
                                crate::plugin::manager::MediaSourceEvent::Set(source) => {
                                    let (cover, hash) = if !source.cover_data.is_empty() {
                                        use std::collections::hash_map::DefaultHasher;
                                        use std::hash::{Hash, Hasher};
                                        let mut hasher = DefaultHasher::new();
                                        source.cover_data.hash(&mut hasher);
                                        (
                                            Some(skia_safe::Data::new_copy(&source.cover_data)),
                                            hasher.finish(),
                                        )
                                    } else {
                                        (None, 0)
                                    };
                                    self.plugin_media_source = Some(super::PluginMediaSource {
                                        resource_id: source.resource_id,
                                        available_controls: source.available_controls,
                                        info: crate::core::smtc::MediaInfo {
                                            title: source.title,
                                            artist: source.artist,
                                            album: source.album,
                                            duration_ms: source.duration_ms,
                                            duration_secs: source.duration_ms / 1000,
                                            position_ms: source.position_ms,
                                            is_playing: source.is_playing,
                                            last_update: Instant::now(),
                                            thumbnail: cover,
                                            thumbnail_hash: hash,
                                            ..Default::default()
                                        },
                                    });
                                }
                                crate::plugin::manager::MediaSourceEvent::Clear => {
                                    self.plugin_media_source = None;
                                }
                            }
                        }
                        if let Some(source) = self.plugin_media_source.as_mut()
                            && source.info.is_playing
                        {
                            let elapsed = source.info.last_update.elapsed().as_millis() as u64;
                            source.info.position_ms =
                                source.info.position_ms.saturating_add(elapsed);
                            source.info.last_update = Instant::now();
                        }
                        let spectrum = self.audio.get_spectrum();
                        let default_media_info = crate::core::smtc::MediaInfo::default();
                        let plugin_media_active = self.plugin_media_source.is_some();
                        let available_controls =
                            if let Some(source) = self.plugin_media_source.as_mut() {
                                source.info.spectrum = spectrum;
                                source.available_controls
                            } else if self.config.smtc_enabled {
                                self.smtc_media_info.spectrum = spectrum;
                                crate::plugin::types::MEDIA_CONTROL_TOGGLE_PLAY
                                    | crate::plugin::types::MEDIA_CONTROL_PREVIOUS
                                    | crate::plugin::types::MEDIA_CONTROL_NEXT
                                    | crate::plugin::types::MEDIA_CONTROL_SEEK
                            } else {
                                0
                            };
                        let media_info = if let Some(source) = self.plugin_media_source.as_ref() {
                            &source.info
                        } else if self.config.smtc_enabled {
                            &self.smtc_media_info
                        } else {
                            &default_media_info
                        };
                        let seeking_media_info = if self.seek.active && self.seek.duration_ms > 0 {
                            let mut preview = media_info.clone();
                            preview.position_ms = self.seek.preview_ms;
                            preview.last_update = Instant::now();
                            Some(preview)
                        } else {
                            None
                        };
                        let media_info = seeking_media_info.as_ref().unwrap_or(media_info);
                        let music_active = !media_info.title.is_empty()
                            && (plugin_media_active || self.config.smtc_enabled);
                        crate::plugin::manager::update_host_media(
                            &media_info.title,
                            &media_info.artist,
                            media_info.is_playing,
                        );
                        self.audio.set_gate_override(music_active && !is_hidden);
                        self.ctx_mgr.set_smtc_active(music_active);
                        crate::plugin::manager::drain_pending_contexts(&mut self.ctx_mgr);
                        let _ = self.ctx_mgr.tick();
                        if crate::plugin::manager::drain_widget_events(&mut self.widget_mgr) {
                            let widgets = self.widget_mgr.configurable_widgets();
                            let mut layout_config = crate::core::persistence::load_config();
                            if crate::core::config::normalize_active_plugin_widget_layout(
                                &layout_config.widget_layout,
                                &mut layout_config.plugin_widget_layout,
                                &widgets,
                            ) {
                                crate::core::persistence::save_config(&layout_config);
                            }
                            self.config.widget_layout = layout_config.widget_layout;
                            self.config.plugin_widget_layout = layout_config.plugin_widget_layout;
                            if let Some(settings) = self.settings.as_mut() {
                                settings.set_plugin_widgets(widgets);
                            }
                        }
                        let mini_content = self.ctx_mgr.current_mini();
                        let (current_secondary_lyric, old_secondary_lyric) =
                            if self.config.show_secondary_lyrics {
                                (
                                    self.lyrics.current_secondary_text.as_str(),
                                    self.lyrics.old_secondary_text.as_str(),
                                )
                            } else {
                                ("", "")
                            };

                        let render_result =
                            renderer.draw(MAIN_D3D_TARGET, |direct_context, surface| {
                                draw_island(
                                    direct_context,
                                    surface,
                                    crate::core::render::DrawIslandParams {
                                        layout: crate::core::render::LayoutParams {
                                            current_w: self.springs.w.value,
                                            current_h: self.springs.h.value,
                                            current_r: self.springs.r.value,
                                            sigmas,
                                            expansion_progress: progress,
                                            view_offset: self.springs.view.value,
                                            global_scale: self.config.global_scale,
                                            hide_progress: self.springs.hide.value
                                                * island_layout.content_hide_ratio,
                                            island_x: island_layout.current_island_x as f32,
                                            island_y: island_layout.current_island_y as f32,
                                            stable_island_y: island_layout.stable_island_y as f32,
                                            base_h: compact_content_h,
                                        },
                                        media: crate::core::render::MediaParams {
                                            media: media_info,
                                            music_active,
                                            available_controls,
                                        },
                                        lyrics: crate::core::render::LyricsParams {
                                            current_lyric: &self.lyrics.current_text,
                                            current_secondary_lyric,
                                            old_lyric: &self.lyrics.old_text,
                                            old_secondary_lyric,
                                            lyric_highlight: self.lyrics.highlight,
                                            lyric_transition: self.lyrics.transition,
                                            lyric_scroll_offset: self.lyrics.scroll_offset,
                                        },
                                        window: crate::core::render::WindowParams {
                                            win_x: self.geom.win_x,
                                            win_y: self.geom.win_y,
                                            monitor_x: self.geom.monitor_pos.0,
                                            monitor_y: self.geom.monitor_pos.1,
                                            monitor_w: self.geom.monitor_size.0,
                                            monitor_h: self.geom.monitor_size.1,
                                        },
                                        style: crate::core::render::StyleParams {
                                            island_style: &self.config.island_style,
                                            use_blur: self.config.motion_blur,
                                            font_size: self.config.font_size,
                                            lyrics_delay: self.config.lyrics_delay,
                                            dt,
                                            widget_layout: &self.config.widget_layout,
                                            plugin_widget_layout: &self.config.plugin_widget_layout,
                                            plugin_widgets: &self.widget_mgr,
                                            compact_widget_layout: &self
                                                .config
                                                .compact_widget_layout,
                                        },
                                        mini_content,
                                        compact_overlay: &self.compact_overlay,
                                    },
                                )
                            });
                        self.renderer = Some(renderer);
                        if let Err(error) = render_result {
                            self.invalidate_renderer(&error, Instant::now());
                        }
                    }
                }
                _ => (),
            }
        }
    }
}
