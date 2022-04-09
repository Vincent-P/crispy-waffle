use super::error::*;
use super::image::*;
use super::instance::*;
use super::physical_device::*;

use exo::pool::Pool;

use arrayvec::ArrayVec;
use erupt::{cstr, vk, DeviceLoader};

use gpu_alloc::{Config, GpuAllocator};

use std::os::raw::c_char;

const VK_KHR_SWAPCHAIN_EXTENSION_NAME: *const c_char = cstr!("VK_KHR_swapchain");

pub struct DeviceSpec<'a> {
    pub physical_device: &'a PhysicalDevice,
    pub push_constant_size: usize,
}

pub struct Device<'a> {
    pub device: Box<DeviceLoader>,
    pub spec: DeviceSpec<'a>,
    pub allocator: GpuAllocator<vk::DeviceMemory>,
    pub graphics_family_idx: u32,
    pub compute_family_idx: u32,
    pub transfer_family_idx: u32,
    pub images: Pool<Image>,
}

impl<'a> Device<'a> {
    pub fn new(instance: &'a Instance, spec: DeviceSpec<'a>) -> VulkanResult<Self> {
        let mut device_extensions = ArrayVec::<_, 8>::new();
        device_extensions.push(VK_KHR_SWAPCHAIN_EXTENSION_NAME);

        let queue_families = unsafe {
            instance
                .instance
                .get_physical_device_queue_family_properties(spec.physical_device.device, None)
        };

        let mut queue_create_infos = ArrayVec::<_, 8>::new();
        let mut graphics_family_idx = None;
        let mut compute_family_idx = None;
        let mut transfer_family_idx = None;

        for i in 0..queue_families.len() {
            let queue_family = &queue_families[i];

            if queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                if graphics_family_idx.is_none() {
                    graphics_family_idx = Some(i as u32);
                    queue_create_infos.push(
                        vk::DeviceQueueCreateInfoBuilder::new()
                            .queue_family_index(i as u32)
                            .queue_priorities(&[0.0]),
                    );
                }
            } else if queue_family.queue_flags.contains(vk::QueueFlags::COMPUTE) {
                if compute_family_idx.is_none() {
                    compute_family_idx = Some(i as u32);
                    queue_create_infos.push(
                        vk::DeviceQueueCreateInfoBuilder::new()
                            .queue_family_index(i as u32)
                            .queue_priorities(&[0.0]),
                    );
                }
            } else if queue_family.queue_flags.contains(vk::QueueFlags::TRANSFER) {
                if transfer_family_idx.is_none() {
                    transfer_family_idx = Some(i as u32);
                    queue_create_infos.push(
                        vk::DeviceQueueCreateInfoBuilder::new()
                            .queue_family_index(i as u32)
                            .queue_priorities(&[0.0]),
                    );
                }
            }
        }

        let graphics_family_idx =
            graphics_family_idx.ok_or(VulkanError::MissingQueue(vk::QueueFlags::GRAPHICS))?;
        let compute_family_idx =
            compute_family_idx.ok_or(VulkanError::MissingQueue(vk::QueueFlags::COMPUTE))?;
        let transfer_family_idx =
            transfer_family_idx.ok_or(VulkanError::MissingQueue(vk::QueueFlags::TRANSFER))?;

        let device_info = vk::DeviceCreateInfoBuilder::new()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extensions);

        let device = unsafe {
            DeviceLoader::new(
                &instance.instance,
                spec.physical_device.device,
                &device_info,
            )
        }
        .unwrap();
        let device = Box::new(device);

        let props = unsafe {
            gpu_alloc_erupt::device_properties(&instance.instance, spec.physical_device.device)
        }?;
        let config = Config::i_am_prototyping();
        let allocator = GpuAllocator::new(config, props);

        Ok(Device {
            device,
            spec,
            allocator,
            graphics_family_idx,
            compute_family_idx,
            transfer_family_idx,
            images: Pool::new(),
        })
    }

    pub fn destroy(self) {
        unsafe { self.device.destroy_device(None) };
    }
}
