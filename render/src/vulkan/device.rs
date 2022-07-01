use super::buffer::*;
use super::compute_pipeline::*;
use super::contexts::*;
use super::descriptor_set::*;
use super::error::*;
use super::fence::*;
use super::framebuffer::*;
use super::graphics_pipeline::*;
use super::image::*;
use super::instance::*;
use super::memory;
use super::physical_device::*;
use super::shader::*;
use super::surface::*;

use exo::{dynamic_array::DynamicArray, pool::Pool};

use erupt::{cstr, vk, DeviceLoader, ExtendableFrom};
use std::ffi::CString;
use std::os::raw::c_char;

const VK_KHR_SWAPCHAIN_EXTENSION_NAME: *const c_char = cstr!("VK_KHR_swapchain");

pub struct DeviceSpec {
    pub push_constant_size: usize,
}

pub struct DeviceDescriptors {
    pub uniform_descriptor_pool: vk::DescriptorPool,
    pub uniform_descriptor_layout: vk::DescriptorSetLayout,
    pub uniform_descriptor_sets: Vec<DynamicBufferDescriptor>,
    pub bindless_set: BindlessSet,
    pub pipeline_layout: vk::PipelineLayout,
}

pub struct Device {
    pub device: Box<DeviceLoader>,
    pub spec: DeviceSpec,
    pub allocator: memory::Allocator,
    pub graphics_family_idx: u32,
    pub compute_family_idx: u32,
    pub transfer_family_idx: u32,
    pub images: Pool<Image>,
    pub buffers: Pool<Buffer>,
    pub framebuffers: Pool<Framebuffer>,
    pub shaders: Pool<Shader>,
    pub descriptors: DeviceDescriptors,
    pub graphics_programs: Pool<GraphicsProgram>,
    pub compute_programs: Pool<ComputeProgram>,
    pub sampler: vk::Sampler,
}

impl Device {
    #[allow(clippy::collapsible_if)]
    pub fn new(
        instance: &Instance,
        spec: DeviceSpec,
        physical_device: &mut PhysicalDevice,
    ) -> VulkanResult<Self> {
        let mut device_extensions = DynamicArray::<_, 8>::new();
        device_extensions.push(VK_KHR_SWAPCHAIN_EXTENSION_NAME);

        let queue_families = unsafe {
            instance
                .instance
                .get_physical_device_queue_family_properties(physical_device.device, None)
        };

        let mut queue_create_infos = DynamicArray::<_, 8>::new();
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
            .extend_from(&mut physical_device.features);

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

            DeviceLoader::new(&instance.instance, physical_device.device, &device_info)
        }
        .unwrap();
        let device = Box::new(device);

        let allocator = unsafe {
            memory::Allocator::new(
                &instance.instance,
                physical_device.device,
                &vk_alloc::AllocatorDescriptor {
                    ..Default::default()
                },
            )
            .unwrap()
        };

        let bindless_set = BindlessSet::new(&device, 1024, 1024, 1024)?;

        let uniform_descriptor_pool = {
            let pool_sizes = [vk::DescriptorPoolSizeBuilder::new()
                ._type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                .descriptor_count(16)];
            let pool_info = vk::DescriptorPoolCreateInfoBuilder::new()
                .pool_sizes(&pool_sizes)
                .max_sets(16);
            unsafe { device.create_descriptor_pool(&pool_info, None).result()? }
        };

        let uniform_descriptor_layout = DynamicBufferDescriptor::new_layout(&device)?;
        let pipeline_layout = {
            let push_constant_ranges = [vk::PushConstantRangeBuilder::new()
                .stage_flags(vk::ShaderStageFlags::ALL)
                .size(spec.push_constant_size as u32)];

            let layouts = [
                bindless_set.vklayout,
                uniform_descriptor_layout,
                uniform_descriptor_layout,
            ];

            let pipeline_layout_info = vk::PipelineLayoutCreateInfoBuilder::new()
                .set_layouts(&layouts)
                .push_constant_ranges(&push_constant_ranges);

            unsafe {
                device
                    .create_pipeline_layout(&pipeline_layout_info, None)
                    .result()?
            }
        };

        let sampler = unsafe {
            let sampler_info = vk::SamplerCreateInfoBuilder::new();
            device.create_sampler(&sampler_info, None).result()?
        };

        let mut device = Device {
            device,
            spec,
            allocator,
            graphics_family_idx,
            compute_family_idx,
            transfer_family_idx,
            images: Pool::new(),
            buffers: Pool::new(),
            framebuffers: Pool::new(),
            shaders: Pool::new(),
            descriptors: DeviceDescriptors {
                uniform_descriptor_pool,
                uniform_descriptor_layout,
                uniform_descriptor_sets: Vec::new(),
                bindless_set,
                pipeline_layout,
            },
            graphics_programs: Pool::new(),
            compute_programs: Pool::new(),
            sampler,
        };

        // Empty image for bindless clear #0
        device
            .create_image(ImageSpec {
                name: String::from("empty"),
                usages: vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::STORAGE,

                ..Default::default()
            })
            .unwrap();

