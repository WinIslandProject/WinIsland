use crate::core::config::AppConfig;
use crate::core::plugin_widget::PluginWidget;
use crate::plugin::manager::InstalledPlugin;
use crate::plugin::marketplace::{MarketplaceCatalog, MarketplacePlugin};
use crate::utils::anim::AnimPool;
use crate::utils::color::{SettingsTheme, dark_settings_theme, light_settings_theme};
use crate::utils::icon::get_app_icon;
use crate::utils::settings_ui::items::{POPUP_MENU_R, SIDEBAR_PAD, SettingsItem};
use crate::utils::settings_ui::{SwitchAnimator, WidgetEditorMode, WidgetEditorSlot, WidgetSource};
use crate::window::d3d::{D3DRenderer, D3DTargetId};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowButtons, WindowId};

pub mod input;
pub mod items;
pub mod pages;
mod popup;
pub mod renderer;
pub mod sidebar;

pub(crate) use popup::PopupState;

pub(crate) const WIN_W: f32 = 760.0;
pub(crate) const WIN_H: f32 = 680.0;
pub(crate) const SIDEBAR_W: f32 = 184.0;
pub(crate) const SIDEBAR_ROW_H: f32 = 34.0;
pub(crate) const SIDEBAR_ROW_GAP: f32 = 2.0;
pub(crate) const SIDEBAR_START_Y: f32 = 64.0;
pub(crate) const SIDEBAR_PAGE_COUNT: usize = 5;
pub(crate) const GENERAL_PAGE_INDEX: usize = 0;
pub(crate) const WIDGETS_PAGE_INDEX: usize = 2;
pub(crate) const PLUGINS_PAGE_INDEX: usize = 3;
pub(crate) const PAGE_NAV_X: f32 = SIDEBAR_W + 18.0;
pub(crate) const PAGE_NAV_Y: f32 = 18.0;
pub(crate) const PAGE_NAV_SIZE: f32 = 28.0;
pub(crate) const PAGE_NAV_GAP: f32 = 4.0;
pub(crate) const SETTINGS_HEADER_H: f32 = 64.0;
pub(crate) const WINDOW_RADIUS: f32 = 16.0;
pub(crate) const WINDOW_CONTROL_CENTERS: [(f32, f32); 3] =
    [(20.0, 20.0), (40.0, 20.0), (60.0, 20.0)];
pub(crate) const WINDOW_CONTROL_RADIUS: f32 = 6.0;
const WINDOW_CONTROL_HIT_RADIUS: f32 = 8.0;
const SCROLLBAR_BOTTOM_INSET: f32 = 8.0;
const SCROLLBAR_RIGHT_INSET: f32 = 5.0;
const SCROLLBAR_THUMB_MIN_H: f32 = 32.0;
const SCROLLBAR_TRACK_TOP_INSET: f32 = 8.0;
const SCROLLBAR_W: f32 = 4.0;
const SCROLLBAR_HIT_W: f32 = 16.0;
const CURSOR_MOVE_THRESHOLD: f32 = 0.5;
const MOUSE_WHEEL_LINE_HEIGHT: f32 = 40.0;
const SIDEBAR_TITLE_HEIGHT: f32 = 60.0;
const POPUP_CLOSE_SPEED: f32 = 0.3;
const WINDOW_CONTROL_CLOSE: usize = 0;
const WINDOW_CONTROL_MINIMIZE: usize = 1;

pub(crate) fn window_control_at(x: f32, y: f32) -> Option<usize> {
    WINDOW_CONTROL_CENTERS.iter().position(|&(cx, cy)| {
        (x - cx).powi(2) + (y - cy).powi(2) <= WINDOW_CONTROL_HIT_RADIUS.powi(2)
    })
}

pub(crate) fn window_controls_hovered(x: f32, y: f32) -> bool {
    (10.0..=70.0).contains(&x) && (10.0..=30.0).contains(&y)
}

