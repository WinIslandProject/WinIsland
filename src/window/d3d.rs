use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use skia_safe::{
    ColorType, Surface,
    gpu::{
        self, BackendRenderTarget, DirectContext, Protected, SurfaceOrigin,
        d3d::{BackendContext, TextureResourceInfo},
        surfaces,
    },
};
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D12::{D3D12_RESOURCE_STATE_COMMON, D3D12CreateDevice, ID3D12Device},
            DirectComposition::{
                DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget,
                IDCompositionVisual,
            },
            Dxgi::{
                Common::{
                    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
                    DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                },
                CreateDXGIFactory2, DXGI_ADAPTER_FLAG, DXGI_ADAPTER_FLAG_NONE,
                DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT,
                DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter1, IDXGIFactory4, IDXGISwapChain3,
            },
        },
    },
    core::Interface,
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

const BUFFER_COUNT: usize = 2;
const GPU_RESOURCE_CACHE_LIMIT: usize = 48 * 1024 * 1024;
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
pub(crate) struct D3DTargetId(u64);

pub(crate) const MAIN_D3D_TARGET: D3DTargetId = D3DTargetId(0);

struct D3DTarget {
    surfaces: Vec<(Surface, BackendRenderTarget)>,
    composition_visual: IDCompositionVisual,
    composition_target: IDCompositionTarget,
    swap_chain: IDXGISwapChain3,
}

pub(crate) struct D3DRenderer {
    targets: HashMap<D3DTargetId, D3DTarget>,
    direct_context: DirectContext,
    composition_device: IDCompositionDevice,
    factory: IDXGIFactory4,
    backend_context: BackendContext,
    next_target_id: u64,
    last_resource_cleanup: Instant,
    failure: Option<String>,
}

