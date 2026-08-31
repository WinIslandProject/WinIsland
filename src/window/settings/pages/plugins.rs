use std::cell::RefCell;
use std::collections::HashMap;

use skia_safe::{
    Canvas, ClipOp, Color, Contains, Data, FontStyle, Image, Paint, Point, RRect, Rect,
};

use crate::core::i18n::tr;
use crate::plugin::manager::InstalledPlugin;
use crate::plugin::marketplace::MarketplacePlugin;
use crate::utils::color::SettingsTheme;
use crate::utils::font::{DrawTextCachedParams, FontManager};
use crate::utils::settings_ui::items::{CONTENT_PADDING, SettingsItem};

use super::super::{
    MarketplaceViewState, PLUGIN_DETAIL_KEY, PluginPageTab, PluginSettingsRequest,
    SETTINGS_HEADER_H, SIDEBAR_W, SettingsApp,
};

mod detail;
mod markdown;

const PLUGIN_CARD_H: f32 = 76.0;
const PLUGIN_CARD_GAP: f32 = 10.0;
const PLUGIN_TABS_Y: f32 = SETTINGS_HEADER_H + 12.0;
const PLUGIN_TABS_H: f32 = 32.0;
const PLUGIN_TAB_W: f32 = 92.0;
const PLUGIN_LIST_TOP: f32 = PLUGIN_TABS_Y + PLUGIN_TABS_H + 16.0;
const DETAIL_W: f32 = 350.0;
const DETAIL_ICON_SIZE: f32 = 64.0;

