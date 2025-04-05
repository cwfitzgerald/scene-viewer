use std::{mem::ManuallyDrop, sync::Arc};

use ash::{
    khr::{surface, swapchain},
    vk,
};

mod init;
mod raii;

pub struct DeviceShared {
    pub device: ash::Device,
    pub queue: vk::Queue,
}

pub struct VulkanRenderer {
    _entry: ash::Entry,
    instance: ash::Instance,

    surface_fn: surface::Instance,
    surface: vk::SurfaceKHR,
    swapchain_fn: swapchain::Device,
    swapchain: vk::SwapchainKHR,

    shared: ManuallyDrop<Arc<DeviceShared>>,
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);

            let shared = ManuallyDrop::take(&mut self.shared);
            if let Some(shared) = Arc::into_inner(shared) {
                shared.device.destroy_device(None);
            }

            self.surface_fn.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}
