use std::{mem::ManuallyDrop, sync::Arc};

use ash::{khr::surface, vk};
use render_common::Renderer;

mod init;
mod raii;
mod swapchain;

const FRAMES_IN_FLIGHT: usize = 2;

struct FrameData {
    command_buffer: vk::CommandBuffer,
}

pub struct DeviceShared {
    device: ash::Device,
    queue: vk::Queue,
}

pub struct VulkanRenderer {
    _entry: ash::Entry,
    instance: ash::Instance,

    swapchain: swapchain::NativeSwapchain,

    shared: ManuallyDrop<Arc<DeviceShared>>,

    command_pool: vk::CommandPool,
    frames: [FrameData; FRAMES_IN_FLIGHT],
}

impl Renderer for VulkanRenderer {
    fn resize(&mut self, resolution: glam::UVec2) -> anyhow::Result<()> {
        self.swapchain.resize(&self.shared.device, resolution)
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            for frame in &self.frames {
                self.shared
                    .device
                    .free_command_buffers(self.command_pool, &[frame.command_buffer]);
            }

            self.shared
                .device
                .destroy_command_pool(self.command_pool, None);

            self.swapchain.dispose(&self.shared.device);

            let shared = ManuallyDrop::take(&mut self.shared);
            if let Some(shared) = Arc::into_inner(shared) {
                shared.device.destroy_device(None);
            } else {
                eprintln!("LEAK! Could not destroy device");
            }

            self.instance.destroy_instance(None);
        }
    }
}
