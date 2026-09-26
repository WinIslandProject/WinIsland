use winisland_render::{Painter, Point, Radius, Rect, Rgba, Vec2};

use crate::plugin::manager::InstalledPlugin;
use crate::plugin::marketplace::MarketplacePlugin;
use crate::utils::color::SettingsTheme;
use crate::utils::color::settings_color;
use crate::utils::settings_ui::{SettingsPainter, ellipsize_text};
use winisland_core::i18n::tr;
use winisland_render::text::FontManager;

use super::super::super::{
    PLUGIN_DETAIL_KEY, PluginPageTab, PluginSettingsRequest, SETTINGS_HEADER_H, SIDEBAR_W,
    SettingsApp,
};
use super::{
    DETAIL_ICON_SIZE, DETAIL_W, MarketplaceAction, draw_plugin_icon, draw_plugin_icon_data,
    draw_toggle, markdown,
};

const DETAIL_PADDING: f32 = 20.0;
const DETAIL_HEADER_Y: f32 = 80.0;
const DETAIL_ACTION_Y: f32 = DETAIL_HEADER_Y + DETAIL_ICON_SIZE + 16.0;
const DETAIL_DESCRIPTION_Y: f32 = DETAIL_ACTION_Y + BUTTON_H + 20.0;
const BUTTON_H: f32 = 28.0;
const UNINSTALL_BUTTON_W: f32 = 76.0;
const ACTION_GAP: f32 = 8.0;

#[derive(Clone)]
enum DetailPlugin {
    Installed(InstalledPlugin),
    Marketplace(MarketplacePlugin),
}

impl DetailPlugin {
    fn name(&self) -> &str {
        match self {
            Self::Installed(plugin) => &plugin.name,
            Self::Marketplace(plugin) => &plugin.name,
        }
    }

    fn author(&self) -> &str {
        match self {
            Self::Installed(plugin) => &plugin.author,
            Self::Marketplace(plugin) => &plugin.author,
        }
    }

    fn version(&self) -> &str {
        match self {
            Self::Installed(plugin) => &plugin.version,
            Self::Marketplace(plugin) => &plugin.version,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Installed(plugin) => &plugin.description,
            Self::Marketplace(plugin) => &plugin.description,
        }
    }

    fn repository(&self) -> &str {
        match self {
            Self::Installed(plugin) => &plugin.github_link,
            Self::Marketplace(plugin) => &plugin.repository,
        }
    }

    fn readme(&self) -> &str {
        match self {
            Self::Installed(plugin) => plugin.readme.as_deref().unwrap_or(""),
            Self::Marketplace(plugin) => &plugin.readme,
        }
    }
}

impl SettingsApp {
    fn plugin_detail_panel_x(&self) -> f32 {
        self.logical_window_size().0 - DETAIL_W * self.anim.get(PLUGIN_DETAIL_KEY)
    }

    pub(crate) fn plugin_detail_contains(&self, mouse_x: f32) -> bool {
        mouse_x >= self.plugin_detail_panel_x()
    }

    pub(super) fn close_plugin_detail(&mut self) {
        self.pending_plugin_uninstall_id = None;
        self.plugin_detail_closing = true;
        self.anim.set_with_speed(PLUGIN_DETAIL_KEY, 0.0, 0.28);
    }

    fn selected_detail_plugin(&self) -> Option<DetailPlugin> {
        let id = self.selected_plugin_id.as_ref()?;
        match self.plugin_page_tab {
            PluginPageTab::Installed => self
                .plugins
                .iter()
                .find(|plugin| &plugin.id == id)
                .cloned()
                .map(DetailPlugin::Installed),
            PluginPageTab::Marketplace => match &self.marketplace_state {
                super::super::super::MarketplaceViewState::Loaded(plugins) => plugins
                    .iter()
                    .find(|plugin| &plugin.id == id)
                    .cloned()
                    .map(DetailPlugin::Marketplace),
                _ => None,
            },
        }
    }

