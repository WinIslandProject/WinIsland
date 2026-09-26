use std::cell::RefCell;
use std::collections::HashMap;
use winisland_render::FontStyle;
use winisland_render::{
    Image, ImageOptions, Mipmapped, Painter, Point, Radius, Rect, Rgba, Sampling, StrokeCap,
};

use crate::utils::color::SettingsTheme;
use crate::utils::settings_ui::items::{SIDEBAR_PAD, SIDEBAR_SEL_RADIUS};
use crate::utils::settings_ui::{SettingsPainter, ellipsize_text, settings_color};
use winisland_core::i18n::tr;
use winisland_render::DrawingContext;
use winisland_render::text::FontManager;

use super::{
    SIDEBAR_KEY_BASE, SIDEBAR_ROW_GAP, SIDEBAR_ROW_H, SIDEBAR_START_Y, SIDEBAR_W, SettingsApp,
    WINDOW_CONTROL_CENTERS, WINDOW_CONTROL_RADIUS,
};

const SIDEBAR_ICON_BYTES: [&[u8]; 5] = [
    include_bytes!("../../../resources/in_app/settings/settings.png"),
    include_bytes!("../../../resources/in_app/settings/music.png"),
    include_bytes!("../../../resources/in_app/settings/widget.png"),
    include_bytes!("../../../resources/in_app/settings/plugin.png"),
    include_bytes!("../../../resources/in_app/settings/about.png"),
];

thread_local! {
    static SIDEBAR_ICONS: RefCell<Option<[Image; 5]>> = const { RefCell::new(None) };
    static PLUGIN_SETTINGS_ICONS: RefCell<HashMap<u64, Image>> = RefCell::new(HashMap::new());
}

fn load_sidebar_icon(drawing_context: &mut DrawingContext<'_>, bytes: &[u8]) -> Image {
    let image = Image::from_encoded(bytes).expect("Failed to load sidebar icon");
    drawing_context
        .prepare_image(image, Mipmapped::Yes)
        .expect("Failed to create mipmapped sidebar icon texture")
}

fn draw_sidebar_icon(
    drawing_context: &mut DrawingContext<'_>,
    painter: Painter<'_>,
    index: usize,
    rect: Rect,
) {
    SIDEBAR_ICONS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.is_none() {
            *cache =
                Some(SIDEBAR_ICON_BYTES.map(|bytes| load_sidebar_icon(drawing_context, bytes)));
        }
        let icons = cache.as_ref().expect("Sidebar icon cache was initialized");
        painter.draw_image(
            &icons[index],
            rect,
            &ImageOptions::default()
                .with_sampling(Sampling::LinearLinear)
                .with_anti_alias(false),
        );
    });
}

pub(super) fn clear_sidebar_icon_cache() {
    SIDEBAR_ICONS.with(|cache| {
        *cache.borrow_mut() = None;
    });
}

pub(super) fn clear_plugin_settings_icon_cache() {
    PLUGIN_SETTINGS_ICONS.with(|cache| cache.borrow_mut().clear());
}

fn draw_plugin_settings_icon(
    drawing_context: &mut DrawingContext<'_>,
    painter: Painter<'_>,
    page: &winisland_core::plugin_settings::PluginSettingsPage,
    rect: Rect,
) {
    if page.icon.is_empty() {
        draw_sidebar_icon(drawing_context, painter, 3, rect);
        return;
    }
    let image = PLUGIN_SETTINGS_ICONS.with(|cache| {
        if let Some(image) = cache.borrow().get(&page.resource_id) {
            return Some(image.clone());
        }
        let image = Image::from_encoded(&page.icon)?;
        let image = drawing_context.prepare_image(image, Mipmapped::Yes)?;
        cache.borrow_mut().insert(page.resource_id, image.clone());
        Some(image)
    });
    if let Some(image) = image {
        painter.draw_image(
            &image,
            rect,
            &ImageOptions::default()
                .with_sampling(Sampling::LinearLinear)
                .with_anti_alias(false),
        );
    } else {
        draw_sidebar_icon(drawing_context, painter, 3, rect);
    }
}

fn draw_sidebar_row_background(
    app: &SettingsApp,
    painter: Painter<'_>,
    theme: &SettingsTheme,
    index: usize,
) -> Rgba {
    let row_y = SIDEBAR_START_Y + index as f32 * (SIDEBAR_ROW_H + SIDEBAR_ROW_GAP);
    let row_x = SIDEBAR_PAD;
    let row_w = SIDEBAR_W - SIDEBAR_PAD * 2.0;
    if app.active_page == index {
        painter.fill_round_rect(
            winisland_render::Rect::from_xywh(row_x, row_y, row_w, SIDEBAR_ROW_H),
            Radius::uniform(SIDEBAR_SEL_RADIUS),
            settings_color(if app.focused {
                theme.selection_bg
            } else {
                theme.card_highlight
            }),
        );
        if app.focused {
            if app.is_light {
                settings_color(theme.selection_text)
            } else {
                Rgba::WHITE
            }
        } else {
            settings_color(theme.text_pri)
        }
    } else {
        let hover = app.anim.get(SIDEBAR_KEY_BASE + index as u64);
        if hover > 0.005 {
            let base = theme.sidebar_hover;
            painter.fill_round_rect(
                winisland_render::Rect::from_xywh(row_x, row_y, row_w, SIDEBAR_ROW_H),
                Radius::uniform(SIDEBAR_SEL_RADIUS),
                Rgba::from_argb(
                    (base.a() as f32 * hover) as u8,
                    base.r(),
                    base.g(),
                    base.b(),
                ),
            );
        }
        settings_color(if app.sidebar_hover == index as i32 {
            theme.text_pri
        } else {
            theme.text_sec
        })
    }
}

