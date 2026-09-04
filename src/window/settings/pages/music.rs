use crate::core::i18n::tr;
use crate::utils::settings_ui::items::SettingsItem;
use crate::utils::settings_ui::{ClickResult, StepDirection};

use super::super::{NumberInputHandler, PopupState, SettingsApp};
use super::{PageInput, SettingsPage};

#[derive(Clone)]
enum MusicAction {
    SmtcEnabled,
    LyricsMode,
    ShowLyrics,
    ShowSecondaryLyrics,
    LyricsSource,
    LyricsDelay,
    LyricsScroll,
    LyricsScrollWidth,
    LyricsFolder,
    App(String),
}

impl SettingsApp {
    fn build_music_page(&self) -> SettingsPage<MusicAction> {
        let show_lyrics = self.config.show_lyrics;
        let mut page = SettingsPage::new();
        page.section(tr("section_playback"));
        page.group_start();
        page.row_switch(
            tr("smtc_control"),
            self.config.smtc_enabled,
            true,
            MusicAction::SmtcEnabled,
        );
        page.group_end();
        page.section(tr("section_lyrics"));
        page.group_start();
        page.row_source(
            tr("lyrics_mode"),
            vec![
                (
                    tr("lyrics_mode_online"),
                    self.config.lyrics_mode == "online",
                ),
                (tr("lyrics_mode_lrc"), self.config.lyrics_mode == "lrc"),
            ],
            true,
            MusicAction::LyricsMode,
        );
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
            page.row_switch(
                tr("show_secondary_lyrics"),
                self.config.show_secondary_lyrics,
                true,
                MusicAction::ShowSecondaryLyrics,
            );
        } else {
            page.row_switch(
                tr("show_lyrics"),
                show_lyrics,
                true,
                MusicAction::ShowLyrics,
            );
            page.row_switch(
                tr("show_secondary_lyrics"),
                show_lyrics && self.config.show_secondary_lyrics,
                show_lyrics,
                MusicAction::ShowSecondaryLyrics,
            );
            page.row_source(
                tr("lyrics_source"),
                vec![
                    (tr("lyrics_source_163"), self.config.lyrics_source == "163"),
                    (tr("lyrics_source_qq"), self.config.lyrics_source == "qq"),
                    (
                        tr("lyrics_source_kugou"),
                        self.config.lyrics_source == "kugou",
                    ),
                    (
                        tr("lyrics_source_lrclib"),
                        self.config.lyrics_source == "lrclib",
                    ),
                ],
                show_lyrics,
                MusicAction::LyricsSource,
            );
            page.row_stepper(
                tr("lyrics_delay"),
                format!("{:.1}", self.config.lyrics_delay),
                show_lyrics,
                MusicAction::LyricsDelay,
            );
            page.row_switch(
                tr("lyrics_scroll"),
                show_lyrics && self.config.lyrics_scroll,
                show_lyrics,
                MusicAction::LyricsScroll,
            );
            if show_lyrics && self.config.lyrics_scroll {
                page.row_stepper(
                    tr("lyrics_scroll_max_width"),
                    (self.config.lyrics_scroll_max_width as i32).to_string(),
                    show_lyrics,
                    MusicAction::LyricsScrollWidth,
                );
            }
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
                    self.config.smtc_enabled,
                    MusicAction::App(app.clone()),
                );
            }
        }
        page.group_end();
        page
    }

    pub(crate) fn build_music_items(&self) -> Vec<SettingsItem> {
        self.build_music_page().into_items()
    }

    pub(crate) fn handle_music_click(&mut self, input: PageInput) {
        let page = self.build_music_page();
        let result = input.hit_test(&page);
        let Some(action) = page.action(&result).cloned() else {
            return;
        };

        if let ClickResult::StepperValue(item_index) = &result {
            let (value, on_commit): (String, NumberInputHandler) = match action {
                MusicAction::LyricsDelay => {
                    (format!("{:.1}", self.config.lyrics_delay), set_lyrics_delay)
                }
                MusicAction::LyricsScrollWidth => (
                    (self.config.lyrics_scroll_max_width as i32).to_string(),
                    set_lyrics_scroll_width,
                ),
                _ => return,
            };
            self.begin_number_input(
                input.stepper_value_rect(&page, *item_index, self.scroll_y),
                value,
                on_commit,
            );
            return;
        }

        let changed = match (&action, &result) {
            (MusicAction::SmtcEnabled, ClickResult::Switch(_)) => {
                self.config.smtc_enabled = !self.config.smtc_enabled;
                true
            }
            (MusicAction::ShowLyrics, ClickResult::Switch(_)) => {
                self.config.show_lyrics = !self.config.show_lyrics;
                true
            }
            (MusicAction::ShowSecondaryLyrics, ClickResult::Switch(_)) => {
                self.config.show_secondary_lyrics = !self.config.show_secondary_lyrics;
                true
            }
            (MusicAction::LyricsScroll, ClickResult::Switch(_)) => {
                self.config.lyrics_scroll = !self.config.lyrics_scroll;
                true
            }
            (MusicAction::LyricsDelay, _) => {
                let Some(direction) = result.step_direction() else {
                    return;
                };
                self.config.lyrics_delay =
                    (step_f64(self.config.lyrics_delay, direction, 0.1, -10.0, 10.0) * 10.0)
                        .round()
                        / 10.0;
                true
            }
            (MusicAction::LyricsScrollWidth, _) => {
                let Some(direction) = result.step_direction() else {
                    return;
                };
                self.config.lyrics_scroll_max_width = step(
                    self.config.lyrics_scroll_max_width,
                    direction,
                    10.0,
                    100.0,
                    500.0,
                );
                true
            }
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
            return;
        }

        let ClickResult::SourceButton(item_index) = result else {
            return;
        };
        let scale = self
            .window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0);
        let button_rect = input.popup_button_rect(&page, item_index, self.scroll_y);
        let popup = match action {
            MusicAction::LyricsMode => PopupState::new(
                select_lyrics_mode,
                button_rect,
                vec![tr("lyrics_mode_online"), tr("lyrics_mode_lrc")],
                vec!["online".to_string(), "lrc".to_string()],
                usize::from(self.config.lyrics_mode == "lrc"),
                self.win_w / scale,
                self.win_h / scale,
            ),
            MusicAction::LyricsSource => PopupState::new(
                select_lyrics_source,
                button_rect,
                vec![
                    tr("lyrics_source_163"),
                    tr("lyrics_source_qq"),
                    tr("lyrics_source_kugou"),
                    tr("lyrics_source_lrclib"),
                ],
                vec![
                    "163".to_string(),
                    "qq".to_string(),
                    "kugou".to_string(),
                    "lrclib".to_string(),
                ],
                ["163", "qq", "kugou", "lrclib"]
                    .iter()
                    .position(|source| *source == self.config.lyrics_source)
                    .unwrap_or_default(),
                self.win_w / scale,
                self.win_h / scale,
            ),
            _ => return,
        };
        self.show_popup(popup);
    }
}

fn step(value: f32, direction: StepDirection, amount: f32, min: f32, max: f32) -> f32 {
    match direction {
        StepDirection::Decrement => value - amount,
        StepDirection::Increment => value + amount,
    }
    .clamp(min, max)
}

fn step_f64(value: f64, direction: StepDirection, amount: f64, min: f64, max: f64) -> f64 {
    match direction {
        StepDirection::Decrement => value - amount,
        StepDirection::Increment => value + amount,
    }
    .clamp(min, max)
}

fn select_lyrics_source(app: &mut SettingsApp, value: &str) {
    app.config.lyrics_source = value.to_string();
}

fn select_lyrics_mode(app: &mut SettingsApp, value: &str) {
    app.config.lyrics_mode = value.to_string();
    if value == "lrc" {
        app.config.show_lyrics = true;
    }
}

fn set_lyrics_delay(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f64>() {
        app.config.lyrics_delay = value.clamp(-10.0, 10.0);
    }
}

fn set_lyrics_scroll_width(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.lyrics_scroll_max_width = value.clamp(100.0, 500.0);
    }
}
