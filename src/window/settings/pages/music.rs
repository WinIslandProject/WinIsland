use crate::utils::settings_ui::ClickResult;
use crate::utils::settings_ui::items::SettingsItem;
use winisland_core::config::AppConfigField;
use winisland_core::i18n::{current_lang, tr};
use winisland_render::{Point, Rect};

use super::super::SettingsApp;
use super::{PageInput, SettingsPage};

pub(super) const MUSIC_NOTICE_HEIGHT: f32 = 150.0;

pub(crate) fn music_notice_button_rect(width: f32, start_y: f32) -> Rect {
    Rect::from_xywh(width - 24.0 - 14.0 - 86.0, start_y + 102.0, 86.0, 26.0)
}

#[derive(Clone)]
enum MusicAction {
    LyricsFolder,
    App(String),
}

impl SettingsApp {
    pub(crate) fn show_music_notice(&self) -> bool {
        current_lang() == "zh_cn" && !self.config.music_notice_acknowledged
    }

    fn build_music_page(&self) -> SettingsPage<MusicAction> {
        let media_available = crate::platform::capabilities().media_session;
        let show_lyrics = self.config.show_lyrics;
        let mut page = SettingsPage::new();
        if self.show_music_notice() {
            page.push(SettingsItem::Custom {
                height: MUSIC_NOTICE_HEIGHT,
            });
        }
        page.section(tr("section_playback"));
        page.group_start();
        page.setting(&self.config, AppConfigField::SmtcEnabled, media_available);
        if !media_available {
            page.row_label(tr("platform_unavailable"));
        }
        page.group_end();

        if self.config.smtc_enabled && media_available {
            page.section(tr("section_lyrics"));
            page.group_start();
            page.setting(&self.config, AppConfigField::LyricsMode, true);
            if self.config.lyrics_mode == "lrc" {
                page.row_folder(
                    tr("lyrics_local_dir"),
                    tr("folder_select"),
                    self.config
                        .lyrics_local_dir
                        .as_ref()
                        .filter(|path| !path.is_empty())
                        .map(|_| tr("folder_clear")),
                    self.config
                        .lyrics_local_dir
                        .clone()
                        .filter(|path| !path.is_empty()),
                    true,
                    MusicAction::LyricsFolder,
                );
                page.setting(&self.config, AppConfigField::ShowSecondaryLyrics, true);
            } else {
                page.setting(&self.config, AppConfigField::ShowLyrics, true);
                if show_lyrics {
                    page.setting(&self.config, AppConfigField::ShowSecondaryLyrics, true);
                    page.setting(&self.config, AppConfigField::LyricsSource, true);
                    page.setting(&self.config, AppConfigField::LyricsDelay, true);
                    page.setting(&self.config, AppConfigField::LyricsScroll, true);
                    if self.config.lyrics_scroll {
                        page.setting(&self.config, AppConfigField::LyricsScrollMaxWidth, true);
                    }
                }
            }
            if show_lyrics {
                page.setting(&self.config, AppConfigField::LyricsSideGap, true);
                page.setting(
                    &self.config,
                    AppConfigField::LyricsTransitionAnimation,
                    true,
                );
            }
            page.group_end();

            page.section(tr("media_apps"));
            page.group_start();
            if self.detected_apps.is_empty() {
                page.row_label(tr("no_sessions"));
            } else {
                for app in &self.detected_apps {
                    page.row_app(
                        app.split('!').next().unwrap_or(app).to_string(),
                        self.config.smtc_apps.contains(app),
                        true,
                        MusicAction::App(app.clone()),
                    );
                }
            }
            page.group_end();
        }
        page
    }

    pub(crate) fn build_music_items(&self) -> Vec<SettingsItem> {
        self.build_music_page().into_items()
    }

    pub(crate) fn handle_music_click(&mut self, input: PageInput) {
        if self.show_music_notice() {
            let button = music_notice_button_rect(input.width, input.start_y);
            if button.contains(Point::new(input.x, input.y)) {
                self.music_notice_pressed = true;
                self.request_redraw();
                return;
            }
        }
        let page = self.build_music_page();
        let result = input.hit_test(&page);
        if self.handle_setting_click(&page, &result, input) {
            return;
        }
        let Some(action) = page.action(&result).cloned() else {
            return;
        };

        let changed = match (&action, &result) {
            (MusicAction::LyricsFolder, ClickResult::FolderSelect(_)) => {
                let Some(path) = rfd::FileDialog::new().pick_folder() else {
                    return;
                };
                self.config.lyrics_local_dir = Some(path.to_string_lossy().into_owned());
                true
            }
            (MusicAction::LyricsFolder, ClickResult::FolderClear(_)) => {
                self.config.lyrics_local_dir = None;
                true
            }
            (MusicAction::App(app), ClickResult::AppItem(_)) => {
                if self.config.smtc_apps.contains(app) {
                    self.config.smtc_apps.retain(|entry| entry != app);
                } else {
                    self.config.smtc_apps.push(app.clone());
                    if !self.config.smtc_known_apps.contains(app) {
                        self.config.smtc_known_apps.push(app.clone());
                    }
                }
                true
            }
            _ => false,
        };
        if changed {
            self.persist_settings_change();
        }
    }
}
