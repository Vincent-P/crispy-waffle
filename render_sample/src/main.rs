#![cfg_attr(debug_assertions, windows_subsystem = "console")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use exo::{dynamic_array::DynamicArray, pool::Handle};
use raw_window_handle::HasRawWindowHandle;
use std::{ffi::CStr, mem::size_of, os::raw::c_char, path::PathBuf, rc::Rc};
use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent},
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

use drawer2d::{drawer::*, font::*, glyph_cache::GlyphEvent, rect::*};

mod profile {
    #[cfg(feature = "optick")]
    pub fn init() {}

    #[cfg(feature = "optick")]
    pub fn next_frame() {
        optick::next_frame();
    }

    #[cfg(feature = "optick")]
    macro_rules! scope {
        ($name:expr) => {
            optick::event!($name);
        };
    }
    #[cfg(feature = "tracy")]
    pub fn init() {
        tracy_client::Client::start();
    }

    #[cfg(feature = "tracy")]
    pub fn next_frame() {
        tracy_client::Client::running().unwrap().frame_mark();
    }

    #[cfg(feature = "tracy")]
    macro_rules! scope {
        ($name:expr) => {
            let _span = tracy_client::span!($name);
        };
    }

    #[cfg(not(any(feature = "optick", feature = "tracy",)))]
    pub fn init() {}

    #[cfg(not(any(feature = "optick", feature = "tracy",)))]
    pub fn next_frame() {}

    #[cfg(not(any(feature = "optick", feature = "tracy",)))]
    macro_rules! scope {
        ($name:expr) => {};
        ($name:expr, $data:expr) => {};
    }

    pub(crate) use scope;
}

const FRAME_QUEUE_LENGTH: usize = 2;
static mut DRAWER_VERTEX_MEMORY: [u8; 64 << 10] = [0; 64 << 10];
static mut DRAWER_INDEX_MEMORY: [u32; 8 << 10] = [0; 8 << 10];
const GLYPH_ATLAS_RESOLUTION: i32 = 256;

struct Renderer {
    instance: vulkan::Instance,
    physical_devices: DynamicArray<vulkan::PhysicalDevice, { vulkan::MAX_PHYSICAL_DEVICES }>,
    device: vulkan::Device,
    i_device: usize,
    surface: vulkan::Surface,
    context_pools: [vulkan::ContextPool; FRAME_QUEUE_LENGTH],
    fence: vulkan::Fence,
    i_frame: usize,
    framebuffers: DynamicArray<Handle<vulkan::Framebuffer>, { vulkan::MAX_SWAPCHAIN_IMAGES }>,
    ui_program: Handle<vulkan::GraphicsProgram>,
    uniform_buffer: RingBuffer,
    dynamic_vertex_buffer: RingBuffer,
    dynamic_index_buffer: RingBuffer,
    upload_buffer: RingBuffer,
    glyph_atlas: Handle<vulkan::Image>,
}

impl Renderer {
    pub fn new<WindowHandle: HasRawWindowHandle>(
        window_handle: &WindowHandle,
        shader_dir: PathBuf,
    ) -> vulkan::VulkanResult<Self> {
        let instance = vulkan::Instance::new(vulkan::InstanceSpec {
            enable_validation: cfg!(debug_assertions),
            ..Default::default()
        })?;
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
            let device_name = unsafe {
                CStr::from_ptr(&physical_devices[0].properties.device_name as *const c_char)
            };
            println!(
                "No discrete GPU found, defaulting to device #0 {:?}.",
                device_name
            )
        }

        let i_selected = i_selected.unwrap();
        let physical_device = &mut physical_devices[i_selected];

        let mut device = vulkan::Device::new(
            &instance,
            vulkan::DeviceSpec {
                push_constant_size: 8,
            },
            physical_device,
        )?;

        let surface = vulkan::Surface::new(&instance, &mut device, physical_device, window_handle)?;

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

        let context_pools: [vulkan::ContextPool; FRAME_QUEUE_LENGTH] =
            [device.create_context_pool()?, device.create_context_pool()?];

        let i_frame: usize = 0;
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

        let uniform_buffer = RingBuffer::new(
            &mut device,
            RingBufferSpec {
                usages: vk::BufferUsageFlags::UNIFORM_BUFFER,
                memory_usage: vulkan::buffer::MemoryUsageFlags::FAST_DEVICE_ACCESS,
                frame_queue_length: FRAME_QUEUE_LENGTH,
                buffer_size: 1024,
            },
        )?;

