use std::sync::Arc;

use skia_safe::{Image, ImageInfo, Surface, gpu};
use winit::window::Window;

use super::backdrop::HostBackdrop;
use super::software::{SoftwareRenderer, SoftwareTargetId};
use super::vulkan::{VulkanRenderer, VulkanTargetId};

pub(crate) fn signal_dwm_composition_changed() {
    super::vulkan::signal_dwm_composition_changed();
}

pub(crate) fn take_dwm_composition_changed() -> bool {
    super::vulkan::take_dwm_composition_changed()
}

pub(crate) struct DrawingContext<'a> {
    direct_context: Option<&'a mut gpu::DirectContext>,
}

impl<'a> DrawingContext<'a> {
    pub(super) fn vulkan(direct_context: &'a mut gpu::DirectContext) -> Self {
        Self {
            direct_context: Some(direct_context),
        }
    }

    pub(super) fn software() -> Self {
        Self {
            direct_context: None,
        }
    }

    pub(crate) fn is_hardware(&self) -> bool {
        self.direct_context.is_some()
    }

    pub(crate) fn prepare_image(
        &mut self,
        image: Image,
        mipmapped: gpu::Mipmapped,
    ) -> Option<Image> {
        match self.direct_context.as_deref_mut() {
            Some(context) => image.new_texture_image(context, mipmapped),
            None => Some(image),
        }
    }

    pub(crate) fn render_surface(&mut self, info: &ImageInfo) -> Option<Surface> {
        match self.direct_context.as_deref_mut() {
            Some(context) => gpu::surfaces::render_target(
                context,
                gpu::Budgeted::Yes,
                info,
                None,
                Some(gpu::SurfaceOrigin::TopLeft),
                None,
                Some(false),
                Some(false),
            ),
            None => skia_safe::surfaces::raster(info, None, None),
        }
    }

