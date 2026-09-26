use winisland_platform::{OverlaySpec, SettingsSpec};
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::monitor::MonitorHandle;
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Icon, Window, WindowAttributes, WindowButtons, WindowLevel};

pub(super) fn backdrop_attributes() -> WindowAttributes {
    Window::default_attributes()
        .with_title("WinIsland Backdrop")
        .with_inner_size(PhysicalSize::new(1, 1))
        .with_transparent(true)
        .with_no_redirection_bitmap(true)
        .with_visible(false)
        .with_decorations(false)
        .with_resizable(false)
        .with_enabled_buttons(WindowButtons::empty())
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_skip_taskbar(true)
}

pub(super) fn overlay_attributes(spec: OverlaySpec, owner: isize) -> WindowAttributes {
    Window::default_attributes()
        .with_title(spec.title)
        .with_inner_size(PhysicalSize::new(spec.size.width, spec.size.height))
        .with_transparent(true)
        .with_no_redirection_bitmap(true)
        .with_visible(false)
        .with_decorations(false)
        .with_resizable(true)
        .with_enabled_buttons(WindowButtons::empty())
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_skip_taskbar(true)
        .with_owner_window(owner)
        .with_window_icon(get_app_icon())
}

pub(super) fn settings_attributes(
    spec: SettingsSpec,
    monitor: Option<&MonitorHandle>,
) -> WindowAttributes {
    let logical_size = LogicalSize::new(spec.logical_size.width, spec.logical_size.height);
    let attrs = Window::default_attributes()
        .with_title(spec.title)
        .with_inner_size(logical_size)
        .with_resizable(true)
        .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE)
        .with_decorations(false)
        .with_transparent(true)
        .with_no_redirection_bitmap(true)
        .with_window_icon(get_app_icon());
    if let Some(monitor) = monitor {
        let origin = monitor.position();
        let resolution = monitor.size();
        let size = logical_size.to_physical::<u32>(monitor.scale_factor());
        attrs.with_position(PhysicalPosition::new(
            origin.x + (resolution.width as i32 - size.width as i32) / 2,
            origin.y + (resolution.height as i32 - size.height as i32) / 2,
        ))
    } else {
        attrs
    }
}

fn get_app_icon() -> Option<Icon> {
    let icon_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/icon-dark.png"
    ));
    let image = image::load_from_memory(icon_bytes).ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).ok()
}
