use std::{mem::ManuallyDrop, sync::Arc};

use anyhow::Context;
use ash::{
    khr::{surface, swapchain},
    vk,
};
use glam::UVec2;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::{DeviceShared, VulkanRenderer};

impl VulkanRenderer {
    pub fn new(
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
        resolution: UVec2,
    ) -> anyhow::Result<Self> {
        unsafe {
            let entry = ash::Entry::load().context("Failed to load Vulkan entry")?;

            let application_info = vk::ApplicationInfo::default()
                .api_version(vk::make_api_version(0, 1, 3, 0))
                .application_name(c"Scene Viewer")
                .application_version(0x1)
                .engine_name(c"cwfitzgerald renderboi")
                .engine_version(0x1);

            let instance_extensions = ash_window::enumerate_required_extensions(display_handle)
                .context("Failed to enumerate required Vulkan extensions")?;

            let instance_create_info = vk::InstanceCreateInfo::default()
                .application_info(&application_info)
                .enabled_extension_names(&instance_extensions);

            let instance = entry
                .create_instance(&instance_create_info, None)
                .context("Failed to create Vulkan instance")?;

            let surface_fn = surface::Instance::new(&entry, &instance);

            let surface =
                ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)
                    .context("Failed to create Vulkan surface")?;

            let devices = instance
                .enumerate_physical_devices()
                .context("Failed to enumerate physical devices")?;

            if devices.is_empty() {
                anyhow::bail!("No Vulkan devices found");
            }

            let mut chosen_physical = None;
            for physical in devices {
                let mut vk_13_features = vk::PhysicalDeviceVulkan13Features::default();
                let mut vk_12_features = vk::PhysicalDeviceVulkan12Features::default();
                let mut vk_11_features = vk::PhysicalDeviceVulkan11Features::default();

                let mut features = vk::PhysicalDeviceFeatures2::default()
                    .push_next(&mut vk_13_features)
                    .push_next(&mut vk_12_features)
                    .push_next(&mut vk_11_features);

                instance.get_physical_device_features2(physical, &mut features);

                let mut vk_13_properties = vk::PhysicalDeviceVulkan13Properties::default();
                let mut vk_12_properties = vk::PhysicalDeviceVulkan12Properties::default();
                let mut vk_11_properties = vk::PhysicalDeviceVulkan11Properties::default();

                let mut properties = vk::PhysicalDeviceProperties2::default()
                    .push_next(&mut vk_13_properties)
                    .push_next(&mut vk_12_properties)
                    .push_next(&mut vk_11_properties);

                instance.get_physical_device_properties2(physical, &mut properties);

                // TODO: Check for required features

                println!(
                    "Found device {} ({:?})",
                    properties
                        .properties
                        .device_name_as_c_str()
                        .unwrap()
                        .to_str()
                        .unwrap(),
                    properties.properties.device_type
                );

                if properties.properties.device_type != vk::PhysicalDeviceType::CPU {
                    chosen_physical = Some(physical);
                    break;
                }
            }
            let physical = chosen_physical
                .ok_or_else(|| anyhow::anyhow!("No suitable physical device found"))?;

            let queue_families = instance.get_physical_device_queue_family_properties(physical);

            let mut graphics_queue_family_index = None;
            for (index, family) in queue_families.iter().enumerate() {
                if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    graphics_queue_family_index = Some(index as u32);
                    break;
                }
            }
            let graphics_queue_family_index = graphics_queue_family_index
                .ok_or_else(|| anyhow::anyhow!("No suitable graphics queue family found"))?;

            let mut vk_13_features = vk::PhysicalDeviceVulkan13Features::default()
                .synchronization2(true)
                .dynamic_rendering(true)
                .maintenance4(true);

            let mut vk_12_features = vk::PhysicalDeviceVulkan12Features::default()
                .buffer_device_address(true)
                .descriptor_indexing(true)
                .descriptor_binding_sampled_image_update_after_bind(true)
                .descriptor_binding_storage_image_update_after_bind(true)
                .runtime_descriptor_array(true)
                .shader_float16(true);

            let mut vk_11_features =
                vk::PhysicalDeviceVulkan11Features::default().shader_draw_parameters(true);

            let vk_10_features = vk::PhysicalDeviceFeatures::default()
                .draw_indirect_first_instance(true)
                .texture_compression_bc(true)
                .full_draw_index_uint32(true)
                .fragment_stores_and_atomics(true)
                .sampler_anisotropy(true);

            let features = vk::PhysicalDeviceFeatures2::default()
                .features(vk_10_features)
                .push_next(&mut vk_11_features)
                .push_next(&mut vk_12_features)
                .push_next(&mut vk_13_features);

            let extension_names = [swapchain::NAME.as_ptr()];

            let queue_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(graphics_queue_family_index)
                .queue_priorities(&[1.0]);

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(std::slice::from_ref(&queue_info))
                .enabled_features(&features.features)
                .enabled_extension_names(&extension_names);

            let device = instance
                .create_device(physical, &device_create_info, None)
                .context("Failed to create Vulkan device")?;

            let queue = device.get_device_queue(graphics_queue_family_index, 0);

            let swapchain_fn = swapchain::Device::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(surface)
                .min_image_count(3)
                .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                .image_format(vk::Format::R8G8B8A8_UNORM)
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

            let swapchain = swapchain_fn
                .create_swapchain(&swapchain_create_info, None)
                .context("Failed to create Vulkan swapchain")?;

            Ok(VulkanRenderer {
                _entry: entry,
                instance,
                surface_fn,
                surface,
                swapchain_fn,
                swapchain,
                shared: ManuallyDrop::new(Arc::new(DeviceShared { device, queue })),
            })
        }
    }
}
