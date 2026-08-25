use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows::ApplicationModel::Package;
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::PCWSTR;
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::core::config::{MAX_LYRIC_WIDTH, PADDING};
use crate::core::persistence::{get_config_path, load_config};
use crate::plugin::marketplace::{self, MarketplacePlugin};
use crate::plugin::zip_loader;
use crate::window::d3d::MAIN_D3D_TARGET;
use crate::window::tray::TrayAction;

use super::App;

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
                                crate::core::i18n::tr("plugin_state_restart"),
                                true,
                            );
                        }
                        Err(error) => settings.set_plugin_status(
                            crate::core::i18n::tr_args("plugin_state_failed", &[&error]),
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
                                crate::core::i18n::tr("plugin_uninstalled"),
                                false,
                            );
                        }
                        Err(error) => settings.set_plugin_status(
                            crate::core::i18n::tr_args("plugin_uninstall_failed", &[&error]),
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
        let mut renderer = self.renderer.take();
        if renderer.is_some() {
            log::warn!("D3D12 renderer invalidated: {reason}");
        }
        if let Some(settings) = self.settings.as_mut() {
            settings.invalidate_renderer_target();
        }
        crate::utils::backdrop::clear_mica_cache();
        crate::utils::backdrop::clear_blurred_cover_cache();
        crate::utils::glass::clear_glass_cache();
        crate::ui::expanded::music_view::clear_cover_cache();
        if let Some(renderer) = renderer.as_mut() {
            renderer.abandon();
        }
        drop(renderer);
        self.renderer_retry_at = Some(now);
        self.next_frame_deadline = now;
    }

    pub(super) fn recover_renderer(
        &mut self,
        window: &Window,
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

        match crate::window::d3d::D3DRenderer::try_new(window, self.geom.os_w, self.geom.os_h) {
            Ok(mut renderer) => {
                if let Some(settings) = self.settings.as_mut()
                    && let Err(error) = settings.recreate_renderer_target(&mut renderer)
                {
                    log::warn!("D3D12 settings renderer recovery failed: {error}");
                    self.renderer_retry_at = Some(now + retry_interval);
                    self.next_frame_deadline = now + retry_interval;
                    return;
                }
                self.renderer = Some(renderer);
                self.renderer_retry_at = None;
                self.last_render_time = now;
                window.request_redraw();
                log::info!("D3D12 renderer recovered");
            }
            Err(error) => {
                log::warn!("D3D12 renderer recovery failed: {error}");
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
                    log::error!("Toast template failed: {:?}", e);
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
                log::error!("CreateToastNotification failed: {:?}", e);
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
                log::error!("CreateToastNotifier failed: {:?}", e);
                return;
            }
        };
        if let Err(e) = notifier.Show(&toast) {
            log::error!("Toast Show failed: {:?}", e);
        }
    }

    pub(super) fn install_zip_drop(&mut self, path: &Path) {
        if self.pending_install.is_some() || self.pending_marketplace_download.is_some() {
            Self::show_toast("Plugin Info", "Another installation is already in progress");
            if let Some(settings) = self.settings.as_mut() {
                settings.set_plugin_status(
                    crate::core::i18n::tr_args(
                        "plugin_install_failed",
                        &["another installation is already in progress"],
                    ),
                    false,
                );
            }
            return;
        }

        let plugin_dir = self.plugin_mgr.plugin_dir().to_path_buf();
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
                    crate::core::i18n::tr("plugin_marketplace_incompatible"),
                    false,
                );
            }
            return;
        }
        if self.pending_install.is_some() || self.pending_marketplace_download.is_some() {
            if let Some(settings) = self.settings.as_mut() {
                settings.finish_marketplace_install();
                settings.set_plugin_status(
                    crate::core::i18n::tr_args(
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
                crate::core::i18n::tr("plugin_marketplace_downloading"),
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
        if crate::core::config::normalize_active_plugin_widget_layout(
            &config.widget_layout,
            &mut config.plugin_widget_layout,
            &plugin_widgets,
        ) {
            crate::core::persistence::save_config(&config);
        }
        let mut settings =
            crate::window::settings::SettingsApp::new(config, Vec::new(), plugin_widgets);
        let Some(renderer) = self.renderer.as_mut() else {
            log::error!("Cannot open settings without the shared D3D12 renderer");
            return;
        };
        settings.create_window(event_loop, renderer);
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
                    let old_scale = self.config.global_scale;
                    let old_max_w = self.config.expanded_width;
                    let old_max_h = self.config.expanded_height;
                    let old_style = self.config.island_style.clone();
                    let old_mini_shape = self.config.mini_cover_shape.clone();
                    let old_expanded_shape = self.config.expanded_cover_shape.clone();
                    let old_font = self.config.custom_font_path.clone();
                    let old_smtc_enabled = self.config.smtc_enabled;
                    let old_replace_native_volume_flyout = self.config.replace_native_volume_flyout;
                    let old_position_x_offset = self.config.position_x_offset;
                    let old_position_y_offset = self.config.position_y_offset;
                    let old_monitor_index = self.config.monitor_index;

                    log::info!("Config changed, reloaded");
                    self.config = current_config;
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
                    if old_smtc_enabled != self.config.smtc_enabled {
                        self.smtc.set_enabled(self.config.smtc_enabled);
                        self.audio.set_target_app_id(self.audio_target_app_id());
                    }

                    if old_style != self.config.island_style {
                        crate::utils::backdrop::clear_mica_cache();
                        crate::utils::glass::clear_glass_cache();
                        crate::utils::backdrop::clear_blurred_cover_cache();
                        if let Ok(handle) = window.window_handle() {
                            let raw = handle.as_raw();
                            if let RawWindowHandle::Win32(win32_handle) = raw {
                                let hwnd =
                                    windows::Win32::Foundation::HWND(win32_handle.hwnd.get() as _);
                                if old_style == "mica" {
                                    crate::utils::backdrop::disable_mica(hwnd);
                                }
                            }
                        }
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

                    let compact_max_w = crate::ui::widget::compact::target_width(
                        &self.config.compact_widget_layout,
                        self.config.base_width,
                        Some(MAX_LYRIC_WIDTH),
                    );
                    let max_w = self.config.expanded_width.max(compact_max_w);
                    let new_os_w = (max_w * self.config.global_scale + PADDING) as u32;
                    let new_os_h =
                        (self.config.expanded_height * self.config.global_scale + PADDING) as u32;

                    let size_changed = new_os_w != self.geom.os_w
                        || new_os_h != self.geom.os_h
                        || (old_scale - self.config.global_scale).abs() > 0.001
                        || (old_max_w - self.config.expanded_width).abs() > 0.1
                        || (old_max_h - self.config.expanded_height).abs() > 0.1;
                    let position_changed = old_position_x_offset != self.config.position_x_offset
                        || old_position_y_offset != self.config.position_y_offset
                        || old_monitor_index != self.config.monitor_index;

                    if size_changed {
                        self.geom.os_w = new_os_w;
                        self.geom.os_h = new_os_h;
                        let _ = window
                            .request_inner_size(PhysicalSize::new(self.geom.os_w, self.geom.os_h));
                        if let Some(renderer) = self.renderer.as_mut() {
                            if let Err(error) =
                                renderer.resize(MAIN_D3D_TARGET, self.geom.os_w, self.geom.os_h)
                            {
                                log::error!("D3D12 renderer resize failed: {error}");
                            } else {
                                crate::utils::backdrop::clear_mica_cache();
                                crate::utils::glass::clear_glass_cache();
                            }
                        }
                    }

                    if (size_changed || position_changed)
                        && let Some(monitor) =
                            Self::get_target_monitor(window, self.config.monitor_index)
                    {
                        let mon_size = monitor.size();
                        let mon_pos = monitor.position();
                        self.update_animation_frame_interval(&monitor);
                        if mon_size.width > 0 && mon_size.height > 0 {
                            self.geom.monitor_size = (mon_size.width, mon_size.height);
                            self.geom.monitor_pos = (mon_pos.x, mon_pos.y);
                            let (position_x, position_y) =
                                self.compute_window_position(mon_pos, mon_size);
                            self.set_configured_window_position(window, position_x, position_y);
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
