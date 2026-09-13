use std::ffi::{CStr, c_void};
use std::rc::Rc;

use ash::{vk, vk::Handle};
use skia_safe::gpu::{self, ContextOptions, DirectContext};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

const API_VERSION: u32 = vk::API_VERSION_1_2;
const INSTANCE_EXTENSIONS: [&CStr; 2] = [ash::khr::surface::NAME, ash::khr::win32_surface::NAME];
const DEVICE_EXTENSIONS: [&CStr; 1] = [ash::khr::swapchain::NAME];

pub(super) struct VulkanInstance {
    pub(super) instance: ash::Instance,
    pub(super) surface: ash::khr::surface::Instance,
    win32_surface: ash::khr::win32_surface::Instance,
    entry: ash::Entry,
}

pub(super) struct VulkanDevice {
    pub(super) device: ash::Device,
    pub(super) instance: Rc<VulkanInstance>,
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) queue: vk::Queue,
    pub(super) queue_family: u32,
    pub(super) swapchain: ash::khr::swapchain::Device,
}

impl VulkanInstance {
    pub(super) fn new() -> Result<Rc<Self>, String> {
        // SAFETY: The loader stays owned by this instance until all Vulkan objects are destroyed.
        let entry = unsafe { ash::Entry::load() }
            .map_err(|error| format!("Vulkan loader unavailable: {error}"))?;
        // SAFETY: This only queries the loaded Vulkan library.
        let version = unsafe { entry.try_enumerate_instance_version() }
            .map_err(|error| format!("Vulkan version query failed: {error}"))?
            .unwrap_or(vk::API_VERSION_1_0);
        if version < API_VERSION {
            return Err("WinIsland requires Vulkan 1.2 or newer".to_string());
        }

        let app_info = vk::ApplicationInfo::default()
            .application_name(c"WinIsland")
            .api_version(API_VERSION);
        let extensions: Vec<_> = INSTANCE_EXTENSIONS
            .iter()
            .map(|name| name.as_ptr())
            .collect();
        let info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions);
        // SAFETY: All descriptor pointers remain valid for this call.
        let instance = unsafe { entry.create_instance(&info, None) }
            .map_err(|error| format!("Vulkan instance creation failed: {error}"))?;
        let surface = ash::khr::surface::Instance::new(&entry, &instance);
        let win32_surface = ash::khr::win32_surface::Instance::new(&entry, &instance);
        Ok(Rc::new(Self {
            instance,
            surface,
            win32_surface,
            entry,
        }))
    }

    pub(super) fn create_surface(&self, window: &Window) -> Result<vk::SurfaceKHR, String> {
        let handle = window
            .window_handle()
            .map_err(|error| format!("Window handle unavailable: {error}"))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("Vulkan rendering requires a Win32 window".to_string());
        };
        let hinstance = handle
            .hinstance
            .ok_or_else(|| "Window instance handle unavailable".to_string())?;
        let info = vk::Win32SurfaceCreateInfoKHR::default()
            .hinstance(hinstance.get())
            .hwnd(handle.hwnd.get());
        // SAFETY: Both native handles belong to the live winit window retained by the caller.
        unsafe { self.win32_surface.create_win32_surface(&info, None) }
            .map_err(|error| format!("Vulkan Win32 surface creation failed: {error}"))
    }

    pub(super) fn destroy_surface(&self, surface: vk::SurfaceKHR) {
        // SAFETY: The renderer waits for the device before destroying each owned surface.
        unsafe { self.surface.destroy_surface(surface, None) };
    }

    pub(super) fn device_for_surface(
        self: &Rc<Self>,
        surface: vk::SurfaceKHR,
    ) -> Result<Rc<VulkanDevice>, String> {
        let mut errors = Vec::new();
        // SAFETY: The instance and surface are live, and every queried handle comes from them.
        unsafe {
            let devices = self
                .instance
                .enumerate_physical_devices()
                .map_err(|error| format!("Vulkan device enumeration failed: {error}"))?;
            for physical_device in devices {
                let properties = self
                    .instance
                    .get_physical_device_properties(physical_device);
                let name = CStr::from_ptr(properties.device_name.as_ptr()).to_string_lossy();
                if properties.api_version < API_VERSION {
                    errors.push(format!("{name}: Vulkan API is older than 1.2"));
                    continue;
                }

                let available = self
                    .instance
                    .enumerate_device_extension_properties(physical_device)
                    .map_err(|error| format!("Vulkan extension query failed: {error}"))?;
                if let Some(missing) = DEVICE_EXTENSIONS.iter().find(|required| {
                    !available.iter().any(|extension| {
                        CStr::from_ptr(extension.extension_name.as_ptr()) == **required
                    })
                }) {
                    errors.push(format!("{name}: missing {}", missing.to_string_lossy()));
                    continue;
                }

                let queue_family = self
                    .instance
                    .get_physical_device_queue_family_properties(physical_device)
                    .iter()
                    .enumerate()
                    .find_map(|(index, queue)| {
                        let index = index as u32;
                        (queue.queue_count > 0
                            && queue.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                            && self
                                .surface
                                .get_physical_device_surface_support(
                                    physical_device,
                                    index,
                                    surface,
                                )
                                .unwrap_or(false))
                        .then_some(index)
                    });
                let Some(queue_family) = queue_family else {
                    errors.push(format!(
                        "{name}: no graphics queue can present to this window"
                    ));
                    continue;
                };

                let priorities = [1.0];
                let queues = [vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(queue_family)
                    .queue_priorities(&priorities)];
                let extensions: Vec<_> =
                    DEVICE_EXTENSIONS.iter().map(|name| name.as_ptr()).collect();
                let features = self.instance.get_physical_device_features(physical_device);
                let info = vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queues)
                    .enabled_extension_names(&extensions)
                    .enabled_features(&features);
                let device = match self.instance.create_device(physical_device, &info, None) {
                    Ok(device) => device,
                    Err(error) => {
                        errors.push(format!("{name}: device creation failed: {error}"));
                        continue;
                    }
                };
                let queue = device.get_device_queue(queue_family, 0);
                let swapchain = ash::khr::swapchain::Device::new(&self.instance, &device);
                log::info!(
                    "Vulkan renderer: {} (API {}.{}.{})",
                    name,
                    vk::api_version_major(properties.api_version),
                    vk::api_version_minor(properties.api_version),
                    vk::api_version_patch(properties.api_version)
                );
                return Ok(Rc::new(VulkanDevice {
                    device,
                    instance: self.clone(),
                    physical_device,
                    queue,
                    queue_family,
                    swapchain,
                }));
            }
        }
        Err(format!(
            "No compatible Vulkan 1.2 presentation device: {}",
            errors.join("; ")
        ))
    }
}

