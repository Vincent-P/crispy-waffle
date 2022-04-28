use exo::pool::Handle;

use super::buffer::*;
use super::device::*;
use super::error::*;
use super::image::*;

use arrayvec::ArrayVec;
use erupt::{vk, DeviceLoader, ExtendableFrom};

pub struct DynamicBufferDescriptor {
    pub buffer: Handle<Buffer>,
    pub vkset: vk::DescriptorSet,
    pub dynamic_offset: u32,
    pub size: u32,
}

type PerSet<T> = [T; 3];
const PER_SAMPLER: usize = 0;
const PER_IMAGE: usize = 1;
const PER_BUFFER: usize = 2;

pub struct BindlessSet {
    pub vkpool: vk::DescriptorPool,
    pub vklayout: vk::DescriptorSetLayout,
    pub vkset: vk::DescriptorSet,
    sampler_images: Vec<Handle<Image>>,
    storage_images: Vec<Handle<Image>>,
    storage_buffers: Vec<Handle<Buffer>>,
    free_lists: PerSet<Vec<usize>>,
    pending_binds: PerSet<Vec<usize>>,
    pending_unbinds: PerSet<Vec<usize>>,
}

impl BindlessSet {
    pub fn new(
        device: &DeviceLoader,
        sampler_count: u32,
        image_count: u32,
        buffer_count: u32,
    ) -> VulkanResult<Self> {
        let pool_sizes = [
            vk::DescriptorPoolSizeBuilder::new()
                ._type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(sampler_count),
            vk::DescriptorPoolSizeBuilder::new()
                ._type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(image_count),
            vk::DescriptorPoolSizeBuilder::new()
                ._type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(buffer_count),
        ];
        let pool_info = vk::DescriptorPoolCreateInfoBuilder::new()
            .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
            .pool_sizes(&pool_sizes)
            .max_sets(3);
        let vkpool = unsafe { device.create_descriptor_pool(&pool_info, None).result()? };

        let mut bindings = ArrayVec::<vk::DescriptorSetLayoutBindingBuilder, 3>::new();
        let mut flags = ArrayVec::<vk::DescriptorBindingFlags, 3>::new();

        bindings.push(
            vk::DescriptorSetLayoutBindingBuilder::new()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(sampler_count)
                .stage_flags(vk::ShaderStageFlags::ALL),
        );
        flags.push(
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
                | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
        );

        bindings.push(
            vk::DescriptorSetLayoutBindingBuilder::new()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(image_count)
                .stage_flags(vk::ShaderStageFlags::ALL),
        );
        flags.push(
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
                | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
        );

        bindings.push(
            vk::DescriptorSetLayoutBindingBuilder::new()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(buffer_count)
                .stage_flags(vk::ShaderStageFlags::ALL),
        );
        flags.push(
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
                | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
        );

        let mut flags_info =
            vk::DescriptorSetLayoutBindingFlagsCreateInfoBuilder::new().binding_flags(&flags);
        let layout_info = vk::DescriptorSetLayoutCreateInfoBuilder::new()
            .extend_from(&mut flags_info)
            .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
            .bindings(&bindings);

        let vklayout = unsafe {
            device
                .create_descriptor_set_layout(&layout_info, None)
                .result()?
        };

        let vklayouts = [vklayout];
        let set_info = vk::DescriptorSetAllocateInfoBuilder::new()
            .descriptor_pool(vkpool)
            .set_layouts(&vklayouts);

        let vkset = unsafe { device.allocate_descriptor_sets(&set_info).result()? }[0];

        let mut free_lists: PerSet<Vec<usize>> = [
            (1..(sampler_count as usize) + 1).collect(),
            (1..(image_count as usize) + 1).collect(),
            (1..(buffer_count as usize) + 1).collect(),
        ];
        free_lists[PER_SAMPLER][(sampler_count as usize) - 1] = !0usize;
        free_lists[PER_IMAGE][(image_count as usize) - 1] = !0usize;
        free_lists[PER_BUFFER][(buffer_count as usize) - 1] = !0usize;

        Ok(Self {
            vkpool,
            vklayout,
            vkset,
            sampler_images: vec![Handle::<Image>::invalid(); sampler_count as usize],
            storage_images: vec![Handle::<Image>::invalid(); image_count as usize],
            storage_buffers: vec![Handle::<Buffer>::invalid(); buffer_count as usize],
            free_lists,
            pending_binds: [vec![], vec![], vec![]],
            pending_unbinds: [vec![], vec![], vec![]],
        })
    }

