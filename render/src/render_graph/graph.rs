pub use super::resource_registry::*;
use crate::{ring_buffer::RingBuffer, vk, vulkan};
use exo::{dynamic_array::DynamicArray, pool::Handle};

enum Pass {
    Graphic(GraphicPass),
    Raw(RawPass),
}

pub struct RenderGraph {
    pub resources: ResourceRegistry,
    passes: Vec<Pass>,
    i_frame: u64,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            resources: ResourceRegistry::new(),
            passes: Vec::new(),
            i_frame: 0,
        }
    }
}

pub struct PassApi<'device, 'buffers> {
    pub instance: &'device vulkan::Instance,
    pub physical_devices:
        &'device mut DynamicArray<vulkan::PhysicalDevice, { vulkan::MAX_PHYSICAL_DEVICES }>,
    pub i_device: usize,
    pub device: &'device mut vulkan::Device,
    pub uniform_buffer: &'buffers mut RingBuffer,
    pub dynamic_vertex_buffer: &'buffers mut RingBuffer,
    pub dynamic_index_buffer: &'buffers mut RingBuffer,
    pub upload_buffer: &'buffers mut RingBuffer,
}

impl RenderGraph {
    pub fn execute(
        &mut self,
        mut api: PassApi,
        context_pool: &mut vulkan::ContextPool,
    ) -> vulkan::VulkanResult<()> {
        let mut ctx = api.device.get_graphics_context(context_pool)?;
        ctx.base().begin(api.device)?;

        // Consume all passes
        let passes = std::mem::take(&mut self.passes);

        for pass in passes {
            match pass {
                Pass::Graphic(mut pass) => {
                    let output_desc = self.resources.texture_descs.get(pass.color_attachment);
                    let output_size = self.resources.texture_desc_size(output_desc.size);

                    let output_image = self
                        .resources
                        .resolve_image(api.device, pass.color_attachment)?;

                    let framebuffer = self.resources.resolve_framebuffer(
                        api.device,
                        &[pass.color_attachment],
                        Handle::invalid(),
                    )?;

                    ctx.base().barrier(
                        api.device,
                        output_image,
                        vulkan::ImageState::ColorAttachment,
                    );

                    ctx.begin_pass(
                        api.device,
                        framebuffer,
                        &[vulkan::LoadOp::ClearColor(
                            vulkan::ClearColorValue::Float32([0.0, 0.0, 0.0, 1.0]),
                        )],
                    )?;
                    ctx.set_viewport(
                        api.device,
                        vk::ViewportBuilder::new()
                            .width(output_size[0] as f32)
                            .height(output_size[1] as f32)
                            .min_depth(0.0)
                            .max_depth(1.0),
                    );
                    ctx.set_scissor(
                        api.device,
                        vk::Rect2DBuilder::new().extent(
                            *vk::Extent2DBuilder::new()
                                .width(output_size[0] as u32)
                                .height(output_size[1] as u32),
                        ),
                    );

                    (pass.execute_cb)(self, &mut api, ctx.as_mut());

                    ctx.end_pass(api.device);
                }
                Pass::Raw(mut pass) => {
                    (pass.execute_cb)(self, &mut api, ctx.as_mut())?;
                }
            }
        }

        self.resources.end_frame(api.device, self.i_frame);

        self.i_frame += 1;

        Ok(())
    }
}

pub struct GraphicPass {
    color_attachment: Handle<TextureDesc>,
    execute_cb: Box<dyn FnMut(&mut RenderGraph, &mut PassApi, &mut vulkan::GraphicsContext)>,
}

impl RenderGraph {
    pub fn graphics_pass(
        &mut self,
        color_attachment: Handle<TextureDesc>,
        execute: impl (FnMut(&mut RenderGraph, &mut PassApi, &mut vulkan::GraphicsContext)) + 'static,
    ) {
        self.passes.push(Pass::Graphic(GraphicPass {
            color_attachment,
            execute_cb: Box::new(execute),
        }))
    }
}

pub struct RawPass {
    execute_cb: Box<
        dyn FnMut(
            &mut RenderGraph,
            &mut PassApi,
            &mut vulkan::ComputeContext,
        ) -> vulkan::VulkanResult<()>,
    >,
}

impl RenderGraph {
    pub fn raw_pass(
        &mut self,
        execute: impl (FnMut(
                &mut RenderGraph,
                &mut PassApi,
                &mut vulkan::ComputeContext,
            ) -> vulkan::VulkanResult<()>)
            + 'static,
    ) {
        self.passes.push(Pass::Raw(RawPass {
            execute_cb: Box::new(execute),
        }))
    }
}

impl RenderGraph {
    pub fn output_image(&mut self, output_desc: TextureDesc) -> Handle<TextureDesc> {
        self.resources.texture_descs.add(output_desc)
    }

    pub fn image_size(&self, desc_handle: Handle<TextureDesc>) -> [i32; 3] {
        let desc = self.resources.texture_descs.get(desc_handle);
        self.resources.texture_desc_size(desc.size)
    }
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}