impl D3DRenderer {
    pub(crate) fn new(window: &Window, width: u32, height: u32) -> Result<Self, String> {
        let mut last_error = None;
        for attempt in 0..INITIALIZATION_ATTEMPTS {
            match Self::new_once(window, width, height) {
                Ok(renderer) => return Ok(renderer),
                Err(error) => {
                    if attempt + 1 < INITIALIZATION_ATTEMPTS {
                        log::warn!(
                            "D3D12 renderer initialization failed; retrying attempt {}/{}: {}",
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
        Err(last_error.unwrap_or_else(|| "D3D12 renderer initialization failed".to_string()))
    }

    pub(crate) fn try_new(window: &Window, width: u32, height: u32) -> Result<Self, String> {
        Self::new_once(window, width, height)
    }

    fn new_once(window: &Window, width: u32, height: u32) -> Result<Self, String> {
        let factory: IDXGIFactory4 = unsafe {
            // SAFETY: DXGI factory creation has no pointer arguments and returns a COM interface
            // managed by the windows crate.
            CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))
        }
        .map_err(|error| format!("CreateDXGIFactory2 failed: {error}"))?;
        let (adapter, device) = hardware_adapter_and_device(&factory)?;
        let queue = unsafe {
            // SAFETY: The command queue descriptor is fully initialized by Default and the
            // returned queue is retained by BackendContext for the renderer lifetime.
            device.CreateCommandQueue(&Default::default())
        }
        .map_err(|error| format!("CreateCommandQueue failed: {error}"))?;
        let backend_context = BackendContext {
            adapter,
            device,
            queue,
            memory_allocator: None,
            protected_context: Protected::No,
        };
        let mut direct_context = unsafe {
            // SAFETY: BackendContext is stored in D3DRenderer and outlives DirectContext.
            gpu::direct_contexts::make_d3d(&backend_context, None)
        }
        .ok_or_else(|| "Skia failed to create a D3D12 DirectContext".to_string())?;
        direct_context.set_resource_cache_limit(GPU_RESOURCE_CACHE_LIMIT);
        let composition_device: IDCompositionDevice = unsafe {
            // SAFETY: DirectComposition owns the device it creates; it is retained for the
            // renderer lifetime and only receives valid COM content below.
            DCompositionCreateDevice2(None)
        }
        .map_err(|error| format!("DCompositionCreateDevice2 failed: {error}"))?;
        let mut renderer = Self {
            targets: HashMap::new(),
            direct_context,
            composition_device,
            factory,
            backend_context,
            next_target_id: 0,
            last_resource_cleanup: Instant::now(),
            failure: None,
        };
        let target_id = renderer.create_target(window, width, height)?;
        debug_assert_eq!(target_id, MAIN_D3D_TARGET);
        Ok(renderer)
    }

    pub(crate) fn create_target(
        &mut self,
        window: &Window,
        width: u32,
        height: u32,
    ) -> Result<D3DTargetId, String> {
        let hwnd = window_hwnd(window)?;
        let swap_chain = create_swap_chain(&self.factory, &self.backend_context, width, height)?;
        let composition_target = unsafe {
            // SAFETY: hwnd belongs to the live winit window retained by the caller.
            self.composition_device.CreateTargetForHwnd(hwnd, false)
        }
        .map_err(|error| format!("CreateTargetForHwnd failed: {error}"))?;
        let composition_visual = unsafe {
            // SAFETY: The composition device is valid and retained by this renderer.
            self.composition_device.CreateVisual()
        }
        .map_err(|error| format!("CreateVisual failed: {error}"))?;
        unsafe {
            // SAFETY: The visual and target belong to the same retained composition device.
            composition_visual
                .SetContent(&swap_chain)
                .map_err(|error| format!("IDCompositionVisual::SetContent failed: {error}"))?;
            composition_target
                .SetRoot(&composition_visual)
                .map_err(|error| format!("IDCompositionTarget::SetRoot failed: {error}"))?;
            self.composition_device
                .Commit()
                .map_err(|error| format!("IDCompositionDevice::Commit failed: {error}"))?;
        }
        let surfaces = create_surfaces(&swap_chain, &mut self.direct_context, width, height)?;
        let target_id = D3DTargetId(self.next_target_id);
        self.next_target_id += 1;
        self.targets.insert(
            target_id,
            D3DTarget {
                surfaces,
                composition_visual,
                composition_target,
                swap_chain,
            },
        );
        Ok(target_id)
    }

    pub(crate) fn draw<T>(
        &mut self,
        target_id: D3DTargetId,
        draw: impl FnOnce(&mut DirectContext, &mut Surface) -> T,
    ) -> Result<T, String> {
        let result = self.draw_inner(target_id, draw);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    fn draw_inner<T>(
        &mut self,
        target_id: D3DTargetId,
        draw: impl FnOnce(&mut DirectContext, &mut Surface) -> T,
    ) -> Result<T, String> {
        self.check_context("before drawing")?;
        let device = self.backend_context.device.clone();
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or_else(|| "D3D12 render target is unavailable".to_string())?;
        let index = unsafe {
            // SAFETY: IDXGISwapChain3 is valid for the renderer lifetime.
            target.swap_chain.GetCurrentBackBufferIndex()
        } as usize;
        let (surface, _) = target
            .surfaces
            .get_mut(index)
            .ok_or_else(|| "DXGI returned an invalid back buffer index".to_string())?;
        let output = draw(&mut self.direct_context, surface);
        self.direct_context.flush_and_submit_surface(surface, None);
        check_context(&mut self.direct_context, &device, "after submitting")?;
        let present_result = unsafe {
            // SAFETY: The current back buffer is rendered and submitted before presentation.
            target.swap_chain.Present(1, DXGI_PRESENT(0)).ok()
        };
        if let Err(error) = present_result {
            return Err(format!(
                "DXGI Present failed: {error}; {}",
                device_status(&device)
            ));
        }
        check_context(&mut self.direct_context, &device, "after presenting")?;
        self.cleanup_unused_resources();
        Ok(output)
    }

    pub(crate) fn resize(
        &mut self,
        target_id: D3DTargetId,
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
        target_id: D3DTargetId,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.check_context("before resizing")?;
        let device = self.backend_context.device.clone();
        self.direct_context.flush_submit_and_sync_cpu();
        check_context(
            &mut self.direct_context,
            &device,
            "while synchronizing resize",
        )?;
        let target = self
            .targets
            .get_mut(&target_id)
            .ok_or_else(|| "D3D12 render target is unavailable".to_string())?;
        target.surfaces.clear();
        self.direct_context
            .purge_unlocked_resources(gpu::PurgeResourceOptions::AllResources);
        let resize_result = unsafe {
            // SAFETY: All Skia surfaces that reference the back buffers were dropped and the
            // DirectContext was synchronized and purged before resizing the retained swap chain.
            target.swap_chain.ResizeBuffers(
                BUFFER_COUNT as u32,
                width,
                height,
                DXGI_FORMAT_R8G8B8A8_UNORM,
                Default::default(),
            )
        };
        if let Err(error) = resize_result {
            return Err(format!(
                "DXGI ResizeBuffers failed: {error}; {}",
                device_status(&device)
            ));
        }
        target.surfaces =
            create_surfaces(&target.swap_chain, &mut self.direct_context, width, height)?;
        check_context(&mut self.direct_context, &device, "after resizing")?;
        Ok(())
    }

    pub(crate) fn take_failure(&mut self) -> Option<String> {
        self.failure.take()
    }

    pub(crate) fn abandon(&mut self) {
        self.direct_context.abandon();
    }

    pub(crate) fn remove_target(&mut self, target_id: D3DTargetId) {
        let Some(mut target) = self.targets.remove(&target_id) else {
            return;
        };
        self.direct_context.flush_submit_and_sync_cpu();
        target.surfaces.clear();
        unsafe {
            // SAFETY: The retained target and visual belong to this live composition device.
            // Detaching both before dropping their COM handles lets DWM release the swap chain.
            if let Err(error) = target
                .composition_target
                .SetRoot(None::<&IDCompositionVisual>)
            {
                log::warn!("Failed to detach DirectComposition target: {error}");
            }
            if let Err(error) = target
                .composition_visual
                .SetContent(None::<&windows::core::IUnknown>)
            {
                log::warn!("Failed to detach DirectComposition content: {error}");
            }
            match self.composition_device.Commit() {
                Ok(()) => {
                    if let Err(error) = self.composition_device.WaitForCommitCompletion() {
                        log::warn!("Failed to wait for DirectComposition cleanup: {error}");
                    }
                }
                Err(error) => log::warn!("Failed to commit DirectComposition cleanup: {error}"),
            }
        }
        drop(target);
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
        let device = self.backend_context.device.clone();
        check_context(&mut self.direct_context, &device, stage)
    }
}

fn check_context(
    direct_context: &mut DirectContext,
    device: &ID3D12Device,
    stage: &str,
) -> Result<(), String> {
    if direct_context.is_device_lost() {
        return Err(format!(
            "Skia D3D12 device lost {stage}: {}",
            device_status(device)
        ));
    }
    if direct_context.oomed() {
        return Err(format!(
            "Skia D3D12 allocation failed {stage}: {}",
            device_status(device)
        ));
    }
    Ok(())
}

fn device_status(device: &ID3D12Device) -> String {
    match unsafe {
        // SAFETY: device is a live COM interface retained by the renderer. The call only queries
        // the device's removal status.
        device.GetDeviceRemovedReason()
    } {
        Ok(()) => "device reports no removal reason".to_string(),
        Err(error) => format!("device removal reason: {error}"),
    }
}

fn window_hwnd(window: &Window) -> Result<HWND, String> {
    let handle = window
        .window_handle()
        .map_err(|error| format!("Window handle unavailable: {error}"))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as _)),
        _ => Err("D3D12 rendering requires a Win32 window".to_string()),
    }
}

fn hardware_adapter_and_device(
    factory: &IDXGIFactory4,
) -> Result<(IDXGIAdapter1, ID3D12Device), String> {
    for index in 0.. {
        let adapter = match unsafe {
            // SAFETY: DXGI owns adapter enumeration and returns a managed COM interface.
            factory.EnumAdapters1(index)
        } {
            Ok(adapter) => adapter,
            Err(_) => break,
        };
        let desc = match unsafe {
            // SAFETY: adapter is a valid COM interface from EnumAdapters1.
            adapter.GetDesc1()
        } {
            Ok(desc) => desc,
            Err(error) => {
                log::warn!("Skipping DXGI adapter {index}: GetDesc1 failed: {error}");
                continue;
            }
        };
        if (DXGI_ADAPTER_FLAG(desc.Flags as _) & DXGI_ADAPTER_FLAG_SOFTWARE)
            != DXGI_ADAPTER_FLAG_NONE
        {
            continue;
        }
        let mut device = None;
        if unsafe {
            // SAFETY: adapter is retained for the device lifetime through BackendContext.
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)
        }
        .is_ok()
            && let Some(device) = device
        {
            return Ok((adapter, device));
        }
    }
    Err("No hardware D3D12 adapter is available".to_string())
}

