use std::time::Duration;

use windows::Win32::Foundation::HWND;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::windows::WindowExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::core::config::{DockPosition, MAX_HIDDEN_WIDTH, MAX_LYRIC_WIDTH, TOP_OFFSET};
use crate::utils::font::FontManager;

use super::{App, DEFAULT_ANIMATION_REFRESH_RATE_MILLIHERTZ, HideEdge, IslandLayout};

impl App {
    pub(super) fn get_target_monitor(
        window: &Window,
        monitor_index: i32,
    ) -> Option<winit::monitor::MonitorHandle> {
        if monitor_index < 0 {
            return window
                .primary_monitor()
                .or_else(|| window.current_monitor());
        }
        use windows::Win32::Graphics::Gdi::*;
        let mut win32_names: Vec<String> = Vec::new();
        // SAFETY: EnumDisplayDevicesW reads display device info. We provide a zeroed
        // DISPLAY_DEVICEW with correct cb size. idx increments safely. No mutable global state.
        unsafe {
            let mut idx = 0u32;
            loop {
                let mut dd: DISPLAY_DEVICEW = std::mem::zeroed();
                dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
                if EnumDisplayDevicesW(None, idx, &mut dd, 0).as_bool() {
                    if (dd.StateFlags & DISPLAY_DEVICE_ACTIVE) != DISPLAY_DEVICE_STATE_FLAGS(0) {
                        let name = String::from_utf16_lossy(&dd.DeviceName)
                            .trim_end_matches('\0')
                            .to_string();
                        win32_names.push(name);
                    }
                    idx += 1;
                } else {
                    break;
                }
            }
        }
        let target_name = win32_names.get(monitor_index as usize);
        let monitors: Vec<_> = window.available_monitors().collect();
        if let Some(name) = target_name {
            for mon in &monitors {
                if let Some(mon_name) = mon.name()
                    && (mon_name.contains(name.trim_start_matches("\\\\.\\"))
                        || name.contains(&mon_name))
                {
                    return Some(mon.clone());
                }
            }
        }
        let idx = monitor_index as usize;
        if idx < monitors.len() {
            monitors.get(idx).cloned()
        } else {
            window
                .primary_monitor()
                .or_else(|| window.current_monitor())
        }
    }

    pub(super) fn update_animation_frame_interval(
        &mut self,
        monitor: &winit::monitor::MonitorHandle,
    ) {
        let refresh_rate_millihertz = monitor
            .refresh_rate_millihertz()
            .filter(|refresh_rate| *refresh_rate > 0)
            .unwrap_or(DEFAULT_ANIMATION_REFRESH_RATE_MILLIHERTZ);
        let rate = if crate::utils::gpu::gpu_profile() == crate::utils::gpu::GpuProfile::Integrated
        {
            refresh_rate_millihertz.min(60_000)
        } else {
            refresh_rate_millihertz
        };
        self.animation_frame_interval =
            Duration::from_nanos(1_000_000_000_000u64 / u64::from(rate));
    }

    pub(super) fn enforce_overlay_window(window: &Window) {
        if let Ok(handle) = window.window_handle()
            && let RawWindowHandle::Win32(raw) = handle.as_raw()
        {
            let hwnd = HWND(raw.hwnd.get() as *mut core::ffi::c_void);
            crate::utils::win32::enforce_overlay_window_styles(hwnd);
            window.set_skip_taskbar(true);
        }
    }

    pub(super) fn set_configured_window_position(
        &mut self,
        window: &Window,
        position_x: i32,
        position_y: i32,
    ) {
        self.geom.configured_x = position_x;
        self.geom.configured_y = position_y;
        self.geom.win_x = position_x;
        self.geom.win_y = position_y;
        window.set_outer_position(PhysicalPosition::new(position_x, position_y));
    }

