use std::{fmt::Debug, mem::ManuallyDrop, sync::Arc};

use ash::vk;

use crate::DeviceShared;

pub trait VkWrappable: Debug + Copy + Clone {
    fn destroy(device: &DeviceShared, handle: Self);

    fn wrap(self, shared: &Arc<DeviceShared>) -> VkWrap<Self> {
        VkWrap::new(self, shared)
    }
}

type WrapBuffer = VkWrap<vk::Buffer>;
impl VkWrappable for vk::Buffer {
    fn destroy(shared: &DeviceShared, handle: Self) {
        unsafe { shared.device.destroy_buffer(handle, None) }
    }
}

type WrapImage = VkWrap<vk::Image>;
impl VkWrappable for vk::Image {
    fn destroy(shared: &DeviceShared, handle: Self) {
        unsafe { shared.device.destroy_image(handle, None) }
    }
}

#[derive(Clone)]
pub struct VkWrap<T>
where
    T: VkWrappable,
{
    ref_count: ManuallyDrop<Arc<()>>,
    shared: Arc<DeviceShared>,
    raw: T,
}

impl<T: VkWrappable> std::fmt::Debug for VkWrap<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VkWrap")
            .field("ref_count", &Arc::strong_count(&self.ref_count))
            .field("raw", &self.raw)
            .finish()
    }
}

impl<T> VkWrap<T>
where
    T: VkWrappable,
{
    pub fn new(raw: T, shared: &Arc<DeviceShared>) -> Self {
        Self {
            ref_count: ManuallyDrop::new(Arc::new(())),
            shared: shared.clone(),
            raw,
        }
    }

    pub fn raw(&self) -> T {
        self.raw
    }
}

impl<T> Drop for VkWrap<T>
where
    T: VkWrappable,
{
    fn drop(&mut self) {
        let arc = unsafe { ManuallyDrop::take(&mut self.ref_count) };
        if let Some(()) = Arc::into_inner(arc) {
            T::destroy(&self.shared, self.raw);
        }
    }
}
