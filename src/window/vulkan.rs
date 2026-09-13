use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ash::{vk, vk::Handle};
use skia_safe::{
    ColorType, Surface,
    gpu::{self, BackendRenderTarget, ContextOptions, DirectContext, SurfaceOrigin, surfaces},
};
use winit::window::Window;

use super::renderer::DrawingContext;

mod device;
use device::{VulkanDevice, VulkanInstance};

const GPU_RESOURCE_CACHE_LIMIT: usize = 12 * 1024 * 1024;
const GPU_GLYPH_CACHE_LIMIT: usize = 2 * 1024 * 1024;
const INITIALIZATION_ATTEMPTS: usize = 3;
const INITIALIZATION_RETRY_DELAY: Duration = Duration::from_millis(500);
const RESOURCE_CLEANUP_INTERVAL: Duration = Duration::from_secs(5);
const RESOURCE_MAX_IDLE_AGE: Duration = Duration::from_secs(10);

static DWM_COMPOSITION_CHANGED: AtomicBool = AtomicBool::new(false);

pub(crate) fn signal_dwm_composition_changed() {
    DWM_COMPOSITION_CHANGED.store(true, Ordering::Release);
}

pub(crate) fn take_dwm_composition_changed() -> bool {
    DWM_COMPOSITION_CHANGED.swap(false, Ordering::AcqRel)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct VulkanTargetId(u64);

pub(crate) const MAIN_VULKAN_TARGET: VulkanTargetId = VulkanTargetId(0);

struct SwapchainImage {
    surface: Surface,
    _backend: BackendRenderTarget,
}

struct SwapchainResources {
    handle: vk::SwapchainKHR,
    images: Vec<SwapchainImage>,
    acquire_fence: vk::Fence,
}

struct VulkanTarget {
    surface: vk::SurfaceKHR,
    swapchain: SwapchainResources,
}

pub(crate) struct VulkanRenderer {
    targets: HashMap<VulkanTargetId, VulkanTarget>,
    direct_context: DirectContext,
    device: Rc<VulkanDevice>,
    next_target_id: u64,
    last_resource_cleanup: Instant,
    failure: Option<String>,
}

impl VulkanRenderer {
    pub(crate) fn new(window: &Window, width: u32, height: u32) -> Result<Self, String> {
        let mut last_error = None;
        for attempt in 0..INITIALIZATION_ATTEMPTS {
            match Self::new_once(window, width, height) {
                Ok(renderer) => return Ok(renderer),
                Err(error) => {
                    if attempt + 1 < INITIALIZATION_ATTEMPTS {
                        log::warn!(
                            "Vulkan renderer initialization failed; retrying attempt {}/{}: {}",
                            attempt + 2,
                            INITIALIZATION_ATTEMPTS,
                            error
                        );
                        std::thread::sleep(INITIALIZATION_RETRY_DELAY);
                    }
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| "Vulkan renderer initialization failed".to_string()))
    }

    pub(crate) fn try_new(window: &Window, width: u32, height: u32) -> Result<Self, String> {
        Self::new_once(window, width, height)
    }

    fn new_once(window: &Window, width: u32, height: u32) -> Result<Self, String> {
        let instance = VulkanInstance::new()?;
        let surface = instance.create_surface(window)?;
        let device = match instance.device_for_surface(surface) {
            Ok(device) => device,
            Err(error) => {
                instance.destroy_surface(surface);
                return Err(error);
            }
        };
        let mut context_options = ContextOptions::new();
        context_options.glyph_cache_texture_maximum_bytes = GPU_GLYPH_CACHE_LIMIT;
        context_options.allow_multiple_glyph_cache_textures =
            gpu::ganesh::context_options::Enable::No;
        context_options.reduce_ops_task_splitting = gpu::ganesh::context_options::Enable::No;
        let mut direct_context = match device.context(&context_options) {
            Ok(context) => context,
            Err(error) => {
                instance.destroy_surface(surface);
                return Err(error);
            }
        };
        direct_context.set_resource_cache_limit(GPU_RESOURCE_CACHE_LIMIT);
        let mut renderer = Self {
            targets: HashMap::new(),
            direct_context,
            device,
            next_target_id: 0,
            last_resource_cleanup: Instant::now(),
            failure: None,
        };
        let target_id = match renderer.insert_target(surface, width, height) {
            Ok(target_id) => target_id,
            Err(error) => {
                renderer.device.instance.destroy_surface(surface);
                return Err(error);
            }
        };
        debug_assert_eq!(target_id, MAIN_VULKAN_TARGET);
        Ok(renderer)
    }

    pub(crate) fn create_target(
        &mut self,
        window: &Window,
        width: u32,
        height: u32,
    ) -> Result<VulkanTargetId, String> {
        let surface = self.device.instance.create_surface(window)?;
        if let Err(error) = self.device.supports_surface(surface) {
            self.device.instance.destroy_surface(surface);
            return Err(error);
        }
        match self.insert_target(surface, width, height) {
            Ok(target_id) => Ok(target_id),
            Err(error) => {
                self.device.instance.destroy_surface(surface);
                Err(error)
            }
        }
    }

    fn insert_target(
        &mut self,
        surface: vk::SurfaceKHR,
        width: u32,
        height: u32,
    ) -> Result<VulkanTargetId, String> {
        let target_id = VulkanTargetId(self.next_target_id);
        let swapchain = create_swapchain(
            &self.device,
            &mut self.direct_context,
            surface,
            width,
            height,
            vk::SwapchainKHR::null(),
        )?;
        self.next_target_id += 1;
        self.targets
            .insert(target_id, VulkanTarget { surface, swapchain });
        Ok(target_id)
    }

    pub(crate) fn draw<T>(
        &mut self,
        target_id: VulkanTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> Result<T, String> {
        match self.draw_inner(target_id, draw) {
            Ok((output, suboptimal)) => {
                if suboptimal {
                    self.failure = Some("Vulkan swapchain became suboptimal".to_string());
                }
                Ok(output)
            }
            Err(error) => {
                self.failure = Some(error.clone());
                Err(error)
            }
        }
    }

    fn draw_inner<T>(
        &mut self,
        target_id: VulkanTargetId,
        draw: impl FnOnce(&mut DrawingContext<'_>, &mut Surface) -> T,
    ) -> Result<(T, bool), String> {
        self.check_context("before drawing")?;
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or_else(|| "Vulkan render target is unavailable".to_string())?;
        // SAFETY: The fence and swapchain belong to this live device and are only used here.
        let (image_index, acquire_suboptimal) = unsafe {
            self.device
                .device
                .reset_fences(&[target.swapchain.acquire_fence])
                .map_err(|error| format!("Vulkan acquire fence reset failed: {error}"))?;
            let result = self.device.swapchain.acquire_next_image(
                target.swapchain.handle,
                u64::MAX,
                vk::Semaphore::null(),
                target.swapchain.acquire_fence,
            );
            let result = match result {
                Ok(result) => result,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    return Err("Vulkan swapchain is out of date".to_string());
                }
                Err(error) => return Err(format!("Vulkan image acquisition failed: {error}")),
            };
            self.device
                .device
                .wait_for_fences(&[target.swapchain.acquire_fence], true, u64::MAX)
                .map_err(|error| format!("Vulkan image acquisition wait failed: {error}"))?;
            result
        };
        let image = target
            .swapchain
            .images
            .get_mut(image_index as usize)
            .ok_or_else(|| "Vulkan returned an invalid swapchain image index".to_string())?;
        let mut context = DrawingContext::vulkan(&mut self.direct_context);
        let output = draw(&mut context, &mut image.surface);
        let present_state = gpu::vk::mutable_texture_states::new_vulkan(
            gpu::vk::ImageLayout::PRESENT_SRC_KHR,
            self.device.queue_family,
        );
        self.direct_context.flush_surface_with_texture_state(
            &mut image.surface,
            &Default::default(),
            Some(&present_state),
        );
        if !self.direct_context.submit(gpu::SubmitInfo {
            sync: gpu::SyncCpu::Yes,
            ..Default::default()
        }) {
            return Err("Vulkan frame submission failed".to_string());
        }
        if self.direct_context.is_device_lost() {
            return Err("Skia Vulkan device lost after submitting".to_string());
        }
        if self.direct_context.oomed() {
            return Err("Skia Vulkan allocation failed after submitting".to_string());
        }

        let swapchains = [target.swapchain.handle];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        // SAFETY: Skia completed rendering and transitioned this acquired image for presentation.
        let present_suboptimal = unsafe {
            match self
                .device
                .swapchain
                .queue_present(self.device.queue, &present_info)
            {
                Ok(suboptimal) => suboptimal,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    return Err("Vulkan swapchain is out of date".to_string());
                }
                Err(error) => return Err(format!("Vulkan presentation failed: {error}")),
            }
        };
        self.cleanup_unused_resources();
        Ok((output, acquire_suboptimal || present_suboptimal))
    }

    pub(crate) fn resize(
        &mut self,
        target_id: VulkanTargetId,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let result = self.resize_inner(target_id, width, height);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    fn resize_inner(
        &mut self,
        target_id: VulkanTargetId,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.check_context("before resizing")?;
        self.direct_context.flush_submit_and_sync_cpu();
        self.device.wait_idle()?;
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or_else(|| "Vulkan render target is unavailable".to_string())?;
        let replacement = create_swapchain(
            &self.device,
            &mut self.direct_context,
            target.surface,
            width,
            height,
            target.swapchain.handle,
        )?;
        let old = std::mem::replace(&mut target.swapchain, replacement);
        destroy_swapchain(&self.device, old);
        self.direct_context
            .purge_unlocked_resources(gpu::PurgeResourceOptions::AllResources);
        self.check_context("after resizing")
    }

    pub(crate) fn take_failure(&mut self) -> Option<String> {
        self.failure.take()
    }

    pub(crate) fn abandon(&mut self) {
        if self.device.wait_idle().is_ok() && !self.direct_context.is_device_lost() {
            self.direct_context.release_resources_and_abandon();
        } else {
            self.direct_context.abandon();
        }
    }

    pub(crate) fn remove_target(&mut self, target_id: VulkanTargetId) {
        let Some(target) = self.targets.remove(&target_id) else {
            return;
        };
        self.direct_context.flush_submit_and_sync_cpu();
        let _ = self.device.wait_idle();
        destroy_swapchain(&self.device, target.swapchain);
        self.device.instance.destroy_surface(target.surface);
        self.direct_context
            .purge_unlocked_resources(gpu::PurgeResourceOptions::AllResources);
        self.last_resource_cleanup = Instant::now();
    }

    fn cleanup_unused_resources(&mut self) {
        if self.last_resource_cleanup.elapsed() < RESOURCE_CLEANUP_INTERVAL {
            return;
        }
        self.direct_context.perform_deferred_cleanup(
            RESOURCE_MAX_IDLE_AGE,
            Some(gpu::PurgeResourceOptions::AllResources),
        );
        self.last_resource_cleanup = Instant::now();
    }

    fn check_context(&mut self, stage: &str) -> Result<(), String> {
        if self.direct_context.is_device_lost() {
            return Err(format!("Skia Vulkan device lost {stage}"));
        }
        if self.direct_context.oomed() {
            return Err(format!("Skia Vulkan allocation failed {stage}"));
        }
        Ok(())
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        let target_ids: Vec<_> = self.targets.keys().copied().collect();
        for target_id in target_ids {
            self.remove_target(target_id);
        }
        self.direct_context.release_resources_and_abandon();
    }
}

fn create_swapchain(
    device: &VulkanDevice,
    direct_context: &mut DirectContext,
    surface: vk::SurfaceKHR,
    width: u32,
    height: u32,
    old_swapchain: vk::SwapchainKHR,
) -> Result<SwapchainResources, String> {
    // SAFETY: Every query uses the selected physical device and a live surface.
    let (capabilities, formats) = unsafe {
        let capabilities = device
            .instance
            .surface
            .get_physical_device_surface_capabilities(device.physical_device, surface)
            .map_err(|error| format!("Vulkan surface capabilities query failed: {error}"))?;
        let formats = device
            .instance
            .surface
            .get_physical_device_surface_formats(device.physical_device, surface)
            .map_err(|error| format!("Vulkan surface format query failed: {error}"))?;
        (capabilities, formats)
    };
    if !capabilities
        .supported_usage_flags
        .contains(vk::ImageUsageFlags::COLOR_ATTACHMENT)
    {
        return Err("Vulkan surface cannot be used as a color attachment".to_string());
    }
    let composite_alpha = if capabilities
        .supported_composite_alpha
        .contains(vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED)
    {
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED
    } else if capabilities
        .supported_composite_alpha
        .contains(vk::CompositeAlphaFlagsKHR::INHERIT)
    {
        vk::CompositeAlphaFlagsKHR::INHERIT
    } else {
        return Err("Vulkan surface does not support transparent composition".to_string());
    };
    let surface_format = choose_surface_format(&formats)?;
    let extent = if capabilities.current_extent.width != u32::MAX {
        capabilities.current_extent
    } else {
        vk::Extent2D {
            width: width.max(1).clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            ),
            height: height.max(1).clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            ),
        }
    };
    let mut image_count = capabilities.min_image_count.saturating_add(1).max(2);
    if capabilities.max_image_count > 0 {
        image_count = image_count.min(capabilities.max_image_count);
    }
    let image_usage = capabilities.supported_usage_flags
        & (vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::TRANSFER_DST);
    let info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(image_usage)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(capabilities.current_transform)
        .composite_alpha(composite_alpha)
        .present_mode(vk::PresentModeKHR::FIFO)
        .clipped(true)
        .old_swapchain(old_swapchain);
    // SAFETY: The descriptor references the live surface and selected device queue family.
    let handle = unsafe { device.swapchain.create_swapchain(&info, None) }
        .map_err(|error| format!("Vulkan swapchain creation failed: {error}"))?;
    log::info!(
        "Vulkan swapchain: format={:?}, extent={}x{}, images={}, alpha={:?}, usage={:?}",
        surface_format.format,
        extent.width,
        extent.height,
        image_count,
        composite_alpha,
        image_usage
    );
    let result = create_swapchain_images(
        device,
        direct_context,
        handle,
        extent,
        surface_format,
        image_usage,
    );
    let images = match result {
        Ok(images) => images,
        Err(error) => {
            // SAFETY: No presentation or rendering uses this newly created swapchain yet.
            unsafe { device.swapchain.destroy_swapchain(handle, None) };
            return Err(error);
        }
    };
    let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
    // SAFETY: The descriptor is fully initialized and the device is live.
    let acquire_fence = match unsafe { device.device.create_fence(&fence_info, None) } {
        Ok(fence) => fence,
        Err(error) => {
            drop(images);
            // SAFETY: No presentation or rendering uses this newly created swapchain yet.
            unsafe { device.swapchain.destroy_swapchain(handle, None) };
            return Err(format!("Vulkan acquire fence creation failed: {error}"));
        }
    };
    Ok(SwapchainResources {
        handle,
        images,
        acquire_fence,
    })
}

fn create_swapchain_images(
    device: &VulkanDevice,
    direct_context: &mut DirectContext,
    swapchain: vk::SwapchainKHR,
    extent: vk::Extent2D,
    surface_format: vk::SurfaceFormatKHR,
    image_usage: vk::ImageUsageFlags,
) -> Result<Vec<SwapchainImage>, String> {
    // SAFETY: The swapchain was created successfully and remains live for the query.
    let images = unsafe { device.swapchain.get_swapchain_images(swapchain) }
        .map_err(|error| format!("Vulkan swapchain image query failed: {error}"))?;
    let (skia_format, color_type) = match surface_format.format {
        vk::Format::B8G8R8A8_UNORM => (gpu::vk::Format::B8G8R8A8_UNORM, ColorType::BGRA8888),
        vk::Format::R8G8B8A8_UNORM => (gpu::vk::Format::R8G8B8A8_UNORM, ColorType::RGBA8888),
        _ => return Err("Vulkan surface has no Skia-compatible color format".to_string()),
    };
    let width =
        i32::try_from(extent.width).map_err(|_| "Render width exceeds Skia limits".to_string())?;
    let height = i32::try_from(extent.height)
        .map_err(|_| "Render height exceeds Skia limits".to_string())?;
    images
        .into_iter()
        .map(|image| {
            // SAFETY: The swapchain owns this image for longer than the wrapped Skia surface.
            let mut image_info = unsafe {
                gpu::vk::ImageInfo::new(
                    image.as_raw() as _,
                    Default::default(),
                    gpu::vk::ImageTiling::OPTIMAL,
                    gpu::vk::ImageLayout::UNDEFINED,
                    skia_format,
                    1,
                    None,
                    None,
                    None,
                    None,
                )
            };
            image_info.image_usage_flags = image_usage.as_raw();
            let backend = gpu::backend_render_targets::make_vk((width, height), &image_info);
            let surface = surfaces::wrap_backend_render_target(
                direct_context,
                &backend,
                SurfaceOrigin::TopLeft,
                color_type,
                None,
                None,
            )
            .ok_or_else(|| {
                format!(
                    "Skia failed to wrap a Vulkan swapchain image (format={:?}, color_type_supported={})",
                    surface_format.format,
                    direct_context.color_type_supported_as_surface(color_type)
                )
            })?;
            Ok(SwapchainImage {
                surface,
                _backend: backend,
            })
        })
        .collect()
}

fn choose_surface_format(formats: &[vk::SurfaceFormatKHR]) -> Result<vk::SurfaceFormatKHR, String> {
    if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
        return Ok(vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_UNORM,
            color_space: formats[0].color_space,
        });
    }
    [vk::Format::B8G8R8A8_UNORM, vk::Format::R8G8B8A8_UNORM]
        .into_iter()
        .find_map(|format| {
            formats
                .iter()
                .copied()
                .find(|candidate| candidate.format == format)
        })
        .ok_or_else(|| "Vulkan surface has no Skia-compatible color format".to_string())
}

fn destroy_swapchain(device: &VulkanDevice, resources: SwapchainResources) {
    drop(resources.images);
    // SAFETY: The device is idle before every call, and these handles belong to it.
    unsafe {
        device.device.destroy_fence(resources.acquire_fence, None);
        device.swapchain.destroy_swapchain(resources.handle, None);
    }
}
