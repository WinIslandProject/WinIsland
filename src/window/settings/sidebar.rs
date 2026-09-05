use std::cell::RefCell;

use crate::core::i18n::tr;
use crate::utils::color::SettingsTheme;
use crate::utils::settings_ui::items::{SIDEBAR_PAD, SIDEBAR_SEL_RADIUS};
use crate::utils::settings_ui::{SettingsPainter, settings_paint};
use skia_safe::{
    Canvas, Color, Data, FilterMode, Image, MipmapMode, Paint, Rect, SamplingOptions,
    gpu::{DirectContext, Mipmapped},
};

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
}

fn load_sidebar_icon(direct_context: &mut DirectContext, bytes: &[u8]) -> Image {
    let image = Image::from_encoded(Data::new_copy(bytes)).expect("Failed to load sidebar icon");
    image
        .new_texture_image(direct_context, Mipmapped::Yes)
        .expect("Failed to create mipmapped sidebar icon texture")
}

fn draw_sidebar_icon(
    direct_context: &mut DirectContext,
    canvas: &Canvas,
    index: usize,
    rect: Rect,
) {
    SIDEBAR_ICONS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.is_none() {
            *cache = Some(SIDEBAR_ICON_BYTES.map(|bytes| load_sidebar_icon(direct_context, bytes)));
        }
        let icons = cache.as_ref().expect("Sidebar icon cache was initialized");
        let paint = Paint::default();
        canvas.draw_image_rect_with_sampling_options(
            &icons[index],
            None,
            rect,
            SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
            &paint,
        );
    });
}

pub(super) fn clear_sidebar_icon_cache() {
    SIDEBAR_ICONS.with(|cache| {
        *cache.borrow_mut() = None;
    });
}

fn draw_window_control(
    canvas: &Canvas,
    center: (f32, f32),
    fill: Color,
    border: Color,
    highlighted: bool,
) {
    let mut paint = settings_paint(Color::from_argb(38, 0, 0, 0));
    canvas.draw_circle(
        (center.0, center.1 + 0.75),
        WINDOW_CONTROL_RADIUS + 0.25,
        &paint,
    );

    paint.set_color(fill);
    canvas.draw_circle(center, WINDOW_CONTROL_RADIUS, &paint);

    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(0.75);
    paint.set_color(border);
    canvas.draw_circle(center, WINDOW_CONTROL_RADIUS - 0.375, &paint);

    if highlighted {
        paint.set_style(skia_safe::paint::Style::Fill);
        paint.set_color(Color::from_argb(36, 255, 255, 255));
        canvas.draw_oval(
            Rect::from_xywh(center.0 - 3.5, center.1 - 4.25, 7.0, 2.25),
            &paint,
        );
    }
}

impl SettingsApp {
    pub(crate) fn draw_sidebar(
        &self,
        direct_context: &mut DirectContext,
        canvas: &Canvas,
        theme: &SettingsTheme,
    ) {
        let mut paint = settings_paint(theme.sidebar_bg);
        canvas.draw_rect(Rect::from_xywh(0.0, 0.0, SIDEBAR_W, self.win_h), &paint);

        let inactive_fill = if self.is_light {
            Color::from_rgb(184, 184, 188)
        } else {
            Color::from_rgb(82, 82, 86)
        };
        let fills = if self.focused {
            [
                Color::from_rgb(255, 95, 87),
                Color::from_rgb(254, 188, 46),
                Color::from_rgb(126, 126, 132),
            ]
        } else {
            [inactive_fill; 3]
        };
        let border = if self.is_light {
            Color::from_argb(38, 0, 0, 0)
        } else {
            Color::from_argb(64, 0, 0, 0)
        };

        for (center, fill) in WINDOW_CONTROL_CENTERS.into_iter().zip(fills) {
            draw_window_control(canvas, center, fill, border, self.focused);
        }

        if self.dots_hovered {
            let mut sym_paint = Paint::default();
            sym_paint.set_anti_alias(true);
            sym_paint.set_style(skia_safe::paint::Style::Stroke);
            sym_paint.set_stroke_width(1.1);
            sym_paint.set_stroke_cap(skia_safe::paint::Cap::Round);

            sym_paint.set_color(if self.focused {
                Color::from_rgb(78, 0, 2)
            } else {
                theme.text_sec
            });
            canvas.draw_line((17.5, 17.5), (22.5, 22.5), &sym_paint);
            canvas.draw_line((22.5, 17.5), (17.5, 22.5), &sym_paint);

            sym_paint.set_color(if self.focused {
                Color::from_rgb(92, 62, 0)
            } else {
                theme.text_sec
            });
            canvas.draw_line((36.5, 20.0), (43.5, 20.0), &sym_paint);
        }

        let mut sep = settings_paint(theme.separator);
        sep.set_stroke_width(0.5);
        sep.set_style(skia_safe::paint::Style::Stroke);
        canvas.draw_line((SIDEBAR_W, 0.0), (SIDEBAR_W, self.win_h), &sep);

        let pages = [
            tr("tab_general"),
            tr("tab_music"),
            tr("tab_widgets"),
            tr("tab_plugins"),
            tr("tab_about"),
        ];
        for (i, label) in pages.iter().enumerate() {
            let row_y = SIDEBAR_START_Y + i as f32 * (SIDEBAR_ROW_H + SIDEBAR_ROW_GAP);
            let row_x = SIDEBAR_PAD;
            let row_w = SIDEBAR_W - SIDEBAR_PAD * 2.0;

            if self.active_page == i {
                paint.set_color(if self.focused {
                    theme.selection_bg
                } else {
                    theme.card_highlight
                });
                canvas.draw_round_rect(
                    Rect::from_xywh(row_x, row_y, row_w, SIDEBAR_ROW_H),
                    SIDEBAR_SEL_RADIUS,
                    SIDEBAR_SEL_RADIUS,
                    &paint,
                );
                paint.set_color(if self.focused {
                    if self.is_light {
                        theme.selection_text
                    } else {
                        Color::WHITE
                    }
                } else {
                    theme.text_pri
                });
            } else {
                let hover_val = self.anim.get(SIDEBAR_KEY_BASE + i as u64);
                if hover_val > 0.005 {
                    let base = theme.sidebar_hover;
                    let alpha = (base.a() as f32 * hover_val) as u8;
                    paint.set_color(Color::from_argb(alpha, base.r(), base.g(), base.b()));
                    canvas.draw_round_rect(
                        Rect::from_xywh(row_x, row_y, row_w, SIDEBAR_ROW_H),
                        SIDEBAR_SEL_RADIUS,
                        SIDEBAR_SEL_RADIUS,
                        &paint,
                    );
                }
                paint.set_color(if self.sidebar_hover == i as i32 {
                    theme.text_pri
                } else {
                    theme.text_sec
                });
            }

            let icon_rect = Rect::from_xywh(row_x + 7.0, row_y + 6.0, 22.0, 22.0);
            draw_sidebar_icon(direct_context, canvas, i, icon_rect);

            SettingsPainter::new(canvas).text(
                label,
                (row_x + 36.0, row_y + 22.0),
                13.0,
                self.active_page == i,
                paint.color(),
            );
        }
    }
}
