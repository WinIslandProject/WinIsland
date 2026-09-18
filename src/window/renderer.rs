use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use skia_safe::{Color, Image, ImageInfo, Surface, gpu};
use winit::window::Window;

use super::backdrop::HostBackdrop;
use super::d3d::{D3DDevice, RenderTarget};

pub(crate) use super::backdrop::HostBackdropParams;

const MAIN_TARGET: RendererTargetId = RendererTargetId(0);
const RESOURCE_CLEANUP_INTERVAL: Duration = Duration::from_secs(5);
const RESOURCE_MAX_IDLE_AGE: Duration = Duration::from_secs(10);

static DWM_COMPOSITION_CHANGED: AtomicBool = AtomicBool::new(false);

pub(crate) fn signal_dwm_composition_changed() {
    DWM_COMPOSITION_CHANGED.store(true, Ordering::Release);
}

pub(crate) fn take_dwm_composition_changed() -> bool {
    DWM_COMPOSITION_CHANGED.swap(false, Ordering::AcqRel)
}

pub(crate) struct DrawingContext<'a> {
    direct_context: &'a mut gpu::DirectContext,
}

impl DrawingContext<'_> {
    pub(crate) fn prepare_image(
        &mut self,
        image: Image,
        mipmapped: gpu::Mipmapped,
    ) -> Option<Image> {
        image.new_texture_image(self.direct_context, mipmapped)
    }

    pub(crate) fn render_surface(&mut self, info: &ImageInfo) -> Option<Surface> {
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

    pub(crate) fn finish_surface(&mut self, surface: &mut Surface) {
        self.direct_context.flush_surface(surface);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RendererTargetId(u64);

pub(crate) struct Renderer {
    targets: HashMap<RendererTargetId, RenderTarget>,
    host_backdrop: Option<HostBackdrop>,
    device: D3DDevice,
    next_target_id: u64,
    last_resource_cleanup: Instant,
    failure: Option<String>,
}

impl Renderer {
    pub(crate) fn new(
        window: &Arc<Window>,
        backdrop_window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let mut device = D3DDevice::new()?;
        let target = device.create_target(window, width, height)?;
        let host_backdrop = match HostBackdrop::new(window, backdrop_window) {
            Ok(backdrop) => Some(backdrop),
            Err(error) => {
                log::warn!("Host backdrop is unavailable: {error}");
                None
            }
        };
        Ok(Self {
            targets: HashMap::from([(MAIN_TARGET, target)]),
            host_backdrop,
            device,
            next_target_id: 1,
            last_resource_cleanup: Instant::now(),
            failure: None,
        })
    }

    pub(crate) fn create_target(
        &mut self,
        window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<RendererTargetId, String> {
        self.check_ready()?;
        let target = self.device.create_target(window, width, height);
        let target = self.record_result(target)?;
        let id = RendererTargetId(self.next_target_id);
        self.next_target_id += 1;
        self.targets.insert(id, target);
        Ok(id)
    }

    pub(crate) fn main_target(&self) -> RendererTargetId {
        MAIN_TARGET
    }

    pub(crate) fn draw<T>(
        &mut self,
        target_id: RendererTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> Result<T, String> {
        let result = self.draw_inner(target_id, draw);
        self.record_result(result)
    }

    fn draw_inner<T>(
        &mut self,
        target_id: RendererTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> Result<T, String> {
        self.check_ready()?;
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or_else(|| "D3D12 render target is unavailable".to_string())?;
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
        if self.last_resource_cleanup.elapsed() >= RESOURCE_CLEANUP_INTERVAL {
            self.device.context.perform_deferred_cleanup(
                RESOURCE_MAX_IDLE_AGE,
                Some(gpu::PurgeResourceOptions::AllResources),
            );
            self.last_resource_cleanup = Instant::now();
        }
        Ok(output)
    }

    pub(crate) fn resize(
        &mut self,
        target_id: RendererTargetId,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let result = self.resize_inner(target_id, width, height);
        self.record_result(result)
    }

    fn resize_inner(
        &mut self,
        target_id: RendererTargetId,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.check_ready()?;
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or_else(|| "D3D12 render target is unavailable".to_string())?;
        self.device.resize_target(target, width, height)
    }

    pub(crate) fn take_failure(&mut self) -> Option<String> {
        self.failure.take()
    }

    pub(crate) fn update_host_backdrop(
        &mut self,
        target_id: RendererTargetId,
        params: HostBackdropParams,
    ) -> bool {
        if target_id != MAIN_TARGET {
            return false;
        }
        let Some(host_backdrop) = self.host_backdrop.as_ref() else {
            return false;
        };
        if let Err(error) = host_backdrop.update(params) {
            log::warn!("Host backdrop update failed: {error}");
            self.host_backdrop = None;
            return false;
        }
        true
    }

    pub(crate) fn hide_host_backdrop(&self) {
        if let Some(host_backdrop) = self.host_backdrop.as_ref() {
            host_backdrop.hide();
        }
    }

    pub(crate) fn remove_target(&mut self, target_id: RendererTargetId) {
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

    fn check_ready(&mut self) -> Result<(), String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        self.device.check_health()
    }

    fn record_result<T>(&mut self, result: Result<T, String>) -> Result<T, String> {
        if let Err(error) = &result {
            self.failure.get_or_insert_with(|| error.clone());
        }
        result
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.host_backdrop.take();
        if self.device.synchronize().is_err() {
            self.device.context.abandon();
        }
        self.targets.clear();
    }
}
