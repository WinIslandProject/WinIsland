use crate::core::config::{APP_AUTHOR, APP_HOMEPAGE, APP_VERSION};
use crate::core::i18n::tr;
use crate::utils::settings_ui::ClickResult;
use crate::utils::settings_ui::items::SettingsItem;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

use super::super::SettingsApp;
use super::{PageInput, SettingsPage};

#[derive(Clone, Copy)]
enum AboutAction {
    Homepage,
}

impl SettingsApp {
    fn build_about_page(&self) -> SettingsPage<AboutAction> {
        let theme = self.theme();
        let mut page = SettingsPage::new();
        page.push(SettingsItem::Spacer { height: 20.0 });
        page.push(SettingsItem::CenterText {
            text: "WinIsland".to_string(),
            size: 28.0,
            color: theme.text_pri,
        });
        page.push(SettingsItem::CenterText {
            text: format!("Version {APP_VERSION}"),
            size: 14.0,
            color: theme.text_sec,
        });
        page.push(SettingsItem::CenterText {
            text: format!("{} {APP_AUTHOR}", tr("created_by")),
            size: 14.0,
            color: theme.text_sec,
        });
        page.push(SettingsItem::Spacer { height: 10.0 });
        page.push_action(
            SettingsItem::CenterLink {
                label: tr("visit_homepage"),
                color: theme.accent,
            },
            AboutAction::Homepage,
        );
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
