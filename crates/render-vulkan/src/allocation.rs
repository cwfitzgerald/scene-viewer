use anyhow::Context;
use ash::vk;
use bytemuck::Pod;
use gpu_allocator::{
    AllocationSizes, AllocatorDebugSettings,
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc},
};
use parking_lot::Mutex;

pub struct AllocatedBuffer {
    buffer: vk::Buffer,
    allocation: Allocation,
    device_address: Option<vk::DeviceAddress>,
}

impl AllocatedBuffer {
    pub fn size(&self) -> u64 {
        self.allocation.size()
    }

    #[expect(dead_code)]
    pub fn mapped_slice<T: Pod>(&self) -> &[T] {
        let raw_slice = self.allocation.mapped_slice().expect("Failed to get mapped slice");

        bytemuck::cast_slice(raw_slice)
    }

    pub fn mapped_slice_mut<T: Pod>(&mut self) -> &mut [T] {
        let raw_slice = self.allocation.mapped_slice_mut().expect("Failed to get mapped slice");

        bytemuck::cast_slice_mut(raw_slice)
    }

    pub fn device_address(&self) -> vk::DeviceAddress {
        self.device_address
            .expect("Tried to get device address from non-device address capable buffer")
    }

    pub fn buffer(&self) -> vk::Buffer {
        self.buffer
    }
}

pub struct GpuMemoryAllocator {
    allocator: Mutex<Allocator>,
}

impl GpuMemoryAllocator {
    pub fn new(
        instance: ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: ash::Device,
    ) -> anyhow::Result<Self> {
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance,
            physical_device,
            device,
            debug_settings: AllocatorDebugSettings::default(),
            buffer_device_address: true,
            allocation_sizes: AllocationSizes::default(),
        })?;

        Ok(Self { allocator: Mutex::new(allocator) })
    }

    pub fn allocate_buffer(
        &self,
        device: &ash::Device,
        name: &str,
        size: u64,
        usages: vk::BufferUsageFlags,
        location: gpu_allocator::MemoryLocation,
    ) -> anyhow::Result<AllocatedBuffer> {
        unsafe {
            let buffer_create_info = vk::BufferCreateInfo::default()
                .size(size)
                .usage(usages)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let buffer = device
                .create_buffer(&buffer_create_info, None)
                .context("Failed to create buffer")?;

            let memory_requirements = device.get_buffer_memory_requirements(buffer);

            let memory_info = AllocationCreateDesc {
                name,
                requirements: memory_requirements,
                location,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            };

            let allocation = self
                .allocator
                .lock()
                .allocate(&memory_info)
                .context("Failed to allocate buffer memory")?;

            let bind_memory_info = vk::BindBufferMemoryInfo::default()
                .buffer(buffer)
                .memory(allocation.memory())
                .memory_offset(allocation.offset());

            device
                .bind_buffer_memory2(std::slice::from_ref(&bind_memory_info))
                .context("Failed to bind memory")?;

            let device_address = if usages.contains(vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS) {
                let device_address_info = vk::BufferDeviceAddressInfoEXT::default().buffer(buffer);
                Some(device.get_buffer_device_address(&device_address_info))
            } else {
                None
            };

            Ok(AllocatedBuffer { buffer, allocation, device_address })
        }
    }

    pub fn dispose_buffer(&self, device: &ash::Device, buffer: AllocatedBuffer) {
        unsafe {
            device.destroy_buffer(buffer.buffer, None);
            self.allocator.lock().free(buffer.allocation).unwrap();
        }
    }

    pub fn dispose(self) {
        drop(self.allocator);
    }
}