    pub(super) fn handle_plugin_detail_click(&mut self) -> bool {
        let Some(plugin) = self.selected_detail_plugin() else {
            return false;
        };
        let panel_x = self.plugin_detail_panel_x();
        let (mouse_x, mouse_y) = self.logical_mouse_pos;
        if mouse_x < panel_x {
            self.close_plugin_detail();
            return true;
        }

        let content_point = Point::new(mouse_x, mouse_y + self.plugin_detail_scroll);
        match &plugin {
            DetailPlugin::Installed(installed) => {
                if toggle_rect(panel_x).contains(content_point) {
                    self.plugin_request = Some(PluginSettingsRequest::SetEnabled {
                        id: installed.id.clone(),
                        enabled: !installed.enabled,
                    });
                    return true;
                }
                if uninstall_rect(panel_x, safe_github_url(&installed.github_link))
                    .contains(content_point)
                {
                    if self
                        .pending_plugin_uninstall_id
                        .as_ref()
                        .is_some_and(|id| id == &installed.id)
                    {
                        self.pending_plugin_uninstall_id = None;
                        self.plugin_request = Some(PluginSettingsRequest::Uninstall {
                            id: installed.id.clone(),
                        });
                    } else {
                        self.pending_plugin_uninstall_id = Some(installed.id.clone());
                    }
                    self.mark_items_dirty();
                    self.request_redraw();
                    return true;
                }
            }
            DetailPlugin::Marketplace(marketplace) => {
                let action = self.marketplace_action(marketplace);
                if detail_action_rect(panel_x, &action.label()).contains(content_point)
                    && action.is_available()
                {
                    self.marketplace_installing_id = Some(marketplace.id.clone());
                    self.plugin_request = Some(PluginSettingsRequest::InstallMarketplace(
                        Box::new(marketplace.clone()),
                    ));
                    self.mark_items_dirty();
                    return true;
                }
            }
        }
        if safe_github_url(plugin.repository()) && github_rect(panel_x).contains(content_point) {
            open_url(plugin.repository());
            return true;
        }

        let fallback = tr("plugin_readme_empty");
        let readme = if plugin.readme().trim().is_empty() {
            &fallback
        } else {
            plugin.readme()
        };
        if let Some(link) = markdown::links(
            readme,
            panel_x + DETAIL_PADDING,
            plugin_readme_y(&plugin),
            DETAIL_W - DETAIL_PADDING * 2.0,
        )
        .iter()
        .find(|link| link.rect.contains(content_point))
        {
            open_url(&link.url);
        }
        true
    }

    pub(super) fn plugin_detail_hovered(&self) -> bool {
        let Some(plugin) = self.selected_detail_plugin() else {
            return false;
        };
        let panel_x = self.plugin_detail_panel_x();
        let point = Point::new(
            self.logical_mouse_pos.0,
            self.logical_mouse_pos.1 + self.plugin_detail_scroll,
        );
        let action_hovered = match &plugin {
            DetailPlugin::Installed(installed) => {
                toggle_rect(panel_x).contains(point)
                    || uninstall_rect(panel_x, safe_github_url(&installed.github_link))
                        .contains(point)
            }
            DetailPlugin::Marketplace(marketplace) => {
                let action = self.marketplace_action(marketplace);
                action.is_available()
                    && detail_action_rect(panel_x, &action.label()).contains(point)
            }
        };
        if action_hovered
            || (safe_github_url(plugin.repository()) && github_rect(panel_x).contains(point))
        {
            return true;
        }
        let fallback = tr("plugin_readme_empty");
        let readme = if plugin.readme().trim().is_empty() {
            &fallback
        } else {
            plugin.readme()
        };
        markdown::links(
            readme,
            panel_x + DETAIL_PADDING,
            plugin_readme_y(&plugin),
            DETAIL_W - DETAIL_PADDING * 2.0,
        )
        .iter()
        .any(|link| link.rect.contains(point))
    }