thread_local! {
    static PLUGIN_ICONS: RefCell<HashMap<String, Image>> = RefCell::new(HashMap::new());
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MarketplaceAction {
    Install,
    Update,
    Downloading,
    Installed,
    Incompatible,
    Revoked,
}

impl MarketplaceAction {
    pub(super) fn label(self) -> String {
        tr(match self {
            Self::Install => "plugin_marketplace_install",
            Self::Update => "plugin_marketplace_update",
            Self::Downloading => "plugin_marketplace_pending",
            Self::Installed => "plugin_marketplace_installed",
            Self::Incompatible => "plugin_marketplace_incompatible",
            Self::Revoked => "plugin_marketplace_revoked",
        })
    }

    pub(super) fn is_available(self) -> bool {
        matches!(self, Self::Install | Self::Update)
    }
}

pub(crate) fn clear_plugin_icon_cache() {
    PLUGIN_ICONS.with(|cache| cache.borrow_mut().clear());
    markdown::clear_cache();
}

impl SettingsApp {
    pub(crate) fn build_plugin_items(&self) -> Vec<SettingsItem> {
        let entry_count = match (&self.plugin_page_tab, &self.marketplace_state) {
            (PluginPageTab::Installed, _) => self.plugins.len(),
            (PluginPageTab::Marketplace, MarketplaceViewState::Loaded(plugins)) => plugins.len(),
            _ => 0,
        };
        let state_height = if entry_count == 0 { 126.0 } else { 0.0 };
        let status_height = if self.plugin_status.is_some() {
            62.0
        } else {
            0.0
        };
        let height = PLUGIN_LIST_TOP - SETTINGS_HEADER_H
            + 30.0
            + entry_count as f32 * (PLUGIN_CARD_H + PLUGIN_CARD_GAP)
            + state_height
            + status_height
            + 24.0;
        vec![SettingsItem::Custom { height }]
    }

    pub(crate) fn draw_plugins_page(
        &mut self,
        direct_context: &mut skia_safe::gpu::DirectContext,
        canvas: &Canvas,
        theme: &SettingsTheme,
        width: f32,
        height: f32,
    ) {
        let content_width = width - SIDEBAR_W;
        canvas.save();
        canvas.clip_rect(
            Rect::from_xywh(
                SIDEBAR_W,
                PLUGIN_LIST_TOP,
                content_width,
                height - PLUGIN_LIST_TOP,
            ),
            ClipOp::Intersect,
            true,
        );
        canvas.translate((SIDEBAR_W, -self.scroll_y));
        match self.plugin_page_tab {
            PluginPageTab::Installed => {
                self.draw_installed_plugins(direct_context, canvas, theme, content_width)
            }
            PluginPageTab::Marketplace => {
                self.draw_marketplace_plugins(direct_context, canvas, theme, content_width)
            }
        }
        canvas.restore();

        draw_plugin_tabs(canvas, theme, width, self.plugin_page_tab);

        let detail_progress = self.anim.get(PLUGIN_DETAIL_KEY);
        if detail_progress > 0.005 {
            self.draw_plugin_detail(
                direct_context,
                canvas,
                theme,
                width,
                height,
                detail_progress,
            );
        }
    }

    fn draw_installed_plugins(
        &self,
        direct_context: &mut skia_safe::gpu::DirectContext,
        canvas: &Canvas,
        theme: &SettingsTheme,
        width: f32,
    ) {
        let mut y = draw_section_title(canvas, theme, tr("plugin_installed"));
        if self.plugins.is_empty() {
            draw_empty_state(canvas, theme, width, y + 48.0, &tr("plugin_empty"), None);
            y += 126.0;
        } else {
            for plugin in &self.plugins {
                let card = plugin_card(width, y);
                draw_card_background(canvas, theme, card, self.card_hovered(card));
                draw_plugin_icon(
                    direct_context,
                    canvas,
                    plugin,
                    Rect::from_xywh(card.left + 12.0, card.top + 12.0, 52.0, 52.0),
                );
                draw_card_text(
                    canvas,
                    theme,
                    card,
                    &plugin.name,
                    &format!("{} · v{}", plugin.author, plugin.version),
                    142.0,
                );
                draw_toggle(
                    canvas,
                    theme,
                    plugin.enabled,
                    card.right - 50.0,
                    card.top + 28.0,
                );
                y += PLUGIN_CARD_H + PLUGIN_CARD_GAP;
            }
        }
        self.draw_plugin_status(canvas, theme, width, y);
    }

    fn draw_marketplace_plugins(
        &self,
        direct_context: &mut skia_safe::gpu::DirectContext,
        canvas: &Canvas,
        theme: &SettingsTheme,
        width: f32,
    ) {
        let mut y = draw_section_title(canvas, theme, tr("plugin_tab_marketplace"));
        match &self.marketplace_state {
            MarketplaceViewState::NotLoaded | MarketplaceViewState::Loading => {
                draw_empty_state(
                    canvas,
                    theme,
                    width,
                    y + 48.0,
                    &tr("plugin_marketplace_loading"),
                    None,
                );
                y += 126.0;
            }
            MarketplaceViewState::Failed(error) => {
                draw_empty_state(
                    canvas,
                    theme,
                    width,
                    y + 39.0,
                    &tr("plugin_marketplace_failed"),
                    Some(error),
                );
                draw_retry_button(canvas, theme, width, y + 84.0);
                y += 126.0;
            }
            MarketplaceViewState::Loaded(plugins) if plugins.is_empty() => {
                draw_empty_state(
                    canvas,
                    theme,
                    width,
                    y + 48.0,
                    &tr("plugin_marketplace_empty"),
                    None,
                );
                y += 126.0;
            }
            MarketplaceViewState::Loaded(plugins) => {
                for plugin in plugins {
                    let card = plugin_card(width, y);
                    draw_card_background(canvas, theme, card, self.card_hovered(card));
                    draw_plugin_icon_data(
                        direct_context,
                        canvas,
                        &plugin.id,
                        &plugin.name,
                        plugin.icon.as_deref(),
                        Rect::from_xywh(card.left + 12.0, card.top + 12.0, 52.0, 52.0),
                    );
                    let action = self.marketplace_action(plugin);
                    let label = action.label();
                    let action_width = action_button_width(&label);
                    draw_card_text(
                        canvas,
                        theme,
                        card,
                        &plugin.name,
                        &marketplace_subtitle(plugin),
                        action_width + 106.0,
                    );
                    draw_marketplace_action(
                        canvas,
                        theme,
                        marketplace_action_rect(card, action_width),
                        &label,
                        action,
                    );
                    y += PLUGIN_CARD_H + PLUGIN_CARD_GAP;
                }
            }
        }
        self.draw_plugin_status(canvas, theme, width, y);
    }

    fn draw_plugin_status(&self, canvas: &Canvas, theme: &SettingsTheme, width: f32, y: f32) {
        let Some((message, restart)) = &self.plugin_status else {
            return;
        };
        let fm = FontManager::global();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let status = Rect::from_xywh(
            CONTENT_PADDING,
            y + 6.0,
            width - CONTENT_PADDING * 2.0,
            48.0,
        );
        paint.set_color(Color::from_argb(
            28,
            theme.accent.r(),
            theme.accent.g(),
            theme.accent.b(),
        ));
        canvas.draw_round_rect(status, 12.0, 12.0, &paint);
        let label = restart.then(|| tr("plugin_restart_now"));
        let label_w = label.as_ref().map_or(0.0, |label| {
            fm.measure_text_cached(label, 12.0, FontStyle::bold())
        });
        let message = ellipsize_text(
            fm,
            message,
            12.0,
            FontStyle::normal(),
            (status.width() - 28.0 - label_w - if *restart { 24.0 } else { 0.0 }).max(20.0),
        );
        paint.set_color(theme.text_pri);
        fm.draw_text_cached(DrawTextCachedParams {
            canvas,
            text: &message,
            x: status.left + 14.0,
            y: status.top + 29.0,
            size: 12.0,
            bold: false,
            paint: &paint,
        });
        if let Some(label) = label {
            paint.set_color(theme.accent);
            fm.draw_text_cached(DrawTextCachedParams {
                canvas,
                text: &label,
                x: status.right - label_w - 14.0,
                y: status.top + 29.0,
                size: 12.0,
                bold: true,
                paint: &paint,
            });
        }
    }

    pub(crate) fn handle_plugin_click(&mut self) {
        if let Some(tab) = self.plugin_tab_hit() {
            self.switch_plugin_tab(tab);
            return;
        }
        let detail_progress = self.anim.get(PLUGIN_DETAIL_KEY);
        if detail_progress > 0.005 && self.handle_plugin_detail_click() {
            return;
        }
        match self.plugin_page_tab {
            PluginPageTab::Installed => self.handle_installed_plugin_click(),
            PluginPageTab::Marketplace => self.handle_marketplace_click(),
        }
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }

    fn handle_installed_plugin_click(&mut self) {
        let Some((index, on_toggle)) = self.installed_plugin_hit() else {
            if self.plugin_restart_hit() {
                self.plugin_request = Some(PluginSettingsRequest::Restart);
            }
            return;
        };
        if on_toggle {
            let plugin = &self.plugins[index];
            self.plugin_request = Some(PluginSettingsRequest::SetEnabled {
                id: plugin.id.clone(),
                enabled: !plugin.enabled,
            });
        } else {
            self.open_plugin_detail(self.plugins[index].id.clone());
        }
    }

    fn handle_marketplace_click(&mut self) {
        if self.marketplace_retry_hit() {
            self.marketplace_state = MarketplaceViewState::Loading;
            self.plugin_request = Some(PluginSettingsRequest::LoadMarketplace);
            self.mark_items_dirty();
            return;
        }
        let Some((index, on_action)) = self.marketplace_plugin_hit() else {
            if self.plugin_restart_hit() {
                self.plugin_request = Some(PluginSettingsRequest::Restart);
            }
            return;
        };
        let plugin = match &self.marketplace_state {
            MarketplaceViewState::Loaded(plugins) => plugins[index].clone(),
            _ => return,
        };
        if on_action && self.marketplace_action(&plugin).is_available() {
            self.marketplace_installing_id = Some(plugin.id.clone());
            self.plugin_request = Some(PluginSettingsRequest::InstallMarketplace(Box::new(plugin)));
            self.mark_items_dirty();
        } else {
            self.open_plugin_detail(plugin.id);
        }
    }

    fn open_plugin_detail(&mut self, id: String) {
        self.pending_plugin_uninstall_id = None;
        self.selected_plugin_id = Some(id);
        self.plugin_detail_closing = false;
        self.plugin_detail_scroll = 0.0;
        self.anim.set_with_speed(PLUGIN_DETAIL_KEY, 1.0, 0.24);
    }

    fn switch_plugin_tab(&mut self, tab: PluginPageTab) {
        if self.plugin_page_tab == tab {
            return;
        }
        self.plugin_page_tab = tab;
        clear_plugin_icon_cache();
        self.pending_plugin_uninstall_id = None;
        self.selected_plugin_id = None;
        self.plugin_detail_closing = false;
        self.plugin_detail_scroll = 0.0;
        self.anim.set_with_speed(PLUGIN_DETAIL_KEY, 0.0, 0.24);
        self.scroll_y = 0.0;
        self.target_scroll_y = 0.0;
        self.scroll_vel_y = 0.0;
        self.mark_items_dirty();
        if tab == PluginPageTab::Marketplace
            && !matches!(self.marketplace_state, MarketplaceViewState::Loading)
        {
            if !matches!(self.marketplace_state, MarketplaceViewState::Loaded(_)) {
                self.marketplace_state = MarketplaceViewState::Loading;
            }
            self.plugin_request = Some(PluginSettingsRequest::LoadMarketplace);
        }
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }

    pub(crate) fn plugin_hovered(&self) -> bool {
        self.plugin_tab_hit().is_some()
            || match self.plugin_page_tab {
                PluginPageTab::Installed => self.installed_plugin_hit().is_some(),
                PluginPageTab::Marketplace => {
                    self.marketplace_plugin_hit().is_some() || self.marketplace_retry_hit()
                }
            }
            || self.plugin_restart_hit()
            || (self.anim.get(PLUGIN_DETAIL_KEY) > 0.005 && self.plugin_detail_hovered())
    }

    pub(super) fn marketplace_action(&self, plugin: &MarketplacePlugin) -> MarketplaceAction {
        if self
            .marketplace_installing_id
            .as_ref()
            .is_some_and(|id| id.eq_ignore_ascii_case(&plugin.id))
        {
            return MarketplaceAction::Downloading;
        }
        if plugin.revoked_reason.is_some() {
            return MarketplaceAction::Revoked;
        }
        if !plugin.is_compatible() {
            return MarketplaceAction::Incompatible;
        }
        match self
            .plugins
            .iter()
            .find(|installed| installed.id.eq_ignore_ascii_case(&plugin.id))
        {
            Some(installed) if plugin.has_update_for(&installed.version) => {
                MarketplaceAction::Update
            }
            Some(_) => MarketplaceAction::Installed,
            None => MarketplaceAction::Install,
        }
    }

    fn card_hovered(&self, card: Rect) -> bool {
        card.contains(Point::new(
            self.logical_mouse_pos.0 - SIDEBAR_W,
            self.logical_mouse_pos.1 + self.scroll_y,
        ))
    }

    fn plugin_tab_hit(&self) -> Option<PluginPageTab> {
        let scale = self
            .window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0);
        let width = self.win_w / scale;
        let point = Point::new(self.logical_mouse_pos.0, self.logical_mouse_pos.1);
        [PluginPageTab::Installed, PluginPageTab::Marketplace]
            .into_iter()
            .find(|tab| plugin_tab_rect(width, *tab).contains(point))
    }

    fn installed_plugin_hit(&self) -> Option<(usize, bool)> {
        let (mx, my) = self.plugin_list_mouse();
        let width = self.plugin_content_width();
        let mut y = PLUGIN_LIST_TOP + 30.0;
        for (index, _) in self.plugins.iter().enumerate() {
            let card = plugin_card(width, y);
            if card.contains(Point::new(mx, my)) {
                return Some((index, mx >= card.right - 70.0));
            }
            y += PLUGIN_CARD_H + PLUGIN_CARD_GAP;
        }
        None
    }

    fn marketplace_plugin_hit(&self) -> Option<(usize, bool)> {
        let MarketplaceViewState::Loaded(plugins) = &self.marketplace_state else {
            return None;
        };
        let (mx, my) = self.plugin_list_mouse();
        let width = self.plugin_content_width();
        let mut y = PLUGIN_LIST_TOP + 30.0;
        for (index, plugin) in plugins.iter().enumerate() {
            let card = plugin_card(width, y);
            if card.contains(Point::new(mx, my)) {
                let action_width = action_button_width(&self.marketplace_action(plugin).label());
                return Some((
                    index,
                    marketplace_action_rect(card, action_width).contains(Point::new(mx, my)),
                ));
            }
            y += PLUGIN_CARD_H + PLUGIN_CARD_GAP;
        }
        None
    }

    fn marketplace_retry_hit(&self) -> bool {
        if !matches!(self.marketplace_state, MarketplaceViewState::Failed(_)) {
            return false;
        }
        let (mx, my) = self.plugin_list_mouse();
        retry_button_rect(self.plugin_content_width(), PLUGIN_LIST_TOP + 30.0 + 84.0)
            .contains(Point::new(mx, my))
    }

    fn plugin_restart_hit(&self) -> bool {
        let Some((_, true)) = &self.plugin_status else {
            return false;
        };
        let entry_count = match (&self.plugin_page_tab, &self.marketplace_state) {
            (PluginPageTab::Installed, _) => self.plugins.len(),
            (PluginPageTab::Marketplace, MarketplaceViewState::Loaded(plugins)) => plugins.len(),
            _ => 0,
        };
        let empty_height = if entry_count == 0 { 126.0 } else { 0.0 };
        let y = PLUGIN_LIST_TOP
            + 30.0
            + entry_count as f32 * (PLUGIN_CARD_H + PLUGIN_CARD_GAP)
            + empty_height
            + 6.0;
        let width = self.plugin_content_width();
        let (mx, my) = self.plugin_list_mouse();
        Rect::from_xywh(width - CONTENT_PADDING - 120.0, y, 120.0, 48.0)
            .contains(Point::new(mx, my))
    }

    fn plugin_list_mouse(&self) -> (f32, f32) {
        (
            self.logical_mouse_pos.0 - SIDEBAR_W,
            self.logical_mouse_pos.1 + self.scroll_y,
        )
    }

    fn plugin_content_width(&self) -> f32 {
        let scale = self
            .window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0);
        self.win_w / scale - SIDEBAR_W
    }
}

