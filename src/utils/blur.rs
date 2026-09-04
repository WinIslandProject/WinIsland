use crate::utils::gpu::{GpuProfile, gpu_profile};

const INTEGRATED_MAX_BLUR: (f32, f32) = (4.0, 3.5);
const DISCRETE_MAX_BLUR: (f32, f32) = (12.0, 10.0);
const SIZE_VELOCITY_BLUR_SCALE: f32 = 0.3;
const VIEW_VELOCITY_BLUR_SCALE: f32 = 0.4;

pub fn calculate_blur_sigmas(
    width_velocity: f32,
    height_velocity: f32,
    view_velocity: f32,
    current_width: f32,
) -> (f32, f32) {
    let (max_horizontal_blur, max_vertical_blur) = if gpu_profile() == GpuProfile::Integrated {
        INTEGRATED_MAX_BLUR
    } else {
        DISCRETE_MAX_BLUR
    };
    let view_pixel_velocity = view_velocity.abs() * current_width;
    let horizontal_blur = (width_velocity.abs() * SIZE_VELOCITY_BLUR_SCALE
        + view_pixel_velocity * VIEW_VELOCITY_BLUR_SCALE)
        .min(max_horizontal_blur);
    let vertical_blur = (height_velocity.abs() * SIZE_VELOCITY_BLUR_SCALE).min(max_vertical_blur);
    (horizontal_blur, vertical_blur)
}
