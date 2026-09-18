use std::sync::Arc;

use skia_safe::gpu::{self, ContextOptions, DirectContext, Protected, d3d::BackendContext};
use windows::Win32::Graphics::{
    Direct3D::D3D_FEATURE_LEVEL_11_0,
    Direct3D12::{
        D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC, D3D12CreateDevice, ID3D12Device,
    },
    DirectComposition::{DCompositionCreateDevice2, IDCompositionDevice},
    Dxgi::{
        CreateDXGIFactory2, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_CREATE_FACTORY_FLAGS,
        DXGI_ERROR_NOT_FOUND, IDXGIAdapter1, IDXGIFactory4,
    },
};
use winit::window::Window;

mod target;
pub(super) use target::RenderTarget;

const GPU_RESOURCE_CACHE_LIMIT: usize = 12 * 1024 * 1024;
const GPU_GLYPH_CACHE_LIMIT: usize = 2 * 1024 * 1024;

pub(super) struct D3DDevice {
    pub(super) context: DirectContext,
    composition: IDCompositionDevice,
    backend: BackendContext,
    factory: IDXGIFactory4,
}

impl D3DDevice {
    pub(super) fn new() -> Result<Self, String> {
        // SAFETY: DXGI returns an owned COM interface without retaining any caller pointers.
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .map_err(|error| format!("CreateDXGIFactory2 failed: {error}"))?;
        let mut errors = Vec::new();
        for index in 0.. {
            // SAFETY: Enumeration uses a live factory and returns an owned adapter interface.
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(adapter) => adapter,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(error) => return Err(format!("DXGI adapter enumeration failed: {error}")),
            };
            // SAFETY: This only queries the live adapter returned by DXGI.
            let desc = unsafe { adapter.GetDesc1() }
                .map_err(|error| format!("DXGI adapter description failed: {error}"))?;
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let name = String::from_utf16_lossy(
                &desc.Description[..desc.Description.iter().position(|&c| c == 0).unwrap_or(128)],
            );
            match Self::from_adapter(factory.clone(), adapter) {
                Ok(device) => {
                    log::info!("D3D12 renderer: {name}");
                    return Ok(device);
                }
                Err(error) => errors.push(format!("{name}: {error}")),
            }
        }
        Err(format!(
            "No usable hardware D3D12 adapter: {}",
            errors.join("; ")
        ))
    }

    fn from_adapter(factory: IDXGIFactory4, adapter: IDXGIAdapter1) -> Result<Self, String> {
        let mut device: Option<ID3D12Device> = None;
        // SAFETY: The adapter is live and the output is an initialized Option owned by this call.
        unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }
            .map_err(|error| format!("D3D12CreateDevice failed: {error}"))?;
        let device = device.ok_or_else(|| "D3D12CreateDevice returned no device".to_string())?;
        let desc = D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        };
        // SAFETY: The descriptor is initialized and the returned queue retains the live device.
        let queue = unsafe { device.CreateCommandQueue(&desc) }
            .map_err(|error| format!("D3D12 command queue creation failed: {error}"))?;
        // SAFETY: DirectComposition returns an owned device without caller-owned resources.
        let composition = unsafe { DCompositionCreateDevice2(None) }
            .map_err(|error| format!("DirectComposition device creation failed: {error}"))?;
        let backend = BackendContext {
            adapter,
            device,
            queue,
            memory_allocator: None,
            protected_context: Protected::No,
        };
        let mut options = ContextOptions::new();
        options.glyph_cache_texture_maximum_bytes = GPU_GLYPH_CACHE_LIMIT;
        options.allow_multiple_glyph_cache_textures = gpu::ganesh::context_options::Enable::No;
        options.reduce_ops_task_splitting = gpu::ganesh::context_options::Enable::No;
        // SAFETY: The retained backend outlives the context, and both are used on the render thread.
        let mut context = unsafe { gpu::direct_contexts::make_d3d(&backend, Some(&options)) }
            .ok_or_else(|| "Skia D3D12 context creation failed".to_string())?;
        context.set_resource_cache_limit(GPU_RESOURCE_CACHE_LIMIT);
        Ok(Self {
            context,
            composition,
            backend,
            factory,
        })
    }

    pub(super) fn create_target(
        &mut self,
        window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<RenderTarget, String> {
        self.check_health()?;
        RenderTarget::new(self, window, width.max(1), height.max(1))
    }

    pub(super) fn resize_target(
        &mut self,
        target: &mut RenderTarget,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if width == 0 || height == 0 || target.size() == (width, height) {
            return Ok(());
        }
        self.validate_size(width, height)?;
        self.synchronize()?;
        target.release_buffers();
        self.context
            .purge_unlocked_resources(gpu::PurgeResourceOptions::AllResources);
        self.synchronize()?;
        target.resize(&mut self.context, width, height)?;
        self.check_health()
    }

    fn validate_size(&self, width: u32, height: u32) -> Result<(), String> {
        let limit = self.context.max_render_target_size() as u32;
        if width == 0 || height == 0 || width > limit || height > limit {
            return Err(format!(
                "Invalid D3D12 target size {width}x{height} (limit {limit})"
            ));
        }
        Ok(())
    }

    pub(super) fn check_health(&mut self) -> Result<(), String> {
        // SAFETY: This only queries the retained D3D12 device's removal status.
        unsafe { self.backend.device.GetDeviceRemovedReason() }
            .map_err(|error| format!("D3D12 device removed: {error}"))?;
        if self.context.abandoned() || self.context.is_device_lost() {
            return Err("Skia D3D12 context is unavailable".to_string());
        }
        if self.context.oomed() {
            return Err("Skia D3D12 allocation failed".to_string());
        }
        Ok(())
    }

    pub(super) fn submit(&mut self, sync: gpu::SyncCpu) -> Result<(), String> {
        if !self.context.submit(sync) {
            self.check_health()?;
            return Err("Skia D3D12 submission failed".to_string());
        }
        self.check_health()
    }

    pub(super) fn synchronize(&mut self) -> Result<(), String> {
        self.check_health()?;
        self.context.flush(&gpu::FlushInfo::default());
        self.submit(gpu::SyncCpu::Yes)
    }
}

impl Drop for D3DDevice {
    fn drop(&mut self) {
        if self.synchronize().is_ok() {
            self.context.release_resources_and_abandon();
        } else {
            self.context.abandon();
        }
    }
}
