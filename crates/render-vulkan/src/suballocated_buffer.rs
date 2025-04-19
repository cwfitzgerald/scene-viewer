use std::ops::{Deref, DerefMut};

use crate::{DeviceShared, allocation::AllocatedBuffer, staging::StagingBelt};

const SUBALLOCATION_UNIT_SIZE: u32 = 32; // 32 bytes

pub struct BufferSuballocation {
    allocation: offset_allocator::Allocation,
    pub offset: u64,
    #[expect(dead_code)]
    pub size: u64,
}

pub struct SuballocatedBuffer {
    buffer: AllocatedBuffer,
    /// Suballocator with 32 byte "units" for mesh data.
    suballocator: offset_allocator::Allocator,
    /// True if there was any uploaded made in this frame.
    data_uploaded: bool,
}

impl SuballocatedBuffer {
    pub fn new(buffer: AllocatedBuffer) -> Self {
        let size_units = (buffer.size() / 32) as u32;

        let suballocator = offset_allocator::Allocator::new(size_units);

        Self { buffer, suballocator, data_uploaded: false }
    }

    pub fn start_frame(&mut self) {
        self.data_uploaded = false;
    }

    pub fn upload_data(&mut self, staging: &mut StagingBelt, data: &[u8]) -> BufferSuballocation {
        let size = data.len() as u64;
        let size_units = size.div_ceil(SUBALLOCATION_UNIT_SIZE as u64) as u32;

        // Allocate space in the suballocator.
        let allocation = self.suballocator.allocate(size_units).unwrap();
        let offset = allocation.offset as u64 * SUBALLOCATION_UNIT_SIZE as u64;

        self.data_uploaded = true;
        staging.write_buffer(self.buffer.buffer(), offset, data);

        BufferSuballocation { allocation, offset, size }
    }

    pub fn dispose_suballocation(&mut self, suballocation: BufferSuballocation) {
        // Deallocate the suballocation in the suballocator.
        self.suballocator.free(suballocation.allocation);
    }

    pub fn data_uploaded_this_frame(&self) -> bool {
        self.data_uploaded
    }

    pub fn dispose(self, ctx: &DeviceShared) {
        if (self.suballocator.storage_report().total_free_space * SUBALLOCATION_UNIT_SIZE) as u64
            != self.buffer.size()
        {
            panic!("Memory leak detected in suballocated buffer");
        }
        ctx.allocator.dispose_buffer(&ctx.device, self.buffer);
    }
}

impl Deref for SuballocatedBuffer {
    type Target = AllocatedBuffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for SuballocatedBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}
