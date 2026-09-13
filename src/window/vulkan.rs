use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ash::{vk, vk::Handle};
use skia_safe::{
    ColorType, Surface,
    gpu::{self, BackendRenderTarget, ContextOptions, DirectContext, SurfaceOrigin, surfaces},
};
use windows::{
    System::DispatcherQueueController,
    UI::Composition::Desktop::DesktopWindowTarget,
    UI::Composition::{
        CompositionGeometricClip, CompositionRoundedRectangleGeometry, Compositor, ContainerVisual,
        SpriteVisual,
    },
    Win32::{
        Foundation::HWND,
        System::WinRT::{
            Composition::ICompositorDesktopInterop, CreateDispatcherQueueController,
            DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
        },
        UI::WindowsAndMessaging::{
            SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
            SetWindowPos,
        },
    },
    core::Interface,
};
use windows_numerics::{Vector2, Vector3};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

mod device;
use device::{VulkanDevice, VulkanInstance};

const GPU_RESOURCE_CACHE_LIMIT: usize = 12 * 1024 * 1024;
const GPU_GLYPH_CACHE_LIMIT: usize = 2 * 1024 * 1024;
const INITIALIZATION_ATTEMPTS: usize = 3;
const INITIALIZATION_RETRY_DELAY: Duration = Duration::from_millis(500);
const RESOURCE_CLEANUP_INTERVAL: Duration = Duration::from_secs(5);
const RESOURCE_MAX_IDLE_AGE: Duration = Duration::from_secs(10);
const HOST_BACKDROP_INSET: f32 = 1.0;

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

struct BackdropCompositionContext {
    _dispatcher_queue: DispatcherQueueController,
    compositor: Compositor,
}

struct HostBackdropTarget {
    _window: Arc<Window>,
    main_hwnd: HWND,
    backdrop_hwnd: HWND,
    target: DesktopWindowTarget,
    _root: ContainerVisual,
    visual: SpriteVisual,
    _clip: CompositionGeometricClip,
    geometry: CompositionRoundedRectangleGeometry,
}

pub(crate) struct HostBackdropParams {
    pub(crate) enabled: bool,
    pub(crate) screen_x: f32,
    pub(crate) screen_y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) radius: f32,
}

thread_local! {
    static BACKDROP_COMPOSITION: OnceCell<Option<BackdropCompositionContext>> = const { OnceCell::new() };
}

pub(crate) struct VulkanRenderer {
    targets: HashMap<VulkanTargetId, VulkanTarget>,
    direct_context: DirectContext,
    device: Rc<VulkanDevice>,
    host_backdrop: Option<HostBackdropTarget>,
    next_target_id: u64,
    last_resource_cleanup: Instant,
    failure: Option<String>,
}