fn scroll_delta(delta: MouseScrollDelta) -> f32 {
    match delta {
        MouseScrollDelta::LineDelta(_, lines) => lines * MOUSE_WHEEL_LINE_HEIGHT,
        MouseScrollDelta::PixelDelta(position) => position.y as f32,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ScrollbarGeometry {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    track_y: f32,
    track_height: f32,
}

impl ScrollbarGeometry {
    pub(crate) fn hit_test(&self, x: f32, y: f32) -> bool {
        x >= self.x + self.width - SCROLLBAR_HIT_W
            && x <= self.x + self.width + SCROLLBAR_RIGHT_INSET
            && y >= self.y
            && y <= self.y + self.height
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PageNavigation {
    Back,
    Forward,
}

pub(crate) const POPUP_OPACITY_KEY: u64 = 1;
pub(crate) const SIDEBAR_KEY_BASE: u64 = 1_000;
pub(crate) const PLUGIN_DETAIL_KEY: u64 = 2_000;
pub(crate) const SCROLL_STIFFNESS: f32 = 55.0;
pub(crate) const SCROLL_DAMPING: f32 = 16.0;

pub(crate) fn widget_drag_move_needs_redraw<T: PartialEq>(
    dragging: bool,
    current_slot: Option<T>,
    new_slot: Option<T>,
) -> bool {
    dragging || current_slot != new_slot
}

pub(crate) fn settings_frame_should_continue(
    has_anim: bool,
    has_popup: bool,
    is_scrolling: bool,
    is_widget_dragging: bool,
    is_number_input_active: bool,
) -> bool {
    has_anim || has_popup || is_scrolling || is_widget_dragging || is_number_input_active
}

pub(crate) type NumberInputHandler = fn(&mut SettingsApp, &str);

pub(crate) struct NumberInput {
    pub(crate) rect: skia_safe::Rect,
    pub(crate) text: String,
    pub(crate) on_commit: NumberInputHandler,
}

pub(crate) enum PluginSettingsRequest {
    Install(std::path::PathBuf),
    LoadMarketplace,
    InstallMarketplace(Box<MarketplacePlugin>),
    SetEnabled { id: String, enabled: bool },
    Uninstall { id: String },
    Restart,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PluginPageTab {
    Installed,
    Marketplace,
}

pub(crate) enum MarketplaceViewState {
    NotLoaded,
    Loading,
    Loaded(Vec<MarketplacePlugin>),
    Failed(String),
}

pub struct SettingsApp {
    pub(crate) window: Option<Arc<Window>>,
    pub(crate) renderer_target: Option<D3DTargetId>,
    pub(crate) config: AppConfig,
    pub(crate) active_page: usize,
    pub(crate) page_history: Vec<usize>,
    pub(crate) page_history_index: usize,
    pub(crate) switch_anim: SwitchAnimator,
    pub(crate) switch_anim_context: (usize, usize),
    pub(crate) anim: AnimPool,
    pub(crate) logical_mouse_pos: (f32, f32),
    pub(crate) last_hover_mouse_pos: (f32, f32),
    pub(crate) frame_count: u64,
    pub(crate) scroll_y: f32,
    pub(crate) target_scroll_y: f32,
    pub(crate) scroll_vel_y: f32,
    pub(crate) last_frame_time: Instant,
    pub(crate) next_frame_deadline: Instant,
    pub(crate) detected_apps: Vec<String>,
    detected_apps_rx: Option<mpsc::Receiver<Vec<String>>>,
    pub(crate) sidebar_hover: i32,
    pub(crate) popup: Option<PopupState>,
    pub(crate) number_input: Option<NumberInput>,
    pub(crate) is_light: bool,
    pub(crate) cached_items: Vec<SettingsItem>,
    pub(crate) items_dirty: bool,
    pub(crate) cached_content_height: f32,
    pub(crate) cached_max_scroll: f32,
    pub(crate) win_w: f32,
    pub(crate) win_h: f32,
    pub(crate) focused: bool,
    pub(crate) dots_hovered: bool,
    pub(crate) scroll_dragging: bool,
    scroll_drag_offset: f32,
    pub(crate) widget_dragging: Option<WidgetSource>,
    pub(crate) widget_drag_hover_slot: Option<WidgetEditorSlot>,
    pub(crate) widget_preview_hover_slot: Option<WidgetEditorSlot>,
    pub(crate) widget_editor_mode: WidgetEditorMode,
    pub(crate) compact_widget_dragging: Option<crate::core::config::CompactWidgetKind>,
    pub(crate) plugin_widgets: Vec<PluginWidget>,
    pub(crate) plugins: Vec<InstalledPlugin>,
    plugin_inventory_rx: Option<mpsc::Receiver<Vec<InstalledPlugin>>>,
    pub(crate) plugin_page_tab: PluginPageTab,
    pub(crate) marketplace_state: MarketplaceViewState,
    pub(crate) marketplace_installing_id: Option<String>,
    pub(crate) pending_plugin_uninstall_id: Option<String>,
    pub(crate) selected_plugin_id: Option<String>,
    pub(crate) plugin_detail_closing: bool,
    pub(crate) plugin_detail_scroll: f32,
    pub(crate) plugin_detail_max_scroll: f32,
    pub(crate) plugin_status: Option<(String, bool)>,
    plugin_request: Option<PluginSettingsRequest>,
    close_requested: bool,
}

impl SettingsApp {
    pub(crate) fn window_scale(&self) -> f32 {
        self.window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0)
    }

    pub(crate) fn logical_window_size(&self) -> (f32, f32) {
        let scale = self.window_scale();
        (self.win_w / scale, self.win_h / scale)
    }

    pub(crate) fn content_width(&self) -> f32 {
        self.logical_window_size().0 - SIDEBAR_W
    }

    pub fn new(
        config: AppConfig,
        plugins: Vec<InstalledPlugin>,
        plugin_widgets: Vec<PluginWidget>,
    ) -> Self {
        let switch_anim = SwitchAnimator::new(&[]);
        let detected_apps = config.smtc_known_apps.clone();
        Self {
            window: None,
            renderer_target: None,
            config,
            active_page: GENERAL_PAGE_INDEX,
            page_history: vec![0],
            page_history_index: 0,
            switch_anim,
            switch_anim_context: (usize::MAX, usize::MAX),
            anim: AnimPool::new(),
            logical_mouse_pos: (0.0, 0.0),
            last_hover_mouse_pos: (-1.0, -1.0),
            frame_count: 0,
            scroll_y: 0.0,
            target_scroll_y: 0.0,
            scroll_vel_y: 0.0,
            last_frame_time: Instant::now(),
            next_frame_deadline: Instant::now(),
            detected_apps,
            detected_apps_rx: None,
            sidebar_hover: -1,
            popup: None,
            number_input: None,
            is_light: false,
            cached_items: Vec::new(),
            items_dirty: true,
            cached_content_height: 0.0,
            cached_max_scroll: 0.0,
            win_w: WIN_W,
            win_h: WIN_H,
            focused: true,
            dots_hovered: false,
            scroll_dragging: false,
            scroll_drag_offset: 0.0,
            widget_dragging: None,
            widget_drag_hover_slot: None,
            widget_preview_hover_slot: None,
            widget_editor_mode: WidgetEditorMode::Expanded,
            compact_widget_dragging: None,
            plugin_widgets,
            plugins,
            plugin_inventory_rx: None,
            plugin_page_tab: PluginPageTab::Installed,
            marketplace_state: MarketplaceViewState::NotLoaded,
            marketplace_installing_id: None,
            pending_plugin_uninstall_id: None,
            selected_plugin_id: None,
            plugin_detail_closing: false,
            plugin_detail_scroll: 0.0,
            plugin_detail_max_scroll: 0.0,
            plugin_status: None,
            plugin_request: None,
            close_requested: false,
        }
    }

    pub(crate) fn theme(&self) -> SettingsTheme {
        if self.is_light {
            light_settings_theme()
        } else {
            dark_settings_theme()
        }
    }

    pub(crate) fn update_theme(&mut self) {
        self.is_light = match self.config.settings_theme.as_str() {
            "light" => true,
            "dark" => false,
            _ => {
                if let Some(win) = &self.window {
                    win.theme() == Some(winit::window::Theme::Light)
                } else {
                    false
                }
            }
        };
        if let Some(win) = &self.window {
            Self::apply_titlebar_theme(win, self.is_light);
            win.request_redraw();
        }
    }

    pub(crate) fn apply_titlebar_theme(window: &Window, is_light: bool) {
        if let Ok(handle) = window.window_handle()
            && let RawWindowHandle::Win32(raw) = handle.as_raw()
        {
            let hwnd = HWND(raw.hwnd.get() as _);
            let use_dark: i32 = if is_light { 0 } else { 1 };
            unsafe {
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWINDOWATTRIBUTE(20),
                    &use_dark as *const _ as *const _,
                    std::mem::size_of::<i32>() as u32,
                );
            }
        }
    }

    pub(crate) fn get_monitor_list() -> Vec<String> {
        use windows::Win32::Graphics::Gdi::{
            DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_STATE_FLAGS, DISPLAY_DEVICEW, EnumDisplayDevicesW,
        };
        let mut monitors: Vec<String> = Vec::new();
        unsafe {
            let mut idx = 0u32;
            let mut active_count = 0;
            loop {
                let mut dd: DISPLAY_DEVICEW = std::mem::zeroed();
                dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
                if EnumDisplayDevicesW(None, idx, &mut dd, 0).as_bool() {
                    if (dd.StateFlags & DISPLAY_DEVICE_ACTIVE) != DISPLAY_DEVICE_STATE_FLAGS(0) {
                        active_count += 1;
                        let name = String::from_utf16_lossy(&dd.DeviceName)
                            .trim_end_matches('\0')
                            .to_string();
                        let mut dm: DISPLAY_DEVICEW = std::mem::zeroed();
                        dm.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
                        let mut label = if EnumDisplayDevicesW(
                            windows::core::PCWSTR(dd.DeviceName.as_ptr()),
                            0,
                            &mut dm,
                            0,
                        )
                        .as_bool()
                        {
                            let friendly = String::from_utf16_lossy(&dm.DeviceString)
                                .trim_end_matches('\0')
                                .to_string();
                            if friendly.is_empty() {
                                name.clone()
                            } else {
                                friendly
                            }
                        } else {
                            name.clone()
                        };
                        label = format!("Display {active_count}: {label}");
                        monitors.push(label);
                    }
                    idx += 1;
                } else {
                    break;
                }
            }
        }
        if monitors.is_empty() {
            monitors.push("Primary".to_string());
        }
        monitors
    }

    pub(crate) fn update_detected_apps(&mut self) {
        let mut changed = false;
        for app in &self.config.smtc_known_apps {
            if !self.detected_apps.contains(app) {
                self.detected_apps.push(app.clone());
                changed = true;
            }
        }
        if changed {
            self.items_dirty = true;
        }
        if self.detected_apps_rx.is_none() {
            self.detected_apps_rx = Some(crate::core::smtc::detect_active_apps_async());
        }
    }

    fn poll_detected_apps(&mut self) {
        let Some(rx) = self.detected_apps_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(apps) => {
                let mut changed = false;
                for app in apps {
                    if !self.detected_apps.contains(&app) {
                        self.detected_apps.push(app);
                        changed = true;
                    }
                }
                if changed {
                    self.items_dirty = true;
                    self.request_redraw();
                }
            }
            Err(mpsc::TryRecvError::Empty) => self.detected_apps_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {}
        }
    }

    fn poll_plugin_inventory(&mut self) {
        let Some(rx) = self.plugin_inventory_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(plugins) => self.set_plugins(plugins),
            Err(mpsc::TryRecvError::Empty) => self.plugin_inventory_rx = Some(rx),
            Err(mpsc::TryRecvError::Disconnected) => {}
        }
    }
}

impl SettingsApp {
    pub(crate) fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        renderer: &mut D3DRenderer,
    ) {
        let attrs = Window::default_attributes()
            .with_title("WinIsland Settings")
            .with_inner_size(LogicalSize::new(WIN_W as f64, WIN_H as f64))
            .with_resizable(true)
            .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE)
            .with_decorations(false)
            .with_transparent(true)
            .with_no_redirection_bitmap(true)
            .with_window_icon(get_app_icon());
        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(window.clone());
        let size = window.inner_size();
        self.win_w = size.width as f32;
        self.win_h = size.height as f32;
        self.renderer_target = match renderer.create_target(&window, size.width, size.height) {
            Ok(target) => Some(target),
            Err(error) => {
                log::error!("D3D12 settings renderer initialization failed: {error}");
                self.close_requested = true;
                return;
            }
        };
        self.close_requested = false;
        self.next_frame_deadline = Instant::now();
        self.update_theme();
        self.update_detected_apps();
    }

    pub(crate) fn invalidate_renderer_target(&mut self) {
        self.renderer_target = None;
        sidebar::clear_sidebar_icon_cache();
        pages::plugins::clear_plugin_icon_cache();
    }

    pub(crate) fn recreate_renderer_target(
        &mut self,
        renderer: &mut D3DRenderer,
    ) -> Result<(), String> {
        let Some(window) = self.window.as_ref() else {
            return Ok(());
        };
        let size = window.inner_size();
        self.renderer_target = Some(renderer.create_target(window, size.width, size.height)?);
        window.request_redraw();
        Ok(())
    }

    pub(crate) fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
        renderer: &mut D3DRenderer,
    ) {
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => self.close_requested = true,
            WindowEvent::Focused(focused) => self.handle_focus_changed(focused),
            WindowEvent::ThemeChanged(theme) if self.config.settings_theme == "system" => {
                self.handle_system_theme_changed(theme);
            }
            WindowEvent::Resized(_)
                if self
                    .window
                    .as_ref()
                    .is_some_and(|window| window.is_maximized()) =>
            {
                if let Some(window) = &self.window {
                    window.set_maximized(false);
                }
            }
            WindowEvent::Resized(new_size) => self.handle_resized(renderer, new_size),
            WindowEvent::ScaleFactorChanged { .. } => self.handle_scale_changed(renderer),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.handle_pressed_key(&event.logical_key);
            }
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor_moved(position),
            WindowEvent::CursorLeft { .. } => self.handle_cursor_left(),
            WindowEvent::DroppedFile(path)
                if self.active_page == PLUGINS_PAGE_INDEX
                    && path
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip")) =>
            {
                self.handle_plugin_file_drop(path);
            }
            WindowEvent::MouseWheel { delta, .. } => self.handle_mouse_wheel(delta),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.handle_left_mouse_pressed(event_loop),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.handle_left_mouse_released(),
            WindowEvent::RedrawRequested => self.draw(renderer),
            _ => (),
        }
    }

    fn handle_focus_changed(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.commit_number_input();
            self.dots_hovered = false;
            self.scroll_dragging = false;
        }
        self.request_redraw();
    }

    fn handle_system_theme_changed(&mut self, theme: winit::window::Theme) {
        self.is_light = theme == winit::window::Theme::Light;
        if let Some(window) = &self.window {
            Self::apply_titlebar_theme(window, self.is_light);
        }
        self.request_redraw();
    }

    fn handle_resized(&mut self, renderer: &mut D3DRenderer, size: PhysicalSize<u32>) {
        self.win_w = size.width as f32;
        self.win_h = size.height as f32;
        self.resize_renderer_target(renderer, size);
        self.request_redraw();
    }

    fn handle_scale_changed(&mut self, renderer: &mut D3DRenderer) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        self.resize_renderer_target(renderer, size);
        self.request_redraw();
    }

    fn resize_renderer_target(&mut self, renderer: &mut D3DRenderer, size: PhysicalSize<u32>) {
        let Some(target) = self.renderer_target else {
            return;
        };
        if let Err(error) = renderer.resize(target, size.width, size.height) {
            log::error!("D3D12 settings renderer resize failed: {error}");
            self.close_requested = true;
        }
    }

    fn handle_pressed_key(&mut self, key: &Key) {
        if self.handle_number_input_key(key) {
            return;
        }
        match key {
            Key::Named(NamedKey::ArrowLeft) => {
                self.navigate_page_history(PageNavigation::Back);
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.navigate_page_history(PageNavigation::Forward);
            }
            _ => {}
        }
    }

    fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let scale = self.window_scale();
        let new_position = (position.x as f32 / scale, position.y as f32 / scale);
        let mouse_moved = (new_position.0 - self.last_hover_mouse_pos.0).abs()
            > CURSOR_MOVE_THRESHOLD
            || (new_position.1 - self.last_hover_mouse_pos.1).abs() > CURSOR_MOVE_THRESHOLD;
        self.logical_mouse_pos = new_position;
        self.update_scroll_drag(new_position.1);

        let mut redraw =
            matches!(self.active_page, WIDGETS_PAGE_INDEX | PLUGINS_PAGE_INDEX) && mouse_moved;
        let dots_hovered = self.focused && window_controls_hovered(new_position.0, new_position.1);
        if dots_hovered != self.dots_hovered {
            self.dots_hovered = dots_hovered;
            redraw = true;
        }
        redraw |= self.update_widget_hover();
        redraw |= self.update_popup_hover();
        if mouse_moved {
            self.last_hover_mouse_pos = new_position;
            redraw |= self.update_sidebar_hover();
        }
        if redraw {
            self.request_redraw();
        }

        let cursor = if self.get_hover_state() {
            winit::window::CursorIcon::Pointer
        } else {
            winit::window::CursorIcon::Default
        };
        if let Some(window) = &self.window {
            window.set_cursor(cursor);
        }
    }

    fn update_widget_hover(&mut self) -> bool {
        if self.widget_drag_active() {
            let new_slot = self.widget_preview_slot_at_mouse();
            let current_slot = self.active_widget_drag_hover_slot();
            if new_slot != current_slot {
                self.set_active_widget_drag_hover_slot(new_slot);
            }
            return widget_drag_move_needs_redraw(true, current_slot, new_slot);
        }
        if self.active_page != WIDGETS_PAGE_INDEX {
            return false;
        }
        let new_slot = self.widget_preview_slot_at_mouse();
        let current_slot = self.active_widget_preview_hover_slot();
        if new_slot == current_slot {
            return false;
        }
        self.set_active_widget_preview_hover_slot(new_slot);
        true
    }

    fn update_popup_hover(&mut self) -> bool {
        let Some(popup) = &mut self.popup else {
            return false;
        };
        let (mouse_x, mouse_y) = self.logical_mouse_pos;
        let hover_index = popup.hit_test_item(mouse_x, mouse_y);
        if hover_index == popup.hover_idx {
            return false;
        }
        popup.hover_idx = hover_index;
        true
    }

    fn update_sidebar_hover(&mut self) -> bool {
        let (mouse_x, mouse_y) = self.logical_mouse_pos;
        let hover_index = (mouse_x < SIDEBAR_W).then(|| {
            (0..SIDEBAR_PAGE_COUNT).find(|index| {
                let row_y = SIDEBAR_START_Y + *index as f32 * (SIDEBAR_ROW_H + SIDEBAR_ROW_GAP);
                (row_y..=row_y + SIDEBAR_ROW_H).contains(&mouse_y)
                    && (SIDEBAR_PAD..=SIDEBAR_W - SIDEBAR_PAD).contains(&mouse_x)
            })
        });
        let hover_index = hover_index.flatten();
        let sidebar_hover = hover_index.map_or(-1, |index| index as i32);
        if sidebar_hover == self.sidebar_hover {
            return false;
        }
        self.sidebar_hover = sidebar_hover;
        for index in 0..SIDEBAR_PAGE_COUNT {
            self.anim.set(
                SIDEBAR_KEY_BASE + index as u64,
                if hover_index == Some(index) { 1.0 } else { 0.0 },
            );
        }
        true
    }

    fn handle_cursor_left(&mut self) {
        if self.dots_hovered {
            self.dots_hovered = false;
            self.request_redraw();
        }
    }

    fn handle_plugin_file_drop(&mut self, path: std::path::PathBuf) {
        self.plugin_status = Some((crate::core::i18n::tr("plugin_installing"), false));
        self.plugin_request = Some(PluginSettingsRequest::Install(path));
        self.mark_items_dirty();
        self.request_redraw();
    }

    fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        if self.popup.is_some() {
            self.popup = None;
            self.anim
                .set_with_speed(POPUP_OPACITY_KEY, 0.0, POPUP_CLOSE_SPEED);
            self.request_redraw();
            return;
        }
        let delta = scroll_delta(delta);
        let (mouse_x, _) = self.logical_mouse_pos;
        if self.active_page == PLUGINS_PAGE_INDEX && self.plugin_detail_contains(mouse_x) {
            self.plugin_detail_scroll =
                (self.plugin_detail_scroll - delta).clamp(0.0, self.plugin_detail_max_scroll);
            self.request_redraw();
        } else if mouse_x >= SIDEBAR_W {
            self.target_scroll_y =
                (self.target_scroll_y - delta).clamp(0.0, self.cached_max_scroll);
            self.request_redraw();
        }
    }

    fn handle_left_mouse_pressed(&mut self, event_loop: &ActiveEventLoop) {
        let (mouse_x, mouse_y) = self.logical_mouse_pos;
        if self.begin_scroll_drag(mouse_x, mouse_y) {
            self.request_redraw();
            return;
        }
        match window_control_at(mouse_x, mouse_y) {
            Some(WINDOW_CONTROL_CLOSE) => self.close_requested = true,
            Some(WINDOW_CONTROL_MINIMIZE) => {
                if let Some(window) = &self.window {
                    window.set_minimized(true);
                }
            }
            Some(_) => {}
            None if self.is_window_drag_region(mouse_x, mouse_y) && self.popup.is_none() => {
                if let Some(window) = &self.window {
                    let _ = window.drag_window();
                }
            }
            None if self.handle_widget_drag_press() => self.request_redraw(),
            None => self.handle_click(event_loop),
        }
    }

    fn is_window_drag_region(&self, mouse_x: f32, mouse_y: f32) -> bool {
        let in_sidebar_title = mouse_x < SIDEBAR_W && mouse_y < SIDEBAR_TITLE_HEIGHT;
        let in_content_title = mouse_x >= SIDEBAR_W
            && mouse_y < SETTINGS_HEADER_H
            && Self::page_navigation_at(mouse_x, mouse_y).is_none()
            && self.widget_mode_at(mouse_x, mouse_y).is_none();
        in_sidebar_title || in_content_title
    }

    fn handle_left_mouse_released(&mut self) {
        let scroll_released = std::mem::take(&mut self.scroll_dragging);
        if scroll_released || self.handle_widget_drag_release() {
            self.request_redraw();
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(crate) fn update(&mut self) -> Option<Instant> {
        self.window.as_ref()?;

        self.frame_count += 1;
        self.poll_detected_apps();
        self.poll_plugin_inventory();
        if self.frame_count.is_multiple_of(120) {
            self.update_detected_apps();
        }

        let has_anim = self.switch_anim.is_animating() || self.anim.is_animating();
        let has_popup = self.popup.is_some();
        let is_scrolling = (self.target_scroll_y - self.scroll_y).abs() > 0.1;
        let is_widget_dragging = self.widget_drag_active();
        let is_number_input_active = self.number_input.is_some();

        if !settings_frame_should_continue(
            has_anim,
            has_popup,
            is_scrolling,
            is_widget_dragging,
            is_number_input_active,
        ) {
            return None;
        }

        let now = Instant::now();
        if now < self.next_frame_deadline {
            return Some(self.next_frame_deadline);
        }

        let mut redraw = is_widget_dragging || is_number_input_active || self.switch_anim.tick();
        if self.anim.tick() {
            redraw = true;
        }
        if self.plugin_detail_closing && self.anim.get(PLUGIN_DETAIL_KEY) <= 0.005 {
            self.plugin_detail_closing = false;
            self.selected_plugin_id = None;
            self.plugin_detail_scroll = 0.0;
        }

        self.ensure_items_cache();
        let max_scroll = self.cached_max_scroll;
        self.target_scroll_y = self.target_scroll_y.clamp(0.0, max_scroll);

        let dt = now
            .duration_since(self.last_frame_time)
            .as_secs_f32()
            .clamp(0.001, 0.05);
        self.last_frame_time = now;

        let diff = self.target_scroll_y - self.scroll_y;
        let accel = diff * SCROLL_STIFFNESS - self.scroll_vel_y * SCROLL_DAMPING;
        self.scroll_vel_y += accel * dt;
        self.scroll_y += self.scroll_vel_y * dt;

        if self.scroll_y < 0.0 {
            self.scroll_y = 0.0;
            self.scroll_vel_y = 0.0;
        } else if self.scroll_y > max_scroll {
            self.scroll_y = max_scroll;
            self.scroll_vel_y = 0.0;
        }

        if diff.abs() > 0.05 || self.scroll_vel_y.abs() > 0.05 {
            redraw = true;
        } else if (self.scroll_y - self.target_scroll_y).abs() > f32::EPSILON {
            self.scroll_y = self.target_scroll_y;
            self.scroll_vel_y = 0.0;
        }

        if redraw {
            self.request_redraw();
            self.next_frame_deadline = now + Duration::from_millis(16);
            Some(self.next_frame_deadline)
        } else {
            None
        }
    }

    pub(crate) fn scrollbar_geometry(&self) -> Option<ScrollbarGeometry> {
        if self.cached_max_scroll <= 0.0 {
            return None;
        }
        let (win_w, win_h) = self.logical_window_size();
        let track_y = SETTINGS_HEADER_H + SCROLLBAR_TRACK_TOP_INSET;
        let track_height = win_h - track_y - SCROLLBAR_BOTTOM_INSET;
        let viewport_height = win_h - SETTINGS_HEADER_H;
        if track_height <= 0.0 || viewport_height <= 0.0 {
            return None;
        }
        let content_height = viewport_height + self.cached_max_scroll;
        let height = (track_height * viewport_height / content_height)
            .clamp(SCROLLBAR_THUMB_MIN_H.min(track_height), track_height);
        let travel = track_height - height;
        let y = track_y + self.scroll_y / self.cached_max_scroll * travel;
        Some(ScrollbarGeometry {
            x: win_w - SCROLLBAR_RIGHT_INSET - SCROLLBAR_W,
            y,
            width: SCROLLBAR_W,
            height,
            track_y,
            track_height,
        })
    }

    fn begin_scroll_drag(&mut self, mouse_x: f32, mouse_y: f32) -> bool {
        let Some(scrollbar) = self.scrollbar_geometry() else {
            return false;
        };
        if !scrollbar.hit_test(mouse_x, mouse_y) {
            return false;
        }
        self.scroll_dragging = true;
        self.scroll_drag_offset = mouse_y - scrollbar.y;
        self.scroll_vel_y = 0.0;
        true
    }

    fn update_scroll_drag(&mut self, mouse_y: f32) {
        if !self.scroll_dragging {
            return;
        }
        let Some(scrollbar) = self.scrollbar_geometry() else {
            self.scroll_dragging = false;
            return;
        };
        let travel = scrollbar.track_height - scrollbar.height;
        if travel <= 0.0 {
            return;
        }
        let thumb_y = (mouse_y - self.scroll_drag_offset)
            .clamp(scrollbar.track_y, scrollbar.track_y + travel);
        let scroll = (thumb_y - scrollbar.track_y) / travel * self.cached_max_scroll;
        self.target_scroll_y = scroll;
        self.scroll_y = scroll;
        self.scroll_vel_y = 0.0;
        self.request_redraw();
    }

    pub(crate) fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|window| window.id())
    }

    pub(crate) fn bring_to_front(&self) {
        if let Some(window) = &self.window {
            window.set_minimized(false);
            crate::utils::win32::bring_window_to_front("WinIsland Settings");
            window.request_redraw();
        }
    }

    pub(crate) fn close_requested(&self) -> bool {
        self.close_requested
    }

    pub(crate) fn close(&mut self) -> Option<D3DTargetId> {
        self.commit_number_input();
        self.popup = None;
        self.widget_dragging = None;
        self.compact_widget_dragging = None;
        sidebar::clear_sidebar_icon_cache();
        pages::plugins::clear_plugin_icon_cache();
        let renderer_target = self.renderer_target.take();
        self.window = None;
        renderer_target
    }

    pub(crate) fn take_plugin_request(&mut self) -> Option<PluginSettingsRequest> {
        self.plugin_request.take()
    }

    pub(crate) fn set_plugins(&mut self, plugins: Vec<InstalledPlugin>) {
        pages::plugins::clear_plugin_icon_cache();
        self.plugins = plugins;
        if self
            .selected_plugin_id
            .as_ref()
            .is_some_and(|id| !self.plugins.iter().any(|plugin| &plugin.id == id))
        {
            self.selected_plugin_id = None;
            self.pending_plugin_uninstall_id = None;
            self.plugin_detail_closing = true;
            self.anim.set_with_speed(PLUGIN_DETAIL_KEY, 0.0, 0.28);
        }
        self.mark_items_dirty();
        self.request_redraw();
    }

    pub(crate) fn set_plugin_inventory_receiver(
        &mut self,
        receiver: mpsc::Receiver<Vec<InstalledPlugin>>,
    ) {
        self.plugin_inventory_rx = Some(receiver);
    }

    pub(crate) fn set_plugin_widgets(&mut self, plugin_widgets: Vec<PluginWidget>) {
        self.plugin_widgets = plugin_widgets;
        let layout_changed = crate::core::config::normalize_active_plugin_widget_layout(
            &self.config.widget_layout,
            &mut self.config.plugin_widget_layout,
            &self.plugin_widgets,
        );
        if self.widget_dragging.as_ref().is_some_and(|source| {
            matches!(source, WidgetSource::Plugin(id) if !self.plugin_widgets.iter().any(|widget| widget.layout_id().as_ref() == Some(id)))
        }) {
            self.widget_dragging = None;
            self.widget_drag_hover_slot = None;
        }
        self.mark_items_dirty();
        if layout_changed {
            crate::core::persistence::save_config(&self.config);
        }
        self.request_redraw();
    }

    pub(crate) fn set_plugin_status(&mut self, message: String, restart: bool) {
        self.plugin_status = Some((message, restart));
        self.mark_items_dirty();
        self.request_redraw();
    }

    pub(crate) fn set_marketplace_loading(&mut self) {
        if !matches!(self.marketplace_state, MarketplaceViewState::Loaded(_)) {
            self.marketplace_state = MarketplaceViewState::Loading;
        }
        self.mark_items_dirty();
        self.request_redraw();
    }

    pub(crate) fn set_marketplace_catalog(&mut self, catalog: MarketplaceCatalog) {
        pages::plugins::clear_plugin_icon_cache();
        self.marketplace_state = MarketplaceViewState::Loaded(catalog.plugins);
        self.mark_items_dirty();
        self.request_redraw();
    }

    pub(crate) fn set_marketplace_error(&mut self, error: String) {
        self.marketplace_state = MarketplaceViewState::Failed(error);
        self.mark_items_dirty();
        self.request_redraw();
    }

    pub(crate) fn finish_marketplace_install(&mut self) {
        self.marketplace_installing_id = None;
        self.mark_items_dirty();
        self.request_redraw();
    }
}