    pub(crate) fn finish_surface(&mut self, surface: &mut Surface) {
        if let Some(context) = self.direct_context.as_deref_mut() {
            context.flush_and_submit_surface(surface, None);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RendererTargetId {
    Vulkan(VulkanTargetId),
    Software(SoftwareTargetId),
}

enum RendererBackend {
    Vulkan(VulkanRenderer),
    Software(SoftwareRenderer),
}

pub(crate) struct Renderer {
    backend: RendererBackend,
    host_backdrop: Option<HostBackdrop>,
}

impl Renderer {
    pub(crate) fn new(
        window: &Arc<Window>,
        backdrop_window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        crate::utils::win32::disable_software_transparency(window);
        let backend = select_backend(window, width, height, false)?;
        Ok(Self {
            backend,
            host_backdrop: create_host_backdrop(window, backdrop_window),
        })
    }

    pub(crate) fn try_new(
        window: &Arc<Window>,
        backdrop_window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        crate::utils::win32::disable_software_transparency(window);
        let backend = select_backend(window, width, height, true)?;
        Ok(Self {
            backend,
            host_backdrop: create_host_backdrop(window, backdrop_window),
        })
    }

    pub(crate) fn create_target(
        &mut self,
        window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<RendererTargetId, String> {
        match &mut self.backend {
            RendererBackend::Vulkan(renderer) => {
                crate::utils::win32::disable_software_transparency(window);
                renderer
                    .create_target(window, width, height)
                    .map(RendererTargetId::Vulkan)
            }
            RendererBackend::Software(renderer) => renderer
                .create_target(window, width, height)
                .map(RendererTargetId::Software),
        }
    }

    pub(crate) fn main_target(&self) -> RendererTargetId {
        match &self.backend {
            RendererBackend::Vulkan(_) => {
                RendererTargetId::Vulkan(super::vulkan::MAIN_VULKAN_TARGET)
            }
            RendererBackend::Software(_) => {
                RendererTargetId::Software(super::software::MAIN_SOFTWARE_TARGET)
            }
        }
    }

    pub(crate) fn draw<T>(
        &mut self,
        target_id: RendererTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> Result<T, String> {
        match (&mut self.backend, target_id) {
            (RendererBackend::Vulkan(renderer), RendererTargetId::Vulkan(target)) => {
                renderer.draw(target, draw)
            }
            (RendererBackend::Software(renderer), RendererTargetId::Software(target)) => {
                renderer.draw(target, draw)
            }
            _ => Err("Renderer target belongs to a different backend".to_string()),
        }
    }

    pub(crate) fn resize(
        &mut self,
        target_id: RendererTargetId,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        match (&mut self.backend, target_id) {
            (RendererBackend::Vulkan(renderer), RendererTargetId::Vulkan(target)) => {
                renderer.resize(target, width, height)
            }
            (RendererBackend::Software(renderer), RendererTargetId::Software(target)) => {
                renderer.resize(target, width, height)
            }
            _ => Err("Renderer target belongs to a different backend".to_string()),
        }
    }

    pub(crate) fn take_failure(&mut self) -> Option<String> {
        match &mut self.backend {
            RendererBackend::Vulkan(renderer) => renderer.take_failure(),
            RendererBackend::Software(renderer) => renderer.take_failure(),
        }
    }

    pub(crate) fn update_host_backdrop(
        &mut self,
        target_id: RendererTargetId,
        params: HostBackdropParams,
    ) -> bool {
        if !self.is_main_target(target_id) {
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

    pub(crate) fn ensure_window_style(&self, window: &Window) {
        if matches!(self.backend, RendererBackend::Software(_))
            && let Err(error) = crate::utils::win32::enable_software_transparency(window)
        {
            log::warn!("Software transparency refresh failed: {error}");
        }
    }

    pub(crate) fn abandon(&mut self) {
        if let RendererBackend::Vulkan(renderer) = &mut self.backend {
            renderer.abandon();
        }
    }

    pub(crate) fn remove_target(&mut self, target_id: RendererTargetId) {
        match (&mut self.backend, target_id) {
            (RendererBackend::Vulkan(renderer), RendererTargetId::Vulkan(target)) => {
                renderer.remove_target(target);
            }
            (RendererBackend::Software(renderer), RendererTargetId::Software(target)) => {
                renderer.remove_target(target);
            }
            _ => {}
        }
    }

    fn is_main_target(&self, target_id: RendererTargetId) -> bool {
        matches!(
            target_id,
            RendererTargetId::Vulkan(super::vulkan::MAIN_VULKAN_TARGET)
                | RendererTargetId::Software(super::software::MAIN_SOFTWARE_TARGET)
        )
    }
}

fn create_host_backdrop(window: &Window, backdrop_window: &Arc<Window>) -> Option<HostBackdrop> {
    match HostBackdrop::new(window, backdrop_window) {
        Ok(backdrop) => Some(backdrop),
        Err(error) => {
            log::warn!("Host backdrop is unavailable: {error}");
            None
        }
    }
}

fn select_backend(
    window: &Arc<Window>,
    width: u32,
    height: u32,
    recovery: bool,
) -> Result<RendererBackend, String> {
    if std::env::var("WINISLAND_RENDERER").is_ok_and(|value| value.eq_ignore_ascii_case("software"))
    {
        log::info!("Software renderer forced by WINISLAND_RENDERER");
        return SoftwareRenderer::new(window, width, height).map(RendererBackend::Software);
    }

    let vulkan = if recovery {
        VulkanRenderer::try_new(window, width, height)
    } else {
        VulkanRenderer::new(window, width, height)
    };
    match vulkan {
        Ok(renderer) => Ok(RendererBackend::Vulkan(renderer)),
        Err(vulkan_error) => {
            log::warn!("Vulkan renderer unavailable; using software fallback: {vulkan_error}");
            SoftwareRenderer::new(window, width, height)
                .map(RendererBackend::Software)
                .map_err(|error| {
                    format!(
                        "Vulkan initialization failed: {vulkan_error}; software fallback failed: {error}"
                    )
                })
        }
    }
}

pub(crate) use super::backdrop::HostBackdropParams;
