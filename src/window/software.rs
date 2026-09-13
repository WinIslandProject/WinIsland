use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;

use skia_safe::{ColorType, Surface, surfaces};
use softbuffer::{Context, Surface as SoftSurface};
use winit::window::Window;

use super::renderer::DrawingContext;

const TRANSPARENT_COLOR: u32 = 0x0001_0001;
const MIN_VISIBLE_ALPHA: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SoftwareTargetId(u64);

pub(crate) const MAIN_SOFTWARE_TARGET: SoftwareTargetId = SoftwareTargetId(0);

type WindowSurface = SoftSurface<Arc<Window>, Arc<Window>>;

struct SoftwareTarget {
    window: Arc<Window>,
    presentation: WindowSurface,
    raster: Surface,
}

pub(crate) struct SoftwareRenderer {
    context: Context<Arc<Window>>,
    targets: HashMap<SoftwareTargetId, SoftwareTarget>,
    next_target_id: u64,
    failure: Option<String>,
}

impl SoftwareRenderer {
    pub(crate) fn new(window: &Arc<Window>, width: u32, height: u32) -> Result<Self, String> {
        let context = Context::new(window.clone())
            .map_err(|error| format!("Softbuffer context creation failed: {error}"))?;
        let mut renderer = Self {
            context,
            targets: HashMap::new(),
            next_target_id: 0,
            failure: None,
        };
        let target = renderer.create_target(window, width, height)?;
        debug_assert_eq!(target, MAIN_SOFTWARE_TARGET);
        log::info!("Software rendering fallback initialized");
        Ok(renderer)
    }

    pub(crate) fn create_target(
        &mut self,
        window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<SoftwareTargetId, String> {
        crate::utils::win32::enable_software_transparency(window)?;
        let mut presentation = SoftSurface::new(&self.context, window.clone())
            .map_err(|error| format!("Softbuffer surface creation failed: {error}"))?;
        resize_presentation(&mut presentation, width, height)?;
        let raster = create_raster_surface(width, height)?;
        let target_id = SoftwareTargetId(self.next_target_id);
        self.next_target_id += 1;
        self.targets.insert(
            target_id,
            SoftwareTarget {
                window: window.clone(),
                presentation,
                raster,
            },
        );
        Ok(target_id)
    }

    pub(crate) fn draw<T>(
        &mut self,
        target_id: SoftwareTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> Result<T, String> {
        let result = self.draw_inner(target_id, draw);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    fn draw_inner<T>(
        &mut self,
        target_id: SoftwareTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> Result<T, String> {
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or_else(|| "Software render target is unavailable".to_string())?;
        let mut context = DrawingContext::software();
        let output = draw(&mut context, &mut target.raster);
        let pixmap = target
            .raster
            .peek_pixels()
            .ok_or_else(|| "Skia raster pixels are unavailable".to_string())?;
        let bytes = pixmap
            .bytes()
            .ok_or_else(|| "Skia raster pixel storage is unavailable".to_string())?;
        let row_bytes = pixmap.row_bytes();
        let width = usize::try_from(pixmap.width())
            .map_err(|_| "Software render width is invalid".to_string())?;
        let height = usize::try_from(pixmap.height())
            .map_err(|_| "Software render height is invalid".to_string())?;
        let color_type = pixmap.color_type();

        target.window.pre_present_notify();
        let mut buffer = target
            .presentation
            .buffer_mut()
            .map_err(|error| format!("Softbuffer frame acquisition failed: {error}"))?;
        if buffer.len() != width * height {
            return Err("Softbuffer returned an unexpected frame size".to_string());
        }
        for y in 0..height {
            let source = &bytes[y * row_bytes..y * row_bytes + width * 4];
            let destination = &mut buffer[y * width..(y + 1) * width];
            for (pixel, output) in source.as_chunks::<4>().0.iter().zip(destination) {
                let (red, green, blue, alpha) = match color_type {
                    ColorType::BGRA8888 => (pixel[2], pixel[1], pixel[0], pixel[3]),
                    ColorType::RGBA8888 => (pixel[0], pixel[1], pixel[2], pixel[3]),
                    _ => return Err("Unsupported Skia software pixel format".to_string()),
                };
                *output = software_pixel(red, green, blue, alpha);
            }
        }
        buffer
            .present()
            .map_err(|error| format!("Softbuffer presentation failed: {error}"))?;
        Ok(output)
    }

    pub(crate) fn resize(
        &mut self,
        target_id: SoftwareTargetId,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or_else(|| "Software render target is unavailable".to_string())?;
        resize_presentation(&mut target.presentation, width, height)?;
        target.raster = create_raster_surface(width, height)?;
        Ok(())
    }

    pub(crate) fn take_failure(&mut self) -> Option<String> {
        self.failure.take()
    }

    pub(crate) fn remove_target(&mut self, target_id: SoftwareTargetId) {
        self.targets.remove(&target_id);
    }
}

fn resize_presentation(
    presentation: &mut WindowSurface,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let width = NonZeroU32::new(width.max(1)).unwrap();
    let height = NonZeroU32::new(height.max(1)).unwrap();
    presentation
        .resize(width, height)
        .map_err(|error| format!("Softbuffer resize failed: {error}"))
}

fn create_raster_surface(width: u32, height: u32) -> Result<Surface, String> {
    let width = i32::try_from(width.max(1))
        .map_err(|_| "Software render width exceeds Skia limits".to_string())?;
    let height = i32::try_from(height.max(1))
        .map_err(|_| "Software render height exceeds Skia limits".to_string())?;
    surfaces::raster_n32_premul((width, height))
        .ok_or_else(|| "Skia failed to create a software surface".to_string())
}

fn software_pixel(red: u8, green: u8, blue: u8, alpha: u8) -> u32 {
    if alpha <= MIN_VISIBLE_ALPHA {
        return TRANSPARENT_COLOR;
    }
    let alpha = u32::from(alpha);
    let unpremultiply = |channel: u8| (u32::from(channel) * 255 / alpha).min(255);
    let color = unpremultiply(red) << 16 | unpremultiply(green) << 8 | unpremultiply(blue);
    if color == TRANSPARENT_COLOR {
        TRANSPARENT_COLOR + 1
    } else {
        color
    }
}