    pub fn destroy(&mut self, device: &DeviceLoader) {
        unsafe {
            device.destroy_descriptor_pool(self.vkpool, None);
            device.destroy_descriptor_set_layout(self.vklayout, None);
        }
        self.vkpool = vk::DescriptorPool::null();
        self.vklayout = vk::DescriptorSetLayout::null();
        self.vkset = vk::DescriptorSet::null();
        self.sampler_images.clear();
        self.storage_images.clear();
        self.storage_buffers.clear();
        for free_list in &mut self.free_lists {
            free_list.clear();
        }
        for pending_bind in &mut self.pending_binds {
            pending_bind.clear();
        }
        for pending_unbind in &mut self.pending_unbinds {
            pending_unbind.clear();
        }
    }

    pub fn bind_sampler_image(&mut self, image_handle: Handle<Image>) -> usize {
        let new_index = self.free_lists[PER_SAMPLER].pop().unwrap();
        self.sampler_images[new_index] = image_handle;
        self.pending_binds[PER_SAMPLER].push(new_index);
        new_index
    }

    pub fn unbind_sampler_image(&mut self, image_index: usize) {
        self.sampler_images[image_index] = Handle::invalid();
        self.free_lists[PER_SAMPLER].push(image_index);
        self.pending_unbinds[PER_SAMPLER].push(image_index);
    }

    pub fn get_sampler_image(&self, image_index: usize) -> Handle<Image> {
        self.sampler_images[image_index]
    }
}

impl DynamicBufferDescriptor {
    pub fn new_layout(device: &DeviceLoader) -> VulkanResult<vk::DescriptorSetLayout> {
        let vklayout = vk::DescriptorSetLayout::null();
        let binding = vk::DescriptorSetLayoutBindingBuilder::new()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::ALL);

        let bindings = [binding];

        let layout_info = vk::DescriptorSetLayoutCreateInfoBuilder::new().bindings(&bindings);

        let vklayout = unsafe {
            device
                .create_descriptor_set_layout(&layout_info, None)
                .result()?
        };
        Ok(vklayout)
    }

    pub fn new(
        device: &Device,
        buffer_handle: Handle<Buffer>,
        range_size: usize,
    ) -> VulkanResult<Self> {
        let buffer = device.buffers.get(buffer_handle);

        let vklayout = vk::DescriptorSetLayout::null();
        let vkpool = vk::DescriptorPool::null();

        let vklayouts = [vklayout];
        let set_info = vk::DescriptorSetAllocateInfoBuilder::new()
            .descriptor_pool(vkpool)
            .set_layouts(&vklayouts);

        let vkset = unsafe { device.device.allocate_descriptor_sets(&set_info).result()? }[0];

        let buffer_infos = [vk::DescriptorBufferInfoBuilder::new()
            .buffer(buffer.vkhandle)
            .offset(0)
            .range(range_size as u64)];

        let writes = [vk::WriteDescriptorSetBuilder::new()
            .dst_set(vkset)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .buffer_info(&buffer_infos)];

        unsafe {
            device.device.update_descriptor_sets(&writes, &[]);
        }

        Ok(Self {
            buffer: buffer_handle,
            vkset,
            dynamic_offset: 0,
            size: range_size as u32,
        })
    }

    pub fn destroy(&mut self, device: &DeviceLoader) -> VulkanResult<()> {
        let vkpool = vk::DescriptorPool::null();
        let vksets = [self.vkset];
        unsafe {
            device.free_descriptor_sets(vkpool, &vksets).result()?;
        }
        self.vkset = vk::DescriptorSet::null();
	Ok(())
    }
}
