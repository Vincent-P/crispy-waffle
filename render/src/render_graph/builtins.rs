use super::graph::*;
use crate::{vk, vulkan};
use exo::pool::Handle;
use std::{cell::RefCell, rc::Rc};

pub struct SwapchainPass {
    pub i_frame: usize,
    pub fence: vulkan::Fence,
    pub surface: vulkan::surface::Surface,
}

impl SwapchainPass {
    pub fn acquire_next_image(
        pass: &Rc<RefCell<Self>>,
        graph: &mut RenderGraph,
    ) -> Handle<TextureDesc> {
        let pass = Rc::clone(pass);
        let output = graph.output_image(TextureDesc::new(TextureSize::ScreenRelative([1.0, 1.0])));

        graph.raw_pass(
            move |graph: &mut RenderGraph, api: &mut PassApi, ctx: &mut vulkan::ComputeContext| {
                let mut pass = pass.borrow_mut();

                let mut outdated = api.device.acquire_next_swapchain(&mut pass.surface)?;
                while outdated {
                    api.device.wait_idle()?;
                    pass.surface.recreate_swapchain(
                        api.instance,
                        api.device,
                        &mut api.physical_devices[api.i_device],
                    )?;

                    for image in &pass.surface.images {
                        graph.resources.drop_image(*image);
                    }

                    outdated = api.device.acquire_next_swapchain(&mut pass.surface)?;
                }

                graph.resources.screen_size =
                    [pass.surface.size[0] as f32, pass.surface.size[1] as f32];
                graph
                    .resources
                    .set_image(output, pass.surface.current_image());

                ctx.base_context_mut().wait_for_acquired(
                    &pass.surface,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                );
                Ok(())
            },
        );

        output
    }

    pub fn present(pass: &Rc<RefCell<Self>>, graph: &mut RenderGraph) {
        let pass = Rc::clone(pass);
        graph.raw_pass(
            move |_graph: &mut RenderGraph, api: &mut PassApi, ctx: &mut vulkan::ComputeContext| {
                let mut pass_ref = pass.borrow_mut();

                ctx.base_context().barrier(
                    api.device,
                    pass_ref.surface.current_image(),
                    vulkan::ImageState::Present,
                );
                ctx.base_context().end(api.device)?;

                ctx.base_context_mut().prepare_present(&pass_ref.surface);
                api.device
                    .submit(&ctx, &[&pass_ref.fence], &[(pass_ref.i_frame as u64) + 1])?;

                pass_ref.i_frame += 1;

                let _outdated = api.device.present(&ctx, &pass_ref.surface)?;

                Ok(())
            },
        )
    }
}