    pub(super) fn draw_plugin_detail(
        &mut self,
        drawing_context: &mut winisland_render::DrawingContext<'_>,
        painter: Painter<'_>,
        theme: &SettingsTheme,
        win_w: f32,
        win_h: f32,
        progress: f32,
    ) {
        let Some(plugin) = self.selected_detail_plugin() else {
            return;
        };
        let panel_x = win_w - DETAIL_W * progress;
        draw_panel_background(painter, theme, panel_x, win_h, progress);
        draw_panel_header(painter, theme, panel_x);

        painter.save();
        painter.clip_rect(Rect::from_xywh(
            panel_x,
            SETTINGS_HEADER_H,
            DETAIL_W,
            win_h - SETTINGS_HEADER_H,
        ));
        painter.translate(Vec2::new(0.0, -self.plugin_detail_scroll));

        let fm = FontManager::global();
        let y = DETAIL_HEADER_Y;
        let icon_rect = Rect::from_xywh(
            panel_x + DETAIL_PADDING,
            y,
            DETAIL_ICON_SIZE,
            DETAIL_ICON_SIZE,
        );
        match &plugin {
            DetailPlugin::Installed(installed) => {
                draw_plugin_icon(drawing_context, painter, installed, icon_rect)
            }
            DetailPlugin::Marketplace(marketplace) => draw_plugin_icon_data(
                drawing_context,
                painter,
                &marketplace.id,
                &marketplace.name,
                marketplace.icon.as_deref(),
                icon_rect,
            ),
        };
        let info_x = panel_x + DETAIL_PADDING + DETAIL_ICON_SIZE + 14.0;
        let name_width = match plugin {
            DetailPlugin::Installed(_) => panel_x + DETAIL_W - DETAIL_PADDING - 46.0 - info_x,
            DetailPlugin::Marketplace(_) => panel_x + DETAIL_W - DETAIL_PADDING - info_x,
        };
        let name = ellipsize_text(
            fm,
            plugin.name(),
            17.0,
            winisland_render::FontStyle::bold(),
            name_width.max(30.0),
        );
        SettingsPainter::new(painter).text(&name, (info_x, y + 22.0), 17.0, true, theme.text_pri);
        let subtitle = ellipsize_text(
            fm,
            &format!("{} · v{}", plugin.author(), plugin.version()),
            11.5,
            winisland_render::FontStyle::normal(),
            DETAIL_W - (info_x - panel_x) - DETAIL_PADDING,
        );
        SettingsPainter::new(painter).text(
            &subtitle,
            (info_x, y + 43.0),
            11.5,
            false,
            theme.text_sec,
        );

        match &plugin {
            DetailPlugin::Installed(installed) => {
                draw_toggle(
                    painter,
                    theme,
                    installed.enabled,
                    panel_x + DETAIL_W - DETAIL_PADDING - 36.0,
                    y + 2.0,
                );
                draw_uninstall_button(
                    painter,
                    panel_x,
                    safe_github_url(&installed.github_link),
                    self.pending_plugin_uninstall_id
                        .as_ref()
                        .is_some_and(|id| id == &installed.id),
                );
            }
            DetailPlugin::Marketplace(marketplace) => {
                let action = self.marketplace_action(marketplace);
                draw_detail_action(painter, theme, panel_x, action);
            }
        }

        if safe_github_url(plugin.repository()) {
            let button = github_rect(panel_x);
            painter.fill_round_rect(
                button,
                Radius::uniform(BUTTON_H / 2.0),
                settings_color(theme.control_bg),
            );
            SettingsPainter::new(painter).centered_text(
                &tr("plugin_open_github"),
                (button.center_x(), button.top + 18.0),
                11.0,
                true,
                settings_color(theme.accent),
            );
        }

        let mut content_y = DETAIL_DESCRIPTION_Y;
        if let DetailPlugin::Marketplace(marketplace) = &plugin
            && let Some(reason) = &marketplace.revoked_reason
        {
            let warning = Rect::from_xywh(
                panel_x + DETAIL_PADDING,
                content_y,
                DETAIL_W - DETAIL_PADDING * 2.0,
                42.0,
            );
            painter.fill_round_rect(
                warning,
                Radius::uniform(10.0),
                Rgba::from_argb(28, 255, 69, 58),
            );
            let reason = ellipsize_text(
                fm,
                reason,
                11.0,
                winisland_render::FontStyle::normal(),
                warning.width() - 20.0,
            );
            SettingsPainter::new(painter).text(
                &reason,
                (warning.left + 10.0, warning.top + 25.0),
                11.0,
                false,
                Rgba::from_rgb(255, 69, 58),
            );
            content_y += 54.0;
        }
        let description = markdown::render(markdown::MarkdownRenderParams {
            painter,
            markdown: plugin.description(),
            origin: (panel_x + DETAIL_PADDING, content_y),
            width: DETAIL_W - DETAIL_PADDING * 2.0,
            visible_range: (
                self.plugin_detail_scroll + SETTINGS_HEADER_H,
                self.plugin_detail_scroll + win_h,
            ),
            colors: markdown_colors(theme),
        });
        content_y += description.height + 14.0;
        let fallback = tr("plugin_readme_empty");
        let readme_text = if plugin.readme().trim().is_empty() {
            &fallback
        } else {
            plugin.readme()
        };
        let readme = markdown::render(markdown::MarkdownRenderParams {
            painter,
            markdown: readme_text,
            origin: (panel_x + DETAIL_PADDING, content_y),
            width: DETAIL_W - DETAIL_PADDING * 2.0,
            visible_range: (
                self.plugin_detail_scroll + SETTINGS_HEADER_H,
                self.plugin_detail_scroll + win_h,
            ),
            colors: markdown_colors(theme),
        });
        content_y += readme.height + 28.0;
        painter.restore();

        self.plugin_detail_max_scroll = (content_y - win_h).max(0.0);
        self.plugin_detail_scroll = self
            .plugin_detail_scroll
            .clamp(0.0, self.plugin_detail_max_scroll);
    }
}