        let dynamic_vertex_buffer = RingBuffer::new(
            &mut device,
            RingBufferSpec {
                usages: vk::BufferUsageFlags::STORAGE_BUFFER,
                memory_usage: vulkan::buffer::MemoryUsageFlags::FAST_DEVICE_ACCESS,
                frame_queue_length: FRAME_QUEUE_LENGTH,
                buffer_size: 128 << 20,
            },
        )?;

        let dynamic_index_buffer = RingBuffer::new(
            &mut device,
            RingBufferSpec {
                usages: vk::BufferUsageFlags::INDEX_BUFFER,
                memory_usage: vulkan::buffer::MemoryUsageFlags::FAST_DEVICE_ACCESS,
                frame_queue_length: FRAME_QUEUE_LENGTH,
                buffer_size: 32 << 20,
            },
        )?;

        let upload_buffer = RingBuffer::new(
            &mut device,
            RingBufferSpec {
                usages: vk::BufferUsageFlags::TRANSFER_SRC,
                memory_usage: vulkan::buffer::MemoryUsageFlags::UPLOAD,
                frame_queue_length: FRAME_QUEUE_LENGTH,
                buffer_size: 32 << 20,
            },
        )?;

        let glyph_atlas = device.create_image(vulkan::ImageSpec {
            size: [GLYPH_ATLAS_RESOLUTION, GLYPH_ATLAS_RESOLUTION, 1],
            format: vk::Format::R8_UNORM,
            ..Default::default()
        })?;

