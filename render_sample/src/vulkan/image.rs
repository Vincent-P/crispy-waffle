use super::device::*;
use super::error::*;

use exo::pool::Handle;

use erupt::vk;
use gpu_alloc::{Request, UsageFlags};
use gpu_alloc_erupt::EruptMemoryDevice;

#[derive(Clone, Copy)]
pub enum ImageState {
    None,
    GraphicsShaderRead,
    GraphicsShaderReadWrite,
    ComputeShaderRead,
    ComputeShaderReadWrite,
    TransferDst,
    TransferSrc,
    ColorAttachment,
    DepthAttachment,
    Present,
}

#[derive(Clone)]
pub struct ImageSpec {
    pub size: [i32; 3],
    pub mip_levels: u32,
    pub image_type: vk::ImageType,
    pub format: vk::Format,
    pub samples: vk::SampleCountFlagBits,
    pub usages: vk::ImageUsageFlags,
}

pub struct ImageView {
    pub range: vk::ImageSubresourceRange,
    pub vkhandle: vk::ImageView,
    pub sampled_idx: u32,
    pub storage_idx: u32,
    pub format: vk::Format,
}

pub struct Image {
    pub vkhandle: vk::Image,
    pub memory_block: Option<gpu_alloc::MemoryBlock<vk::DeviceMemory>>,
    pub spec: ImageSpec,
    pub full_view: ImageView,
    pub state: ImageState,
}

impl Device {
    fn create_image_view(
        &mut self,
        image: vk::Image,
        range: vk::ImageSubresourceRange,
        format: vk::Format,
        view_type: vk::ImageViewType,
    ) -> VulkanResult<ImageView> {
        let view_create_info = vk::ImageViewCreateInfoBuilder::new()
            .image(image)
            .format(format)
            .subresource_range(range)
            .view_type(view_type);

        let vkhandle =
            unsafe { self.device.create_image_view(&view_create_info, None) }.result()?;

        Ok(ImageView {
            range,
            vkhandle,
            sampled_idx: 0,
            storage_idx: 0,
            format,
        })
    }

    pub fn create_image(&mut self, spec: ImageSpec) -> VulkanResult<Handle<Image>> {
        let image_create_info = vk::ImageCreateInfoBuilder::new()
            .image_type(spec.image_type)
            .format(spec.format)
            .extent(vk::Extent3D {
                width: spec.size[0] as u32,
                height: spec.size[1] as u32,
                depth: spec.size[2] as u32,
            })
            .mip_levels(spec.mip_levels)
            .array_layers(1)
            .samples(spec.samples)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(spec.usages)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .tiling(vk::ImageTiling::OPTIMAL);

        let vkimage = unsafe { self.device.create_image(&image_create_info, None) }.result()?;

        let mem_requirements = unsafe { self.device.get_image_memory_requirements(vkimage) };

        let memory_block = unsafe {
            self.allocator.alloc(
                EruptMemoryDevice::wrap(&self.device),
                Request {
                    size: mem_requirements.size,
                    align_mask: 1,
                    usage: UsageFlags::FAST_DEVICE_ACCESS,
                    memory_types: !0,
                },
            )
        }?;

        unsafe {
            self.device
                .bind_image_memory(vkimage, *memory_block.memory(), 0)
        }
        .result()?;

        let is_depth = spec.format == vk::Format::D32_SFLOAT;
        let full_range = vk::ImageSubresourceRangeBuilder::new()
            .aspect_mask(if is_depth {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
            })
            .base_mip_level(0)
            .level_count(image_create_info.mip_levels)
            .base_array_layer(0)
            .layer_count(image_create_info.array_layers);

        let full_view_type = match spec.image_type {
            vk::ImageType::_1D => vk::ImageViewType::_1D,
            vk::ImageType::_2D => vk::ImageViewType::_2D,
            vk::ImageType::_3D => vk::ImageViewType::_3D,
            _ => unreachable!(),
        };
        let full_view =
            self.create_image_view(vkimage, *full_range, spec.format, full_view_type)?;

        Ok(self.images.add(Image {
            vkhandle: vkimage,
            memory_block: Some(memory_block),
            spec,
            full_view,
            state: ImageState::None,
        }))
    }

    pub fn create_image_proxy(
        &mut self,
        spec: ImageSpec,
        proxy: vk::Image,
    ) -> VulkanResult<Handle<Image>> {
        let is_depth = spec.format == vk::Format::D32_SFLOAT;
        let full_range = vk::ImageSubresourceRangeBuilder::new()
            .aspect_mask(if is_depth {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
            })
            .base_mip_level(0)
            .level_count(spec.mip_levels)
            .base_array_layer(0)
            .layer_count(1);

        let full_view_type = match spec.image_type {
            vk::ImageType::_1D => vk::ImageViewType::_1D,
            vk::ImageType::_2D => vk::ImageViewType::_2D,
            vk::ImageType::_3D => vk::ImageViewType::_3D,
            _ => unreachable!(),
        };
        let full_view = self.create_image_view(proxy, *full_range, spec.format, full_view_type)?;

        Ok(self.images.add(Image {
            vkhandle: proxy,
            memory_block: None,
            spec,
            full_view,
            state: ImageState::None,
        }))
    }

    pub fn destroy_image(&mut self, image_handle: Handle<Image>) {
        let image = self.images.get_mut(image_handle);
        if let Some(block) = image.memory_block.take() {
            unsafe {
                self.allocator
                    .dealloc(EruptMemoryDevice::wrap(&self.device), block);
            }
        }

        unsafe {
            self.device
                .destroy_image_view(image.full_view.vkhandle, None);
        }
        self.images.remove(image_handle);
    }
}