fn draw_detail_action(
    painter: Painter<'_>,
    theme: &SettingsTheme,
    panel_x: f32,
    action: MarketplaceAction,
) {
    let label = action.label();
    let rect = detail_action_rect(panel_x, &label);
    let color = if action.is_available() {
        settings_color(theme.accent).with_alpha(32)
    } else {
        settings_color(theme.control_bg)
    };
    painter.fill_round_rect(rect, Radius::uniform(BUTTON_H / 2.0), color);
    let text_color = if action.is_available() {
        theme.accent
    } else {
        theme.text_sec
    };
    SettingsPainter::new(painter).centered_text(
        &label,
        (rect.center_x(), rect.top + 18.0),
        11.0,
        true,
        settings_color(text_color),
    );
}

fn draw_uninstall_button(painter: Painter<'_>, panel_x: f32, has_github: bool, confirming: bool) {
    let rect = uninstall_rect(panel_x, has_github);
    let color = Rgba::from_argb(if confirming { 50 } else { 28 }, 255, 69, 58);
    painter.fill_round_rect(rect, Radius::uniform(BUTTON_H / 2.0), color);
    SettingsPainter::new(painter).centered_text(
        &tr(if confirming {
            "plugin_uninstall_confirm"
        } else {
            "plugin_uninstall"
        }),
        (rect.center_x(), rect.top + 18.0),
        11.0,
        true,
        Rgba::from_rgb(255, 69, 58),
    );
}

fn draw_panel_background(
    painter: Painter<'_>,
    theme: &SettingsTheme,
    panel_x: f32,
    win_h: f32,
    progress: f32,
) {
    painter.fill_rect(
        Rect::from_xywh(SIDEBAR_W, 0.0, panel_x - SIDEBAR_W, win_h),
        Rgba::from_argb((72.0 * progress) as u8, 0, 0, 0),
    );
    painter.fill_rect(
        Rect::from_xywh(panel_x, 0.0, DETAIL_W, win_h),
        settings_color(theme.win_bg),
    );
    painter.fill_rect(
        Rect::from_xywh(panel_x, 0.0, 0.5, win_h),
        settings_color(theme.separator),
    );
}

fn draw_panel_header(painter: Painter<'_>, theme: &SettingsTheme, panel_x: f32) {
    SettingsPainter::new(painter).text(
        &tr("plugin_details"),
        (panel_x + DETAIL_PADDING, 37.0),
        15.0,
        true,
        settings_color(theme.text_pri),
    );
}

fn toggle_rect(panel_x: f32) -> Rect {
    Rect::from_xywh(
        panel_x + DETAIL_W - DETAIL_PADDING - 42.0,
        DETAIL_HEADER_Y - 4.0,
        48.0,
        32.0,
    )
}

fn detail_action_rect(panel_x: f32, label: &str) -> Rect {
    let width = (FontManager::global().measure_text_cached(
        label,
        11.0,
        winisland_render::FontStyle::bold(),
    ) + 24.0)
        .clamp(64.0, 112.0);
    Rect::from_xywh(
        panel_x + DETAIL_PADDING + 116.0 + ACTION_GAP,
        DETAIL_ACTION_Y,
        width,
        BUTTON_H,
    )
}

fn github_rect(panel_x: f32) -> Rect {
    Rect::from_xywh(panel_x + DETAIL_PADDING, DETAIL_ACTION_Y, 116.0, BUTTON_H)
}

fn uninstall_rect(panel_x: f32, has_github: bool) -> Rect {
    Rect::from_xywh(
        panel_x + DETAIL_PADDING + if has_github { 116.0 + ACTION_GAP } else { 0.0 },
        DETAIL_ACTION_Y,
        UNINSTALL_BUTTON_W,
        BUTTON_H,
    )
}

fn plugin_readme_y(plugin: &DetailPlugin) -> f32 {
    DETAIL_DESCRIPTION_Y
        + match plugin {
            DetailPlugin::Marketplace(plugin) if plugin.revoked_reason.is_some() => 54.0,
            _ => 0.0,
        }
        + markdown::markdown_height(plugin.description(), DETAIL_W - DETAIL_PADDING * 2.0)
        + 14.0
}

fn safe_github_url(url: &str) -> bool {
    url.starts_with("https://github.com/")
}

fn markdown_colors(theme: &SettingsTheme) -> markdown::MarkdownColors {
    markdown::MarkdownColors {
        text: crate::utils::color::settings_color(theme.text_pri),
        secondary: crate::utils::color::settings_color(theme.text_sec),
        accent: crate::utils::color::settings_color(theme.accent),
        code_background: crate::utils::color::settings_color(theme.control_bg),
        quote_background: crate::utils::color::settings_color(theme.group_bg),
        separator: crate::utils::color::settings_color(theme.separator),
    }
}

fn open_url(url: &str) {
    if !markdown::safe_web_url(url) {
        return;
    }
    if let Err(error) = crate::platform::shell().open_url(url) {
        log::warn!("Could not open link: {error}");
    }
}