fn draw_window_control(
    painter: Painter<'_>,
    center: (f32, f32),
    fill: Rgba,
    border: Rgba,
    highlighted: bool,
) {
    painter.fill_circle(
        Point::new(center.0, center.1 + 0.75),
        WINDOW_CONTROL_RADIUS + 0.25,
        Rgba::from_argb(38, 0, 0, 0),
    );

    painter.fill_circle(Point::new(center.0, center.1), WINDOW_CONTROL_RADIUS, fill);

    painter.stroke_circle(
        Point::new(center.0, center.1),
        WINDOW_CONTROL_RADIUS - 0.375,
        0.75,
        border,
    );

    if highlighted {
        painter.fill_oval(
            winisland_render::Rect::from_xywh(center.0 - 3.5, center.1 - 4.25, 7.0, 2.25),
            Rgba::from_argb(36, 255, 255, 255),
        );
    }
}

impl SettingsApp {
    pub(crate) fn draw_sidebar(
        &self,
        drawing_context: &mut DrawingContext<'_>,
        painter: Painter<'_>,
        theme: &SettingsTheme,
    ) {
        let logical_height = self.logical_window_size().1;
        painter.fill_rect(
            winisland_render::Rect::from_xywh(0.0, 0.0, SIDEBAR_W, logical_height),
            settings_color(theme.sidebar_bg),
        );

        let inactive_fill = if self.is_light {
            Rgba::from_rgb(184, 184, 188)
        } else {
            Rgba::from_rgb(82, 82, 86)
        };
        let fills = if self.focused {
            [
                Rgba::from_rgb(255, 95, 87),
                Rgba::from_rgb(254, 188, 46),
                Rgba::from_rgb(126, 126, 132),
            ]
        } else {
            [inactive_fill; 3]
        };
        let border = if self.is_light {
            Rgba::from_argb(38, 0, 0, 0)
        } else {
            Rgba::from_argb(64, 0, 0, 0)
        };

        for (center, fill) in WINDOW_CONTROL_CENTERS.into_iter().zip(fills) {
            draw_window_control(painter, center, fill, border, self.focused);
        }

        if self.dots_hovered {
            let close_color = if self.focused {
                Rgba::from_rgb(78, 0, 2)
            } else {
                settings_color(theme.text_sec)
            };
            painter.stroke_line(
                Point::new(17.5, 17.5),
                Point::new(22.5, 22.5),
                1.1,
                close_color,
                StrokeCap::Round,
            );
            painter.stroke_line(
                Point::new(22.5, 17.5),
                Point::new(17.5, 22.5),
                1.1,
                close_color,
                StrokeCap::Round,
            );

            let minimize_color = if self.focused {
                Rgba::from_rgb(92, 62, 0)
            } else {
                settings_color(theme.text_sec)
            };
            painter.stroke_line(
                Point::new(36.5, 20.0),
                Point::new(43.5, 20.0),
                1.1,
                minimize_color,
                StrokeCap::Round,
            );
        }

        painter.stroke_line(
            Point::new(SIDEBAR_W, 0.0),
            Point::new(SIDEBAR_W, logical_height),
            0.5,
            settings_color(theme.separator),
            StrokeCap::Butt,
        );

        let pages = [
            tr("tab_general"),
            tr("tab_music"),
            tr("tab_widgets"),
            tr("tab_plugins"),
            tr("tab_about"),
        ];
        for (index, label) in pages.iter().enumerate() {
            let row_y = SIDEBAR_START_Y + index as f32 * (SIDEBAR_ROW_H + SIDEBAR_ROW_GAP);
            let text_color = draw_sidebar_row_background(self, painter, theme, index);
            let icon_rect = Rect::from_xywh(SIDEBAR_PAD + 7.0, row_y + 6.0, 22.0, 22.0);
            draw_sidebar_icon(drawing_context, painter, index, icon_rect);
            SettingsPainter::new(painter).text(
                label,
                (SIDEBAR_PAD + 36.0, row_y + 22.0),
                13.0,
                self.active_page == index,
                text_color,
            );
        }

        for (offset, page) in self.plugin_settings_pages.iter().enumerate() {
            let index = super::BUILTIN_SIDEBAR_PAGE_COUNT + offset;
            let row_y = SIDEBAR_START_Y + index as f32 * (SIDEBAR_ROW_H + SIDEBAR_ROW_GAP);
            let text_color = draw_sidebar_row_background(self, painter, theme, index);
            let icon_rect = Rect::from_xywh(SIDEBAR_PAD + 7.0, row_y + 6.0, 22.0, 22.0);
            draw_plugin_settings_icon(drawing_context, painter, page, icon_rect);
            let label = ellipsize_text(
                FontManager::global(),
                &page.title,
                13.0,
                FontStyle::normal(),
                SIDEBAR_W - SIDEBAR_PAD * 2.0 - 40.0,
            );
            SettingsPainter::new(painter).text(
                &label,
                (SIDEBAR_PAD + 36.0, row_y + 22.0),
                13.0,
                self.active_page == index,
                text_color,
            );
        }
    }
}