    pub(super) fn compute_window_position(
        &self,
        mon_pos: PhysicalPosition<i32>,
        mon_size: PhysicalSize<u32>,
    ) -> (i32, i32) {
        let dock_position = self.automatic_dock_position(mon_pos, mon_size);
        let (collapsed_center_x, collapsed_center_y) =
            self.collapsed_island_center(mon_pos, mon_size);
        let scale = self.config.global_scale as f64;
        let base_half_w = self.config.base_width as f64 * scale / 2.0;
        let base_half_h = self.config.base_height as f64 * scale / 2.0;

        let (anchor_x, local_anchor_x) = if dock_position.is_left() {
            (collapsed_center_x - base_half_w, TOP_OFFSET as f64)
        } else if dock_position.is_right() {
            (
                collapsed_center_x + base_half_w,
                self.geom.os_w as f64 - TOP_OFFSET as f64,
            )
        } else {
            (collapsed_center_x, self.geom.os_w as f64 / 2.0)
        };
        let (anchor_y, local_anchor_y) = if dock_position.is_bottom() {
            (
                collapsed_center_y + base_half_h,
                self.geom.os_h as f64 - TOP_OFFSET as f64,
            )
        } else {
            (collapsed_center_y - base_half_h, TOP_OFFSET as f64)
        };

        (
            (anchor_x - local_anchor_x).round() as i32,
            (anchor_y - local_anchor_y).round() as i32,
        )
    }

    fn collapsed_island_center(
        &self,
        mon_pos: PhysicalPosition<i32>,
        mon_size: PhysicalSize<u32>,
    ) -> (f64, f64) {
        let scale = self.config.global_scale as f64;
        (
            mon_pos.x as f64 + mon_size.width as f64 / 2.0 + self.config.position_x_offset as f64,
            mon_pos.y as f64
                + TOP_OFFSET as f64
                + self.config.base_height as f64 * scale / 2.0
                + self.config.position_y_offset as f64,
        )
    }

    fn automatic_dock_position(
        &self,
        mon_pos: PhysicalPosition<i32>,
        mon_size: PhysicalSize<u32>,
    ) -> DockPosition {
        let (center_x, center_y) = self.collapsed_island_center(mon_pos, mon_size);
        let scale = self.config.global_scale as f64;
        let base_half_h = self.config.base_height as f64 * scale / 2.0;
        let expanded_half_w = self.config.expanded_width as f64 * scale / 2.0;
        let expanded_h = self.config.expanded_height as f64 * scale;
        let horizontal = if center_x - expanded_half_w <= mon_pos.x as f64 {
            -1
        } else if center_x + expanded_half_w >= mon_pos.x as f64 + mon_size.width as f64 {
            1
        } else {
            0
        };
        let bottom =
            center_y - base_half_h + expanded_h >= mon_pos.y as f64 + mon_size.height as f64;

        match (bottom, horizontal) {
            (false, -1) => DockPosition::TopLeft,
            (false, 1) => DockPosition::TopRight,
            (false, _) => DockPosition::TopCenter,
            (true, -1) => DockPosition::BottomLeft,
            (true, 1) => DockPosition::BottomRight,
            (true, _) => DockPosition::BottomCenter,
        }
    }

    pub(super) fn migrate_legacy_dock_position(
        &mut self,
        mon_pos: PhysicalPosition<i32>,
        mon_size: PhysicalSize<u32>,
    ) -> bool {
        let Some(dock_position) = self.config.legacy_dock_position.take() else {
            return false;
        };
        let scale = self.config.global_scale as f64;
        let base_half_w = self.config.base_width as f64 * scale / 2.0;
        let base_half_h = self.config.base_height as f64 * scale / 2.0;
        let center_x = if dock_position.is_left() {
            mon_pos.x as f64
                + TOP_OFFSET as f64
                + self.config.position_x_offset as f64
                + base_half_w
        } else if dock_position.is_right() {
            mon_pos.x as f64 + mon_size.width as f64 - TOP_OFFSET as f64
                + self.config.position_x_offset as f64
                - base_half_w
        } else {
            mon_pos.x as f64 + mon_size.width as f64 / 2.0 + self.config.position_x_offset as f64
        };
        let center_y = if dock_position.is_bottom() {
            mon_pos.y as f64 + mon_size.height as f64 - TOP_OFFSET as f64
                + self.config.position_y_offset as f64
                - base_half_h
        } else {
            mon_pos.y as f64
                + TOP_OFFSET as f64
                + self.config.position_y_offset as f64
                + base_half_h
        };

        self.config.position_x_offset =
            (center_x - mon_pos.x as f64 - mon_size.width as f64 / 2.0).round() as i32;
        self.config.position_y_offset =
            (center_y - mon_pos.y as f64 - TOP_OFFSET as f64 - base_half_h).round() as i32;
        crate::core::persistence::save_config(&self.config);
        log::info!("Migrated legacy dock position to automatic placement");
        true
    }

