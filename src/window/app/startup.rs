use std::sync::Arc;
use std::time::Duration;

use winit::dpi::PhysicalSize;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowButtons, WindowLevel};

use crate::core::config::WINDOW_TITLE;
use crate::utils::icon::get_app_icon;
use crate::window::tray::TrayManager;

use super::App;

impl App {
    pub(super) fn on_resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        if self.window.is_none() {
            Self::set_aumid();
            let window_size = self.required_window_size();
            self.geom.os_w = window_size.width;
            self.geom.os_h = window_size.height;
            let attrs = Window::default_attributes()
                .with_title(WINDOW_TITLE)
                .with_inner_size(PhysicalSize::new(self.geom.os_w, self.geom.os_h))
                .with_transparent(true)
                .with_no_redirection_bitmap(true)
                .with_visible(false)
                .with_decorations(false)
                .with_resizable(true)
                .with_enabled_buttons(WindowButtons::empty())
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_skip_taskbar(true)
                .with_window_icon(get_app_icon());
            let window = Arc::new(event_loop.create_window(attrs).unwrap());

            self.window = Some(window.clone());
            log::info!(
                "Window created: {}x{} (base {}x{})",
                self.geom.os_w,
                self.geom.os_h,
                self.config.base_width,
                self.config.base_height
            );

            let mut monitor_opt = None;
            for _ in 0..10 {
                if let Some(monitor) = Self::get_target_monitor(&window, self.config.monitor_index)
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
                self.set_configured_window_position(&window, position_x, position_y);
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
            self.renderer =
                match crate::window::d3d::D3DRenderer::new(&window, self.geom.os_w, self.geom.os_h)
                {
                    Ok(renderer) => Some(renderer),
                    Err(error) => {
                        log::error!("D3D12 renderer initialization failed: {error}");
                        self.renderer_retry_at = Some(std::time::Instant::now());
                        None
                    }
                };
            let is_light = window.theme() == Some(winit::window::Theme::Light);
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
            self.tray = Some(TrayManager::new(is_light));
            log::info!(
                "Tray icon created (theme={})",
                if is_light { "light" } else { "dark" }
            );
            Self::enforce_overlay_window(&window);
            window.set_visible(true);
            window.request_redraw();
        }
    }
}
