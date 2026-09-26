use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use winisland_platform::{HostBackdropParams, TrayAction, WindowSize};

use crate::core::persistence::{get_config_path, load_config};
use crate::platform::{WindowRef, window};
use winisland_plugin_package::activate as zip_loader;
use winisland_plugin_package::marketplace::{self, MarketplacePlugin};
use winisland_render::RendererOptions;

use super::App;

pub(super) fn update_host_backdrop(
    host_backdrop: &mut bool,
    id: winisland_platform::WindowId,
    params: HostBackdropParams,
) -> bool {
    if !*host_backdrop {
        return false;
    }
    match window().update_host_backdrop(id, params) {
        Ok(available) => available,
        Err(error) => {
            log::warn!("Host backdrop update failed: {error}");
            window().release_host_backdrop(id);
            *host_backdrop = false;
            crate::platform::update_capabilities(|caps| caps.host_backdrop = false);
            false
        }
    }
}

impl App {
    pub(super) fn handle_plugin_settings_request(&mut self) {
        let request = self
            .settings
            .as_mut()
            .and_then(crate::window::settings::SettingsApp::take_plugin_request);
        match request {
            Some(crate::window::settings::PluginSettingsRequest::HideIsland) => {
                self.visible = false;
                if let Some(window) = &self.window {
                    window.set_visible(false);
                }
                self.hide_host_backdrop();
            }
            Some(crate::window::settings::PluginSettingsRequest::Exit) => {
                self.close_settings();
                window().exit();
            }
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
                if let Some(host) = &self.plugin_host
                    && let Err(error) = host.unload_if_loaded(&id)
                {
                    if let Some(settings) = self.settings.as_mut() {
                        settings.set_plugin_status(
                            winisland_core::i18n::tr_args(
                                "plugin_uninstall_failed",
                                &[&error.to_string()],
                            ),
                            false,
                        );
                    }
                    return;
                }
                let result = self.plugin_mgr.uninstall_plugin(&id);
                if result.is_ok() {
                    self.refresh_v2_widgets();
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
                window().exit();
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
        if let Some(window_ref) = self.window {
            window().release_host_backdrop(window_ref.id());
        }
        self.host_backdrop = false;
        drop(renderer);
        self.renderer_retry_at = Some(now);
        self.next_frame_deadline = now;
    }

    pub(super) fn create_host_backdrop(&mut self, window_ref: &WindowRef) {
        self.host_backdrop = match window().start_host_backdrop(window_ref.id()) {
            Ok(()) => true,
            Err(error) => {
                log::warn!("Host backdrop is unavailable: {error}");
                false
            }
        };
        if !self.host_backdrop {
            crate::platform::update_capabilities(|caps| caps.host_backdrop = false);
        }
    }

    pub(super) fn hide_host_backdrop(&self) {
        if self.host_backdrop
            && let Some(window_ref) = self.window
        {
            window().hide_host_backdrop(window_ref.id());
        }
    }

    pub(super) fn recover_renderer(
        &mut self,
        window_ref: &WindowRef,
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

        match winisland_render::Renderer::new(
            match window().native_surface(window_ref.id()) {
                Some(surface) => surface,
                None => {
                    log::warn!("Renderer recovery failed: window surface unavailable");
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
                self.create_host_backdrop(window_ref);
                self.renderer_retry_at = None;
                self.last_render_time = now;
                window_ref.request_redraw();
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
        crate::platform::notify().set_app_identity();
    }

    pub(super) fn show_toast(title: &str, message: &str) {
        crate::platform::notify().show_toast(title, message);
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
            let result = zip_loader::extract_plugin(
                &zip_path,
                &plugin_dir,
                winisland_plugin_api::abi::ABI_VERSION_2,
            )
            .map_err(|error| {
                if error.contains("Unsupported plugin ABI version 1") {
                    format!(
                        "该插件使用已废弃的 ABI v1，请升级或联系作者；建议先禁用该插件。 {error}"
                    )
                } else {
                    error
                }
            });
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
            let result = marketplace::load_catalog(winisland_plugin_api::abi::ABI_VERSION_2).await;
            let _ = tx.send(result);
            crate::platform::wake();
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
            let result =
                marketplace::download_plugin(&plugin, winisland_plugin_api::abi::ABI_VERSION_2)
                    .await;
            let _ = tx.send(result);
            crate::platform::wake();
        });
        self.pending_marketplace_download = Some(rx);
    }

    pub(super) fn open_settings(&mut self) {
        if let Some(settings) = &self.settings {
            settings.bring_to_front();
            return;
        }

        self.refresh_v2_widgets();
        let mut config = load_config();
        let plugin_widgets = self.widget_mgr.configurable_widgets();
        if winisland_core::config::normalize_active_plugin_widget_layout(
            &config.widget_layout,
            &mut config.plugin_widget_layout,
            &plugin_widgets,
        ) {
            crate::core::persistence::save_config(&config);
        }
        let plugin_settings_pages = self
            .plugin_host
            .as_ref()
            .and_then(|host| host.settings_pages_snapshot())
            .map(|(_, pages)| pages)
            .unwrap_or_default();
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
        settings.set_plugin_host(self.plugin_host.clone());
        let Some(renderer) = self.renderer.as_mut() else {
            log::error!("Cannot open settings without the shared D3D12 renderer");
            return;
        };
        settings.create_window(renderer, target_monitor);
        settings.set_plugin_inventory_receiver(self.plugin_mgr.installed_plugins_async());
        if let Some(catalog) = self.marketplace_catalog.clone() {
            settings.set_marketplace_catalog(catalog);
        }
        self.settings = Some(settings);
        log::info!("Settings window opened in main process");
    }

    pub(super) fn close_settings(&mut self) {
        if !self.tray_installed && !self.visible {
            self.visible = true;
            if let Some(window) = &self.window {
                window.set_visible(true);
                window.request_redraw();
            }
        }
        if let Some(mut settings) = self.settings.take() {
            if let Some(target) = settings.close()
                && let Some(renderer) = self.renderer.as_mut()
            {
                renderer.remove_target(target);
            }
            drop(settings);
            if !self.expanded {
                if let Err(error) = crate::platform::metrics().trim_working_set() {
                    log::warn!("Working set trim failed: {error}");
                }
                self.last_working_set_trim = Instant::now();
            }
            log::info!("Settings window closed and resources released");
        }
    }

    pub(super) fn handle_tray_events(&mut self, window_ref: &WindowRef) {
        if self.tray_installed
            && let Some(action) = crate::platform::shell()
                .poll_tray_events()
                .into_iter()
                .next()
        {
            match action {
                TrayAction::ToggleVisibility => {
                    self.visible = !self.visible;
                    window_ref.set_visible(self.visible);
                    if !self.visible {
                        self.hide_host_backdrop();
                    } else {
                        window_ref.request_redraw();
                    }
                    let _ = crate::platform::shell().tray_update(
                        crate::platform::tray_theme(self.is_light_theme),
                        crate::platform::tray_labels(self.visible),
                    );
                    log::info!("Tray: visibility toggled to {}", self.visible);
                }
                TrayAction::OpenSettings => {
                    log::info!("Tray: opening settings");
                    self.open_settings();
                }
                TrayAction::Restart => {
                    log::info!("Tray: restarting application");
                    self.close_settings();
                    let _ = crate::platform::shell().restart(&["--restart".to_string()]);
                    window().exit();
                }
                TrayAction::Exit => {
                    log::info!("Tray: exiting application");
                    self.close_settings();
                    window().exit();
                }
            }
        }
    }

    pub(super) fn reload_config_if_changed(&mut self, window_ref: &WindowRef) {
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
                        Self::get_target_monitor(window_ref, self.config.monitor_index)
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
                        winisland_render::text::FontManager::global()
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
                        let _ = window_ref
                            .request_inner_size(WindowSize::new(self.geom.os_w, self.geom.os_h));
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
                            Self::get_target_monitor(window_ref, self.config.monitor_index)
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
                            self.set_configured_window_position(window_ref, position_x, position_y);
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
        if let Some(monitor) = Self::get_target_monitor(window_ref, self.config.monitor_index) {
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
                self.set_configured_window_position(window_ref, position_x, position_y);
                self.geom.position_restore_after = Some(now + Duration::from_millis(750));
            }
        }
    }
}
