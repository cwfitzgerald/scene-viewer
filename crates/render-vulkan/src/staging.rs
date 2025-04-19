use ash::vk;
use gpu_allocator::MemoryLocation;

use crate::{DeviceShared, FRAMES_IN_FLIGHT, allocation::AllocatedBuffer};

const STAGING_BUFFER_SIZE: u64 = 8 * 1024 * 1024; // 8 MB

#[derive(Debug)]
struct StoredCopy {
    dest: vk::Buffer,
    dst_offset: u64,
    size: u64,
    src_offset: u64,
}

struct StagingBuffer {
    buffer: AllocatedBuffer,
    used: u64,
    stored_copies: Vec<StoredCopy>,
}

pub struct StagingBelt {
    buffers: [StagingBuffer; FRAMES_IN_FLIGHT],
    current_frame: usize,
}

impl StagingBelt {
    pub fn new(ctx: &DeviceShared) -> anyhow::Result<Self> {
        let mut buffers = <[_; FRAMES_IN_FLIGHT]>::default();

        for i in 0..FRAMES_IN_FLIGHT {
            let buffer = ctx.allocator.allocate_buffer(
                &ctx.device,
                &format!("Staging buffer #{i}"),
                STAGING_BUFFER_SIZE,
                vk::BufferUsageFlags::TRANSFER_SRC,
                MemoryLocation::CpuToGpu,
            )?;

            buffers[i] = Some(StagingBuffer { buffer, used: 0, stored_copies: Vec::new() });
        }

        Ok(Self { buffers: buffers.map(Option::unwrap), current_frame: 0 })
    }

    pub fn start_frame(&mut self, frame: usize) {
        self.current_frame = frame;

        let data = &mut self.buffers[self.current_frame];
        data.used = 0;
        data.stored_copies.clear();
    }

    pub fn write_buffer(&mut self, dest: vk::Buffer, dst_offset: u64, data: &[u8]) {
        let data_size = data.len() as u64;
        let buffer = &mut self.buffers[self.current_frame];

        if buffer.used + data_size > STAGING_BUFFER_SIZE {
            panic!("Staging buffer overflow");
        }

        let src_offset = buffer.used;
        buffer.used += data_size;

        buffer.buffer.mapped_slice_mut()[src_offset as usize..][..data_size as usize]
            .copy_from_slice(data);

        buffer.stored_copies.push(StoredCopy { dest, dst_offset, size: data_size, src_offset });
    }

    pub fn flush_copies(&self, ctx: &DeviceShared, command_buffer: vk::CommandBuffer) {
        let buffer = &self.buffers[self.current_frame];

        for copy in &buffer.stored_copies {
            unsafe {
                ctx.device.cmd_copy_buffer(
                    command_buffer,
                    buffer.buffer.buffer(),
                    copy.dest,
                    &[vk::BufferCopy::default()
                        .src_offset(copy.src_offset)
                        .dst_offset(copy.dst_offset)
                        .size(copy.size)],
                );
            }
        }
    }

    pub fn dispose(self, ctx: &DeviceShared) {
        for buffer in self.buffers {
            ctx.allocator.dispose_buffer(&ctx.device, buffer.buffer);
        }
    }
}