fn create_swap_chain(
    factory: &IDXGIFactory4,
    backend_context: &BackendContext,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain3, String> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width.max(1),
        Height: height.max(1),
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: 0,
    };
    let swap_chain = unsafe {
        // SAFETY: The queue is retained by BackendContext and the descriptor is fully initialized.
        factory.CreateSwapChainForComposition(&backend_context.queue, &desc, None)
    }
    .map_err(|error| format!("CreateSwapChainForComposition failed: {error}"))?;
    swap_chain
        .cast()
        .map_err(|error| format!("IDXGISwapChain3 cast failed: {error}"))
}

fn create_surfaces(
    swap_chain: &IDXGISwapChain3,
    direct_context: &mut DirectContext,
    width: u32,
    height: u32,
) -> Result<Vec<(Surface, BackendRenderTarget)>, String> {
    (0..BUFFER_COUNT)
        .map(|index| {
            let resource = unsafe {
                // SAFETY: index is within BUFFER_COUNT and the swap chain owns the buffer.
                swap_chain.GetBuffer(index as u32)
            }
            .map_err(|error| format!("GetBuffer({index}) failed: {error}"))?;
            let backend_render_target = gpu::backend_render_targets::make_d3d(
                (width as i32, height as i32),
                &TextureResourceInfo {
                    resource,
                    alloc: None,
                    resource_state: D3D12_RESOURCE_STATE_COMMON,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    sample_count: 1,
                    level_count: 0,
                    sample_quality_pattern: DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                    protected: Protected::No,
                },
            );
            let surface = surfaces::wrap_backend_render_target(
                direct_context,
                &backend_render_target,
                SurfaceOrigin::TopLeft,
                ColorType::RGBA8888,
                None,
                None,
            )
            .ok_or_else(|| format!("Skia failed to wrap D3D12 buffer {index}"))?;
            Ok((surface, backend_render_target))
        })
        .collect()
}
