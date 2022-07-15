use drawer2d::drawer::Drawer;
use exo::{dynamic_array::DynamicArray, pool::Handle};
use render::{bindings, render_graph::graph::*, shader_path, vk, vulkan};
use std::{mem::size_of, rc::Rc};

pub struct UiPass {
    pub glyph_atlas: Handle<vulkan::Image>,
    ui_program: Handle<vulkan::GraphicsProgram>,
}

impl UiPass {
    pub fn new(
        device: &mut vulkan::Device,
        glyph_atlas_size: [i32; 2],
    ) -> vulkan::VulkanResult<Self> {
        let ui_gfx_state = vulkan::GraphicsState {
            vertex_shader: device.create_shader(shader_path!("ui.vert.spv")).unwrap(),
            fragment_shader: device.create_shader(shader_path!("ui.frag.spv")).unwrap(),
            attachments_format: vulkan::FramebufferFormat {
                attachment_formats: DynamicArray::from([vk::Format::R8G8B8A8_UNORM]),
                ..Default::default()
            },
        };

        let ui_program = device.create_graphics_program(ui_gfx_state)?;
        device.compile_graphics_program(
            ui_program,
            vulkan::RenderState {
                depth: vulkan::DepthState {
                    test: None,
                    enable_write: false,
                    bias: 0.0,
                },
                rasterization: vulkan::RasterizationState {
                    enable_conservative_rasterization: false,
                    culling: false,
                },
                input_assembly: vulkan::InputAssemblyState {
                    topology: vulkan::PrimitiveTopology::TriangleList,
                },
                alpha_blending: true,
            },
        )?;

        let glyph_atlas = device.create_image(vulkan::ImageSpec {
            name: String::from("glyph atlas"),
            size: [glyph_atlas_size[0], glyph_atlas_size[1], 1],
            format: vk::Format::R8_UNORM,
            usages: vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::STORAGE,
            ..Default::default()
        })?;

        Ok(Self {
            glyph_atlas,
            ui_program,
        })
    }

    pub fn register_graph(
        &self,
        graph: &mut RenderGraph,
        output: Handle<TextureDesc>,
        drawer: &Rc<Drawer<'static>>,
    ) {
        let glyph_atlas = self.glyph_atlas;
        let ui_program = self.ui_program;
        let drawer = Rc::clone(drawer);
        let drawer2 = Rc::clone(&drawer);

        let execute = move |_graph: &mut RenderGraph,
                            api: &mut PassApi,
                            ctx: &mut vulkan::ComputeContext|
              -> vulkan::VulkanResult<()> {
            use drawer2d::glyph_cache::GlyphEvent;
            let drawer = Rc::clone(&drawer);

            let mut glyphs_to_upload: Vec<vulkan::BufferImageCopy> = Vec::with_capacity(32);
            drawer
                .glyph_cache()
                .process_events(|cache_event, glyph_image, glyph_atlas_pos| {
                    // Copy new glyphs to the upload buffer
                    if let GlyphEvent::New(_, _) = cache_event {
                        if let Some(atlas_pos) = glyph_atlas_pos {
                            let image = glyph_image.unwrap();
                            let (slice, offset) = api.upload_buffer.allocate(image.data.len(), 256);
                            unsafe {
                                (*slice).copy_from_slice(&image.data);
                            }

                            let image_offset = [atlas_pos[0], atlas_pos[1], 0];

                            glyphs_to_upload.push(vulkan::BufferImageCopy {
                                buffer_offset: offset as u64,
                                buffer_size: image.data.len() as u32,
                                image_offset,
                                image_extent: [
                                    image.placement.width as u32,
                                    image.placement.height as u32,
                                    1,
                                ],
                            });
                        }
                    }
                });
            if !glyphs_to_upload.is_empty() {
                ctx.base_context().barrier(
                    api.device,
                    glyph_atlas,
                    vulkan::ImageState::TransferDst,
                );
                ctx.transfer_mut().copy_buffer_to_image(
                    api.device,
                    api.upload_buffer.buffer,
                    glyph_atlas,
                    &glyphs_to_upload,
                );
                ctx.base_context().barrier(
                    api.device,
                    glyph_atlas,
                    vulkan::ImageState::GraphicsShaderRead,
                );
            }

            Ok(())
        };
        graph.raw_pass(execute);

        let drawer = drawer2;
        let execute = move |graph: &mut RenderGraph,
                            api: &mut PassApi,
                            ctx: &mut vulkan::GraphicsContext| {
            let vertices = drawer.get_vertices();
            let (slice, vertices_offset) = api
                .dynamic_vertex_buffer
                .allocate(vertices.len(), Drawer::get_primitive_alignment());
            unsafe {
                (*slice).copy_from_slice(vertices);
            }
            let indices = drawer.get_indices();
            let indices_byte_length = indices.len() * size_of::<u32>();
            let (slice, indices_offset) = api
                .dynamic_index_buffer
                .allocate(indices_byte_length, size_of::<u32>());
            unsafe {
                let gpu_indices = std::slice::from_raw_parts_mut(
                    (*slice).as_mut_ptr() as *mut u32,
                    (*slice).len() / size_of::<u32>(),
                );
                gpu_indices.copy_from_slice(indices);
            }
            #[repr(C, packed)]
            struct Options {
                pub scale: [f32; 2],
                pub translation: [f32; 2],
                pub vertices_descriptor_index: u32,
                pub primitive_bytes_offset: u32,
                pub glyph_atlas_descriptor: u32,
            }

            let options = bindings::bind_shader_options(
                api.device,
                api.uniform_buffer,
                &ctx,
                size_of::<Options>(),
            )
            .unwrap();

            let output_size = graph.image_size(output);
            let glyph_atlas_descriptor = api.device.images.get(glyph_atlas).full_view.sampled_idx;
            unsafe {
                let p_options =
                    std::slice::from_raw_parts_mut((*options).as_ptr() as *mut Options, 1);
                p_options[0] = Options {
                    scale: [2.0 / (output_size[0] as f32), 2.0 / (output_size[1] as f32)],
                    translation: [-1.0, -1.0],
                    vertices_descriptor_index: api
                        .device
                        .buffers
                        .get(api.dynamic_vertex_buffer.buffer)
                        .storage_idx,
                    primitive_bytes_offset: vertices_offset,
                    glyph_atlas_descriptor,
                };
            }
            ctx.bind_index_buffer(
                api.device,
                api.dynamic_index_buffer.buffer,
                vk::IndexType::UINT32,
                indices_offset as usize,
            );
            ctx.bind_graphics_pipeline(api.device, ui_program, 0);
            ctx.draw_indexed(
                api.device,
                vulkan::DrawIndexedOptions {
                    vertex_count: indices.len() as u32,
                    ..Default::default()
                },
            );
        };

        graph.graphics_pass(output, execute);
    }
}
