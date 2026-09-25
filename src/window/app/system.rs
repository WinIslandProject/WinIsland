use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use windows::ApplicationModel::Package;
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::PCWSTR;
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::core::persistence::{get_config_path, load_config};
use crate::plugin::marketplace::{self, MarketplacePlugin};
use crate::plugin::zip_loader;
use crate::window::backdrop::{HostBackdrop, HostBackdropParams};
use crate::window::renderer::RendererOptions;
use crate::window::tray::TrayAction;

use super::App;

/// 更新宿主背景合成。失败时永久丢弃 `HostBackdrop`（与迁移前的 `Renderer` 行为一致）。
/// 之所以是自由函数：调用点同时持有 `App` 其他字段的不可变借用，无法取 `&mut self`。
pub(super) fn update_host_backdrop(
    host_backdrop: &mut Option<HostBackdrop>,
    params: HostBackdropParams,
) -> bool {
    let Some(backdrop) = host_backdrop.as_ref() else {
        return false;
    };
    if let Err(error) = backdrop.update(params) {
        log::warn!("Host backdrop update failed: {error}");
        *host_backdrop = None;
        return false;
    }
    true
}

impl App {
    pub(super) fn handle_plugin_settings_request(&mut self, event_loop: &ActiveEventLoop) {
        let request = self
            .settings
            .as_mut()
            .and_then(crate::window::settings::SettingsApp::take_plugin_request);
        match request {
            Some(crate::window::settings::PluginSettingsRequest::Install(path)) => {
                self.install_zip_drop(&path);
            }
            Some(crate::window::settings::PluginSettingsRequest::LoadMarketplace) => {
                self.load_plugin_marketplace();
            }
            Some(crate::window::settings::PluginSettingsRequest::InstallMarketplace(plugin)) => {
                self.install_marketplace_plugin(*plugin);
            }
            Some(crate::window::settings::PluginSettingsRequest::SetEnabled { id, enabled }) => {
                let result = self.plugin_mgr.set_plugin_enabled(&id, enabled);
                let plugin_inventory = result
                    .is_ok()
                    .then(|| self.plugin_mgr.installed_plugins_async());
                if let Some(settings) = self.settings.as_mut() {
                    match result {
                        Ok(()) => {
                            if let Some(receiver) = plugin_inventory {
                                settings.set_plugin_inventory_receiver(receiver);
                            }
                            settings.set_plugin_status(
                                winisland_core::i18n::tr("plugin_state_restart"),
                                true,
                            );
                        }
                        Err(error) => settings.set_plugin_status(
                            winisland_core::i18n::tr_args("plugin_state_failed", &[&error]),
                            false,
                        ),
                    }
                }
            }
            Some(crate::window::settings::PluginSettingsRequest::Uninstall { id }) => {
                let result = self.plugin_mgr.uninstall_plugin(&id);
                if result.is_ok() {
                    crate::plugin::manager::drain_widget_events(&mut self.widget_mgr);
                }
                let plugin_inventory = result
                    .is_ok()
                    .then(|| self.plugin_mgr.installed_plugins_async());
                if let Some(settings) = self.settings.as_mut() {
                    match result {
                        Ok(()) => {
                            settings.set_plugin_widgets(self.widget_mgr.configurable_widgets());
                            if let Some(receiver) = plugin_inventory {
                                settings.set_plugin_inventory_receiver(receiver);
                            }
                            settings.set_plugin_status(
                                winisland_core::i18n::tr("plugin_uninstalled"),
                                false,
                            );
                        }
                        Err(error) => settings.set_plugin_status(
                            winisland_core::i18n::tr_args("plugin_uninstall_failed", &[&error]),
                            false,
                        ),
                    }
                }
            }
            Some(crate::window::settings::PluginSettingsRequest::Restart) => {
                self.close_settings();
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("--restart").spawn();
                }
                event_loop.exit();
            }
            None => {}
        }
    }

    pub(super) fn invalidate_renderer(&mut self, reason: &str, now: Instant) {
        let renderer = self.renderer.take();
        if renderer.is_some() {
            log::warn!("Renderer invalidated: {reason}");
        }
        if let Some(settings) = self.settings.as_mut() {
            settings.invalidate_renderer_target();
        }
        crate::utils::backdrop::clear_blurred_cover_cache();
        crate::ui::expanded::music_view::clear_cover_cache();
        self.host_backdrop = None;
        drop(renderer);
        self.renderer_retry_at = Some(now);
        self.next_frame_deadline = now;
    }

    pub(super) fn create_host_backdrop(
        &mut self,
        window: &Arc<Window>,
        backdrop_window: &Arc<Window>,
    ) {
        self.host_backdrop = match HostBackdrop::new(window, backdrop_window) {
            Ok(backdrop) => Some(backdrop),
            Err(error) => {
                log::warn!("Host backdrop is unavailable: {error}");
                None
            }
        };
    }

    pub(super) fn hide_host_backdrop(&self) {
        if let Some(host_backdrop) = self.host_backdrop.as_ref() {
            host_backdrop.hide();
        }
    }

    pub(super) fn recover_renderer(
        &mut self,
        window: &Arc<Window>,
        now: Instant,
        retry_interval: Duration,
    ) {
        let Some(retry_at) = self.renderer_retry_at else {
            return;
        };
        if now < retry_at {
            self.next_frame_deadline = self.next_frame_deadline.min(retry_at);
            return;
        }

        let Some(backdrop_window) = self.backdrop_window.clone() else {
            self.renderer_retry_at = Some(now + retry_interval);
            self.next_frame_deadline = now + retry_interval;
            return;
        };
        match crate::window::renderer::Renderer::new(
            match crate::window::native_surface(window) {
                Ok(surface) => surface,
                Err(error) => {
                    log::warn!("Renderer recovery failed: {error}");
                    self.renderer_retry_at = Some(now + retry_interval);
                    self.next_frame_deadline = now + retry_interval;
                    return;
                }
            },
            RendererOptions::new(self.geom.os_w, self.geom.os_h),
        ) {
            Ok(mut renderer) => {
                if let Some(settings) = self.settings.as_mut()
                    && let Err(error) = settings.recreate_renderer_target(&mut renderer)
                {
                    log::warn!("Settings renderer recovery failed: {error}");
                    self.renderer_retry_at = Some(now + retry_interval);
                    self.next_frame_deadline = now + retry_interval;
                    return;
                }
                self.renderer = Some(renderer);
                self.create_host_backdrop(window, &backdrop_window);
                self.renderer_retry_at = None;
                self.last_render_time = now;
                window.request_redraw();
                log::info!("Renderer recovered");
            }
            Err(error) => {
                log::warn!("Renderer recovery failed: {error}");
                self.renderer_retry_at = Some(now + retry_interval);
                self.next_frame_deadline = now + retry_interval;
            }
        }
    }

    pub(super) fn set_aumid() {
        if Package::Current().is_ok() {
            return;
        }
        let aumid = "WinIsland.PluginManager";
        let wide: Vec<u16> = aumid.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: SetCurrentProcessExplicitAppUserModelID sets a process-wide string identifier.
        // The wide string is valid and null-terminated. Called once during init before any windows.
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(PCWSTR::from_raw(wide.as_ptr()));
        }
    }

    pub(super) fn show_toast(title: &str, message: &str) {
        use windows::UI::Notifications::{
            ToastNotification, ToastNotificationManager, ToastTemplateType,
        };
        use windows::core::HSTRING;
        Self::set_aumid();
        let tmpl =
            match ToastNotificationManager::GetTemplateContent(ToastTemplateType::ToastText02) {
                Ok(t) => t,
                Err(e) => {
                    log::error!("Toast template failed: {e:?}");
                    return;
                }
            };
        if let Ok(nodes) = tmpl.SelectNodes(&HSTRING::from("//text")) {
            if let Ok(node) = nodes.Item(0) {
                let _ = node.SetInnerText(&HSTRING::from(title));
            }
            if let Ok(node) = nodes.Item(1) {
                let _ = node.SetInnerText(&HSTRING::from(message));
            }
        }
        let toast = match ToastNotification::CreateToastNotification(&tmpl) {
            Ok(t) => t,
            Err(e) => {
                log::error!("CreateToastNotification failed: {e:?}");
                return;
            }
        };
        let notifier_result = if Package::Current().is_ok() {
            ToastNotificationManager::CreateToastNotifier()
        } else {
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
                "WinIsland.PluginManager",
            ))
        };
        let notifier = match notifier_result {
            Ok(n) => n,
            Err(e) => {
                log::error!("CreateToastNotifier failed: {e:?}");
                return;
            }
        };
        if let Err(e) = notifier.Show(&toast) {
            log::error!("Toast Show failed: {e:?}");
        }
    }

    pub(super) fn install_zip_drop(&mut self, path: &Path) {
        if self.pending_install.is_some() || self.pending_marketplace_download.is_some() {
            Self::show_toast("Plugin Info", "Another installation is already in progress");
            if let Some(settings) = self.settings.as_mut() {
                settings.set_plugin_status(
                    winisland_core::i18n::tr_args(
                        "plugin_install_failed",
                        &["another installation is already in progress"],
                    ),
                    false,
                );
            }
            return;
        }

        let plugin_dir = self.plugin_mgr.plugin_dir.clone();
        let zip_path = path.to_path_buf();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let result = zip_loader::extract_plugin(&zip_path, &plugin_dir);
            let _ = tx.send(result);
        });

        self.pending_install = Some(rx);
        log::info!("Plugin extraction started in background thread");
    }

    fn load_plugin_marketplace(&mut self) {
        if self.pending_marketplace_catalog.is_some() {
            return;
        }
        if let Some(settings) = self.settings.as_mut() {
            settings.set_marketplace_loading();
        }
        let (tx, rx) = mpsc::channel();
        tokio::spawn(async move {
            let result = marketplace::load_catalog().await;
            let _ = tx.send(result);
            crate::utils::event_loop::wake();
        });
        self.pending_marketplace_catalog = Some(rx);
    }

    fn install_marketplace_plugin(&mut self, plugin: MarketplacePlugin) {
        if plugin.revoked_reason.is_some() || !plugin.is_compatible() {
            if let Some(settings) = self.settings.as_mut() {
                settings.finish_marketplace_install();
                settings.set_plugin_status(
                    winisland_core::i18n::tr("plugin_marketplace_incompatible"),
                    false,
                );
            }
            return;
        }
        if self.pending_install.is_some() || self.pending_marketplace_download.is_some() {
            if let Some(settings) = self.settings.as_mut() {
                settings.finish_marketplace_install();
                settings.set_plugin_status(
                    winisland_core::i18n::tr_args(
                        "plugin_install_failed",
                        &["another installation is already in progress"],
                    ),
                    false,
                );
            }
            return;
        }
        if let Some(settings) = self.settings.as_mut() {
            settings.set_plugin_status(
                winisland_core::i18n::tr("plugin_marketplace_downloading"),
                false,
            );
        }
        let (tx, rx) = mpsc::channel();
        tokio::spawn(async move {
            let result = marketplace::download_plugin(&plugin).await;
            let _ = tx.send(result);
            crate::utils::event_loop::wake();
        });
        self.pending_marketplace_download = Some(rx);
    }

    pub(super) fn open_settings(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(settings) = &self.settings {
            settings.bring_to_front();
            return;
        }

        crate::plugin::manager::drain_widget_events(&mut self.widget_mgr);
        let mut config = load_config();
        let plugin_widgets = self.widget_mgr.configurable_widgets();
        if winisland_core::config::normalize_active_plugin_widget_layout(
            &config.widget_layout,
            &mut config.plugin_widget_layout,
            &plugin_widgets,
        ) {
            crate::core::persistence::save_config(&config);
        }
        let plugin_settings_pages = crate::plugin::manager::plugin_settings_pages();
        let target_monitor = self
            .window
            .as_ref()
            .and_then(|window| Self::get_target_monitor(window, self.config.monitor_index));
        let mut settings = crate::window::settings::SettingsApp::new(
            config,
            Vec::new(),
            plugin_widgets,
            plugin_settings_pages,
        );
        let Some(renderer) = self.renderer.as_mut() else {
            log::error!("Cannot open settings without the shared D3D12 renderer");
            return;
        };
        settings.create_window(event_loop, renderer, target_monitor);
        settings.set_plugin_inventory_receiver(self.plugin_mgr.installed_plugins_async());
        if let Some(catalog) = self.marketplace_catalog.clone() {
            settings.set_marketplace_catalog(catalog);
        }
        self.settings = Some(settings);
        log::info!("Settings window opened in main process");
    }

    pub(super) fn close_settings(&mut self) {
        if let Some(mut settings) = self.settings.take() {
            if let Some(target) = settings.close()
                && let Some(renderer) = self.renderer.as_mut()
            {
                renderer.remove_target(target);
            }
            drop(settings);
            if !self.expanded {
                crate::utils::win32::trim_process_working_set();
                self.last_working_set_trim = Instant::now();
            }
            log::info!("Settings window closed and resources released");
        }
    }

    pub(super) fn handle_tray_events(&mut self, window: &Window, event_loop: &ActiveEventLoop) {
        if let Some(tray) = &self.tray
            && let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv()
        {
            match TrayAction::from_id(event.id, tray) {
                Some(TrayAction::ToggleVisibility) => {
                    self.visible = !self.visible;
                    window.set_visible(self.visible);
                    if !self.visible {
                        self.hide_host_backdrop();
                    } else {
                        window.request_redraw();
                    }
                    tray.update_item_text(self.visible);
                    log::info!("Tray: visibility toggled to {}", self.visible);
                }
                Some(TrayAction::OpenSettings) => {
                    log::info!("Tray: opening settings");
                    self.open_settings(event_loop);
                }
                Some(TrayAction::Restart) => {
                    log::info!("Tray: restarting application");
                    self.close_settings();
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).arg("--restart").spawn();
                    }
                    event_loop.exit();
                }
                Some(TrayAction::Exit) => {
                    log::info!("Tray: exiting application");
                    self.close_settings();
                    event_loop.exit();
                }
                None => (),
            }
        }
    }

    pub(super) fn reload_config_if_changed(&mut self, window: &Window) {
        let now = Instant::now();
        if now.duration_since(self.last_config_check) >= Duration::from_millis(500) {
            self.last_config_check = now;
            let modified = std::fs::metadata(get_config_path())
                .and_then(|metadata| metadata.modified())
                .ok();
            if modified != self.last_config_modified {
                self.last_config_modified = modified;
                let current_config = load_config();
                if current_config != self.config {
                    let old_compact_scale = self.config.compact_scale;
                    let old_expanded_scale = self.config.expanded_scale;
                    let old_base_w = self.config.base_width;
                    let old_base_h = self.config.base_height;
                    let old_max_w = self.config.expanded_width;
                    let old_max_h = self.config.expanded_height;
                    let old_style = self.config.island_style.clone();
                    let old_mini_shape = self.config.mini_cover_shape.clone();
                    let old_expanded_shape = self.config.expanded_cover_shape.clone();
                    let old_font = self.config.custom_font_path.clone();
                    let old_smtc_enabled = self.config.smtc_enabled;
                    let old_replace_native_volume_flyout = self.config.replace_native_volume_flyout;
                    let old_brightness_overlay_enabled = self.config.brightness_overlay_enabled;
                    let old_position_x_offset = self.config.position_x_offset;
                    let old_position_y_offset = self.config.position_y_offset;
                    let old_monitor_index = self.config.monitor_index;

                    log::info!("Config changed, reloaded");
                    self.config = current_config;
                    crate::ui::widget::resource_usage::set_configs(
                        &self.config.resource_metrics,
                        &self.config.compact_resource_metrics,
                    );
                    winisland_core::config::set_resource_widget_span(
                        self.config.resource_widget_columns,
                        self.config.resource_widget_rows,
                    );
                    if let Some(monitor) =
                        Self::get_target_monitor(window, self.config.monitor_index)
                    {
                        self.migrate_legacy_dock_position(monitor.position(), monitor.size());
                    }
                    self.smtc.set_lyrics_mode(self.config.lyrics_mode.clone());
                    self.smtc
                        .set_lyrics_source(self.config.lyrics_source.clone());
                    self.smtc
                        .set_lyrics_local_dir(self.config.lyrics_local_dir.clone());
                    self.smtc.set_allowed_apps(self.config.smtc_apps.clone());
                    if old_replace_native_volume_flyout != self.config.replace_native_volume_flyout
                    {
                        self.compact_overlay
                            .set_native_volume_flyout_replacement_enabled(
                                self.config.replace_native_volume_flyout,
                            );
                    }
                    if old_brightness_overlay_enabled != self.config.brightness_overlay_enabled {
                        self.compact_overlay
                            .set_brightness_overlay_enabled(self.config.brightness_overlay_enabled);
                    }
                    if old_smtc_enabled != self.config.smtc_enabled {
                        self.smtc.set_enabled(self.config.smtc_enabled);
                        self.audio.set_target_app_id(self.audio_target_app_id());
                    }

                    if old_style != self.config.island_style {
                        crate::utils::backdrop::clear_blurred_cover_cache();
                    }

                    if old_mini_shape != self.config.mini_cover_shape
                        || old_expanded_shape != self.config.expanded_cover_shape
                    {
                        crate::ui::expanded::music_view::clear_cover_cache();
                    }

                    if old_font != self.config.custom_font_path {
                        crate::utils::font::FontManager::global()
                            .set_custom_font_path(self.config.custom_font_path.as_deref());
                    }

                    let window_size = self.required_window_size();
                    let surface_size_changed =
                        window_size.width != self.geom.os_w || window_size.height != self.geom.os_h;
                    let layout_size_changed = (old_compact_scale - self.config.compact_scale).abs()
                        > 0.001
                        || (old_expanded_scale - self.config.expanded_scale).abs() > 0.001
                        || (old_base_w - self.config.base_width).abs() > 0.1
                        || (old_base_h - self.config.base_height).abs() > 0.1
                        || (old_max_w - self.config.expanded_width).abs() > 0.1
                        || (old_max_h - self.config.expanded_height).abs() > 0.1;
                    let position_changed = old_position_x_offset != self.config.position_x_offset
                        || old_position_y_offset != self.config.position_y_offset
                        || old_monitor_index != self.config.monitor_index;

                    if surface_size_changed {
                        self.geom.os_w = window_size.width;
                        self.geom.os_h = window_size.height;
                        let _ = window
                            .request_inner_size(PhysicalSize::new(self.geom.os_w, self.geom.os_h));
                        if let Some(renderer) = self.renderer.as_mut() {
                            let target = renderer.main_target();
                            if let Err(error) =
                                renderer.resize(target, self.geom.os_w, self.geom.os_h)
                            {
                                log::error!("Renderer resize failed: {error}");
                            }
                        }
                    }

                    if (layout_size_changed || surface_size_changed || position_changed)
                        && let Some(monitor) =
                            Self::get_target_monitor(window, self.config.monitor_index)
                    {
                        if let Some(settings) = self.settings.as_mut() {
                            settings.set_target_monitor(monitor.clone());
                        }
                        let mon_size = monitor.size();
                        let mon_pos = monitor.position();
                        self.update_animation_frame_interval(&monitor);
                        if mon_size.width > 0 && mon_size.height > 0 {
                            self.geom.monitor_size = (mon_size.width, mon_size.height);
                            self.geom.monitor_pos = (mon_pos.x, mon_pos.y);
                            let (position_x, position_y) =
                                self.compute_window_position(mon_pos, mon_size);
                            self.set_configured_window_position(window, position_x, position_y);
                            self.geom.position_restore_after = None;
                        }
                    }
                }
            }
        }

        if now.duration_since(self.last_monitor_check) < Duration::from_secs(1) {
            return;
        }
        self.last_monitor_check = now;
        if let Some(monitor) = Self::get_target_monitor(window, self.config.monitor_index) {
            if let Some(settings) = self.settings.as_mut() {
                settings.set_target_monitor(monitor.clone());
            }
            let mon_size = monitor.size();
            let mon_pos = monitor.position();
            self.update_animation_frame_interval(&monitor);
            let cur_mon_size = (mon_size.width, mon_size.height);
            let cur_mon_pos = (mon_pos.x, mon_pos.y);
            if (cur_mon_size != self.geom.monitor_size || cur_mon_pos != self.geom.monitor_pos)
                && cur_mon_size.0 > 0
                && cur_mon_size.1 > 0
            {
                self.geom.monitor_size = cur_mon_size;
                self.geom.monitor_pos = cur_mon_pos;
                let (position_x, position_y) = self.compute_window_position(mon_pos, mon_size);
                self.set_configured_window_position(window, position_x, position_y);
                self.geom.position_restore_after = Some(now + Duration::from_millis(750));
            }
        }
    }
}
