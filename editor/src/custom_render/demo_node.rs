use exo::pool::Handle;
use render::{bindings, render_graph::graph::*, shader_path, vulkan};
use std::{cell::RefCell, rc::Rc};

pub struct DemoNode {
    program: Handle<vulkan::ComputeProgram>,
    resolved_output_descriptor: u32,
}

impl DemoNode {
    pub fn new(device: &mut vulkan::Device) -> vulkan::VulkanResult<DemoNode> {
        let shader_handle = device.create_shader(shader_path!("demo.comp.spv"))?;
        let node = DemoNode {
            program: device.create_compute_program(String::from("demo"), shader_handle)?,
            resolved_output_descriptor: 1,
        };
        Ok(node)
    }

    pub fn output_descriptor(&self) -> u32 {
        self.resolved_output_descriptor
    }

    pub fn register_graph(
        pass: &Rc<RefCell<Self>>,
        graph: &mut RenderGraph,
        output: Handle<TextureDesc>,
        dt: f32,
        t: f32,
    ) {
        let demo_program = pass.borrow().program;
        let pass = Rc::clone(pass);

        graph.raw_pass(
            move |graph: &mut RenderGraph,
                  api: &mut PassApi,
                  ctx: &mut vulkan::ComputeContext|
                  -> vulkan::VulkanResult<()> {
                {
                    let output_image = graph.resources.resolve_image(api.device, output)?;

                    let output_descriptor =
                        api.device.images.get(output_image).full_view.storage_idx;
                    let output_sampled_descriptor =
                        api.device.images.get(output_image).full_view.sampled_idx;
                    pass.borrow_mut().resolved_output_descriptor = output_sampled_descriptor;

                    #[repr(C, packed)]
                    struct Options {
                        pub storage_output_frame: u32,
                        pub i_frame: u32,
                        pub dt: f32,
                        pub t: f32,
                    }

                    bindings::bind_and_copy_shader_options(
                        api.device,
                        api.uniform_buffer,
                        &ctx,
                        Options {
                            storage_output_frame: output_descriptor,
                            i_frame: graph.i_frame() as u32,
                            dt,
                            t,
                        },
                    )?;

                    ctx.bind_compute_pipeline(api.device, demo_program);

                    let output_size = graph.resources.texture_desc_handle_size(output);
                    let size = [
                        ((output_size[0] as u32) / 16) + 1,
                        ((output_size[1] as u32) / 16) + 1,
                        1,
                    ];
                    ctx.dispatch(api.device, size);
                }

                Ok(())
            },
        );
    }
}
