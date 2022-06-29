use crate::{vk, vulkan};
use exo::pool::*;
use std::collections::HashMap;

struct ImageMetadata {
    pub(crate) resolved_desc: Handle<TextureDesc>,
    pub(crate) last_frame_used: u64,
}

pub struct ResourceRegistry {
    pub(crate) texture_descs: Pool<TextureDesc>,
    image_pool: HashMap<Handle<vulkan::Image>, ImageMetadata>,
    framebuffers: Vec<Handle<vulkan::Framebuffer>>,
    framebuffer_pool: HashMap<Handle<vulkan::Framebuffer>, u64>,
    pub(crate) screen_size: [f32; 2],
    i_frame: u64,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            texture_descs: Default::default(),
            image_pool: Default::default(),
            framebuffers: Vec::new(),
            framebuffer_pool: Default::default(),
            screen_size: [1.0, 1.0],
            i_frame: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum TextureSize {
    ScreenRelative([f32; 2]),
    Absolute([i32; 3]),
}

pub struct TextureDesc {
    pub name: String,
    pub size: TextureSize,
    pub format: vk::Format,
    pub image_type: vk::ImageType,
    resolved_image: Handle<vulkan::Image>,
}

impl TextureDesc {
    pub fn new(name: String, size: TextureSize) -> Self {
        Self {
            name,
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
    fn update_framebuffer_metadata(
        framebuffer_pool: &mut HashMap<Handle<vulkan::Framebuffer>, u64>,
        i_frame: u64,
        framebuffer: Handle<vulkan::Framebuffer>,
    ) {
        let new_metadata = 0;
        let metadata = framebuffer_pool.entry(framebuffer).or_insert(new_metadata);
        *metadata = i_frame;
    }

    fn update_image_metadata(&mut self, image: Handle<vulkan::Image>, desc: Handle<TextureDesc>) {
        let new_metadata = ImageMetadata {
            resolved_desc: desc,
            last_frame_used: 0,
        };
        let metadata = self.image_pool.entry(image).or_insert(new_metadata);
        metadata.last_frame_used = self.i_frame;
    }

    pub fn begin_frame(&mut self, device: &mut vulkan::Device, i_frame: u64) {
        self.i_frame = i_frame;

        let mut img_to_remove: Vec<Handle<vulkan::Image>> = Default::default();
        for (image_handle, metadata) in &mut self.image_pool {
            // Unbind images from the bindless set unused for 2 frames
            if (metadata.last_frame_used + 4) < i_frame {
                device.unbind_image(*image_handle);
            }

            // Destroy images unused for 19 frames
            if (metadata.last_frame_used + 19) < i_frame {
                img_to_remove.push(*image_handle);
            }
        }

        let mut fb_to_remove: Vec<Handle<vulkan::Framebuffer>> = Default::default();
        for (fb_handle, last_frame_used) in &self.framebuffer_pool {
            if (*last_frame_used + 3) < i_frame {
                fb_to_remove.push(*fb_handle);
            }
        }

        for handle in fb_to_remove {
            device.destroy_framebuffer(handle);
            let i_fb = self
                .framebuffers
                .iter()
                .position(|fb_handle| *fb_handle == handle)
                .unwrap();
            self.framebuffers.swap_remove(i_fb);
            self.framebuffer_pool.remove(&handle);
        }

        for handle in img_to_remove {
            device.destroy_image(handle);
            self.image_pool.remove(&handle);
        }
    }

    pub fn end_frame(&mut self) {
        // Resolve images each frame
        self.texture_descs.clear();
        for metadata in self.image_pool.values_mut() {
            metadata.resolved_desc = Handle::invalid();
        }
    }

    pub fn set_image(
        &mut self,
        desc_handle: Handle<TextureDesc>,
        image_handle: Handle<vulkan::Image>,
    ) {
        let desc = self.texture_descs.get_mut(desc_handle);
        desc.resolved_image = image_handle;
        self.update_image_metadata(image_handle, desc_handle);
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

        self.image_pool.remove(&image_handle);
    }

    pub(crate) fn resolve_image(
        &mut self,
        device: &mut vulkan::Device,
        desc_handle: Handle<TextureDesc>,
    ) -> vulkan::VulkanResult<Handle<vulkan::Image>> {
        let desc = self.texture_descs.get(desc_handle);

        let resolved_image_handle = if desc.resolved_image.is_valid() {
            // The image has already been resolved
            desc.resolved_image
        } else {
            // Find a free image in our pool that matches the spec
            let desc_spec = vulkan::ImageSpec {
                name: desc.name.clone(),
                size: self.texture_desc_size(desc.size),
                mip_levels: 1,
                image_type: desc.image_type,
                format: desc.format,
                ..Default::default()
            };

            let mut resolved_image_handle = None;
            for (image_handle, metadata) in &self.image_pool {
                if !metadata.resolved_desc.is_valid() {
                    let image = device.images.get(*image_handle);
                    if image.spec == desc_spec {
                        resolved_image_handle = Some(*image_handle);
                        break;
                    }
                }
            }

            // If there isn't any matching image in the pool, create a new one
            if resolved_image_handle.is_none() {
                resolved_image_handle = Some(device.create_image(desc_spec)?);
            }

            let resolved_image_handle = resolved_image_handle.unwrap();

            // Link the image to the desc to avoid resolving the same desc multiple time per frame
            self.texture_descs.get_mut(desc_handle).resolved_image = resolved_image_handle;
            resolved_image_handle
        };

        self.update_image_metadata(resolved_image_handle, desc_handle);
        Ok(resolved_image_handle)
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
                Self::update_framebuffer_metadata(
                    &mut self.framebuffer_pool,
                    self.i_frame,
                    *framebuffer_handle,
                );
                return Ok(*framebuffer_handle);
            }
        }

        let new_handle = device.create_framebuffer(size, &color_attachments, depth_attachment)?;
        Self::update_framebuffer_metadata(&mut self.framebuffer_pool, self.i_frame, new_handle);
        self.framebuffers.push(new_handle);
        Ok(new_handle)
    }
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
