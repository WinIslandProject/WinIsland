use std::sync::OnceLock;

use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_ADAPTER_FLAG, DXGI_ADAPTER_FLAG_NONE, DXGI_ADAPTER_FLAG_SOFTWARE,
    DXGI_CREATE_FACTORY_FLAGS, IDXGIFactory4,
};

const INTEGRATED_GPU_MEMORY_THRESHOLD: usize = 1_073_741_824;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuProfile {
    Discrete,
    Integrated,
}

static GPU_PROFILE: OnceLock<GpuProfile> = OnceLock::new();

pub fn gpu_profile() -> GpuProfile {
    *GPU_PROFILE.get_or_init(detect_gpu_profile)
}

fn detect_gpu_profile() -> GpuProfile {
    if let Some(profile) = profile_from_env() {
        return profile;
    }
    // SAFETY: CreateDXGIFactory2 requires no COM initialization. Adapter enumeration
    // only reads adapter information and releases all COM objects automatically.
    unsafe {
        let Ok(factory) = CreateDXGIFactory2::<IDXGIFactory4>(DXGI_CREATE_FACTORY_FLAGS(0)) else {
            return GpuProfile::Discrete;
        };
        for index in 0.. {
            let Ok(adapter) = factory.EnumAdapters1(index) else {
                break;
            };
            let Ok(desc) = adapter.GetDesc1() else {
                continue;
            };
            if (DXGI_ADAPTER_FLAG(desc.Flags as _) & DXGI_ADAPTER_FLAG_SOFTWARE)
                != DXGI_ADAPTER_FLAG_NONE
            {
                continue;
            }
            return if desc.DedicatedVideoMemory < INTEGRATED_GPU_MEMORY_THRESHOLD {
                GpuProfile::Integrated
            } else {
                GpuProfile::Discrete
            };
        }
    }
    GpuProfile::Discrete
}

fn profile_from_env() -> Option<GpuProfile> {
    match std::env::var("WINISLAND_GPU_PROFILE") {
        Ok(value) if value.eq_ignore_ascii_case("integrated") => Some(GpuProfile::Integrated),
        Ok(value) if value.eq_ignore_ascii_case("discrete") => Some(GpuProfile::Discrete),
        _ => None,
    }
}