        Ok(device)
    }

    pub fn destroy(self) {
        unsafe { self.device.destroy_device(None) };
    }

    pub fn submit<Context: AsRef<TransferContext>>(
        &self,
        context: &Context,
        signal_fences: &[&Fence],
        signal_values: &[u64],
    ) -> VulkanResult<()> {
        let context = context.as_ref().base_context();

        let mut signal_list = DynamicArray::<vk::Semaphore, 4>::new();
        let mut local_signal_values = DynamicArray::<u64, 4>::new();
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

        let mut semaphore_list = DynamicArray::<vk::Semaphore, { MAX_SEMAPHORES + 1 }>::new();
        let mut value_list = DynamicArray::<u64, { MAX_SEMAPHORES + 1 }>::new();
        let mut stage_list = DynamicArray::<vk::PipelineStageFlags, { MAX_SEMAPHORES + 1 }>::new();

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

    pub fn present<Context: AsRef<TransferContext>>(
        &self,
        context: &Context,
        surface: &Surface,
    ) -> VulkanResult<bool> {
        let context = context.as_ref().base_context();

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

    pub fn update_bindless_set(&mut self) {
        let bindless_set = &mut self.descriptors.bindless_set;

        let total_bind_count = bindless_set
            .pending_binds
            .iter()
            .fold(0, |r, arr| r + arr.len());
        let total_unbind_count = bindless_set
            .pending_unbinds
            .iter()
            .fold(0, |r, arr| r + arr.len());

        if total_bind_count == 0 && total_unbind_count == 0 {
            return;
        }

        let mut descriptor_writes: Vec<vk::WriteDescriptorSetBuilder> = vec![];
        let mut descriptor_copies: Vec<vk::CopyDescriptorSetBuilder> = vec![];
        descriptor_writes.reserve(total_bind_count);
        descriptor_copies.reserve(total_unbind_count);

        let mut image_infos: Vec<vk::DescriptorImageInfoBuilder> = vec![];
        let mut buffer_infos: Vec<vk::DescriptorBufferInfoBuilder> = vec![];
        image_infos.reserve(
            bindless_set.pending_binds[PER_SAMPLER].len()
                + bindless_set.pending_binds[PER_IMAGE].len(),
        );
        buffer_infos.reserve(bindless_set.pending_binds[PER_BUFFER].len());

        // Hack for borrow checker
        let mut writes_indirection: Vec<(usize, usize, bool)> = vec![];

        let descriptor_types: [vk::DescriptorType; BINDLESS_SETS] = [
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            vk::DescriptorType::STORAGE_IMAGE,
            vk::DescriptorType::STORAGE_BUFFER,
        ];

        for (i_set, descriptor_type) in descriptor_types.into_iter().enumerate() {
            let image_layout = if descriptor_type == vk::DescriptorType::COMBINED_IMAGE_SAMPLER {
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
            } else {
                vk::ImageLayout::GENERAL
            };

            for to_bind in &bindless_set.pending_binds[i_set] {
                assert!(*to_bind < std::u32::MAX as usize);
                descriptor_writes.push(
                    vk::WriteDescriptorSetBuilder::new()
                        .dst_set(bindless_set.vkset)
                        .dst_binding(i_set as u32)
                        .dst_array_element(*to_bind as u32)
                        .descriptor_type(descriptor_types[i_set]),
                );

                match i_set {
                    PER_SAMPLER => {
                        let image_handle = bindless_set.sampler_images[*to_bind];
                        let image = self.images.get(image_handle);
                        let i_info = image_infos.len();
                        image_infos.push(
                            vk::DescriptorImageInfoBuilder::new()
                                .sampler(self.sampler)
                                .image_view(image.full_view.vkhandle)
                                .image_layout(image_layout),
                        );
                        writes_indirection.push((i_info, i_info + 1, true));
                    }
                    PER_IMAGE => {
                        let image_handle = bindless_set.sampler_images[*to_bind];
                        let image = self.images.get(image_handle);
                        let i_info = image_infos.len();
                        image_infos.push(
                            vk::DescriptorImageInfoBuilder::new()
                                .sampler(self.sampler)
                                .image_view(image.full_view.vkhandle)
                                .image_layout(image_layout),
                        );
                        writes_indirection.push((i_info, i_info + 1, true));
                    }
                    PER_BUFFER => {
                        let buffer_handle = bindless_set.storage_buffers[*to_bind];
                        let buffer = self.buffers.get(buffer_handle);
                        let i_info = buffer_infos.len();
                        buffer_infos.push(
                            vk::DescriptorBufferInfoBuilder::new()
                                .buffer(buffer.vkhandle)
                                .range(buffer.spec.size as u64),
                        );
                        writes_indirection.push((i_info, i_info + 1, false));
                    }
                    _ => unreachable!(),
                }
            }

            for to_unbind in &bindless_set.pending_unbinds[i_set] {
                assert!(*to_unbind < std::u32::MAX as usize);
                if bindless_set.pending_binds[i_set].contains(to_unbind) {
                    continue;
                }

                descriptor_copies.push(
                    vk::CopyDescriptorSetBuilder::new()
                        .src_set(bindless_set.vkset)
                        .src_binding(i_set as u32)
                        .src_array_element(0)
                        .dst_set(bindless_set.vkset)
                        .dst_binding(i_set as u32)
                        .dst_array_element(*to_unbind as u32)
                        .descriptor_count(1),
                );
            }

            bindless_set.pending_binds[i_set].clear();
            bindless_set.pending_unbinds[i_set].clear();
        }

        for (i, &(start, end, is_image)) in writes_indirection.iter().enumerate() {
            if is_image {
                descriptor_writes[i] = descriptor_writes[i].image_info(&image_infos[start..end]);
            } else {
                descriptor_writes[i] = descriptor_writes[i].buffer_info(&buffer_infos[start..end]);
            }
        }

        unsafe {
            self.device
                .update_descriptor_sets(&descriptor_writes, &descriptor_copies);
        }
    }
}
