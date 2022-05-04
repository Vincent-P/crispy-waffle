use anyhow::Result;
use exo::{dynamic_array::DynamicArray, pool::Handle};
use std::{ffi::CStr, mem::size_of, os::raw::c_char, path::PathBuf};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

use render::{
    bindings,
    ring_buffer::*,
    vk, vulkan,
    vulkan::{
        contexts::{GraphicsContextMethods, TransferContextMethods},
        error::VulkanResult,
    },
};

use drawer2d::{drawer::*, rect::*};

const FRAME_QUEUE_LENGTH: usize = 2;
static mut DRAWER_VERTEX_MEMORY: [u8; 64 << 20] = [0; 64 << 20];
static mut DRAWER_INDEX_MEMORY: [u32; 8 << 20] = [0; 8 << 20];

fn main() -> Result<()> {
    let mut shader_dir = PathBuf::from(concat!(env!("OUT_DIR"), "/"));
    shader_dir.push("dummy_file");
    println!("Shaders directory: {:?}", &shader_dir);

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Cripsy Waffle")
        .build(&event_loop)
        .unwrap();

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
            push_constant_size: 8,
        },
    )?;

    let mut surface = vulkan::Surface::new(&instance, &mut device, &window)?;

    let ui_gfx_state = vulkan::GraphicsState {
        vertex_shader: device
            .create_shader(shader_dir.with_file_name("ui.vert.spv"))
            .unwrap(),
        fragment_shader: device
            .create_shader(shader_dir.with_file_name("ui.frag.spv"))
            .unwrap(),
        attachments_format: vulkan::FramebufferFormat {
            attachment_formats: DynamicArray::from([surface.format.format]),
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

    let mut context_pools: [vulkan::ContextPool; FRAME_QUEUE_LENGTH] =
        [device.create_context_pool()?, device.create_context_pool()?];

    let mut i_frame: usize = 0;
    let fence = device.create_fence()?;

    let mut framebuffers =
        DynamicArray::<Handle<vulkan::Framebuffer>, { vulkan::MAX_SWAPCHAIN_IMAGES }>::new();
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

    let mut uniform_buffer = RingBuffer::new(
        &mut device,
        RingBufferSpec {
            usages: vk::BufferUsageFlags::UNIFORM_BUFFER,
            frame_queue_length: FRAME_QUEUE_LENGTH,
            buffer_size: 1024,
        },
    )?;

    let mut dynamic_vertex_buffer = RingBuffer::new(
        &mut device,
        RingBufferSpec {
            usages: vk::BufferUsageFlags::STORAGE_BUFFER,
            frame_queue_length: FRAME_QUEUE_LENGTH,
            buffer_size: 128 << 20,
        },
    )?;

    let mut dynamic_index_buffer = RingBuffer::new(
        &mut device,
        RingBufferSpec {
            usages: vk::BufferUsageFlags::INDEX_BUFFER,
            frame_queue_length: FRAME_QUEUE_LENGTH,
            buffer_size: 32 << 20,
        },
    )?;

    let mut drawer = unsafe { Drawer::new(&mut DRAWER_VERTEX_MEMORY, &mut DRAWER_INDEX_MEMORY) };

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

            // Test
            drawer.clear();
            drawer.draw_colored_rect(
                Rect {
                    pos: [35.0, 35.0],
                    size: [50.0, 50.0],
                },
                0,
                ColorU32(0xFF0000FF),
            );
            drawer.draw_colored_rect(
                Rect {
                    pos: [50.0, 50.0],
                    size: [250.0, 250.0],
                },
                0,
                ColorU32(0xFF00FF00),
            );
            drawer.draw_colored_rect(
                Rect {
                    pos: [250.0, 250.0],
                    size: [150.0, 150.0],
                },
                0,
                ColorU32(0xFFFF0000),
            );
            // Test fin

            device.update_bindless_set();

            let mut ctx = device.get_graphics_context(context_pool)?;
            ctx.begin(&device)?;
            ctx.wait_for_acquired(&surface, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT);
            ctx.barrier(
                &mut device,
                surface.images[surface.current_image as usize],
                vulkan::ImageState::ColorAttachment,
            );

            let options =
                bindings::bind_shader_options(&mut device, &mut uniform_buffer, &ctx, 16)?;
            unsafe {
                let float_options = std::slice::from_raw_parts_mut(
                    (*options).as_ptr() as *mut f32,
                    (*options).len() / size_of::<f32>(),
                );
                float_options[0] = 1.0;
                float_options[1] = 1.0;
                float_options[2] = 0.0;
                float_options[3] = 1.0;
            }

            ctx.begin_pass(
                &mut device,
                framebuffers[surface.current_image as usize],
                &[vulkan::LoadOp::ClearColor(
                    vulkan::ClearColorValue::Float32([0.0, 0.0, 0.0, 1.0]),
                )],
            )?;
            ctx.set_viewport(
                &device,
                vk::ViewportBuilder::new()
                    .width(surface.size[0] as f32)
                    .height(surface.size[1] as f32)
                    .min_depth(0.0)
                    .max_depth(1.0),
            );
            ctx.set_scissor(
                &device,
                vk::Rect2DBuilder::new().extent(
                    *vk::Extent2DBuilder::new()
                        .width(surface.size[0] as u32)
                        .height(surface.size[1] as u32),
                ),
            );
            {
                let vertices = drawer.get_vertex_buffer();
                let (slice, vertices_offset) = dynamic_vertex_buffer.allocate(vertices.len(), 256);
                unsafe {
                    (*slice).copy_from_slice(vertices);
                }
                let indices = drawer.get_index_buffer();
                let indices_byte_length = indices.len() * size_of::<u32>();
                let (slice, indices_offset) =
                    dynamic_index_buffer.allocate(indices_byte_length, 256);
                unsafe {
                    let indices = std::slice::from_raw_parts(
                        indices.as_ptr() as *const u8,
                        indices_byte_length,
                    );
                    (*slice).copy_from_slice(indices);
                }
                #[repr(C, packed)]
                struct Options {
                    pub scale: [f32; 2],
                    pub translation: [f32; 2],
                    pub vertices_descriptor_index: u32,
                    pub primitive_bytes_offset: u32,
                }

                let options = bindings::bind_shader_options(
                    &mut device,
                    &mut uniform_buffer,
                    &ctx,
                    size_of::<Options>(),
                )?;
                unsafe {
                    let p_options =
                        std::slice::from_raw_parts_mut((*options).as_ptr() as *mut Options, 1);
                    p_options[0] = Options {
                        scale: [
                            2.0 / (surface.size[0] as f32),
                            2.0 / (surface.size[1] as f32),
                        ],
                        translation: [-1.0, -1.0],
                        vertices_descriptor_index: device
                            .buffers
                            .get(dynamic_vertex_buffer.buffer)
                            .storage_idx,
                        primitive_bytes_offset: vertices_offset,
                    };
                }
                let indices_offset = indices_offset as usize;
                assert!(indices_offset % size_of::<u32>() == 0);
                let index_offset = indices_offset / size_of::<u32>();
                ctx.bind_index_buffer(
                    &device,
                    dynamic_index_buffer.buffer,
                    vk::IndexType::UINT32,
                    index_offset,
                );
            }
            ctx.bind_graphics_pipeline(&device, ui_program, 0);
            ctx.draw_indexed(
                &device,
                vulkan::DrawIndexedOptions {
                    vertex_count: drawer.get_index_offset() as u32,
                    ..Default::default()
                },
            );

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
            let _outdated = device.present(&ctx, &surface)?;

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

    for framebuffer in &framebuffers {
        device.destroy_framebuffer(*framebuffer);
    }

    device.destroy_fence(fence);
    for context_pool in context_pools {
        device.destroy_context_pool(context_pool);
    }

    surface.destroy(&instance, &mut device);

    device.destroy_program(ui_program);
    // device.destroy_shader(ui_vertex);
    // device.destroy_shader(ui_frag);

    device.destroy();
    instance.destroy();
    Ok(())
}