        Ok(Self {
            instance,
            physical_devices,
            device,
            i_device: i_selected,
            surface,
            context_pools,
            fence,
            i_frame,
            framebuffers,
            ui_program,
            uniform_buffer,
            dynamic_vertex_buffer,
            dynamic_index_buffer,
            upload_buffer,
            glyph_atlas,
        })
    }

    pub fn destroy(mut self) {
        self.device.wait_idle().unwrap();

        for framebuffer in &self.framebuffers {
            self.device.destroy_framebuffer(*framebuffer);
        }

        self.device.destroy_fence(self.fence);
        for context_pool in self.context_pools {
            self.device.destroy_context_pool(context_pool);
        }

        self.surface.destroy(&self.instance, &mut self.device);

        self.device.destroy_program(self.ui_program);
        // device.destroy_shader(ui_vertex);
        // device.destroy_shader(ui_frag);

        self.device.destroy();
        self.instance.destroy();
    }

    pub fn get_glyph_atlas_descriptor(&self) -> u32 {
        let image = self.device.images.get(self.glyph_atlas);
        image.full_view.sampled_idx
    }

    pub fn draw(&mut self, mut drawer: Option<&mut Drawer>) -> VulkanResult<()> {
        profile::next_frame();

        let current_frame = self.i_frame % FRAME_QUEUE_LENGTH;
        let context_pool = &mut self.context_pools[current_frame];

        let wait_value: u64 = if self.i_frame < FRAME_QUEUE_LENGTH {
            0
        } else {
            (self.i_frame - FRAME_QUEUE_LENGTH + 1) as u64
        };
        self.device.wait_for_fences(&[&self.fence], &[wait_value])?;

        self.device.reset_context_pool(context_pool)?;
        let mut outdated = self.device.acquire_next_swapchain(&mut self.surface)?;
        while outdated {
            profile::scope!("resize");
            self.device.wait_idle()?;
            self.surface.recreate_swapchain(
                &self.instance,
                &mut self.device,
                &mut self.physical_devices[self.i_device],
            )?;

            for i_image in 0..self.surface.images.len() {
                self.device.destroy_framebuffer(self.framebuffers[i_image]);
                self.framebuffers[i_image] = self.device.create_framebuffer(
                    &vulkan::FramebufferFormat {
                        size: [self.surface.size[0], self.surface.size[1], 1],
                        ..Default::default()
                    },
                    &[self.surface.images[i_image]],
                    Handle::<vulkan::Image>::invalid(),
                )?;
            }
            outdated = self.device.acquire_next_swapchain(&mut self.surface)?;
        }

        self.device.update_bindless_set();
        self.uniform_buffer.start_frame();
        self.dynamic_vertex_buffer.start_frame();
        self.dynamic_index_buffer.start_frame();
        self.upload_buffer.start_frame();

        let mut ctx = self.device.get_graphics_context(context_pool)?;
        {
            profile::scope!("command recording");
            ctx.begin(&self.device)?;
            ctx.wait_for_acquired(
                &self.surface,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            );

            if self.i_frame == 0 {
                ctx.barrier(
                    &mut self.device,
                    self.glyph_atlas,
                    vulkan::ImageState::TransferDst,
                );
                ctx.clear_image(
                    &self.device,
                    self.glyph_atlas,
                    vulkan::ClearColorValue::Float32([0.0, 0.0, 0.0, 1.0]),
                );
            }

            ctx.barrier(
                &mut self.device,
                self.surface.images[self.surface.current_image as usize],
                vulkan::ImageState::ColorAttachment,
            );
            if let Some(ref mut drawer) = drawer {
                let mut glyphs_to_upload: Vec<vulkan::BufferImageCopy> = Vec::with_capacity(32);
                drawer.get_glyph_cache_mut().process_events(
                    |cache_event, glyph_image, glyph_atlas_pos| {
                        // Copy new glyphs to the upload buffer
                        if let GlyphEvent::New(_, _) = cache_event {
                            if let Some(atlas_pos) = glyph_atlas_pos {
                                let image = glyph_image.unwrap();
                                let (slice, offset) =
                                    self.upload_buffer.allocate(image.data.len(), 256);
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
                    },
                );
                if !glyphs_to_upload.is_empty() {
                    ctx.barrier(
                        &mut self.device,
                        self.glyph_atlas,
                        vulkan::ImageState::TransferDst,
                    );
                    ctx.copy_buffer_to_image(
                        &self.device,
                        self.upload_buffer.buffer,
                        self.glyph_atlas,
                        &glyphs_to_upload,
                    );
                    ctx.barrier(
                        &mut self.device,
                        self.glyph_atlas,
                        vulkan::ImageState::GraphicsShaderRead,
                    );
                }
            }
            ctx.begin_pass(
                &mut self.device,
                self.framebuffers[self.surface.current_image as usize],
                &[vulkan::LoadOp::ClearColor(
                    vulkan::ClearColorValue::Float32([0.0, 0.0, 0.0, 1.0]),
                )],
            )?;
            ctx.set_viewport(
                &self.device,
                vk::ViewportBuilder::new()
                    .width(self.surface.size[0] as f32)
                    .height(self.surface.size[1] as f32)
                    .min_depth(0.0)
                    .max_depth(1.0),
            );
            ctx.set_scissor(
                &self.device,
                vk::Rect2DBuilder::new().extent(
                    *vk::Extent2DBuilder::new()
                        .width(self.surface.size[0] as u32)
                        .height(self.surface.size[1] as u32),
                ),
            );
            if let Some(drawer) = drawer {
                let vertices = drawer.get_vertices();
                let (slice, vertices_offset) = self
                    .dynamic_vertex_buffer
                    .allocate(vertices.len(), Drawer::get_primitive_alignment());
                unsafe {
                    profile::scope!("copy drawer vertices");
                    (*slice).copy_from_slice(vertices);
                }
                let indices = drawer.get_indices();
                let indices_byte_length = indices.len() * size_of::<u32>();
                let (slice, indices_offset) = self
                    .dynamic_index_buffer
                    .allocate(indices_byte_length, size_of::<u32>());
                unsafe {
                    profile::scope!("copy drawer indices");
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
                    &mut self.device,
                    &mut self.uniform_buffer,
                    &ctx,
                    size_of::<Options>(),
                )?;
                unsafe {
                    let p_options =
                        std::slice::from_raw_parts_mut((*options).as_ptr() as *mut Options, 1);
                    p_options[0] = Options {
                        scale: [
                            2.0 / (self.surface.size[0] as f32),
                            2.0 / (self.surface.size[1] as f32),
                        ],
                        translation: [-1.0, -1.0],
                        vertices_descriptor_index: self
                            .device
                            .buffers
                            .get(self.dynamic_vertex_buffer.buffer)
                            .storage_idx,
                        primitive_bytes_offset: vertices_offset,
                        glyph_atlas_descriptor: self.get_glyph_atlas_descriptor(),
                    };
                }
                ctx.bind_index_buffer(
                    &self.device,
                    self.dynamic_index_buffer.buffer,
                    vk::IndexType::UINT32,
                    indices_offset as usize,
                );
                ctx.bind_graphics_pipeline(&self.device, self.ui_program, 0);
                ctx.draw_indexed(
                    &self.device,
                    vulkan::DrawIndexedOptions {
                        vertex_count: indices.len() as u32,
                        ..Default::default()
                    },
                );
            }

            ctx.end_pass(&self.device);
            ctx.barrier(
                &mut self.device,
                self.surface.images[self.surface.current_image as usize],
                vulkan::ImageState::Present,
            );
            ctx.end(&self.device)?;
        }

        {
            profile::scope!("submit and present");
            ctx.prepare_present(&self.surface);
            self.device
                .submit(&ctx, &[&self.fence], &[(self.i_frame as u64) + 1])?;
            self.i_frame += 1;
            let _outdated = self.device.present(&ctx, &self.surface)?;
        }

        Ok(())
    }
}

struct App {
    pub renderer: Renderer,
    pub drawer: Drawer<'static>,
    pub ui: ui::Ui,
}

impl App {
    pub fn draw_menubar(&mut self, fullscreen: Rect) {
        let ui_face = self.ui.theme.face();
        let (label_run, label_layout) = self.drawer.shape_and_layout_text(&ui_face, "Open File");
        let label_metrics = label_run.metrics();

        let (menubar_rect, content_rect) =
            fullscreen.split_top_pixels(label_metrics.ascent + label_metrics.descent + 20.0);

        self.drawer
            .draw_colored_rect(menubar_rect, 0, ColorU32::greyscale(0xE8));

        let (label_rect, menubar_rest_rect) =
            menubar_rect.split_left_pixels(label_layout.size()[0] + 2.0 * 10.0);
        let label_rect = label_rect.inset(5.0);

        self.ui.button(
            &mut self.drawer,
            ui::Button {
                label: "Open File",
                pos: label_rect.pos,
                margins: [2.0, 5.0],
            },
        );
    }

    pub fn draw_ui(&mut self, viewport_size: [f32; 2]) {
        profile::scope!("ui draw");
        self.drawer.clear();
        self.ui.new_frame();

        let fullscreen = Rect {
            pos: [0.0, 0.0],
            size: viewport_size,
        };

        self.draw_menubar(fullscreen);

        self.drawer.draw_label(
            &self.ui.theme.face(),
            &format!("mouse {:?}", self.ui.mouse_position()),
            fullscreen,
            0,
        );

        let button_width = (viewport_size[0] * 0.33).max(400.0);

        if self.ui.button(
            &mut self.drawer,
            ui::Button {
                label: &format!("TtegtI     (  {}x{} )", button_width, 50.0),
                pos: [250.0, 250.0],
                margins: [button_width * 0.5, 2.5],
            },
        ) {
            println!("button pressed!");
        }

        self.ui.end_frame();
    }

    pub fn draw_gpu(&mut self) -> VulkanResult<()> {
        self.renderer.draw(Some(&mut self.drawer))
    }
}

fn main() -> Result<()> {
    profile::init();

    let mut shader_dir = PathBuf::from(concat!(env!("OUT_DIR"), "/"));
    shader_dir.push("dummy_file");
    println!("Shaders directory: {:?}", &shader_dir);

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Cripsy Waffle")
        .build(&event_loop)
        .unwrap();

    let ui_font = Font::from_file(
        shader_dir
            .with_file_name("iAWriterQuattroS-Regular.ttf")
            .to_str()
            .unwrap(),
        0,
    )
    .unwrap();

    let renderer = Renderer::new(&window, shader_dir)?;
    let drawer = Drawer::new(
        unsafe { &mut DRAWER_VERTEX_MEMORY },
        unsafe { &mut DRAWER_INDEX_MEMORY },
        [GLYPH_ATLAS_RESOLUTION, GLYPH_ATLAS_RESOLUTION],
        renderer.get_glyph_atlas_descriptor(),
    );
    let ui = ui::Ui::new(Rc::new(ui_font), 24.0);

    let mut app = App {
        renderer,
        drawer,
        ui,
    };

    event_loop.run_return(|event, _, control_flow| {
        profile::scope!("window event");

        // Only runs event loop when there are events, ControlFlow::Poll runs the loop even when empty
        *control_flow = ControlFlow::Poll;

        match event {
            // Close when exit is requested
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            Event::WindowEvent {
                event:
                    WindowEvent::CursorMoved {
                        device_id,
                        position,
                        ..
                    },
                window_id,
            } => {
                let mouse_position: winit::dpi::LogicalPosition<f32> = position.to_logical(1.0);
                app.ui
                    .set_mouse_position([mouse_position.x, mouse_position.y]);
            }

            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                window_id,
            } => {
                if button == MouseButton::Left {
                    app.ui
                        .set_left_mouse_button_pressed(state == ElementState::Pressed);
                }
            }

            Event::RedrawRequested(window_id) if window_id == window.id() => {}

            Event::MainEventsCleared => {
                let window_size: winit::dpi::LogicalSize<f32> = window.inner_size().to_logical(1.0);

                app.draw_ui([window_size.width, window_size.height]);

                if let Err(e) = app.draw_gpu() {
                    eprintln!("Renderer error: {:?}", e);
                    *control_flow = ControlFlow::Exit;
                }
            }

            _ => (),
        }
    });

    app.renderer.destroy();

    Ok(())
}