fn draw_plugin_tabs(canvas: &Canvas, theme: &SettingsTheme, width: f32, active: PluginPageTab) {
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let background = Rect::from_xywh(
        SIDEBAR_W + CONTENT_PADDING,
        PLUGIN_TABS_Y,
        PLUGIN_TAB_W * 2.0,
        PLUGIN_TABS_H,
    );
    paint.set_color(theme.control_bg);
    canvas.draw_round_rect(background, 9.0, 9.0, &paint);
    for tab in [PluginPageTab::Installed, PluginPageTab::Marketplace] {
        let rect = plugin_tab_rect(width, tab);
        if tab == active {
            paint.set_color(theme.card_highlight);
            canvas.draw_round_rect(
                Rect::from_xywh(
                    rect.left + 2.0,
                    rect.top + 2.0,
                    rect.width() - 4.0,
                    rect.height() - 4.0,
                ),
                7.0,
                7.0,
                &paint,
            );
        }
        paint.set_color(if tab == active {
            theme.text_pri
        } else {
            theme.text_sec
        });
        draw_centered_text(
            canvas,
            fm,
            &tr(match tab {
                PluginPageTab::Installed => "plugin_tab_installed",
                PluginPageTab::Marketplace => "plugin_tab_marketplace",
            }),
            (rect.center_x(), rect.top + 21.0),
            12.0,
            tab == active,
            &paint,
        );
    }
}

