use crate::utils::settings_ui::hover_test;
use crate::utils::settings_ui::items::SIDEBAR_PAD;
use winit::keyboard::{Key, NamedKey};

use super::pages::PageInput;
use super::{
    NumberInput, NumberInputHandler, PAGE_NAV_GAP, PAGE_NAV_SIZE, PAGE_NAV_X, PAGE_NAV_Y,
    PLUGINS_PAGE_INDEX, POPUP_OPACITY_KEY, PageNavigation, SETTINGS_HEADER_H, SIDEBAR_PAGE_COUNT,
    SIDEBAR_ROW_GAP, SIDEBAR_ROW_H, SIDEBAR_START_Y, SIDEBAR_W, SettingsApp,
};

impl SettingsApp {
    pub(super) fn handle_click(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let (mouse_x, mouse_y) = self.logical_mouse_pos;

        self.commit_number_input();

        if self.popup.is_some() {
            let selection = self.popup.as_ref().and_then(|popup| {
                popup
                    .hit_test_item(mouse_x, mouse_y)
                    .and_then(|index| popup.values.get(index))
                    .map(|value| (popup.on_select, value.clone()))
            });
            self.popup = None;
            self.anim.set_with_speed(POPUP_OPACITY_KEY, 0.0, 0.3);
            if let Some((on_select, value)) = selection {
                on_select(self, &value);
                self.persist_settings_change();
            } else if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        if mouse_x < SIDEBAR_W {
            for page in 0..SIDEBAR_PAGE_COUNT {
                let row_y = SIDEBAR_START_Y + page as f32 * (SIDEBAR_ROW_H + SIDEBAR_ROW_GAP);
                if mouse_y >= row_y
                    && mouse_y <= row_y + SIDEBAR_ROW_H
                    && (SIDEBAR_PAD..=SIDEBAR_W - SIDEBAR_PAD).contains(&mouse_x)
                {
                    self.visit_page(page);
                    return;
                }
            }
            return;
        }

        if self.handle_widget_mode_click(mouse_x, mouse_y) {
            return;
        }

        if let Some(direction) = Self::page_navigation_at(mouse_x, mouse_y) {
            self.navigate_page_history(direction);
            return;
        }

        let scale = self
            .window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0);
        let content_width = self.win_w / scale - SIDEBAR_W;

        let input = PageInput {
            x: mouse_x - SIDEBAR_W,
            y: mouse_y + self.scroll_y,
            width: content_width,
            start_y: SETTINGS_HEADER_H,
        };

        match self.active_page {
            0 => self.handle_general_click(input),
            1 => self.handle_music_click(input),
            2 => {
                if self.handle_widget_click()
                    && let Some(window) = &self.window
                {
                    window.request_redraw();
                }
            }
            3 => self.handle_plugin_click(),
            4 => self.handle_about_click(input),
            _ => {}
        }
    }

    fn reset_scroll(&mut self) {
        self.scroll_y = 0.0;
        self.target_scroll_y = 0.0;
        self.scroll_vel_y = 0.0;
        self.mark_items_dirty();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(super) fn get_hover_state(&mut self) -> bool {
        let (mouse_x, mouse_y) = self.logical_mouse_pos;
        if self.scroll_dragging
            || self
                .scrollbar_geometry()
                .is_some_and(|scrollbar| scrollbar.hit_test(mouse_x, mouse_y))
        {
            return true;
        }
        if let Some(direction) = Self::page_navigation_at(mouse_x, mouse_y) {
            let is_enabled = match direction {
                PageNavigation::Back => self.can_navigate_back(),
                PageNavigation::Forward => self.can_navigate_forward(),
            };
            if is_enabled {
                return true;
            }
        }
        if self.widget_mode_at(mouse_x, mouse_y).is_some() {
            return true;
        }

        if let Some(popup) = &self.popup {
            let menu = popup.menu_rect();
            if mouse_x >= menu.left
                && mouse_x <= menu.right
                && mouse_y >= menu.top
                && mouse_y <= menu.bottom
            {
                return true;
            }
        }

        if mouse_x < SIDEBAR_W {
            for page in 0..SIDEBAR_PAGE_COUNT {
                let row_y = SIDEBAR_START_Y + page as f32 * (SIDEBAR_ROW_H + SIDEBAR_ROW_GAP);
                if mouse_y >= row_y
                    && mouse_y <= row_y + SIDEBAR_ROW_H
                    && (SIDEBAR_PAD..=SIDEBAR_W - SIDEBAR_PAD).contains(&mouse_x)
                {
                    return true;
                }
            }
            return false;
        }

        let scale = self
            .window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0);
        let content_width = self.win_w / scale - SIDEBAR_W;
        if self.widget_drag_active() {
            return true;
        }
        if self.active_page == PLUGINS_PAGE_INDEX {
            return self.plugin_hovered();
        }
        if self.widget_preview_hovered_at_mouse() {
            return true;
        }

        self.ensure_items_cache();
        hover_test(
            &self.cached_items,
            mouse_x - SIDEBAR_W,
            mouse_y + self.scroll_y,
            SETTINGS_HEADER_H,
            content_width,
        )
    }

