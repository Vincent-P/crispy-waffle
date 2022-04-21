use super::device::*;
use super::error::*;
use super::image::*;
use super::instance::*;

use exo::pool::Handle;

use arrayvec::ArrayVec;
use erupt::vk;
use raw_window_handle::HasRawWindowHandle;

const MAX_SWAPCHAIN_IMAGES: usize = 4;

type PerImage<T> = ArrayVec<T, MAX_SWAPCHAIN_IMAGES>;

pub struct Surface {
    pub surface: vk::SurfaceKHR,
    pub swapchain: vk::SwapchainKHR,
    pub present_mode: vk::PresentModeKHR,
    pub format: vk::SurfaceFormatKHR,
    pub size: [i32; 2],
    pub current_image: u32,
    pub previous_image: u32,
    pub images: PerImage<Handle<Image>>,
    pub image_acquired_semaphores: PerImage<vk::Semaphore>,
    pub can_present_semaphores: PerImage<vk::Semaphore>,
}

impl Surface {
    pub fn new(
        instance: &Instance,
        device: &mut Device,
        window_handle: &impl HasRawWindowHandle,
    ) -> VulkanResult<Surface> {
        let surface = unsafe {
            erupt::utils::surface::create_surface(&instance.instance, window_handle, None)
        }
        .result()?;

        let graphics_present_support = unsafe {
            instance.instance.get_physical_device_surface_support_khr(
                device.spec.physical_device.device,
                device.graphics_family_idx,
                surface,
            )
        };

        let present_modes = unsafe {
            instance
                .instance
                .get_physical_device_surface_present_modes_khr(
                    device.spec.physical_device.device,
                    surface,
                    None,
                )
                .result()?
        };

        let present_mode = present_modes
            .iter()
            .find(|&&m| m == vk::PresentModeKHR::MAILBOX_KHR)
            .or_else(|| {
                present_modes
                    .iter()
                    .find(|&&m| m == vk::PresentModeKHR::IMMEDIATE_KHR)
            })
            .copied()
            .unwrap_or(vk::PresentModeKHR::FIFO_KHR);

        let surface_formats = unsafe {
            instance
                .instance
                .get_physical_device_surface_formats_khr(
                    device.spec.physical_device.device,
                    surface,
                    None,
                )
                .result()?
        };

        let mut format = surface_formats[0];
        if format.format == vk::Format::UNDEFINED {
            format.format = vk::Format::B8G8R8A8_UNORM;
            format.color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR_KHR;
        } else {
            for surface_format in surface_formats {
                if surface_format.format == vk::Format::B8G8R8A8_UNORM
                    && surface_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR_KHR
                {
                    format = surface_format;
                    break;
                }
            }
        }

        let mut surface = Surface {
            surface,
            swapchain: vk::SwapchainKHR::null(),
            present_mode,
            format,
            size: [0, 0],
            current_image: 0,
            previous_image: 0,
            images: ArrayVec::new(),
            image_acquired_semaphores: ArrayVec::new(),
            can_present_semaphores: ArrayVec::new(),
        };

        surface.create_swapchain(instance, device)?;

        Ok(surface)
    }

    pub fn destroy(mut self, instance: &Instance, device: &mut Device) {
        self.destroy_swapchain(device);
        unsafe {
            instance.instance.destroy_surface_khr(self.surface, None);
        }
    }

    pub fn create_swapchain(
        &mut self,
        instance: &Instance,
        device: &mut Device,
    ) -> VulkanResult<()> {
        let capabilities = unsafe {
            instance
                .instance
                .get_physical_device_surface_capabilities_khr(
                    device.spec.physical_device.device,
                    self.surface,
                )
                .result()?
        };
        self.size[0] = capabilities.current_extent.width as i32;
        self.size[1] = capabilities.current_extent.height as i32;

        let image_count = (capabilities.min_image_count + 1).max(capabilities.max_image_count);

        let image_usages = vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::STORAGE
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::TRANSFER_DST;

        let swapchain_create_info = vk::SwapchainCreateInfoKHRBuilder::new()
            .surface(self.surface)
            .min_image_count(image_count)
            .image_format(self.format.format)
            .image_color_space(self.format.color_space)
            .image_extent(capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(image_usages)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagBitsKHR::OPAQUE_KHR)
            .present_mode(self.present_mode)
            .clipped(true);

        self.swapchain = unsafe {
            device
                .device
                .create_swapchain_khr(&swapchain_create_info, None)
                .result()?
        };

        let swapchain_images = unsafe {
            device
                .device
                .get_swapchain_images_khr(self.swapchain, Some(MAX_SWAPCHAIN_IMAGES as u32))
        }
        .result()?;

        assert!(self.images.is_empty());
        for i_image in 0..swapchain_images.len() {
            self.images.push(device.create_image_proxy(
                ImageSpec {
                    size: [self.size[0], self.size[1], 1],
                    format: self.format.format,
                    usages: image_usages,
                    ..Default::default()
                },
                swapchain_images[i_image],
            )?);
        }

        let semaphore_create_info = vk::SemaphoreCreateInfoBuilder::new();
        for i in 0..self.images.len() {
            unsafe {
                self.image_acquired_semaphores.push(
                    device
                        .device
                        .create_semaphore(&semaphore_create_info, None)
                        .result()?,
                );
                self.can_present_semaphores.push(
                    device
                        .device
                        .create_semaphore(&semaphore_create_info, None)
                        .result()?,
                );
            }
        }

        Ok(())
    }

    pub fn destroy_swapchain(&mut self, device: &mut Device) {
        for &image in &self.images {
            device.destroy_image(image);
        }
        self.images.clear();

        for &semaphore in &self.image_acquired_semaphores {
            unsafe {
                device.device.destroy_semaphore(semaphore, None);
            }
        }

        for &semaphore in &self.can_present_semaphores {
            unsafe {
                device.device.destroy_semaphore(semaphore, None);
            }
        }

        unsafe { device.device.destroy_swapchain_khr(self.swapchain, None) }
        self.swapchain = vk::SwapchainKHR::null();
    }
}
