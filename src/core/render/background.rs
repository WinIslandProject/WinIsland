use skia_safe::canvas::SrcRectConstraint;
use skia_safe::{
    Canvas, ClipOp, Color, FilterMode, MipmapMode, Paint, Path, Rect, SamplingOptions,
    gpu::DirectContext,
};

use crate::core::smtc::MediaInfo;
use crate::utils::backdrop::{get_blurred_cover_background, get_mica_background};
use crate::utils::glass::{GlassBackgroundParams, get_glass_background};

pub(super) struct BackgroundParams<'a> {
    pub(super) canvas: &'a Canvas,
    pub(super) direct_context: &'a mut DirectContext,
    pub(super) rect: Rect,
    pub(super) island_path: &'a Path,
    pub(super) island_style: &'a str,
    pub(super) media: &'a MediaInfo,
    pub(super) win_x: i32,
    pub(super) win_y: i32,
    pub(super) offset_x: f32,
    pub(super) offset_y: f32,
    pub(super) current_w: f32,
    pub(super) current_h: f32,
    pub(super) global_scale: f32,
    pub(super) monitor_x: i32,
    pub(super) monitor_y: i32,
    pub(super) monitor_w: u32,
    pub(super) monitor_h: u32,
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

fn draw_glass(
    canvas: &Canvas,
    direct_context: &mut DirectContext,
    rect: Rect,
    params: GlassBackgroundParams,
) -> bool {
    let Some(background) = get_glass_background(direct_context, params) else {
        return false;
    };
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let source_rect = Rect::from_wh(background.width as f32, background.height as f32);
    canvas.draw_image_rect_with_sampling_options(
        &background.image,
        Some((&source_rect, SrcRectConstraint::Fast)),
        rect,
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        &paint,
    );
    paint.set_color(Color::from_argb(130, 10, 10, 14));
    paint.set_blend_mode(skia_safe::BlendMode::Multiply);
    canvas.draw_rect(rect, &paint);
    true
}

pub(super) fn draw_background(params: BackgroundParams<'_>) {
    let BackgroundParams {
        canvas,
        direct_context,
        rect,
        island_path,
        island_style,
        media,
        win_x,
        win_y,
        offset_x,
        offset_y,
        current_w,
        current_h,
        global_scale,
        monitor_x,
        monitor_y,
        monitor_w,
        monitor_h,
    } = params;
    let bg_color = Color::BLACK;
    let screen_x = win_x + offset_x as i32;
    let screen_y = win_y + offset_y as i32;
    let surface_info = canvas.image_info();
    let glass_params = || GlassBackgroundParams {
        screen_x,
        screen_y,
        width: current_w as u32,
        height: current_h as u32,
        blur_sigma: 40.0 * global_scale,
        surface_width: surface_info.width() as u32,
        surface_height: surface_info.height() as u32,
        monitor_x,
        monitor_y,
        monitor_w,
        monitor_h,
    };
    let fallback_color = Color::from_argb(205, 32, 32, 36);

    canvas.save();
    canvas.clip_path(island_path, ClipOp::Intersect, true);
    if matches!(island_style, "glass" | "mica" | "dynamic") {
        draw_effect_base(canvas, rect);
    }
    match island_style {
        "glass" => {
            if !draw_glass(canvas, direct_context, rect, glass_params()) {
                draw_solid(canvas, island_path, fallback_color);
            }
        }
        "mica" => {
            if let Some(bg_img) =
                get_mica_background(direct_context, monitor_x, monitor_y, monitor_w, monitor_h)
            {
                let crop_x = (screen_x - monitor_x).max(0) as f32;
                let crop_y = (screen_y - monitor_y).max(0) as f32;
                let source_rect = Rect::from_xywh(
                    crop_x / monitor_w as f32 * bg_img.width() as f32,
                    crop_y / monitor_h as f32 * bg_img.height() as f32,
                    (current_w / monitor_w as f32 * bg_img.width() as f32).max(1.0),
                    (current_h / monitor_h as f32 * bg_img.height() as f32).max(1.0),
                );
                let mut paint = Paint::default();
                paint.set_anti_alias(true);
                canvas.draw_image_rect_with_sampling_options(
                    &bg_img,
                    Some((&source_rect, SrcRectConstraint::Fast)),
                    rect,
                    SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                    &paint,
                );
                paint.set_color(Color::from_argb(110, 32, 32, 32));
                canvas.draw_path(island_path, &paint);
            } else {
                draw_solid(canvas, island_path, fallback_color);
            }
        }
        "dynamic" => {
            if let Some(blurred_cover) = get_blurred_cover_background(direct_context, media) {
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
            } else if !draw_glass(canvas, direct_context, rect, glass_params()) {
                draw_solid(canvas, island_path, fallback_color);
            }
        }
        _ => draw_solid(canvas, island_path, bg_color),
    }
    canvas.restore();
}
