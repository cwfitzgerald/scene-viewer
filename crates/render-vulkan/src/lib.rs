use std::{mem::ManuallyDrop, sync::Arc};

use anyhow::Context;
use ash::vk;
use render_common::Renderer;

mod init;
mod mesh;
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

    resolution: glam::UVec2,
    current_frame: u64,
}

impl Renderer for VulkanRenderer {
    fn resize(&mut self, resolution: glam::UVec2) -> anyhow::Result<()> {
        unsafe {
            self.resolution = resolution;

            self.shared
                .device
                .device_wait_idle()
                .context("Failed to wait for device idle")?;

            self.swapchain.resize(&self.shared.device, resolution)
        }
    }

    fn render(&mut self) -> anyhow::Result<()> {
        unsafe {
            println!("Rendering frame {}", self.current_frame);

            let frame_data_index = self.current_frame as usize % FRAMES_IN_FLIGHT;
            println!("Frame data index: {}", frame_data_index);

            let frame = &self.frames[frame_data_index];

            let frame_to_wait_for = self.current_frame.saturating_sub(2);

            println!("Waiting for semaphore value {}", frame_to_wait_for);

            self.shared
                .device
                .wait_semaphores(
                    &vk::SemaphoreWaitInfo::default()
                        .semaphores(std::slice::from_ref(&self.timeline_semaphore))
                        .values(std::slice::from_ref(&frame_to_wait_for)),
                    5 * NANOSECONDS_PER_SECOND,
                )
                .context("Failed to wait for timeline semaphore")?;

            let (image, image_view, semaphores) = self.swapchain.acquire()?;

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

            self.shared.device.cmd_pipeline_barrier2(
                frame.command_buffer,
                &vk::DependencyInfo::default().image_memory_barriers(&[
                    vk::ImageMemoryBarrier2::default()
                        .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                        .src_access_mask(vk::AccessFlags2::NONE)
                        .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .image(image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                ]),
            );

            let attachment = vk::RenderingAttachmentInfo::default()
                .image_view(image_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .resolve_mode(vk::ResolveModeFlags::NONE)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [1.0, 0.0, 0.0, 1.0],
                    },
                });

            let rendering_info = vk::RenderingInfo::default()
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: self.resolution.x,
                        height: self.resolution.y,
                    },
                })
                .layer_count(1)
                .color_attachments(std::slice::from_ref(&attachment));

            self.shared
                .device
                .cmd_begin_rendering(frame.command_buffer, &rendering_info);

            self.shared.device.cmd_end_rendering(frame.command_buffer);

            self.shared.device.cmd_pipeline_barrier2(
                frame.command_buffer,
                &vk::DependencyInfo::default().image_memory_barriers(&[
                    vk::ImageMemoryBarrier2::default()
                        .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                        .dst_stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE)
                        .dst_access_mask(vk::AccessFlags2::NONE)
                        .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                        .image(image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                ]),
            );

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
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::ALL_COMMANDS])
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
            let _ = self.shared.device.device_wait_idle();

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