impl VulkanRenderer {
    pub(crate) fn new(
        window: &Window,
        backdrop_window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let mut last_error = None;
        for attempt in 0..INITIALIZATION_ATTEMPTS {
            match Self::new_once(window, backdrop_window, width, height) {
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

    pub(crate) fn try_new(
        window: &Window,
        backdrop_window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        Self::new_once(window, backdrop_window, width, height)
    }

    fn new_once(
        window: &Window,
        backdrop_window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
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
        let host_backdrop = backdrop_compositor().and_then(|compositor| {
            match create_host_backdrop_target(&compositor, window, backdrop_window) {
                Ok(target) => Some(target),
                Err(error) => {
                    log::warn!("Host backdrop is unavailable: {error}");
                    None
                }
            }
        });
        let mut renderer = Self {
            targets: HashMap::new(),
            direct_context,
            device,
            host_backdrop,
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
        draw: impl FnOnce(&mut DirectContext, &mut Surface) -> T,
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
        draw: impl FnOnce(&mut DirectContext, &mut Surface) -> T,
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
        let output = draw(&mut self.direct_context, &mut image.surface);
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

    pub(crate) fn update_host_backdrop(
        &mut self,
        target_id: VulkanTargetId,
        params: HostBackdropParams,
    ) -> bool {
        if target_id != MAIN_VULKAN_TARGET {
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

impl HostBackdropTarget {
    fn update(&self, params: HostBackdropParams) -> windows::core::Result<()> {
        let enabled = params.enabled && params.width > 0.0 && params.height > 0.0;
        if !enabled {
            self.hide();
            return Ok(());
        }

        let window_width = params.width.ceil().max(1.0) as i32;
        let window_height = params.height.ceil().max(1.0) as i32;
        let width = (params.width - HOST_BACKDROP_INSET * 2.0).max(0.0);
        let height = (params.height - HOST_BACKDROP_INSET * 2.0).max(0.0);
        let radius = (params.radius - HOST_BACKDROP_INSET).max(0.0);
        self.visual.SetOffset(Vector3 {
            X: HOST_BACKDROP_INSET,
            Y: HOST_BACKDROP_INSET,
            Z: 0.0,
        })?;
        self.visual.SetSize(Vector2 {
            X: width,
            Y: height,
        })?;
        self.geometry.SetSize(Vector2 {
            X: width,
            Y: height,
        })?;
        self.geometry.SetCornerRadius(Vector2 {
            X: radius,
            Y: radius,
        })?;
        self.visual.SetIsVisible(true)?;
        // SAFETY: Both HWND values belong to live windows on this thread. Placing the backdrop
        // immediately behind the owned Vulkan window preserves their z-order without activation.
        unsafe {
            SetWindowPos(
                self.backdrop_hwnd,
                Some(self.main_hwnd),
                params.screen_x.floor() as i32,
                params.screen_y.floor() as i32,
                window_width,
                window_height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
        }
        Ok(())
    }

    fn hide(&self) {
        let _ = self.visual.SetIsVisible(false);
        // SAFETY: The backdrop HWND remains owned by `_window`; this only hides it.
        unsafe {
            let _ = SetWindowPos(
                self.backdrop_hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_HIDEWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }
}

impl Drop for HostBackdropTarget {
    fn drop(&mut self) {
        self.hide();
        let _ = self.target.Close();
    }
}

fn backdrop_compositor() -> Option<Compositor> {
    BACKDROP_COMPOSITION.with(|cell| {
        cell.get_or_init(|| match create_backdrop_composition_context() {
            Ok(context) => Some(context),
            Err(error) => {
                log::warn!("Windows host backdrop initialization failed: {error}");
                None
            }
        })
        .as_ref()
        .map(|context| context.compositor.clone())
    })
}

fn create_backdrop_composition_context() -> Result<BackdropCompositionContext, String> {
    let options = DispatcherQueueOptions {
        dwSize: size_of::<DispatcherQueueOptions>() as u32,
        threadType: DQTYPE_THREAD_CURRENT,
        apartmentType: DQTAT_COM_NONE,
    };
    let dispatcher_queue = unsafe {
        // SAFETY: winit initializes the main thread's STA before renderer creation. The options
        // attach a dispatcher queue to that current thread without changing its COM apartment.
        CreateDispatcherQueueController(options)
    }
    .map_err(|error| format!("CreateDispatcherQueueController failed: {error}"))?;
    let compositor = Compositor::new().map_err(|error| format!("Compositor failed: {error}"))?;
    Ok(BackdropCompositionContext {
        _dispatcher_queue: dispatcher_queue,
        compositor,
    })
}

fn create_host_backdrop_target(
    compositor: &Compositor,
    main_window: &Window,
    backdrop_window: &Arc<Window>,
) -> Result<HostBackdropTarget, String> {
    let main_hwnd = window_hwnd(main_window)?;
    let backdrop_hwnd = window_hwnd(backdrop_window)?;
    if !crate::utils::win32::enable_host_backdrop(backdrop_hwnd) {
        return Err("DWM host backdrop support could not be enabled".to_string());
    }
    let interop: ICompositorDesktopInterop = compositor
        .cast()
        .map_err(|error| format!("ICompositorDesktopInterop is unavailable: {error}"))?;
    let target = unsafe {
        // SAFETY: backdrop_hwnd belongs to the companion window and has no other composition tree.
        interop.CreateDesktopWindowTarget(backdrop_hwnd, false)
    }
    .map_err(|error| format!("CreateDesktopWindowTarget failed: {error}"))?;
    let root = compositor
        .CreateContainerVisual()
        .map_err(|error| format!("CreateContainerVisual failed: {error}"))?;
    let visual = compositor
        .CreateSpriteVisual()
        .map_err(|error| format!("CreateSpriteVisual failed: {error}"))?;
    let geometry = compositor
        .CreateRoundedRectangleGeometry()
        .map_err(|error| format!("CreateRoundedRectangleGeometry failed: {error}"))?;
    let clip = compositor
        .CreateGeometricClipWithGeometry(&geometry)
        .map_err(|error| format!("CreateGeometricClipWithGeometry failed: {error}"))?;
    let brush = compositor
        .CreateHostBackdropBrush()
        .map_err(|error| format!("CreateHostBackdropBrush failed: {error}"))?;
    visual
        .SetBrush(&brush)
        .map_err(|error| format!("Host backdrop brush assignment failed: {error}"))?;
    visual
        .SetClip(&clip)
        .map_err(|error| format!("Host backdrop clip assignment failed: {error}"))?;
    visual
        .SetIsVisible(false)
        .map_err(|error| format!("Host backdrop visibility setup failed: {error}"))?;
    root.Children()
        .and_then(|children| children.InsertAtTop(&visual))
        .map_err(|error| format!("Host backdrop visual insertion failed: {error}"))?;
    target
        .SetRoot(&root)
        .map_err(|error| format!("Host backdrop root assignment failed: {error}"))?;
    Ok(HostBackdropTarget {
        _window: backdrop_window.clone(),
        main_hwnd,
        backdrop_hwnd,
        target,
        _root: root,
        visual,
        _clip: clip,
        geometry,
    })
}

fn window_hwnd(window: &Window) -> Result<HWND, String> {
    let handle = window
        .window_handle()
        .map_err(|error| format!("Window handle unavailable: {error}"))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as _)),
        _ => Err("Vulkan rendering requires a Win32 window".to_string()),
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
