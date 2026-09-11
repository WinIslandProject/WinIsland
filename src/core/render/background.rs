use skia_safe::{
    Canvas, ClipOp, Color, FilterMode, MipmapMode, Paint, Path, Rect, SamplingOptions,
    gpu::DirectContext,
};

use crate::core::smtc::MediaInfo;
use crate::utils::backdrop::get_blurred_cover_background;

pub(super) struct BackgroundParams<'a> {
    pub(super) canvas: &'a Canvas,
    pub(super) direct_context: &'a mut DirectContext,
    pub(super) rect: Rect,
    pub(super) island_path: &'a Path,
    pub(super) island_style: &'a str,
    pub(super) host_backdrop: bool,
    pub(super) media: &'a MediaInfo,
}

fn draw_solid(canvas: &Canvas, path: &Path, color: Color) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(color);
    canvas.draw_path(path, &paint);
}

fn draw_effect_base(canvas: &Canvas, rect: Rect) {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgb(32, 32, 36));
    canvas.draw_rect(rect, &paint);
}

fn draw_host_glass(canvas: &Canvas, path: &Path) {
    draw_solid(canvas, path, Color::from_argb(150, 10, 10, 14));
}

pub(super) fn draw_background(params: BackgroundParams<'_>) {
    let BackgroundParams {
        canvas,
        direct_context,
        rect,
        island_path,
        island_style,
        host_backdrop,
        media,
    } = params;
    let bg_color = Color::BLACK;
    let fallback_color = Color::from_argb(205, 32, 32, 36);

    canvas.save();
    canvas.clip_path(island_path, ClipOp::Intersect, true);
    match island_style {
        "glass" => {
            if host_backdrop {
                draw_host_glass(canvas, island_path);
            } else {
                draw_solid(canvas, island_path, fallback_color);
            }
        }
        "mica" => {
            if host_backdrop {
                draw_solid(canvas, island_path, Color::from_argb(185, 32, 32, 36));
            } else {
                draw_solid(canvas, island_path, fallback_color);
            }
        }
        "dynamic" => {
            if let Some(blurred_cover) = get_blurred_cover_background(direct_context, media) {
                draw_effect_base(canvas, rect);
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

                let cx = rect.left() + rect.width() / 2.0;
                let cy = rect.top() + rect.height() / 2.0;

                let diagonal = rect.width().hypot(rect.height());
                let side_len = diagonal * 1.3f32;

                canvas.save();
                canvas.translate((cx + dx as f32, cy + dy as f32));
                canvas.rotate(angle_deg as f32, None);

                let draw_rect =
                    Rect::from_xywh(-side_len / 2.0, -side_len / 2.0, side_len, side_len);

                let mut paint = Paint::default();
                paint.set_anti_alias(true);
                canvas.draw_image_rect_with_sampling_options(
                    &blurred_cover,
                    None,
                    draw_rect,
                    SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                    &paint,
                );
                canvas.restore();
                paint.set_color(Color::from_argb(120, 20, 20, 24));
                canvas.draw_rect(rect, &paint);
            } else if host_backdrop {
                draw_host_glass(canvas, island_path);
            } else {
                draw_solid(canvas, island_path, fallback_color);
            }
        }
        _ => draw_solid(canvas, island_path, bg_color),
    }
    canvas.restore();
}
