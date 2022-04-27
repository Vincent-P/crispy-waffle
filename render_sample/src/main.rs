use anyhow::Result;
use exo::pool::Handle;
use std::{ffi::CStr, os::raw::c_char};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

use render::{
    arrayvec::ArrayVec,
    vk, vulkan,
    vulkan::{
        contexts::{GraphicsContextMethods, TransferContextMethods},
        error::VulkanResult,
    },
};

const FRAME_QUEUE_LENGTH: usize = 2;

fn main() -> Result<()> {
    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let instance = vulkan::Instance::new(vulkan::InstanceSpec::default())?;
    let mut physical_devices = instance.get_physical_devices()?;

    let mut i_selected = None;
    for (i_device, physical_device) in (&physical_devices).into_iter().enumerate() {
        let device_name =
            unsafe { CStr::from_ptr(&physical_device.properties.device_name as *const c_char) };
        println!("Found device: {:?}", device_name);
        if i_selected.is_none()
            && physical_device.properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
        {
            println!(
                "Prioritizing device {:?} because it is a discrete GPU.",
                device_name
            );
            i_selected = Some(i_device);
        }
    }

    if i_selected.is_none() {
        i_selected = Some(0);
        let device_name =
            unsafe { CStr::from_ptr(&physical_devices[0].properties.device_name as *const c_char) };
        println!(
            "No discrete GPU found, defaulting to device #0 {:?}.",
            device_name
        )
    }

    let i_selected = i_selected.unwrap();

    let mut device = vulkan::Device::new(
        &instance,
        vulkan::DeviceSpec {
            physical_device: &mut physical_devices[i_selected],
            push_constant_size: 0,
        },
    )?;

    let mut surface = vulkan::Surface::new(&instance, &mut device, &window)?;
    let mut context_pools: [vulkan::ContextPool; FRAME_QUEUE_LENGTH] =
        [device.create_context_pool()?, device.create_context_pool()?];

    let mut i_frame: usize = 0;
    let fence = device.create_fence()?;

    let mut framebuffers =
        ArrayVec::<Handle<vulkan::Framebuffer>, { vulkan::MAX_SWAPCHAIN_IMAGES }>::new();
    for i_image in 0..surface.images.len() {
        framebuffers.push(device.create_framebuffer(
            &vulkan::FramebufferFormat {
                size: [surface.size[0], surface.size[1], 1],
                ..Default::default()
            },
            &[surface.images[i_image]],
            Handle::<vulkan::Image>::invalid(),
        )?);
    }

    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        let mut draw = || -> VulkanResult<_> {
            let current_frame = i_frame % FRAME_QUEUE_LENGTH;
            let context_pool = &mut context_pools[current_frame];

            let wait_value: u64 = if i_frame < FRAME_QUEUE_LENGTH {
                0
            } else {
                (i_frame - FRAME_QUEUE_LENGTH + 1) as u64
            };
            device.wait_for_fences(&[&fence], &[wait_value])?;

            device.reset_context_pool(context_pool)?;
            let mut outdated = device.acquire_next_swapchain(&mut surface)?;
            while outdated {
                device.wait_idle()?;
                surface.recreate_swapchain(&instance, &mut device)?;

                for i_image in 0..surface.images.len() {
                    device.destroy_framebuffer(framebuffers[i_image]);
                    framebuffers[i_image] = device.create_framebuffer(
                        &vulkan::FramebufferFormat {
                            size: [surface.size[0], surface.size[1], 1],
                            ..Default::default()
                        },
                        &[surface.images[i_image]],
                        Handle::<vulkan::Image>::invalid(),
                    )?;
                }
                outdated = device.acquire_next_swapchain(&mut surface)?;
            }

            let framebuffer = framebuffers[surface.current_image as usize];
            let mut ctx = device.get_graphics_context(context_pool)?;
            ctx.begin(&device)?;
            ctx.wait_for_acquired(&surface, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT);
            ctx.barrier(
                &mut device,
                surface.images[surface.current_image as usize],
                vulkan::ImageState::ColorAttachment,
            );
            ctx.begin_pass(
                &mut device,
                framebuffer,
                &[vulkan::LoadOp::ClearColor(
                    vulkan::ClearColorValue::Float32([1.0, 0.0, 1.0, 1.0]),
                )],
            )?;
            ctx.end_pass(&device);
            ctx.barrier(
                &mut device,
                surface.images[surface.current_image as usize],
                vulkan::ImageState::Present,
            );
            ctx.end(&device)?;
            ctx.prepare_present(&surface);
            device.submit(&ctx, &[&fence], &[(i_frame as u64) + 1])?;
            i_frame += 1;
            let outdated = device.present(&ctx, &surface)?;

            Ok(())
        };

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            Event::MainEventsCleared => {
                // Application update code.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw, in
                // applications which do not always need to. Applications that redraw continuously
                // can just render here instead.
                window.request_redraw();
            }

            Event::RedrawRequested(window_id) if window_id == window.id() => {
                if let Err(e) = draw() {
                    eprintln!("Error occured: {:?}", e);
                    *control_flow = ControlFlow::Exit;
                }
            }

            _ => (),
        }
    });

    device.wait_idle()?;

    for framebuffer in framebuffers {
        device.destroy_framebuffer(framebuffer);
    }

    device.destroy_fence(fence);
    for context_pool in context_pools {
        device.destroy_context_pool(context_pool);
    }

    surface.destroy(&instance, &mut device);
    device.destroy();
    instance.destroy();
    Ok(())
}
