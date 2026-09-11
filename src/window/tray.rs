use crate::core::config::WINDOW_TITLE;
use crate::core::i18n::tr;
use tray_icon::menu::{Menu, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct TrayManager {
    tray: TrayIcon,
    toggle_item: MenuItem,
    settings_item: MenuItem,
    restart_item: MenuItem,
    quit_item: MenuItem,
    is_light: bool,
}

impl TrayManager {
    pub fn new(is_light: bool) -> Self {
        let menu = Menu::new();
        let toggle_item = MenuItem::new(tr("tray_hide"), true, None);
        let settings_item = MenuItem::new(tr("tray_settings"), true, None);
        let restart_item = MenuItem::new(tr("tray_restart"), true, None);
        let quit_item = MenuItem::new(tr("tray_exit"), true, None);
        let _ = menu.append(&toggle_item);
        let _ = menu.append(&settings_item);
        let _ = menu.append(&restart_item);
        let _ = menu.append(&quit_item);

        let tray = TrayIconBuilder::new()
            .with_tooltip(WINDOW_TITLE)
            .with_menu(Box::new(menu))
            .with_icon(Self::load_tray_icon(is_light))
            .build()
            .unwrap();
        Self {
            tray,
            toggle_item,
            settings_item,
            restart_item,
            quit_item,
            is_light,
        }
    }

    pub fn update_theme(&mut self, is_light: bool) {
        if self.is_light != is_light {
            self.is_light = is_light;
            let _ = self.tray.set_icon(Some(Self::load_tray_icon(is_light)));
            log::info!(
                "Tray icon theme updated to {}",
                if is_light { "light" } else { "dark" }
            );
        }
    }

    pub fn update_item_text(&self, visible: bool) {
        if visible {
            self.toggle_item.set_text(tr("tray_hide"));
        } else {
            self.toggle_item.set_text(tr("tray_show"));
        }
    }

    fn load_tray_icon(is_light: bool) -> Icon {
        let icon_bytes: &[u8] = if is_light {
            include_bytes!("../../resources/icon-dark.png")
        } else {
            include_bytes!("../../resources/icon.png")
        };
        let image =
            image::load_from_memory(icon_bytes).expect("Failed to load icon from resources");
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let rgba_vec = rgba.into_raw();
        Icon::from_rgba(rgba_vec, width, height).expect("Failed to create tray icon from RGBA data")
    }
}
impl TrayAction {
    pub fn from_id(id: tray_icon::menu::MenuId, tray: &TrayManager) -> Option<Self> {
        if id == tray.toggle_item.id() {
            Some(TrayAction::ToggleVisibility)
        } else if id == tray.settings_item.id() {
            Some(TrayAction::OpenSettings)
        } else if id == tray.restart_item.id() {
            Some(TrayAction::Restart)
        } else if id == tray.quit_item.id() {
            Some(TrayAction::Exit)
        } else {
            None
        }
    }
}
pub enum TrayAction {
    ToggleVisibility,
    OpenSettings,
    Restart,
    Exit,
}
