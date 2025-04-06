use std::{mem::ManuallyDrop, sync::Arc};

use anyhow::Context;
use ash::{khr::surface, vk};
use render_common::Renderer;

mod init;
mod raii;
mod swapchain;

const FRAMES_IN_FLIGHT: usize = 2;
const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

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

    timeline_semaphore: vk::Semaphore,
    frames: [FrameData; FRAMES_IN_FLIGHT],

    current_frame: u64,
}

impl Renderer for VulkanRenderer {
    fn resize(&mut self, resolution: glam::UVec2) -> anyhow::Result<()> {
        self.swapchain.resize(&self.shared.device, resolution)
    }

    fn render(&mut self) -> anyhow::Result<()> {
        unsafe {
            let frame = &self.frames[self.current_frame as usize % FRAMES_IN_FLIGHT];

            let frame_to_wait_for = self.current_frame.saturating_sub(2);

            self.shared
                .device
                .wait_semaphores(
                    &vk::SemaphoreWaitInfo::default()
                        .semaphores(std::slice::from_ref(&self.timeline_semaphore))
                        .values(std::slice::from_ref(&frame_to_wait_for)),
                    5 * NANOSECONDS_PER_SECOND,
                )
                .context("Failed to wait for timeline semaphore")?;

            let (image_view, semaphores) = self.swapchain.acquire()?;

            self.shared.device.reset_command_buffer(
                frame.command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )?;

            self.shared
                .device
                .begin_command_buffer(
                    frame.command_buffer,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .context("Failed to begin command buffer")?;

            self.shared
                .device
                .end_command_buffer(frame.command_buffer)
                .context("Failed to end command buffer")?;

            let signal_semaphores = [semaphores.present, self.timeline_semaphore];
            let signal_values = [0, self.current_frame];

            let mut timeline_submit_info = vk::TimelineSemaphoreSubmitInfo::default()
                .signal_semaphore_values(&signal_values)
                .wait_semaphore_values(&[0]);

            let submit_info = vk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(&frame.command_buffer))
                .signal_semaphores(&signal_semaphores)
                .wait_semaphores(std::slice::from_ref(&semaphores.acquire))
                .push_next(&mut timeline_submit_info);

            self.shared
                .device
                .queue_submit(
                    self.shared.queue,
                    std::slice::from_ref(&submit_info),
                    vk::Fence::null(),
                )
                .context("Failed to submit command buffer")?;

            self.swapchain.present(self.shared.queue)?;

            self.current_frame += 1;

            Ok(())
        }
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

            self.shared
                .device
                .destroy_semaphore(self.timeline_semaphore, None);

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
