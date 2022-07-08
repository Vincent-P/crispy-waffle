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
        let output = graph.output_image(TextureDesc::new(
            String::from("swapchain desc"),
            TextureSize::ScreenRelative([1.0, 1.0]),
        ));

        graph.raw_pass(
            move |graph: &mut RenderGraph, api: &mut PassApi, ctx: &mut vulkan::ComputeContext| {
                let mut pass = pass.borrow_mut();

                let mut swapchain_is_outdated =
                    api.device.acquire_next_swapchain(&mut pass.surface)?;
                pass.surface.is_outdated = pass.surface.is_outdated || swapchain_is_outdated;
                while pass.surface.is_outdated {
                    api.device.wait_idle()?;

                    for image in &pass.surface.images {
                        graph.resources.drop_image(*image);
                    }

                    pass.surface.recreate_swapchain(
                        api.instance,
                        api.device,
                        &mut api.physical_devices[api.i_device],
                    )?;

                    pass.surface.is_outdated =
                        api.device.acquire_next_swapchain(&mut pass.surface)?;
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

    pub fn present(pass: &Rc<RefCell<Self>>, graph: &mut RenderGraph, signal_value: u64) {
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
                let signal_values = [signal_value];
                api.device
                    .submit(&ctx, &[&pass_ref.fence], &signal_values)?;

                pass_ref.i_frame += 1;

                let _swapchain_is_outdated = api.device.present(&ctx, &mut pass_ref.surface)?;

                Ok(())
            },
        )
    }
}

pub fn copy_image(
    graph: &mut RenderGraph,
    input: Handle<TextureDesc>,
    output: Handle<TextureDesc>,
) {
    assert!(input != output);
    graph.raw_pass(
        move |graph: &mut RenderGraph,
              api: &mut PassApi,
              ctx: &mut vulkan::ComputeContext|
              -> vulkan::VulkanResult<()> {
            let input = graph.resources.resolve_image(api.device, input)?;
            let output = graph.resources.resolve_image(api.device, output)?;

            ctx.base_context()
                .barrier(api.device, input, vulkan::ImageState::TransferSrc);
            ctx.base_context()
                .barrier(api.device, output, vulkan::ImageState::TransferDst);

            ctx.transfer().copy_image(api.device, input, output);
            Ok(())
        },
    );
}

pub fn blit_image(
    graph: &mut RenderGraph,
    input: Handle<TextureDesc>,
    output: Handle<TextureDesc>,
) {
    assert!(input != output);
    graph.raw_pass(
        move |graph: &mut RenderGraph,
              api: &mut PassApi,
              ctx: &mut vulkan::ComputeContext|
              -> vulkan::VulkanResult<()> {
            let input = graph.resources.resolve_image(api.device, input)?;
            let output = graph.resources.resolve_image(api.device, output)?;

            ctx.base_context()
                .barrier(api.device, input, vulkan::ImageState::TransferSrc);
            ctx.base_context()
                .barrier(api.device, output, vulkan::ImageState::TransferDst);

            ctx.transfer().blit_image(api.device, input, output);
            Ok(())
        },
    );
}