fn plugin_tab_rect(_width: f32, tab: PluginPageTab) -> Rect {
    Rect::from_xywh(
        SIDEBAR_W
            + CONTENT_PADDING
            + if tab == PluginPageTab::Marketplace {
                PLUGIN_TAB_W
            } else {
                0.0
            },
        PLUGIN_TABS_Y,
        PLUGIN_TAB_W,
        PLUGIN_TABS_H,
    )
}

fn draw_section_title(canvas: &Canvas, theme: &SettingsTheme, title: String) -> f32 {
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(theme.text_pri);
    fm.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: &title,
        x: CONTENT_PADDING + 4.0,
        y: PLUGIN_LIST_TOP + 18.0,
        size: 13.0,
        bold: true,
        paint: &paint,
    });
    PLUGIN_LIST_TOP + 30.0
}

fn plugin_card(width: f32, y: f32) -> Rect {
    Rect::from_xywh(
        CONTENT_PADDING,
        y,
        width - CONTENT_PADDING * 2.0,
        PLUGIN_CARD_H,
    )
}

fn draw_card_background(canvas: &Canvas, theme: &SettingsTheme, card: Rect, hovered: bool) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(if hovered {
        theme.card_highlight
    } else {
        theme.group_bg
    });
    canvas.draw_round_rect(card, 13.0, 13.0, &paint);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(0.75);
    paint.set_color(theme.group_border);
    canvas.draw_round_rect(
        Rect::from_xywh(
            card.left + 0.375,
            card.top + 0.375,
            card.width() - 0.75,
            card.height() - 0.75,
        ),
        12.625,
        12.625,
        &paint,
    );
}

