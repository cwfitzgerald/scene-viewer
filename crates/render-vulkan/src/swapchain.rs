use anyhow::Context;
use ash::{khr, vk};
use glam::UVec2;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

#[derive(Clone, Copy)]
pub struct SwapchainSemaphores {
    pub acquire: vk::Semaphore,
    pub present: vk::Semaphore,
}

struct PerImageData {
    image: vk::Image,
    view: vk::ImageView,
    semaphores: SwapchainSemaphores,
}

pub struct NativeSwapchain {
    surface_fn: khr::surface::Instance,
    surface: vk::SurfaceKHR,

    swapchain_fn: khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,

    image_data: Vec<PerImageData>,

    format: vk::Format,

    current_image: u32,
}

impl NativeSwapchain {
    pub fn new(
        entry: &ash::Entry,
        instance: &ash::Instance,
        device: &ash::Device,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
        resolution: UVec2,
    ) -> anyhow::Result<Self> {
        unsafe {
            let surface_fn = khr::surface::Instance::new(&entry, &instance);

            let surface =
                ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)
                    .context("Failed to create Vulkan surface")?;

            let format = vk::Format::R8G8B8A8_SRGB;

            let swapchain_fn = khr::swapchain::Device::new(&instance, &device);

            let mut this = Self {
                surface_fn,
                surface,

                swapchain_fn,
                swapchain: vk::SwapchainKHR::null(),

                image_data: Vec::new(),

                format,
                current_image: 0,
            };

            this.configure(device, resolution)?;

            Ok(this)
        }
    }

    fn configure(&mut self, device: &ash::Device, resolution: UVec2) -> anyhow::Result<()> {
        unsafe {
            println!("Configuring swapchain with resolution: {:?}", resolution);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(self.surface)
                .min_image_count(3)
                .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                .image_format(self.format)
                .image_extent(vk::Extent2D {
                    width: resolution.x,
                    height: resolution.y,
                })
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(vk::PresentModeKHR::FIFO)
                .clipped(true)
                .image_array_layers(1);

            self.swapchain = self
                .swapchain_fn
                .create_swapchain(&swapchain_create_info, None)
                .context("Failed to create Vulkan swapchain")?;

            let images = self
                .swapchain_fn
                .get_swapchain_images(self.swapchain)
                .context("Failed to get swapchain images")?;

            for (idx, image) in images.into_iter().enumerate() {
                let create_view_info = vk::ImageViewCreateInfo::default()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_SRGB)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(image);
                let view = device
                    .create_image_view(&create_view_info, None)
                    .with_context(|| {
                        format!("Failed to create image view for swapchain image {}", idx)
                    })?;

                let semaphores = SwapchainSemaphores {
                    acquire: device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .context("Failed to create acquire semaphore")?,
                    present: device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .context("Failed to create present semaphore")?,
                };

                self.image_data.push(PerImageData {
                    image,
                    view,
                    semaphores,
                });
            }

            Ok(())
        }
    }

    fn unconfigure(&mut self, device: &ash::Device) {
        unsafe {
            for data in self.image_data.drain(..) {
                device.destroy_image_view(data.view, None);
                // Images are owned by the swapchain, so we don't destroy them here.

                device.destroy_semaphore(data.semaphores.acquire, None);
                device.destroy_semaphore(data.semaphores.present, None);
            }

            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
        }
    }

    pub fn resize(&mut self, device: &ash::Device, resolution: UVec2) -> anyhow::Result<()> {
        self.unconfigure(device);
        self.configure(device, resolution)?;

        Ok(())
    }

    pub fn acquire(&mut self) -> anyhow::Result<(vk::ImageView, SwapchainSemaphores)> {
        unsafe {
            let data = &self.image_data[self.current_image as usize];

            let (index, _suboptimal) = self
                .swapchain_fn
                .acquire_next_image(
                    self.swapchain,
                    u64::MAX,
                    data.semaphores.acquire,
                    vk::Fence::null(),
                )
                .context("Failed to acquire next image from swapchain")?;

            assert_eq!(index, self.current_image);

            Ok((data.view, data.semaphores))
        }
    }

    pub fn present(&mut self, queue: vk::Queue) -> anyhow::Result<()> {
        unsafe {
            let data = &self.image_data[self.current_image as usize];

            let present_info = vk::PresentInfoKHR::default()
                .swapchains(std::slice::from_ref(&self.swapchain))
                .image_indices(std::slice::from_ref(&self.current_image))
                .wait_semaphores(std::slice::from_ref(&data.semaphores.present));

            self.swapchain_fn
                .queue_present(queue, &present_info)
                .context("Failed to present swapchain image")?;

            let image_count = self.image_data.len() as u32;
            self.current_image = (self.current_image + 1) % image_count;

            Ok(())
        }
    }

    pub fn dispose(&mut self, device: &ash::Device) {
        unsafe {
            self.unconfigure(device);

            self.surface_fn.destroy_surface(self.surface, None);
        }
    }
}
