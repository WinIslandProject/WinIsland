use winisland_render::{ImageOptions, Painter, Path, Rect, Rgba, Sampling, Vec2};

use crate::core::smtc::MediaInfo;
use crate::utils::backdrop::get_blurred_cover_background;
use winisland_render::DrawingContext;

pub(super) struct BackgroundParams<'a, 'context> {
    pub(super) painter: Painter<'a>,
    pub(super) drawing_context: &'a mut DrawingContext<'context>,
    pub(super) rect: Rect,
    pub(super) island_path: &'a Path,
    pub(super) island_style: &'a str,
    pub(super) host_backdrop: bool,
    pub(super) media: &'a MediaInfo,
}

fn draw_solid(painter: Painter<'_>, path: &Path, color: Rgba) {
    painter.fill_path(path, color);
}

fn draw_effect_base(painter: Painter<'_>, rect: Rect) {
    painter.fill_rect(rect, Rgba::from_rgb(32, 32, 36));
}

fn draw_host_glass(painter: Painter<'_>, path: &Path) {
    draw_solid(painter, path, Rgba::from_argb(150, 10, 10, 14));
}

pub(super) fn draw_background(params: BackgroundParams<'_, '_>) {
    let BackgroundParams {
        painter,
        drawing_context,
        rect,
        island_path,
        island_style,
        host_backdrop,
        media,
    } = params;
    let bg_color = Rgba::BLACK;
    let fallback_color = Rgba::from_argb(205, 32, 32, 36);

    painter.save();
    painter.clip_path(island_path);
    match island_style {
        "glass" => {
            if host_backdrop {
                draw_host_glass(painter, island_path);
            } else {
                draw_solid(painter, island_path, fallback_color);
            }
        }
        "dynamic" => {
            if let Some(blurred_cover) = get_blurred_cover_background(drawing_context, media) {
                draw_effect_base(painter, rect);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();

                let integrated =
                    crate::utils::gpu::gpu_profile() == crate::utils::gpu::GpuProfile::Integrated;
                let (rotate_speed, drift_speed_x, drift_speed_y, drift_amp_x, drift_amp_y) =
                    if integrated {
                        (0.015, 0.075, 0.06, 10.0, 7.5)
                    } else {
                        (0.03, 0.15, 0.12, 20.0, 15.0)
                    };

                let angle_rad = (now * rotate_speed) % (2.0 * std::f64::consts::PI);
                let angle_deg = angle_rad.to_degrees();

                let dx = (now * drift_speed_x).sin() * drift_amp_x;
                let dy = (now * drift_speed_y).cos() * drift_amp_y;

                let cx = rect.left + rect.width() / 2.0;
                let cy = rect.top + rect.height() / 2.0;

                let diagonal = rect.width().hypot(rect.height());
                let side_len = diagonal * 1.3f32;

                painter.save();
                painter.translate(Vec2::new(cx + dx as f32, cy + dy as f32));
                painter.rotate_degrees(angle_deg as f32);

                let draw_rect =
                    Rect::from_xywh(-side_len / 2.0, -side_len / 2.0, side_len, side_len);

                painter.draw_image(
                    &blurred_cover,
                    draw_rect,
                    &ImageOptions::default().with_sampling(Sampling::LinearNone),
                );
                painter.restore();
                painter.fill_rect(rect, Rgba::from_argb(120, 20, 20, 24));
            } else if host_backdrop {
                draw_host_glass(painter, island_path);
            } else {
                draw_solid(painter, island_path, fallback_color);
            }
        }
        _ => draw_solid(painter, island_path, bg_color),
    }
    painter.restore();
}
