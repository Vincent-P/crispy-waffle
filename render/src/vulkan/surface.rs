use super::device::*;
use super::error::*;
use super::image::*;
use super::instance::*;
use super::physical_device::*;

use exo::{dynamic_array::DynamicArray, pool::Handle};

use erupt::vk;
use raw_window_handle::HasRawWindowHandle;

pub const MAX_SWAPCHAIN_IMAGES: usize = 6;

type PerImage<T> = DynamicArray<T, MAX_SWAPCHAIN_IMAGES>;

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
    pub is_outdated: bool,
    pub size_requested: Option<[i32; 2]>,
}

impl Surface {
    pub fn new<WindowHandle: HasRawWindowHandle>(
        instance: &Instance,
        device: &mut Device,
        physical_device: &mut PhysicalDevice,
        window_handle: &WindowHandle,
        size_requested: Option<[i32; 2]>,
    ) -> VulkanResult<Surface> {
        let surface = unsafe {
            erupt::utils::surface::create_surface(&instance.instance, window_handle, None)
        }
        .result()?;

        let _graphics_present_support = unsafe {
            instance.instance.get_physical_device_surface_support_khr(
                physical_device.device,
                device.graphics_family_idx,
                surface,
            )
        };

        let present_modes = unsafe {
            instance
                .instance
                .get_physical_device_surface_present_modes_khr(
                    physical_device.device,
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
                .get_physical_device_surface_formats_khr(physical_device.device, surface, None)
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
            images: DynamicArray::new(),
            image_acquired_semaphores: DynamicArray::new(),
            can_present_semaphores: DynamicArray::new(),
            is_outdated: false,
            size_requested: size_requested,
        };

        surface.create_swapchain(instance, device, physical_device)?;

        Ok(surface)
    }

    pub fn destroy(&mut self, instance: &Instance, device: &mut Device) {
        self.destroy_swapchain(device);
        unsafe {
            instance.instance.destroy_surface_khr(self.surface, None);
        }
    }

    pub fn create_swapchain(
        &mut self,
        instance: &Instance,
        device: &mut Device,
        physical_device: &mut PhysicalDevice,
    ) -> VulkanResult<()> {
        let capabilities = unsafe {
            instance
                .instance
                .get_physical_device_surface_capabilities_khr(physical_device.device, self.surface)
                .result()?
        };

        let has_current_extent = capabilities.current_extent.width != 0xFFFFFFFF
            && capabilities.current_extent.height != 0xFFFFFFFF;

        let size_requested = self.size_requested.take();
        if let Some(size) = size_requested {
            self.size = size;
        } else if has_current_extent {
            self.size[0] = capabilities.current_extent.width as i32;
            self.size[1] = capabilities.current_extent.height as i32;
        } else {
            eprintln!("Default swapchain size: 1024x1024");
            self.size[0] = 1024;
            self.size[1] = 1024;
        }

        let max_count = if capabilities.max_image_count == 0 {
            u32::MAX
        } else {
            capabilities.max_image_count
        };
        let image_count = (capabilities.min_image_count + 1).min(max_count);

        let image_usages = vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::STORAGE
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::TRANSFER_DST;

        let swapchain_create_info = vk::SwapchainCreateInfoKHRBuilder::new()
            .surface(self.surface)
            .min_image_count(image_count)
            .image_format(self.format.format)
            .image_color_space(self.format.color_space)
            .image_extent(vk::Extent2D {
                width: self.size[0] as u32,
                height: self.size[1] as u32,
            })
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

        let swapchain_images =
            unsafe { device.device.get_swapchain_images_khr(self.swapchain, None) }.result()?;

        assert!(self.images.is_empty());
        for i_image in 0..swapchain_images.len().min(MAX_SWAPCHAIN_IMAGES) {
            if swapchain_images[i_image] == vk::Image::null() {
                break;
            }

            self.images.push(device.create_image_proxy(
                ImageSpec {
                    name: format!("swapchain #{}", i_image),
                    size: [self.size[0], self.size[1], 1],
                    format: self.format.format,
                    usages: image_usages,
                    ..Default::default()
                },
                swapchain_images[i_image],
            )?);
        }
        assert!(!self.images.is_empty());

        let semaphore_create_info = vk::SemaphoreCreateInfoBuilder::new();
        for i in 0..self.images.len() {
            unsafe {
                self.image_acquired_semaphores.push(
                    device
                        .device
                        .create_semaphore(&semaphore_create_info, None)
                        .result()?,
                );

                let raw_handle = self.image_acquired_semaphores.back().0;
                device.set_vk_name(
                    raw_handle,
                    vk::ObjectType::SEMAPHORE,
                    &format!("swapchain image_acquired #{}", i),
                )?;

                self.can_present_semaphores.push(
                    device
                        .device
                        .create_semaphore(&semaphore_create_info, None)
                        .result()?,
                );

                let raw_handle = self.can_present_semaphores.back().0;
                device.set_vk_name(
                    raw_handle,
                    vk::ObjectType::SEMAPHORE,
                    &format!("swapchain can_present #{}", i),
                )?;
            }
        }

        Ok(())
    }

    fn destroy_swapchain(&mut self, device: &mut Device) {
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
        self.image_acquired_semaphores.clear();
        self.can_present_semaphores.clear();
    }

    pub fn recreate_swapchain(
        &mut self,
        instance: &Instance,
        device: &mut Device,
        physical_device: &mut PhysicalDevice,
    ) -> VulkanResult<()> {
        self.destroy_swapchain(device);
        self.create_swapchain(instance, device, physical_device)
    }

    pub fn current_image(&self) -> Handle<Image> {
        self.images[self.current_image as usize]
    }
}