    pub(super) fn page_navigation_at(mouse_x: f32, mouse_y: f32) -> Option<PageNavigation> {
        if !(PAGE_NAV_Y..=PAGE_NAV_Y + PAGE_NAV_SIZE).contains(&mouse_y) {
            return None;
        }

        if (PAGE_NAV_X..=PAGE_NAV_X + PAGE_NAV_SIZE).contains(&mouse_x) {
            return Some(PageNavigation::Back);
        }

        let forward_x = PAGE_NAV_X + PAGE_NAV_SIZE + PAGE_NAV_GAP;
        if (forward_x..=forward_x + PAGE_NAV_SIZE).contains(&mouse_x) {
            return Some(PageNavigation::Forward);
        }

        None
    }

    pub(crate) fn can_navigate_back(&self) -> bool {
        self.page_history_index > 0
    }

    pub(crate) fn can_navigate_forward(&self) -> bool {
        self.page_history_index + 1 < self.page_history.len()
    }

    pub(super) fn navigate_page_history(&mut self, direction: PageNavigation) -> bool {
        let next_index = match direction {
            PageNavigation::Back if self.can_navigate_back() => self.page_history_index - 1,
            PageNavigation::Forward if self.can_navigate_forward() => self.page_history_index + 1,
            _ => return false,
        };
        self.page_history_index = next_index;
        self.active_page = self.page_history[next_index];
        if self.active_page != PLUGINS_PAGE_INDEX {
            self.selected_plugin_id = None;
            self.plugin_detail_closing = false;
            self.anim
                .set_with_speed(super::PLUGIN_DETAIL_KEY, 0.0, 0.28);
        }
        self.reset_scroll();
        true
    }

    fn visit_page(&mut self, page: usize) {
        if self.active_page == page {
            return;
        }
        self.page_history.truncate(self.page_history_index + 1);
        self.page_history.push(page);
        self.page_history_index = self.page_history.len() - 1;
        self.active_page = page;
        if page != 3 {
            self.selected_plugin_id = None;
            self.plugin_detail_closing = false;
            self.anim
                .set_with_speed(super::PLUGIN_DETAIL_KEY, 0.0, 0.28);
        }
        self.reset_scroll();
    }

    pub(crate) fn begin_number_input(
        &mut self,
        rect: skia_safe::Rect,
        value: String,
        on_commit: NumberInputHandler,
    ) {
        self.number_input = Some(NumberInput {
            rect,
            text: value,
            on_commit,
        });
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(super) fn commit_number_input(&mut self) {
        let Some(input) = self.number_input.take() else {
            return;
        };
        (input.on_commit)(self, &input.text);
        self.persist_settings_change();
    }

    pub(super) fn handle_number_input_key(&mut self, key: &Key) -> bool {
        let Some(input) = &mut self.number_input else {
            return false;
        };

        match key {
            Key::Named(NamedKey::Backspace) => {
                input.text.pop();
            }
            Key::Named(NamedKey::Enter) => {
                self.commit_number_input();
                return true;
            }
            Key::Named(NamedKey::Escape) => {
                self.number_input = None;
            }
            Key::Character(value)
                if value.chars().all(|character| {
                    character.is_ascii_digit() || matches!(character, '.' | '-')
                }) =>
            {
                input.text.push_str(value);
            }
            _ => return false,
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
        true
    }
}
