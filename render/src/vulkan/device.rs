use super::contexts::*;
use super::error::*;
use super::fence::*;
use super::framebuffer::*;
use super::graphics_pipeline::*;
use super::image::*;
use super::instance::*;
use super::physical_device::*;
use super::shader::*;
use super::surface::*;

use exo::pool::Pool;

use arrayvec::ArrayVec;
use erupt::{cstr, vk, DeviceLoader, ExtendableFrom};
use gpu_alloc::{Config, GpuAllocator};
use std::ffi::CString;
use std::os::raw::c_char;

const VK_KHR_SWAPCHAIN_EXTENSION_NAME: *const c_char = cstr!("VK_KHR_swapchain");

pub struct DeviceSpec<'a> {
    pub physical_device: &'a mut PhysicalDevice,
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
    pub framebuffers: Pool<Framebuffer>,
    pub shaders: Pool<Shader>,
    pub graphics_programs: Pool<GraphicsProgram>,
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
            .enabled_extension_names(&device_extensions)
            .extend_from(&mut spec.physical_device.features);

        let device = unsafe {
            let device_info = device_info.build_dangling();

            let mut base_struct = &device_info as *const _ as *const vk::BaseInStructure;
            loop {
                if base_struct.is_null() {
                    break;
                }

                eprintln!("stype: {:?}", (*base_struct).s_type);
                base_struct = (*base_struct).p_next as *const vk::BaseInStructure;
            }

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
            framebuffers: Pool::new(),
            shaders: Pool::new(),
            graphics_programs: Pool::new(),
        })
    }

    pub fn destroy(self) {
        unsafe { self.device.destroy_device(None) };
    }

    pub fn submit(
        &self,
        context: &dyn HasBaseContext,
        signal_fences: &[&Fence],
        signal_values: &[u64],
    ) -> VulkanResult<()> {
        let context = context.base_context();

        let mut signal_list = ArrayVec::<vk::Semaphore, 4>::new();
        let mut local_signal_values = ArrayVec::<u64, 4>::new();
        for fence in signal_fences {
            signal_list.push(fence.timeline_semaphore);
        }

        for value in signal_values {
            local_signal_values.push(*value);
        }

        if let Some(semaphore) = context.can_present_semaphore {
            signal_list.push(semaphore);
            local_signal_values.push(0);
        }

        let mut semaphore_list = ArrayVec::<vk::Semaphore, { MAX_SEMAPHORES + 1 }>::new();
        let mut value_list = ArrayVec::<u64, { MAX_SEMAPHORES + 1 }>::new();
        let mut stage_list = ArrayVec::<vk::PipelineStageFlags, { MAX_SEMAPHORES + 1 }>::new();

        for i in 0..context.wait_fence_list.len() {
            semaphore_list.push(context.wait_fence_list[i].timeline_semaphore);
            value_list.push(context.wait_value_list[i]);
            stage_list.push(context.wait_stage_list[i]);
        }

        if let Some(semaphore) = context.image_acquired_semaphore {
            semaphore_list.push(semaphore);
            value_list.push(0);
            stage_list.push(context.image_acquired_stage.unwrap());
        }

        let mut timeline_info = vk::TimelineSemaphoreSubmitInfoBuilder::new()
            .wait_semaphore_values(&value_list)
            .signal_semaphore_values(&local_signal_values);

        let command_buffers = [context.cmd];

        let submit_info = vk::SubmitInfoBuilder::new()
            .extend_from(&mut timeline_info)
            .wait_semaphores(&semaphore_list)
            .wait_dst_stage_mask(&stage_list)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_list);

        unsafe {
            self.device
                .queue_submit(context.queue, &[submit_info], vk::Fence::null())
                .result()?;
        }

        Ok(())
    }

    pub fn acquire_next_swapchain(&self, surface: &mut Surface) -> VulkanResult<bool> {
        surface.previous_image = surface.current_image;

        let res = unsafe {
            self.device.acquire_next_image_khr(
                surface.swapchain,
                0,
                surface.image_acquired_semaphores[surface.current_image as usize],
                vk::Fence::null(),
            )
        };

        if let Some(next_image) = res.value {
            surface.current_image = next_image;
        }

        match res.raw {
            vk::Result::SUCCESS => Ok(false),
            vk::Result::SUBOPTIMAL_KHR | vk::Result::ERROR_OUT_OF_DATE_KHR => Ok(true),
            _ => Err(VulkanError::from(res.raw)),
        }
    }

    pub fn present(&self, context: &dyn HasBaseContext, surface: &Surface) -> VulkanResult<bool> {
        let context = context.base_context();

        let wait_semaphores = [surface.can_present_semaphores[surface.current_image as usize]];
        let swapchains = [surface.swapchain];
        let image_indices = [surface.current_image];

        let present_info = vk::PresentInfoKHRBuilder::new()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let res = unsafe { self.device.queue_present_khr(context.queue, &present_info) };

        match res.raw {
            vk::Result::SUCCESS => Ok(false),
            vk::Result::SUBOPTIMAL_KHR | vk::Result::ERROR_OUT_OF_DATE_KHR => Ok(true),
            _ => Err(VulkanError::from(res.raw)),
        }
    }

    pub fn set_vk_name(
        &self,
        raw_handle: u64,
        object_type: vk::ObjectType,
        name: &str,
    ) -> VulkanResult<()> {
        let name = CString::new(name).unwrap();
        let name_info = vk::DebugUtilsObjectNameInfoEXTBuilder::new()
            .object_handle(raw_handle)
            .object_name(&name)
            .object_type(object_type);

        unsafe {
            self.device
                .set_debug_utils_object_name_ext(&name_info)
                .result()?;
        }

        Ok(())
    }

    pub fn wait_idle(&self) -> VulkanResult<()> {
        unsafe { self.device.device_wait_idle().result()? }
        Ok(())
    }
}