fn draw_card_text(
    canvas: &Canvas,
    theme: &SettingsTheme,
    card: Rect,
    name: &str,
    subtitle: &str,
    reserved_width: f32,
) {
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let available = (card.width() - reserved_width).max(20.0);
    paint.set_color(theme.text_pri);
    let name = ellipsize_text(fm, name, 14.0, FontStyle::bold(), available);
    fm.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: &name,
        x: card.left + 76.0,
        y: card.top + 29.0,
        size: 14.0,
        bold: true,
        paint: &paint,
    });
    paint.set_color(theme.text_sec);
    let subtitle = ellipsize_text(fm, subtitle, 11.5, FontStyle::normal(), available);
    fm.draw_text_cached(DrawTextCachedParams {
        canvas,
        text: &subtitle,
        x: card.left + 76.0,
        y: card.top + 51.0,
        size: 11.5,
        bold: false,
        paint: &paint,
    });
}

fn draw_marketplace_action(
    canvas: &Canvas,
    theme: &SettingsTheme,
    rect: Rect,
    label: &str,
    action: MarketplaceAction,
) {
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    if action.is_available() {
        paint.set_color(Color::from_argb(
            32,
            theme.accent.r(),
            theme.accent.g(),
            theme.accent.b(),
        ));
    } else {
        paint.set_color(theme.control_bg);
    }
    canvas.draw_round_rect(rect, rect.height() / 2.0, rect.height() / 2.0, &paint);
    paint.set_color(if action.is_available() {
        theme.accent
    } else {
        theme.text_sec
    });
    draw_centered_text(
        canvas,
        fm,
        label,
        (rect.center_x(), rect.top + 19.0),
        11.0,
        true,
        &paint,
    );
}

