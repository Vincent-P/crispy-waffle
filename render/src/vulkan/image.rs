use super::device::*;
use super::error::*;
use super::memory;

use exo::pool::Handle;

use erupt::vk;

#[derive(Clone, Copy, Debug)]
pub enum ImageState {
    Null,
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

#[derive(Debug)]
pub struct ImageAccess {
    pub stage: vk::PipelineStageFlags,
    pub access: vk::AccessFlags,
    pub layout: vk::ImageLayout,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ImageSpec {
    pub name: String,
    pub size: [i32; 3],
    pub mip_levels: u32,
    pub image_type: vk::ImageType,
    pub format: vk::Format,
    pub samples: vk::SampleCountFlagBits,
    pub usages: vk::ImageUsageFlags,
}

impl Default for ImageSpec {
    fn default() -> Self {
        Self {
            name: String::from("unnamed"),
            size: [1, 1, 1],
            mip_levels: 1,
            image_type: vk::ImageType::_2D,
            format: vk::Format::R8G8B8A8_UNORM,
            samples: vk::SampleCountFlagBits::_1,
            usages: vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::COLOR_ATTACHMENT,
        }
    }
}

#[derive(Debug)]
pub struct ImageView {
    pub range: vk::ImageSubresourceRange,
    pub vkhandle: vk::ImageView,
    pub sampled_idx: u32,
    pub storage_idx: u32,
    pub format: vk::Format,
}

#[derive(Debug)]
pub struct Image {
    pub vkhandle: vk::Image,
    pub memory_block: Option<memory::Allocation>,
    pub spec: ImageSpec,
    pub full_view: ImageView,
    pub state: ImageState,
    is_proxy: bool,
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

        let memory_block = unsafe {
            self.allocator.allocate_memory_for_image(
                &self.device,
                vkimage,
                vk_alloc::MemoryLocation::GpuOnly,
                memory::Lifetime::Image,
                true,
            )
        }?;

        unsafe {
            self.device.bind_image_memory(
                vkimage,
                memory_block.device_memory(),
                memory_block.offset(),
            )
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

        self.set_vk_name(vkimage.0, vk::ObjectType::IMAGE, &spec.name)?;
        self.set_vk_name(full_view.vkhandle.0, vk::ObjectType::IMAGE_VIEW, &spec.name)?;
        self.set_vk_name(
            memory_block.device_memory().0,
            vk::ObjectType::DEVICE_MEMORY,
            &spec.name,
        )?;

        let image_handle = self.images.add(Image {
            vkhandle: vkimage,
            memory_block: Some(memory_block),
            spec,
            full_view,
            state: ImageState::Null,
            is_proxy: false,
        });

        let image_mut = self.images.get_mut(image_handle);
        image_mut.full_view.sampled_idx = self
            .descriptors
            .bindless_set
            .bind_sampler_image(image_handle) as u32;

        if image_mut.spec.usages.contains(vk::ImageUsageFlags::STORAGE) {
            image_mut.full_view.storage_idx =
                self.descriptors
                    .bindless_set
                    .bind_storage_image(image_handle) as u32;
        }

        Ok(image_handle)
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

        self.set_vk_name(proxy.0, vk::ObjectType::IMAGE, &spec.name)?;
        self.set_vk_name(full_view.vkhandle.0, vk::ObjectType::IMAGE_VIEW, &spec.name)?;

        let res = self.images.add(Image {
            vkhandle: proxy,
            memory_block: None,
            spec,
            full_view,
            state: ImageState::Null,
            is_proxy: true,
        });

        Ok(res)
    }

    pub fn unbind_image(&mut self, image_handle: Handle<Image>) {
        let image = self.images.get_mut(image_handle);

        if image.full_view.sampled_idx > 0 {
            self.descriptors
                .bindless_set
                .unbind_sampler_image(image.full_view.sampled_idx as usize);
            image.full_view.sampled_idx = 0;
        }

        if image.full_view.storage_idx > 0 {
            self.descriptors
                .bindless_set
                .unbind_storage_image(image.full_view.storage_idx as usize);
            image.full_view.storage_idx = 0;
        }
    }

    pub fn destroy_image(&mut self, image_handle: Handle<Image>) {
        self.unbind_image(image_handle);

        let image = self.images.get_mut(image_handle);

        unsafe {
            if !image.is_proxy {
                self.device.destroy_image(image.vkhandle, None);
            }
        }

        if let Some(block) = image.memory_block.take() {
            unsafe {
                self.allocator.deallocate(&self.device, &block).unwrap();
            }
        }

        unsafe {
            self.device
                .destroy_image_view(image.full_view.vkhandle, None);
        }
        self.images.remove(image_handle);
    }
}

impl ImageState {
    pub fn get_src_access(self) -> ImageAccess {
        let (stage, access, layout) = match self {
            Self::Null => (
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::AccessFlags::NONE,
                vk::ImageLayout::UNDEFINED,
            ),
            Self::GraphicsShaderRead => (
                vk::PipelineStageFlags::VERTEX_SHADER,
                vk::AccessFlags::NONE,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ),
            Self::GraphicsShaderReadWrite => (
                vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::SHADER_WRITE,
                vk::ImageLayout::GENERAL,
            ),
            Self::ComputeShaderRead => (
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::AccessFlags::NONE,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ),
            Self::ComputeShaderReadWrite => (
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::AccessFlags::SHADER_WRITE,
                vk::ImageLayout::GENERAL,
            ),
            Self::TransferDst => (
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ),
            Self::TransferSrc => (
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::NONE,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            ),
            Self::ColorAttachment => (
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            ),

            Self::DepthAttachment => (
                vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            ),

            Self::Present => (
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::AccessFlags::NONE,
                vk::ImageLayout::PRESENT_SRC_KHR,
            ),
        };

        ImageAccess {
            stage,
            access,
            layout,
        }
    }

    pub fn get_dst_access(self) -> ImageAccess {
        let (stage, access, layout) = match self {
            Self::Null => (
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::AccessFlags::NONE,
                vk::ImageLayout::UNDEFINED,
            ),
            Self::GraphicsShaderRead => (
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::SHADER_READ,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ),
            Self::GraphicsShaderReadWrite => (
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::SHADER_WRITE,
                vk::ImageLayout::GENERAL,
            ),
            Self::ComputeShaderRead => (
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::AccessFlags::SHADER_READ,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ),
            Self::ComputeShaderReadWrite => (
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
                vk::ImageLayout::GENERAL,
            ),
            Self::TransferDst => (
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ),
            Self::TransferSrc => (
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::TRANSFER_READ,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            ),
            Self::ColorAttachment => (
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::COLOR_ATTACHMENT_READ,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            ),

            Self::DepthAttachment => (
                vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
                vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            ),

            Self::Present => (
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::AccessFlags::NONE,
                vk::ImageLayout::PRESENT_SRC_KHR,
            ),
        };

        ImageAccess {
            stage,
            access,
            layout,
        }
    }
}
