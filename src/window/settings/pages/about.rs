use std::cell::OnceCell;

use skia_safe::{Data, Image};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

use crate::core::config::{APP_AUTHOR, APP_HOMEPAGE, APP_VERSION};
use crate::core::i18n::tr;
use crate::utils::settings_ui::ClickResult;
use crate::utils::settings_ui::items::SettingsItem;

use super::super::SettingsApp;
use super::{PageInput, SettingsPage};

const ABOUT_ICON_SIZE: f32 = 96.0;
const ABOUT_ICON_HEIGHT: f32 = 112.0;

thread_local! {
    static ABOUT_ICON: OnceCell<Image> = const { OnceCell::new() };
}

fn app_icon() -> Image {
    ABOUT_ICON.with(|icon| {
        icon.get_or_init(|| {
            Image::from_encoded(Data::new_copy(include_bytes!(
                "../../../../resources/icon-dark.png"
            )))
            .expect("Failed to load about page icon")
        })
        .clone()
    })
}

#[derive(Clone, Copy)]
enum AboutAction {
    Homepage,
}

impl SettingsApp {
    fn build_about_page(&self) -> SettingsPage<AboutAction> {
        let theme = self.theme();
        let mut page = SettingsPage::new();
        page.spacer(20.0);
        page.center_image(app_icon(), ABOUT_ICON_SIZE, ABOUT_ICON_HEIGHT);
        page.center_text("WinIsland".to_string(), 28.0, theme.text_pri);
        page.center_text(format!("Version {APP_VERSION}"), 14.0, theme.text_sec);
        page.center_text(
            format!("{} {APP_AUTHOR}", tr("created_by")),
            14.0,
            theme.text_sec,
        );
        page.spacer(10.0);
        page.center_link(tr("visit_homepage"), theme.accent, AboutAction::Homepage);
        page
    }

    pub(crate) fn build_about_items(&self) -> Vec<SettingsItem> {
        self.build_about_page().into_items()
    }

    pub(crate) fn handle_about_click(&self, input: PageInput) {
        let page = self.build_about_page();
        let result = input.hit_test(&page);
        if matches!(
            (page.action(&result), result),
            (Some(AboutAction::Homepage), ClickResult::CenterLink(_))
        ) {
            let homepage: Vec<u16> = APP_HOMEPAGE
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            // SAFETY: `homepage` is null-terminated and remains valid for the duration of the call.
            unsafe {
                let _ = ShellExecuteW(
                    None,
                    None,
                    PCWSTR(homepage.as_ptr()),
                    None,
                    None,
                    SW_SHOWNORMAL,
                );
            }
        }
    }
}