fn marketplace_action_rect(card: Rect, width: f32) -> Rect {
    Rect::from_xywh(card.right - width - 14.0, card.top + 23.0, width, 30.0)
}

fn action_button_width(label: &str) -> f32 {
    (FontManager::global().measure_text_cached(label, 11.0, FontStyle::bold()) + 24.0)
        .clamp(64.0, 112.0)
}

fn marketplace_subtitle(plugin: &MarketplacePlugin) -> String {
    match plugin.categories.first() {
        Some(category) => format!("{} · v{} · {}", plugin.author, plugin.version, category),
        None => format!("{} · v{}", plugin.author, plugin.version),
    }
}

fn draw_empty_state(
    canvas: &Canvas,
    theme: &SettingsTheme,
    width: f32,
    baseline: f32,
    title: &str,
    detail: Option<&str>,
) {
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(theme.text_sec);
    draw_centered_text(
        canvas,
        fm,
        title,
        (width / 2.0, baseline),
        13.0,
        false,
        &paint,
    );
    if let Some(detail) = detail {
        let detail = ellipsize_text(
            fm,
            detail,
            10.5,
            FontStyle::normal(),
            width - CONTENT_PADDING * 4.0,
        );
        paint.set_color(Color::from_argb(
            150,
            theme.text_sec.r(),
            theme.text_sec.g(),
            theme.text_sec.b(),
        ));
        draw_centered_text(
            canvas,
            fm,
            &detail,
            (width / 2.0, baseline + 21.0),
            10.5,
            false,
            &paint,
        );
    }
}

fn draw_retry_button(canvas: &Canvas, theme: &SettingsTheme, width: f32, y: f32) {
    let rect = retry_button_rect(width, y);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(theme.control_bg);
    canvas.draw_round_rect(rect, rect.height() / 2.0, rect.height() / 2.0, &paint);
    paint.set_color(theme.accent);
    draw_centered_text(
        canvas,
        FontManager::global(),
        &tr("plugin_marketplace_retry"),
        (rect.center_x(), rect.top + 18.0),
        11.0,
        true,
        &paint,
    );
}

