use skia_safe::Rect;

use crate::utils::settings_ui::items::{POPUP_ITEM_H, POPUP_MENU_PAD};
use winisland_render::text::FontManager;

use super::SettingsApp;

pub(crate) type PopupSelectHandler = fn(&mut SettingsApp, &str);

pub(crate) struct PopupState {
    pub(crate) on_select: PopupSelectHandler,
    pub(crate) button_rect: Rect,
    pub(crate) menu_rect: Rect,
    pub(crate) options: Vec<String>,
    pub(crate) values: Vec<String>,
    pub(crate) selected_idx: usize,
    pub(crate) hover_idx: Option<usize>,
}

impl PopupState {
    pub(crate) fn new(
        on_select: PopupSelectHandler,
        button_rect: Rect,
        options: Vec<String>,
        values: Vec<String>,
        selected_idx: usize,
        window_width: f32,
        window_height: f32,
    ) -> Self {
        let font_manager = FontManager::global();
        let content_width = options
            .iter()
            .map(|option| {
                font_manager.measure_text_cached(
                    option,
                    12.0,
                    winisland_render::FontStyle::normal(),
                )
            })
            .fold(120.0_f32, f32::max);
        let menu_width = content_width + 36.0;
        let menu_height = options.len() as f32 * POPUP_ITEM_H + POPUP_MENU_PAD * 2.0;
        let max_x = (window_width - menu_width - 10.0).max(0.0);
        let max_y = (window_height - menu_height - 10.0).max(0.0);
        let menu_x = (button_rect.right - menu_width).clamp(0.0, max_x);
        let below_y = button_rect.bottom + 4.0;
        let above_y = button_rect.top - menu_height - 4.0;
        let opens_upward = below_y + menu_height > window_height - 10.0 && above_y >= 10.0;
        let menu_y = if opens_upward {
            above_y
        } else {
            below_y.min(max_y)
        };

        Self {
            on_select,
            button_rect,
            menu_rect: Rect::from_xywh(menu_x, menu_y, menu_width, menu_height),
            options,
            values,
            selected_idx,
            hover_idx: None,
        }
    }

    pub(crate) fn menu_rect(&self) -> Rect {
        self.menu_rect
    }

    pub(crate) fn item_rect(&self, index: usize) -> Rect {
        let y = self.menu_rect.top + POPUP_MENU_PAD + index as f32 * POPUP_ITEM_H;
        Rect::from_xywh(
            self.menu_rect.left + POPUP_MENU_PAD,
            y,
            self.menu_rect.width() - POPUP_MENU_PAD * 2.0,
            POPUP_ITEM_H,
        )
    }

    pub(crate) fn hit_test_item(&self, mouse_x: f32, mouse_y: f32) -> Option<usize> {
        let inner_top = self.menu_rect.top + POPUP_MENU_PAD;
        let inner_bottom = self.menu_rect.bottom - POPUP_MENU_PAD;
        if mouse_x < self.menu_rect.left
            || mouse_x > self.menu_rect.right
            || mouse_y < inner_top
            || mouse_y > inner_bottom
        {
            return None;
        }

        let index = ((mouse_y - inner_top) / POPUP_ITEM_H).floor() as usize;
        (index < self.options.len()).then_some(index)
    }
}
