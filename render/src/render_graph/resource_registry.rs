use crate::{vk, vulkan};
use exo::pool::*;

pub struct ResourceRegistry {
    pub(crate) texture_descs: Pool<TextureDesc>,
    pub(crate) texture_to_remove: Vec<Handle<vulkan::Image>>,
    pub(crate) framebuffers: Vec<Handle<vulkan::Framebuffer>>,
    pub(crate) screen_size: [f32; 2],
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            texture_descs: Pool::new(),
            texture_to_remove: Vec::new(),
            framebuffers: Vec::new(),
            screen_size: [1.0, 1.0],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum TextureSize {
    ScreenRelative([f32; 2]),
    Absolute([i32; 3]),
}

pub struct TextureDesc {
    pub size: TextureSize,
    pub format: vk::Format,
    pub image_type: vk::ImageType,
    resolved_image: Handle<vulkan::Image>,
}

impl TextureDesc {
    pub fn new(size: TextureSize) -> Self {
        Self {
            size,
            format: vk::Format::R8G8B8A8_UNORM,
            image_type: vk::ImageType::_2D,
            resolved_image: Handle::invalid(),
        }
    }

    pub fn format(mut self, format: vk::Format) -> Self {
        self.format = format;
        self
    }

    pub fn image_type(mut self, image_type: vk::ImageType) -> Self {
        self.image_type = image_type;
        self
    }
}

impl ResourceRegistry {
    pub fn import_image(&mut self, _: Handle<vulkan::Image>) -> Handle<TextureDesc> {
        unimplemented!()
    }

    pub fn set_image(
        &mut self,
        desc_handle: Handle<TextureDesc>,
        image_handle: Handle<vulkan::Image>,
    ) {
        let desc = self.texture_descs.get_mut(desc_handle);
        desc.resolved_image = image_handle;
    }

    pub fn drop_image(&mut self, image_handle: Handle<vulkan::Image>) {
        let matching_descs: Vec<Handle<TextureDesc>> = self
            .texture_descs
            .iter()
            .filter(|(_handle, desc)| desc.resolved_image == image_handle)
            .map(|(handle, _desc)| handle)
            .collect();

        for matching_desc in matching_descs {
            self.texture_descs.get_mut(matching_desc).resolved_image = Handle::invalid();
        }

        self.texture_to_remove.push(image_handle);
    }

    pub(crate) fn resolve_image(
        &mut self,
        device: &mut vulkan::Device,
        desc_handle: Handle<TextureDesc>,
    ) -> vulkan::VulkanResult<Handle<vulkan::Image>> {
        let desc = self.texture_descs.get(desc_handle);
        if desc.resolved_image.is_valid() {
            return Ok(desc.resolved_image);
        }

        let image_spec = vulkan::ImageSpec {
            size: self.texture_desc_size(desc.size),
            mip_levels: 1,
            image_type: desc.image_type,
            format: desc.format,
            ..Default::default()
        };

        device.create_image(image_spec)
    }

    pub(crate) fn texture_desc_size(&self, texture_size: TextureSize) -> [i32; 3] {
        match texture_size {
            TextureSize::Absolute(absolute) => absolute,
            TextureSize::ScreenRelative(relative) => {
                let width = (relative[0] * self.screen_size[0]) as i32;
                let height = (relative[1] * self.screen_size[1]) as i32;
                [width, height, 1]
            }
        }
    }

    pub(crate) fn resolve_framebuffer(
        &mut self,
        device: &mut vulkan::Device,
        color_attachments: &[Handle<TextureDesc>],
        depth_attachment: Handle<TextureDesc>,
    ) -> vulkan::VulkanResult<Handle<vulkan::Framebuffer>> {
        let color_attachments: Vec<Handle<vulkan::Image>> = color_attachments
            .iter()
            .map(|desc_handle| self.texture_descs.get(*desc_handle).resolved_image)
            .collect();

        let depth_attachment = if depth_attachment.is_valid() {
            self.resolve_image(device, depth_attachment)?
        } else {
            Handle::invalid()
        };

        let size = {
            let handle = if !color_attachments.is_empty() {
                color_attachments[0]
            } else {
                assert!(depth_attachment.is_valid());
                depth_attachment
            };
            device.images.get(handle).spec.size
        };

        for framebuffer_handle in &self.framebuffers {
            let framebuffer = device.framebuffers.get(*framebuffer_handle);
            if framebuffer.color_attachments.as_slice() == color_attachments.as_slice()
                && framebuffer.depth_attachment == depth_attachment
                && framebuffer.format.size == size
            {
                return Ok(*framebuffer_handle);
            }
        }

        let new_handle = device.create_framebuffer(size, &color_attachments, depth_attachment)?;
        self.framebuffers.push(new_handle);
        Ok(new_handle)
    }
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
