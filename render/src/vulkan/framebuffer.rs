use super::device::*;
use super::error::*;
use super::image::*;

use exo::{dynamic_array::DynamicArray, pool::Handle};

use erupt::{vk, DeviceLoader};

pub const MAX_ATTACHMENTS: usize = 4;
pub const MAX_RENDERPASS: usize = 4; // max number of renderpasses per framebuffer

#[derive(Clone)]
pub struct FramebufferFormat {
    pub size: [i32; 3],
    pub attachment_formats: DynamicArray<vk::Format, MAX_ATTACHMENTS>,
    pub depth_format: Option<vk::Format>,
}

impl Default for FramebufferFormat {
    fn default() -> Self {
        FramebufferFormat {
            size: [1, 1, 1],
            attachment_formats: DynamicArray::new(),
            depth_format: None,
        }
    }
}

pub struct Framebuffer {
    pub vkhandle: vk::Framebuffer,
    pub format: FramebufferFormat,
    pub color_attachments: DynamicArray<Handle<Image>, MAX_ATTACHMENTS>,
    pub depth_attachment: Handle<Image>,
    pub render_passes: DynamicArray<Renderpass, MAX_RENDERPASS>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ClearColorValue {
    Float32([f32; 4]),
    Int32([i32; 4]),
    Uint32([u32; 4]),
}

#[derive(Clone, Copy, PartialEq)]
pub struct ClearDepthValue {
    depth: f32,
    stencil: u32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LoadOp {
    Load,
    ClearColor(ClearColorValue),
    ClearDepth(ClearDepthValue),
    Ignore,
}

pub struct Renderpass {
    pub vkhandle: vk::RenderPass,
    pub load_ops: DynamicArray<LoadOp, MAX_ATTACHMENTS>,
}

pub fn create_renderpass(
    device: &DeviceLoader,
    format: &FramebufferFormat,
    load_ops: &[LoadOp],
) -> VulkanResult<Renderpass> {
    let attachment_count =
        format.attachment_formats.len() + if format.depth_format.is_some() { 1 } else { 0 };
    assert!(load_ops.len() == attachment_count);

    let mut color_refs = DynamicArray::<vk::AttachmentReferenceBuilder, MAX_ATTACHMENTS>::new();
    let mut attachment_descs =
        DynamicArray::<vk::AttachmentDescriptionBuilder, MAX_ATTACHMENTS>::new();

    for i_color in 0..format.attachment_formats.len() {
        color_refs.push(
            vk::AttachmentReferenceBuilder::new()
                .attachment(attachment_descs.len() as u32)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
        );

        attachment_descs.push(
            vk::AttachmentDescriptionBuilder::new()
                .format(format.attachment_formats[i_color])
                .samples(vk::SampleCountFlagBits::_1)
                .load_op(load_ops[i_color].to_vk())
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(if let LoadOp::ClearColor(_) = load_ops[i_color] {
                    vk::ImageLayout::UNDEFINED
                } else {
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
                })
                .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
        );
    }

    let subpass_info = vk::SubpassDescriptionBuilder::new()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_refs);

    let subpasses = [subpass_info];

    let renderpass_info = vk::RenderPassCreateInfoBuilder::new()
        .attachments(&attachment_descs)
        .subpasses(&subpasses);

    let vkhandle = unsafe { device.create_render_pass(&renderpass_info, None).result()? };