impl VulkanDevice {
    pub(super) fn context(&self, options: &ContextOptions) -> Result<DirectContext, String> {
        // SAFETY: Skia supplies NUL-terminated names and handles from this retained instance/device.
        let get_proc = |of| unsafe {
            let proc = match of {
                gpu::vk::GetProcOf::Instance(instance, name) => self
                    .instance
                    .entry
                    .get_instance_proc_addr(vk::Instance::from_raw(instance as _), name),
                gpu::vk::GetProcOf::Device(device, name) => self
                    .instance
                    .instance
                    .get_device_proc_addr(vk::Device::from_raw(device as _), name),
            };
            proc.map_or(std::ptr::null(), |proc| proc as *const c_void)
        };
        // SAFETY: The renderer retains this device until after releasing its Skia context.
        let backend = unsafe {
            gpu::vk::BackendContext::new_builder(
                self.instance.instance.handle().as_raw() as _,
                self.physical_device.as_raw() as _,
                self.device.handle().as_raw() as _,
                (self.queue.as_raw() as _, self.queue_family as usize),
                &get_proc,
                Some(API_VERSION.into()),
            )
            .with_extensions(
                &["VK_KHR_surface", "VK_KHR_win32_surface"],
                &["VK_KHR_swapchain"],
            )
            .build()
        };
        gpu::direct_contexts::make_vulkan(&backend, Some(options))
            .ok_or_else(|| "Skia failed to create a Vulkan 1.2 context".to_string())
    }

    pub(super) fn supports_surface(&self, surface: vk::SurfaceKHR) -> Result<(), String> {
        // SAFETY: The physical device, queue family, and surface remain live.
        let supported = unsafe {
            self.instance.surface.get_physical_device_surface_support(
                self.physical_device,
                self.queue_family,
                surface,
            )
        }
        .map_err(|error| format!("Vulkan surface support query failed: {error}"))?;
        if supported {
            Ok(())
        } else {
            Err("The Vulkan graphics queue cannot present to this window".to_string())
        }
    }

    pub(super) fn wait_idle(&self) -> Result<(), String> {
        // SAFETY: All submissions and destruction run on the render thread with a live device.
        unsafe { self.device.device_wait_idle() }
            .map_err(|error| format!("Vulkan synchronization failed: {error}"))
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        // SAFETY: All swapchains and the Skia context are released before the last owner.
        unsafe { self.device.destroy_device(None) };
    }
}

impl Drop for VulkanInstance {
    fn drop(&mut self) {
        // SAFETY: Every device retains this instance; none remain when its final owner is dropped.
        unsafe { self.instance.destroy_instance(None) };
    }
}
