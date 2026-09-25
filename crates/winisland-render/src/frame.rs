use std::collections::HashMap;

use skia_safe::{Color, Image, ImageInfo, Paint, Rect, Surface, gpu, image_filters};

use crate::backend::{D3DDevice, RenderTarget, check_surface_supported};
use crate::error::{RenderError, RenderResult};
use crate::surface::NativeSurface;

const MAIN_TARGET: RendererTargetId = RendererTargetId(0);

/// 渲染器创建参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RendererOptions {
    /// 主目标的初始宽度（物理像素）。
    pub width: u32,
    /// 主目标的初始高度（物理像素）。
    pub height: u32,
}

impl RendererOptions {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// 帧回调可用的即时上下文：GPU 上传与离屏 surface。
///
/// 生命周期：只在 `Renderer::draw` 的回调期间有效，不可保存。
/// 失败语义：所有方法返回 `Option`，`None` 表示后端拒绝该操作，调用方需走降级路径。
/// 线程要求：仅渲染线程可构造与使用。
pub struct DrawingContext<'a> {
    direct_context: &'a mut gpu::DirectContext,
}

impl DrawingContext<'_> {
    /// GPU 上传；`mipmapped` 决定是否为纹理生成 mipmap（实测 4 处调用点全部传 `Yes`）。
    pub fn prepare_image(&mut self, image: Image, mipmapped: gpu::Mipmapped) -> Option<Image> {
        image.new_texture_image(self.direct_context, mipmapped)
    }

    /// 申请一个 GPU 离屏渲染目标。
    pub fn render_surface(&mut self, info: &ImageInfo) -> Option<Surface> {
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

    /// 把离屏 surface 的内容提交到后端队列。
    pub fn finish_surface(&mut self, surface: &mut Surface) {
        self.direct_context.flush_surface(surface);
    }
}

/// 渲染目标句柄。`Copy`、newtype，不可由裸整数构造。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RendererTargetId(u64);

/// 设备 + 渲染目标表 + 帧生命周期。
///
/// 线程要求：`Renderer` 只在渲染线程使用。
/// 失败语义：任何操作失败都会被 `record_result` 记住，此后所有请求都会返回首个错误
/// （"失败即整体不可用"），直到重建 `Renderer`。
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

    pub fn draw<T>(
        &mut self,
        target_id: RendererTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> RenderResult<T> {
        let result = self.draw_inner(target_id, draw);
        self.record_result(result)
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

    /// 取出并清除首个失败原因（供上层记录日志并重建渲染器）。
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