    let load_ops = DynamicArray::<LoadOp, MAX_ATTACHMENTS>::from(load_ops);
    Ok(Renderpass { vkhandle, load_ops })
}

impl Device<'_> {
    pub fn create_framebuffer(
        &mut self,
        format: &FramebufferFormat,
        color_attachments: &[Handle<Image>],
        depth_attachment: Handle<Image>,
    ) -> VulkanResult<Handle<Framebuffer>> {
        let mut framebuffer = Framebuffer {
            vkhandle: vk::Framebuffer::null(),
            format: format.clone(),
            color_attachments: DynamicArray::new(),
            depth_attachment: Handle::invalid(),
            render_passes: DynamicArray::new(),
        };

        let attachment_count =
            color_attachments.len() + if depth_attachment.is_valid() { 1 } else { 0 };

        let mut attachment_views = DynamicArray::<vk::ImageView, MAX_ATTACHMENTS>::new();
        for attachment in color_attachments {
            let image = self.images.get(*attachment);
            attachment_views.push(image.full_view.vkhandle);
            framebuffer
                .format
                .attachment_formats
                .push(image.spec.format);
        }

        if depth_attachment.is_valid() {
            let image = self.images.get(depth_attachment);
            attachment_views.push(image.full_view.vkhandle);
            framebuffer.format.depth_format = Some(image.spec.format);
        }

        let mut load_ops = DynamicArray::<LoadOp, MAX_ATTACHMENTS>::new();
        for _ in 0..attachment_count {
            load_ops.push(LoadOp::Ignore);
        }

        framebuffer.render_passes.push(create_renderpass(
            &self.device,
            &framebuffer.format,
            &load_ops,
        )?);

        let framebuffer_info = vk::FramebufferCreateInfoBuilder::new()
            .render_pass(
                framebuffer
                    .render_passes
                    .as_slice()
                    .last()
                    .unwrap()
                    .vkhandle,
            )
            .attachments(&attachment_views)
            .width(framebuffer.format.size[0] as u32)
            .height(framebuffer.format.size[1] as u32)
            .layers(framebuffer.format.size[2] as u32);

        framebuffer.vkhandle = unsafe {
            self.device
                .create_framebuffer(&framebuffer_info, None)
                .result()?
        };

        Ok(self.framebuffers.add(framebuffer))
    }

    pub fn destroy_framebuffer(&mut self, framebuffer_handle: Handle<Framebuffer>) {
        let framebuffer = self.framebuffers.get(framebuffer_handle);
        unsafe {
            self.device.destroy_framebuffer(framebuffer.vkhandle, None);
            for renderpass in &framebuffer.render_passes {
                self.device.destroy_render_pass(renderpass.vkhandle, None);
            }
        }
        self.framebuffers.remove(framebuffer_handle);
    }

    pub fn find_framebuffer_renderpass(
        &mut self,
        framebuffer_handle: Handle<Framebuffer>,
        load_ops: &[LoadOp],
    ) -> VulkanResult<(&Framebuffer, &Renderpass)> {
        let framebuffer = self.framebuffers.get_mut(framebuffer_handle);

        let mut i_renderpass = framebuffer
            .render_passes
            .iter()
            .position(|renderpass| renderpass.load_ops.as_slice() == load_ops);

        if i_renderpass.is_none() {
            framebuffer.render_passes.push(create_renderpass(
                &self.device,
                &framebuffer.format,
                load_ops,
            )?);
            i_renderpass = Some(framebuffer.render_passes.len() - 1);
        }

        Ok((
            framebuffer,
            &framebuffer.render_passes[i_renderpass.unwrap()],
        ))
    }
}

// --

impl ClearColorValue {
    pub fn to_vk(self) -> vk::ClearColorValue {
        match self {
            ClearColorValue::Float32(values) => vk::ClearColorValue { float32: values },
            ClearColorValue::Int32(values) => vk::ClearColorValue { int32: values },
            ClearColorValue::Uint32(values) => vk::ClearColorValue { uint32: values },
        }
    }
}

impl ClearDepthValue {
    pub fn to_vk(self) -> vk::ClearDepthStencilValue {
        vk::ClearDepthStencilValue {
            depth: self.depth,
            stencil: self.stencil,
        }
    }
}

impl LoadOp {
    pub fn to_vk(self) -> vk::AttachmentLoadOp {
        match self {
            LoadOp::Load => vk::AttachmentLoadOp::LOAD,
            LoadOp::ClearDepth(_) | LoadOp::ClearColor(_) => vk::AttachmentLoadOp::CLEAR,
            LoadOp::Ignore => vk::AttachmentLoadOp::DONT_CARE,
        }
    }

    pub fn clear_value(&self) -> vk::ClearValue {
        match self {
            LoadOp::ClearColor(value) => vk::ClearValue {
                color: value.to_vk(),
            },
            LoadOp::ClearDepth(value) => vk::ClearValue {
                depth_stencil: value.to_vk(),
            },
            LoadOp::Load | LoadOp::Ignore => vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.0; 4] },
            },
        }
    }
}
