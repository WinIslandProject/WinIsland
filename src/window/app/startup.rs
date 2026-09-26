use std::time::Duration;

use winisland_platform::{OverlaySpec, Theme};

use crate::platform::{WindowRef, window};
use crate::utils::logger;
use winisland_core::config::WINDOW_TITLE;
use winisland_core::i18n::tr;

use super::App;

impl App {
    pub(super) fn on_resumed(&mut self) {
        if self.window.is_none() {
            Self::set_aumid();
            let window_size = self.required_window_size();
            self.geom.os_w = window_size.width;
            self.geom.os_h = window_size.height;
            let id = match window().create_overlay(OverlaySpec {
                title: WINDOW_TITLE,
                size: window_size,
            }) {
                Ok(id) => id,
                Err(error) => {
                    log::error!("Window creation failed: {error}");
                    window().exit();
                    return;
                }
            };
            let window_ref = WindowRef(id);
            self.window = Some(window_ref);
            log::info!(
                "Window created: {}x{} (base {}x{})",
                self.geom.os_w,
                self.geom.os_h,
                self.config.base_width,
                self.config.base_height
            );

            let mut monitor_opt = None;
            for _ in 0..10 {
                if let Some(monitor) =
                    Self::get_target_monitor(&window_ref, self.config.monitor_index)
                {
                    let size = monitor.size();
                    if size.width > 0 && size.height > 0 {
                        monitor_opt = Some(monitor);
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            if let Some(monitor) = monitor_opt {
                let mon_size = monitor.size();
                let mon_pos = monitor.position();
                self.update_animation_frame_interval(&monitor);
                self.geom.monitor_size = (mon_size.width, mon_size.height);
                self.geom.monitor_pos = (mon_pos.x, mon_pos.y);
                self.migrate_legacy_dock_position(mon_pos, mon_size);
                let (position_x, position_y) = self.compute_window_position(mon_pos, mon_size);
                self.set_configured_window_position(&window_ref, position_x, position_y);
                log::info!(
                    "Monitor: {}x{} @ ({}, {}); window @ ({}, {})",
                    mon_size.width,
                    mon_size.height,
                    mon_pos.x,
                    mon_pos.y,
                    self.geom.win_x,
                    self.geom.win_y
                );
            }
            let renderer = match window()
                .native_surface(id)
                .ok_or_else(|| "Window handle unavailable".to_string())
                .and_then(|surface| {
                    winisland_render::Renderer::new(
                        surface,
                        winisland_render::RendererOptions::new(self.geom.os_w, self.geom.os_h),
                    )
                    .map_err(|error| error.to_string())
                }) {
                Ok(renderer) => renderer,
                Err(error) => {
                    log::error!("Renderer initialization failed: {error}");
                    logger::show_error_message(
                        &tr("renderer_init_failed_title"),
                        &format!("{}\n\n{error}", tr("renderer_init_failed_desc")),
                    );
                    window().exit();
                    return;
                }
            };
            self.renderer = Some(renderer);
            self.create_host_backdrop(&window_ref);
            let is_light = window().theme(id) == Some(Theme::Light);
            self.is_light_theme = is_light;
            crate::plugin::manager::update_host_state(crate::plugin::types::HostState {
                theme: if is_light {
                    "light".to_string()
                } else {
                    "dark".to_string()
                },
                ..Default::default()
            });
            self.plugin_mgr.load_all();
            log::info!("{} plugin(s) loaded", self.plugin_mgr.len());
            match crate::platform::shell().tray_install(
                crate::platform::tray_theme(is_light),
                crate::platform::tray_labels(true),
            ) {
                Ok(()) => {
                    self.tray_installed = true;
                    log::info!(
                        "Tray icon created (theme={})",
                        if is_light { "light" } else { "dark" }
                    );
                }
                Err(error) => {
                    crate::platform::update_capabilities(|caps| caps.tray = false);
                    log::warn!("Tray icon unavailable: {error}");
                    crate::platform::shell().information_dialog(
                        &tr("tray_unavailable_title"),
                        &tr("tray_unavailable_desc"),
                    );
                    self.open_settings();
                }
            }
            Self::enforce_overlay_window(&window_ref);
            window_ref.set_visible(true);
            window_ref.request_redraw();
        }
    }
}