fn retry_button_rect(width: f32, y: f32) -> Rect {
    Rect::from_xywh(width / 2.0 - 42.0, y, 84.0, 28.0)
}

fn draw_toggle(canvas: &Canvas, theme: &SettingsTheme, enabled: bool, x: f32, y: f32) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(if enabled {
        theme.toggle_on
    } else {
        theme.toggle_off
    });
    canvas.draw_round_rect(Rect::from_xywh(x, y, 36.0, 20.0), 10.0, 10.0, &paint);
    paint.set_color(Color::WHITE);
    canvas.draw_circle(
        (x + if enabled { 26.0 } else { 10.0 }, y + 10.0),
        8.0,
        &paint,
    );
}

pub(super) fn draw_plugin_icon(
    direct_context: &mut skia_safe::gpu::DirectContext,
    canvas: &Canvas,
    plugin: &InstalledPlugin,
    rect: Rect,
) {
    draw_plugin_icon_data(
        direct_context,
        canvas,
        &plugin.id,
        &plugin.name,
        plugin.icon.as_deref(),
        rect,
    );
}

pub(super) fn draw_plugin_icon_data(
    direct_context: &mut skia_safe::gpu::DirectContext,
    canvas: &Canvas,
    id: &str,
    name: &str,
    icon: Option<&[u8]>,
    rect: Rect,
) {
    if let Some(image) = icon.and_then(|bytes| {
        PLUGIN_ICONS.with(|cache| {
            if let Some(image) = cache.borrow().get(id) {
                return Some(image.clone());
            }
            let image = Image::from_encoded(Data::new_copy(bytes))?
                .new_texture_image(direct_context, skia_safe::gpu::Mipmapped::Yes)?;
            cache.borrow_mut().insert(id.to_string(), image.clone());
            Some(image)
        })
    }) {
        let save_count = canvas.save();
        canvas.clip_rrect(
            RRect::new_rect_xy(rect, rect.width() * 0.22, rect.height() * 0.22),
            ClipOp::Intersect,
            true,
        );
        canvas.draw_image_rect(image, None, rect, &Paint::default());
        canvas.restore_to_count(save_count);
        return;
    }
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_rgb(175, 82, 222));
    canvas.draw_round_rect(rect, rect.width() * 0.22, rect.height() * 0.22, &paint);
    paint.set_color(Color::WHITE);
    let initial = name
        .chars()
        .next()
        .unwrap_or('P')
        .to_uppercase()
        .to_string();
    draw_centered_text(
        canvas,
        FontManager::global(),
        &initial,
        (rect.center_x(), rect.center_y() + rect.height() * 0.16),
        rect.height() * 0.44,
        true,
        &paint,
    );
}

pub(super) fn ellipsize_text(
    fm: &FontManager,
    text: &str,
    size: f32,
    style: FontStyle,
    max_width: f32,
) -> String {
    if fm.measure_text_cached(text, size, style) <= max_width {
        return text.to_string();
    }
    let ellipsis = "…";
    let ellipsis_width = fm.measure_text_cached(ellipsis, size, style);
    let mut fitted = String::new();
    for character in text.chars() {
        fitted.push(character);
        if fm.measure_text_cached(&fitted, size, style) + ellipsis_width > max_width {
            fitted.pop();
            break;
        }
    }
    fitted.push_str(ellipsis);
    fitted
}

pub(super) fn draw_centered_text(
    canvas: &Canvas,
    fm: &FontManager,
    text: &str,
    position: (f32, f32),
    size: f32,
    bold: bool,
    paint: &Paint,
) {
    let (center_x, baseline) = position;
    let style = if bold {
        FontStyle::bold()
    } else {
        FontStyle::normal()
    };
    let width = fm.measure_text_cached(text, size, style);
    fm.draw_text_cached(DrawTextCachedParams {
        canvas,
        text,
        x: center_x - width / 2.0,
        y: baseline,
        size,
        bold,
        paint,
    });
}