    pub(super) fn nearest_hide_edge(&self) -> HideEdge {
        if self.geom.monitor_size.0 == 0 || self.geom.monitor_size.1 == 0 {
            return self.hide.edge;
        }
        let layout = self.compute_island_layout();
        let island_x = self.geom.win_x + layout.current_island_x.round() as i32;
        let island_y = self.geom.win_y + layout.current_island_y.round() as i32;
        let island_w = self.springs.w.value.round().max(1.0) as i32;
        let island_h = self.springs.h.value.round().max(1.0) as i32;
        let mon_right = self.geom.monitor_pos.0 + self.geom.monitor_size.0 as i32;
        let mon_bottom = self.geom.monitor_pos.1 + self.geom.monitor_size.1 as i32;
        [
            ((island_y - self.geom.monitor_pos.1).max(0), HideEdge::Top),
            ((mon_bottom - island_y - island_h).max(0), HideEdge::Bottom),
            ((island_x - self.geom.monitor_pos.0).max(0), HideEdge::Left),
            ((mon_right - island_x - island_w).max(0), HideEdge::Right),
        ]
        .into_iter()
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, edge)| edge)
        .unwrap_or(HideEdge::Top)
    }

    pub(super) fn snap_to_hide_edge(&mut self, window: &Window) {
        let Some(monitor) = Self::get_target_monitor(window, self.config.monitor_index) else {
            return;
        };
        let layout = self.compute_island_layout();
        let mon_pos = monitor.position();
        let mon_size = monitor.size();
        let mon_right = mon_pos.x + mon_size.width as i32;
        let mon_bottom = mon_pos.y + mon_size.height as i32;
        let island_w = self.springs.w.value.round() as i32;
        let island_h = self.springs.h.value.round() as i32;

        match self.hide.edge {
            HideEdge::Top => {
                self.geom.win_y = mon_pos.y + TOP_OFFSET - layout.island_y.round() as i32
            }
            HideEdge::Bottom => {
                self.geom.win_y =
                    mon_bottom - TOP_OFFSET - island_h - layout.island_y.round() as i32
            }
            HideEdge::Left => {
                self.geom.win_x = mon_pos.x + TOP_OFFSET - layout.offset_x.round() as i32
            }
            HideEdge::Right => {
                self.geom.win_x = mon_right - TOP_OFFSET - island_w - layout.offset_x.round() as i32
            }
        }
        window.set_outer_position(PhysicalPosition::new(self.geom.win_x, self.geom.win_y));
    }

    pub(super) fn restore_hide_origin(&mut self, window: &Window) {
        if self.springs.hide.value > 0.001 {
            return;
        }
        if let Some((win_x, win_y)) = self.hide.origin.take() {
            self.geom.win_x = win_x;
            self.geom.win_y = win_y;
            window.set_outer_position(PhysicalPosition::new(win_x, win_y));
        }
    }

    fn hidden_visible_width(&self, hide_edge: HideEdge) -> f64 {
        let edge_size = match hide_edge {
            HideEdge::Top | HideEdge::Bottom => self.springs.h.value as f64,
            HideEdge::Left | HideEdge::Right => self.springs.w.value as f64,
        };
        if self.config.hidden_width >= MAX_HIDDEN_WIDTH {
            edge_size
        } else {
            let configured = self.config.hidden_width as f64 * self.config.global_scale as f64;
            if configured <= f64::EPSILON {
                1.0_f64.min(edge_size)
            } else {
                configured.min(edge_size)
            }
        }
    }

    pub(super) fn can_hide_to_edge(&self, hide_edge: HideEdge) -> bool {
        let edge_size = match hide_edge {
            HideEdge::Top | HideEdge::Bottom => self.springs.h.value as f64,
            HideEdge::Left | HideEdge::Right => self.springs.w.value as f64,
        };
        edge_size - self.hidden_visible_width(hide_edge) > f64::EPSILON
    }

    pub(super) fn prepare_hide(&mut self, window: &Window, hide_edge: HideEdge) -> bool {
        if !self.can_hide_to_edge(hide_edge) {
            return false;
        }
        self.hide.edge = hide_edge;
        if self.hide.origin.is_none() {
            self.hide.origin = Some((self.geom.win_x, self.geom.win_y));
            self.snap_to_hide_edge(window);
        }
        true
    }

    pub(super) fn compute_island_layout(&self) -> IslandLayout {
        let dock_position = if self.geom.monitor_size.0 > 0 && self.geom.monitor_size.1 > 0 {
            self.automatic_dock_position(
                PhysicalPosition::new(self.geom.monitor_pos.0, self.geom.monitor_pos.1),
                PhysicalSize::new(self.geom.monitor_size.0, self.geom.monitor_size.1),
            )
        } else {
            DockPosition::TopCenter
        };
        let dock_bottom = dock_position.is_bottom();
        let island_y = if dock_bottom {
            self.geom.os_h as f64 - TOP_OFFSET as f64 - self.springs.h.value as f64
        } else {
            TOP_OFFSET as f64
        };

        let offset_x = if dock_position.is_left() {
            TOP_OFFSET as f64
        } else if dock_position.is_right() {
            (self.geom.os_w as f64 - TOP_OFFSET as f64 - self.springs.w.value as f64).max(0.0)
        } else {
            (self.geom.os_w as f64 - self.springs.w.value as f64) / 2.0
        };

        let hide_edge = self.hide.edge;
        let edge_size = match hide_edge {
            HideEdge::Top | HideEdge::Bottom => self.springs.h.value as f64,
            HideEdge::Left | HideEdge::Right => self.springs.w.value as f64,
        };
        let hidden_visible_width = self.hidden_visible_width(hide_edge);
        let concealed_width = (edge_size - hidden_visible_width).max(0.0);
        let hide_distance = if concealed_width > f64::EPSILON {
            concealed_width + TOP_OFFSET as f64
        } else {
            0.0
        };
        let content_hide_ratio = if edge_size > f64::EPSILON {
            (concealed_width / edge_size) as f32
        } else {
            0.0
        };
        let hide_offset = self.springs.hide.value as f64 * hide_distance;
        let (current_island_x, current_island_y) = match hide_edge {
            HideEdge::Top => (offset_x, island_y - hide_offset),
            HideEdge::Bottom => (offset_x, island_y + hide_offset),
            HideEdge::Left => (offset_x - hide_offset, island_y),
            HideEdge::Right => (offset_x + hide_offset, island_y),
        };
        let compact_content_h = self
            .compact_content_height()
            .min(self.springs.h.value)
            .max(0.0) as f64;
        let stable_base_y = if dock_bottom {
            self.geom.os_h as f64 - TOP_OFFSET as f64 - compact_content_h
        } else {
            TOP_OFFSET as f64
        };
        let stable_island_y = match hide_edge {
            HideEdge::Top => stable_base_y - hide_offset,
            HideEdge::Bottom => stable_base_y + hide_offset,
            HideEdge::Left | HideEdge::Right => stable_base_y,
        };
        let (hidden_reveal_x, hidden_reveal_y, hidden_reveal_w, hidden_reveal_h) = match hide_edge {
            HideEdge::Top => (
                current_island_x,
                current_island_y + self.springs.h.value as f64 - 1.0,
                self.springs.w.value as f64,
                1.0,
            ),
            HideEdge::Bottom => (
                current_island_x,
                current_island_y - 1.0,
                self.springs.w.value as f64,
                1.0,
            ),
            HideEdge::Left => (
                current_island_x + self.springs.w.value as f64 - 1.0,
                current_island_y,
                1.0,
                self.springs.h.value as f64,
            ),
            HideEdge::Right => (
                current_island_x - 1.0,
                current_island_y,
                1.0,
                self.springs.h.value as f64,
            ),
        };

        IslandLayout {
            offset_x,
            island_y,
            current_island_x,
            current_island_y,
            stable_island_y,
            hide_distance,
            content_hide_ratio,
            hidden_reveal_x,
            hidden_reveal_y,
            hidden_reveal_w,
            hidden_reveal_h,
        }
    }

    pub(super) fn compact_content_height(&self) -> f32 {
        let scale = self.config.global_scale.max(f32::EPSILON);
        let base_height = self.config.base_height * scale;
        let has_secondary_lyric = !self.lyrics.current_secondary_text.is_empty()
            || (!self.lyrics.old_secondary_text.is_empty() && self.lyrics.transition < 1.0);
        if !self.config.show_lyrics
            || !self.config.show_secondary_lyrics
            || !has_secondary_lyric
            || !matches!(
                self.ctx_mgr.current_mini(),
                Some(crate::core::context::MiniContent::Music)
            )
        {
            return base_height;
        }

        base_height.max(crate::core::render::mini_lyric_pair_height(
            self.config.font_size,
            scale,
        ))
    }

    fn displayed_lyric_texts(&self) -> (&str, &str) {
        let (primary, secondary) = if !self.lyrics.current_text.is_empty() {
            (
                self.lyrics.current_text.as_str(),
                self.lyrics.current_secondary_text.as_str(),
            )
        } else {
            (
                self.lyrics.old_text.as_str(),
                self.lyrics.old_secondary_text.as_str(),
            )
        };
        let secondary = if self.config.show_secondary_lyrics {
            secondary
        } else {
            ""
        };
        (primary, secondary)
    }

    pub(super) fn measure_lyric_text_width(&self, text: &str) -> f32 {
        let scale = self.config.global_scale.max(f32::EPSILON);
        let font_size = crate::core::render::mini_lyric_font_size(self.config.font_size, scale);
        FontManager::global().measure_text_cached(text, font_size, skia_safe::FontStyle::normal())
            / scale
    }

    fn measure_lyric_pair_width(&self, primary: &str, secondary: &str) -> f32 {
        self.measure_lyric_text_width(primary)
            .max(self.measure_lyric_text_width(secondary))
    }

    pub(super) fn compute_lyric_target_width(
        &mut self,
        window: &Window,
        music_active: bool,
        is_paused: bool,
        dt: f32,
    ) -> f32 {
        let target_base_w = if music_active && !self.expanded && !self.is_width_hiding() {
            let has_visible_lyrics = self.config.show_lyrics
                && (!self.lyrics.current_text.is_empty()
                    || (!self.lyrics.old_text.is_empty() && self.lyrics.transition < 1.0));

            if has_visible_lyrics {
                if self.config.lyrics_scroll {
                    let (primary, secondary) = self.displayed_lyric_texts();
                    let text_w = self.measure_lyric_pair_width(primary, secondary);
                    let natural_w = 60.0 + text_w;
                    let max_w = self.config.lyrics_scroll_max_width;
                    if natural_w > max_w {
                        let fixed_w = max_w;
                        let available_text_w = (fixed_w - 59.0) * self.config.global_scale;
                        let full_text_w = text_w * self.config.global_scale;
                        let overflow = full_text_w - available_text_w;
                        if overflow > 0.0 && self.lyrics.transition >= 1.0 && !is_paused {
                            if self.lyrics.scroll_offset < overflow {
                                if self.lyrics.scroll_pause > 0.0 {
                                    self.lyrics.scroll_pause -= dt / 60.0;
                                } else {
                                    self.lyrics.scroll_offset += 0.8 * dt;
                                    if self.lyrics.scroll_offset >= overflow {
                                        self.lyrics.scroll_offset = overflow;
                                    }
                                }
                                window.request_redraw();
                            }
                        } else {
                            self.lyrics.scroll_offset = 0.0;
                        }
                        fixed_w
                    } else {
                        self.lyrics.scroll_offset = 0.0;
                        let min_w = self.config.base_width + 35.0;
                        natural_w.clamp(min_w.min(max_w), max_w)
                    }
                } else {
                    let (primary, secondary) = self.displayed_lyric_texts();
                    let text_w = self.measure_lyric_pair_width(primary, secondary);
                    self.lyrics.scroll_offset = 0.0;
                    let min_w = self.config.base_width + 35.0;
                    let w: f32 = 60.0 + text_w;
                    w.clamp(min_w.min(MAX_LYRIC_WIDTH), MAX_LYRIC_WIDTH)
                }
            } else {
                self.config.base_width + 35.0
            }
        } else {
            self.lyrics.scroll_offset = 0.0;
            self.config.base_width
        };
        (if self.expanded {
            self.config.expanded_width
        } else {
            target_base_w
        }) * self.config.global_scale
    }
}
