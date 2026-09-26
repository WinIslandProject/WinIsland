use std::collections::HashMap;

use skia_safe::{Color, ImageInfo, Paint, Rect, Surface, gpu, image_filters};

use crate::backend::{D3DDevice, RenderTarget, check_surface_supported};
use crate::error::{RenderError, RenderResult};
use crate::image::Image;
use crate::painter::Painter;
use crate::surface::NativeSurface;
use crate::types::Mipmapped;
use crate::types::{Sampling, TileMode};

const MAIN_TARGET: RendererTargetId = RendererTargetId(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RendererOptions {
    pub width: u32,
    pub height: u32,
}

impl RendererOptions {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

pub struct DrawingContext<'a> {
    direct_context: &'a mut gpu::DirectContext,
}

impl DrawingContext<'_> {
    pub fn prepare_image(&mut self, image: Image, mipmapped: Mipmapped) -> Option<Image> {
        let mipmapped = match mipmapped {
            Mipmapped::No => gpu::Mipmapped::No,
            Mipmapped::Yes => gpu::Mipmapped::Yes,
        };
        image
            .as_skia()
            .new_texture_image(self.direct_context, mipmapped)
            .map(Image::from_skia)
    }

    fn render_surface(&mut self, info: &ImageInfo) -> Option<Surface> {
        gpu::surfaces::render_target(
            self.direct_context,
            gpu::Budgeted::Yes,
            info,
            None,
            Some(gpu::SurfaceOrigin::TopLeft),
            None,
            Some(false),
            Some(false),
        )
    }

    fn finish_surface(&mut self, surface: &mut Surface) {
        self.direct_context.flush_surface(surface);
    }

    pub fn scale_image(&mut self, image: &Image, width: i32, height: i32) -> Option<Image> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let info = ImageInfo::new_n32_premul((width, height), None);
        let mut surface = self.render_surface(&info)?;
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        surface.canvas().draw_image_rect_with_sampling_options(
            image.as_skia(),
            None,
            Rect::from_xywh(0.0, 0.0, width as f32, height as f32),
            crate::convert::to_skia_sampling(Sampling::LinearNone),
            &paint,
        );
        self.finish_surface(&mut surface);
        Some(Image::from_skia(surface.image_snapshot()))
    }

    pub fn blur_image(
        &mut self,
        image: &Image,
        width: i32,
        height: i32,
        sigma: (f32, f32),
        tile: Option<TileMode>,
    ) -> Option<Image> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let info = ImageInfo::new_n32_premul((width, height), None);
        let mut surface = self.render_surface(&info)?;
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        if let Some(filter) = image_filters::blur(
            sigma,
            tile.map(crate::convert::to_skia_tile_mode),
            None,
            None,
        ) {
            paint.set_image_filter(filter);
        }
        surface
            .canvas()
            .draw_image(image.as_skia(), (0, 0), Some(&paint));
        self.finish_surface(&mut surface);
        Some(Image::from_skia(surface.image_snapshot()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RendererTargetId(u64);

pub struct Renderer {
    targets: HashMap<RendererTargetId, RenderTarget>,
    device: D3DDevice,
    next_target_id: u64,
    failure: Option<String>,
}

impl Renderer {
    pub fn new(surface: NativeSurface, options: RendererOptions) -> RenderResult<Self> {
        check_surface_supported(&surface)?;
        let mut device = D3DDevice::new()?;
        let target = device.create_target(surface, options.width, options.height)?;
        Self::prewarm_expansion_effects(&mut device);
        Ok(Self {
            targets: HashMap::from([(MAIN_TARGET, target)]),
            device,
            next_target_id: 1,
            failure: None,
        })
    }

    pub fn create_target(
        &mut self,
        surface: NativeSurface,
        width: u32,
        height: u32,
    ) -> RenderResult<RendererTargetId> {
        check_surface_supported(&surface)?;
        self.check_ready()?;
        let target = self.device.create_target(surface, width, height);
        let target = self.record_result(target)?;
        let id = RendererTargetId(self.next_target_id);
        self.next_target_id += 1;
        self.targets.insert(id, target);
        Ok(id)
    }

    fn prewarm_expansion_effects(device: &mut D3DDevice) {
        let info = ImageInfo::new_n32_premul((96, 96), None);
        let mut drawing_context = DrawingContext {
            direct_context: &mut device.context,
        };
        let Some(mut surface) = drawing_context.render_surface(&info) else {
            return;
        };
        if let Some(filter) = image_filters::blur((12.0, 10.0), None, None, None) {
            let mut paint = Paint::default();
            paint.set_image_filter(filter);
            let canvas = surface.canvas();
            canvas.save_layer(&skia_safe::canvas::SaveLayerRec::default().paint(&paint));
            canvas.draw_rect(Rect::from_xywh(8.0, 8.0, 80.0, 80.0), &Paint::default());
            canvas.restore();
            drawing_context.finish_surface(&mut surface);
            drop(surface);
            if let Err(error) = device.submit(gpu::SyncCpu::Yes) {
                log::warn!("Expanded effect warm-up failed: {error}");
            }
        }
    }

    pub fn main_target(&self) -> RendererTargetId {
        MAIN_TARGET
    }

    fn draw<T>(
        &mut self,
        target_id: RendererTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> RenderResult<T> {
        let result = self.draw_inner(target_id, draw);
        self.record_result(result)
    }

    pub fn frame<T>(
        &mut self,
        target_id: RendererTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, Painter<'_>) -> T,
    ) -> RenderResult<T> {
        self.draw(target_id, |drawing_context, surface| {
            draw(
                drawing_context,
                Painter {
                    canvas: surface.canvas(),
                },
            )
        })
    }

    fn draw_inner<T>(
        &mut self,
        target_id: RendererTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> RenderResult<T> {
        self.check_ready()?;
        let target = self.targets.get_mut(&target_id).ok_or_else(|| {
            RenderError::Backend("D3D12 render target is unavailable".to_string())
        })?;
        let surface = target.current_surface()?;
        let canvas = surface.canvas();
        canvas.restore_to_count(1);
        canvas.reset_matrix();
        canvas.clear(Color::TRANSPARENT);
        canvas.save();
        let output = draw(
            &mut DrawingContext {
                direct_context: &mut self.device.context,
            },
            surface,
        );
        surface.canvas().restore_to_count(1);
        self.device.context.flush_surface_with_access(
            surface,
            skia_safe::surfaces::BackendSurfaceAccess::Present,
            &gpu::FlushInfo::default(),
        );
        self.device.submit(gpu::SyncCpu::No)?;
        let present_result = target.present();
        self.device.check_health()?;
        present_result?;
        Ok(output)
    }

    pub fn resize(
        &mut self,
        target_id: RendererTargetId,
        width: u32,
        height: u32,
    ) -> RenderResult<()> {
        let result = self.resize_inner(target_id, width, height);
        self.record_result(result)
    }

    fn resize_inner(
        &mut self,
        target_id: RendererTargetId,
        width: u32,
        height: u32,
    ) -> RenderResult<()> {
        self.check_ready()?;
        let target = self.targets.get_mut(&target_id).ok_or_else(|| {
            RenderError::Backend("D3D12 render target is unavailable".to_string())
        })?;
        self.device.resize_target(target, width, height)
    }

    pub fn take_failure(&mut self) -> Option<String> {
        self.failure.take()
    }

    pub fn remove_target(&mut self, target_id: RendererTargetId) {
        if !self.targets.contains_key(&target_id) {
            return;
        }
        let result = self.device.synchronize();
        if let Err(error) = self.record_result(result) {
            log::warn!("D3D12 target cleanup failed: {error}");
            self.device.context.abandon();
        }
        self.targets.remove(&target_id);
        self.device
            .context
            .purge_unlocked_resources(gpu::PurgeResourceOptions::AllResources);
    }

    fn check_ready(&mut self) -> RenderResult<()> {
        if let Some(error) = &self.failure {
            return Err(RenderError::Backend(error.clone()));
        }
        self.device.check_health()
    }

    fn record_result<T>(&mut self, result: RenderResult<T>) -> RenderResult<T> {
        if let Err(error) = &result {
            let message = error.to_string();
            self.failure.get_or_insert(message);
        }
        result
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if self.device.synchronize().is_err() {
            self.device.context.abandon();
        }
        self.targets.clear();
    }
}
