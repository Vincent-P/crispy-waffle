#![cfg_attr(debug_assertions, windows_subsystem = "console")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

mod custom_ui {
    const FPS_HISTOGRAM_LENGTH: usize = 512;
    pub struct FpsHistogram {
        frame_times: [f32; FPS_HISTOGRAM_LENGTH],
    }

    impl FpsHistogram {
        pub fn new() -> Self {
            Self {
                frame_times: [0.0; FPS_HISTOGRAM_LENGTH],
            }
        }

        pub fn push_time(&mut self, dt: f32) {
            self.frame_times.rotate_right(1);
            self.frame_times[0] = dt;
        }
    }

    impl Default for FpsHistogram {
        fn default() -> Self {
            Self::new()
        }
    }

    pub mod widgets {
        use drawer2d::{drawer::*, rect::*};

        pub struct FpsHistogram<'a> {
            pub histogram: &'a super::FpsHistogram,
            pub rect: Rect,
        }

        #[allow(clippy::excessive_precision)]
        fn turbo_colormap(x: f32) -> [f32; 3] {
            const RED_VEC4: [f32; 4] = [0.13572138, 4.61539260, -42.66032258, 132.13108234];
            const GREEN_VEC4: [f32; 4] = [0.09140261, 2.19418839, 4.84296658, -14.18503333];
            const BLUE_VEC4: [f32; 4] = [0.10667330, 12.64194608, -60.58204836, 110.36276771];
            const RED_VEC2: [f32; 2] = [-152.94239396, 59.28637943];
            const GREEN_VEC2: [f32; 2] = [4.27729857, 2.82956604];
            const BLUE_VEC2: [f32; 2] = [-89.90310912, 27.34824973];
            let dot4 =
                |a: [f32; 4], b: [f32; 4]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
            let dot2 = |a: [f32; 2], b: [f32; 2]| a[0] * b[0] + a[1] * b[1];

            let x = x.clamp(0.0, 1.0);
            let v4 = [1.0, x, x * x, x * x * x];
            let v2 = [v4[2] * v4[2], v4[3] * v4[2]];

            [
                dot4(v4, RED_VEC4) + dot2(v2, RED_VEC2),
                dot4(v4, GREEN_VEC4) + dot2(v2, GREEN_VEC2),
                dot4(v4, BLUE_VEC4) + dot2(v2, BLUE_VEC2),
            ]
        }

        // https://www.asawicki.info/news_1758_an_idea_for_visualization_of_frame_times
        pub fn histogram(ui: &mut ui::Ui, drawer: &mut Drawer, widget: FpsHistogram) {
            let mut cursor = [
                widget.rect.pos[0] + widget.rect.size[0],
                widget.rect.pos[1] + widget.rect.size[1],
            ];

            drawer.draw_colored_rect(
                ColoredRect::new(widget.rect).color(ColorU32::from_f32(0.0, 0.0, 0.0, 0.5)),
            );
            ui.state.add_rect_to_last_container(widget.rect);

            for dt in widget.histogram.frame_times {
                if cursor[0] < widget.rect.pos[0] {
                    break;
                }

                let target_fps: f32 = 144.0;
                let max_frame_time: f32 = 1.0 / 15.0; // in seconds

                let rect_width = dt / (1.0 / target_fps);
                let height_factor = (dt.log2() - (1.0 / target_fps).log2())
                    / ((max_frame_time).log2() - (1.0 / target_fps).log2());
                let rect_height = height_factor.clamp(0.1, 1.0) * widget.rect.size[1];
                let rect_color = turbo_colormap(dt / (1.0 / 120.0));
                let rect_color =
                    ColorU32::from_f32(rect_color[0], rect_color[1], rect_color[2], 1.0);

                let rect_width = rect_width.max(1.0);
                let rect_height = rect_height.max(1.0);

                cursor[0] -= rect_width;

                let rect = Rect {
                    pos: [cursor[0].ceil(), (cursor[1] - rect_height).ceil()],
                    size: [rect_width, rect_height],
                };
                drawer.draw_colored_rect(ColoredRect::new(rect).color(rect_color));
                ui.state.add_rect_to_last_container(rect);
            }
        }
    }
}

mod custom_render {
    use drawer2d::drawer::Drawer;
    use exo::{dynamic_array::DynamicArray, pool::Handle};
    use render::{bindings, render_graph::graph::*, vk, vulkan};
    use std::{mem::size_of, path::PathBuf, rc::Rc};

    pub struct UiPass {
        pub glyph_atlas: Handle<vulkan::Image>,
        ui_program: Handle<vulkan::GraphicsProgram>,
    }

    impl UiPass {
        pub fn new(
            device: &mut vulkan::Device,
            glyph_atlas_size: [i32; 2],
        ) -> vulkan::VulkanResult<Self> {
            let mut shader_dir = PathBuf::from(concat!(env!("OUT_DIR"), "/"));
            shader_dir.push("dummy_file");

            let ui_gfx_state = vulkan::GraphicsState {
                vertex_shader: device
                    .create_shader(shader_dir.with_file_name("ui.vert.spv"))
                    .unwrap(),
                fragment_shader: device
                    .create_shader(shader_dir.with_file_name("ui.frag.spv"))
                    .unwrap(),
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
                                let (slice, offset) =
                                    api.upload_buffer.allocate(image.data.len(), 256);
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
                let glyph_atlas_descriptor =
                    api.device.images.get(glyph_atlas).full_view.sampled_idx;
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
}

use anyhow::Result;
use drawer2d::{drawer::*, font::*, rect::*};
use exo::dynamic_array::DynamicArray;
use raw_window_handle::HasRawWindowHandle;
use render::{render_graph, ring_buffer::*, vk, vulkan, vulkan::error::VulkanResult};
use std::{cell::RefCell, ffi::CStr, os::raw::c_char, path::PathBuf, rc::Rc, time::Instant};
use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

const FRAME_QUEUE_LENGTH: usize = 2;
static mut DRAWER_VERTEX_MEMORY: [u8; 64 << 10] = [0; 64 << 10];
static mut DRAWER_INDEX_MEMORY: [u32; 8 << 10] = [0; 8 << 10];
const GLYPH_ATLAS_RESOLUTION: i32 = 4096;

struct Renderer {
    instance: vulkan::Instance,
    physical_devices: DynamicArray<vulkan::PhysicalDevice, { vulkan::MAX_PHYSICAL_DEVICES }>,
    device: vulkan::Device,
    i_device: usize,
    context_pools: [vulkan::ContextPool; FRAME_QUEUE_LENGTH],
    uniform_buffer: RingBuffer,
    dynamic_vertex_buffer: RingBuffer,
    dynamic_index_buffer: RingBuffer,
    upload_buffer: RingBuffer,
    render_graph: render_graph::graph::RenderGraph,
    ui_node: custom_render::UiPass,
    swapchain_node: Rc<RefCell<render_graph::builtins::SwapchainPass>>,
    frame_count: usize,
}

impl Renderer {
    pub fn new<WindowHandle: HasRawWindowHandle>(
        window_handle: &WindowHandle,
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
        let swapchain_node = Rc::new(RefCell::new(render_graph::builtins::SwapchainPass {
            i_frame: 0,
            fence: device.create_fence()?,
            surface,
        }));

        let context_pools: [vulkan::ContextPool; FRAME_QUEUE_LENGTH] =
            [device.create_context_pool()?, device.create_context_pool()?];

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

        let render_graph = render_graph::graph::RenderGraph::new();
        let ui_node = custom_render::UiPass::new(
            &mut device,
            [GLYPH_ATLAS_RESOLUTION, GLYPH_ATLAS_RESOLUTION],
        )?;

        Ok(Self {
            instance,
            physical_devices,
            device,
            i_device: i_selected,
            context_pools,
            uniform_buffer,
            dynamic_vertex_buffer,
            dynamic_index_buffer,
            upload_buffer,
            ui_node,
            render_graph,
            swapchain_node,
            frame_count: 0,
        })
    }

    pub fn destroy(mut self) {
        self.device.wait_idle().unwrap();

        self.device
            .destroy_fence(&self.swapchain_node.borrow().fence);
        for context_pool in self.context_pools {
            self.device.destroy_context_pool(context_pool);
        }

        self.swapchain_node
            .borrow_mut()
            .surface
            .destroy(&self.instance, &mut self.device);

        self.device.destroy();
        self.instance.destroy();
    }

    pub fn render(&mut self, drawer: Option<&Rc<Drawer<'static>>>) -> VulkanResult<()> {
        use render_graph::{
            builtins,
            graph::{TextureDesc, TextureSize},
        };

        profile::next_frame();

        let i_frame = {
            let b = self.swapchain_node.borrow();
            b.i_frame
        };

        let intermediate_buffer = self.render_graph.output_image(TextureDesc::new(
            String::from("render buffer desc"),
            TextureSize::ScreenRelative([1.0, 1.0]),
        ));

        if let Some(drawer) = drawer {
            self.ui_node
                .register_graph(&mut self.render_graph, intermediate_buffer, drawer);
        }

        let swapchain_output = builtins::SwapchainPass::acquire_next_image(
            &self.swapchain_node,
            &mut self.render_graph,
        );

        builtins::blit_image(
            &mut self.render_graph,
            intermediate_buffer,
            swapchain_output,
        );

        builtins::SwapchainPass::present(&self.swapchain_node, &mut self.render_graph);

        let current_frame = i_frame % FRAME_QUEUE_LENGTH;
        let context_pool = &mut self.context_pools[current_frame];

        let wait_value: u64 = if i_frame < FRAME_QUEUE_LENGTH {
            0
        } else {
            (i_frame - FRAME_QUEUE_LENGTH + 1) as u64
        };
        {
            let fence = &self.swapchain_node.borrow().fence;
            self.device.wait_for_fences(&[fence], &[wait_value])?;
        }

        self.device.reset_context_pool(context_pool)?;

        self.device.update_bindless_set();
        self.uniform_buffer.start_frame();
        self.dynamic_vertex_buffer.start_frame();
        self.dynamic_index_buffer.start_frame();
        self.upload_buffer.start_frame();

        let pass_api = render_graph::graph::PassApi {
            instance: &self.instance,
            physical_devices: &mut self.physical_devices,
            i_device: self.i_device,
            device: &mut self.device,
            uniform_buffer: &mut self.uniform_buffer,
            dynamic_vertex_buffer: &mut self.dynamic_vertex_buffer,
            dynamic_index_buffer: &mut self.dynamic_index_buffer,
            upload_buffer: &mut self.upload_buffer,
        };

        self.render_graph.execute(pass_api, context_pool)?;
        self.frame_count += 1;

        Ok(())
    }

    pub fn get_glyph_atlas_descriptor(&self) -> u32 {
        self.device
            .images
            .get(self.ui_node.glyph_atlas)
            .full_view
            .sampled_idx
    }
}

struct App {
    pub renderer: Renderer,
    pub drawer: Rc<Drawer<'static>>,
    pub ui: ui::Ui,
    fps_histogram: custom_ui::FpsHistogram,
    docking: ui_docking::Docking,
    show_fps: bool,
    font_size: f32,
}

impl App {
    pub fn update(&mut self, dt: f32) {
        self.fps_histogram.push_time(dt);
    }
}

pub fn draw_menubar(drawer: &mut Drawer, ui: &mut ui::Ui, content_rect: &mut Rect) {
    let em = ui.theme.font_size;

    let top_margin_rect = content_rect.split_top(0.25 * em);

    let mut middle_menubar = content_rect.split_top(1.5 * em);
    drawer.draw_colored_rect(ColoredRect::new(middle_menubar).color(ColorU32::greyscale(0xE8)));

    let mut menubar_split = rectsplit(&mut middle_menubar, SplitDirection::Left);

    menubar_split.split(0.5 * em);
    let _pressed_one = ui.rectbutton(
        drawer,
        &mut menubar_split,
        ui::RectButton { label: "Open File" },
    );

    menubar_split.split(0.5 * em);
    let _pressed_two = ui.rectbutton(
        drawer,
        &mut menubar_split,
        ui::RectButton { label: "Second" },
    );

    let bottom_margin_rect = content_rect.split_top(0.25 * em);

    drawer.draw_colored_rect(ColoredRect::new(top_margin_rect).color(ColorU32::greyscale(0xE8)));
    drawer.draw_colored_rect(ColoredRect::new(bottom_margin_rect).color(ColorU32::greyscale(0xE8)));
}

impl App {
    pub fn draw_ui(&mut self, viewport_size: [f32; 2]) {
        profile::scope!("ui draw");
        let drawer = Rc::get_mut(&mut self.drawer).unwrap();

        drawer.clear();
        self.ui.new_frame();

        let em = self.ui.em();

        let fullscreen = Rect {
            pos: [0.0, 0.0],
            size: viewport_size,
        };
        let mut content_rect = fullscreen;

        draw_menubar(drawer, &mut self.ui, &mut content_rect);
        let footer_rect = content_rect.split_bottom(3.0 * em);

        // -- Content

        let draw_area = |ui: &mut ui::Ui,
                         drawer: &mut Drawer,
                         rect: Rect,
                         color: ColorU32,
                         label: Option<&str>| {
            drawer.draw_colored_rect(ColoredRect::new(rect).color(color));
            drawer.draw_colored_rect(
                ColoredRect::new(rect.inset(1.0 * em))
                    .color(ColorU32::from_f32(0.25, 0.25, 0.25, 0.25)),
            );

            if let Some(label_str) = label {
                drawer.draw_label(
                    &ui.theme.face(),
                    label_str,
                    rect,
                    0,
                    ColorU32::greyscale(0x00),
                );
            }
        };

        drawer.draw_label(
            &self.ui.theme.face(),
            "Surprise",
            content_rect,
            0,
            ColorU32::red(),
        );

        self.docking.begin_docking(&self.ui, content_rect);

        if let Some(content1_rect) = self.docking.tabview("Content 1") {
            draw_area(
                &mut self.ui,
                drawer,
                content1_rect,
                ColorU32::from_f32(0.53, 0.13, 0.13, 1.0),
                Some(&format!(
                    "content number uno: frame {}",
                    self.renderer.frame_count
                )),
            );

            let mut cursor = content1_rect.pos;
            cursor = [cursor[0] + 2.0 * em, cursor[1] + 1.0 * em];
            if self.ui.button(
                drawer,
                ui::Button::with_label("Toggle show histogram").rect(Rect {
                    pos: cursor,
                    size: [20.0 * em, 1.5 * em],
                }),
            ) {
                self.show_fps = !self.show_fps;
            }
            cursor[1] += 3.0 * em;
        }

        if let Some(content2_rect) = self.docking.tabview("Content 2") {
            let mut cursor = content2_rect.pos;
            cursor = [cursor[0] + 2.0 * em, cursor[1] + 1.0 * em];

            drawer.draw_colored_rect(
                ColoredRect::new(content2_rect).color(ColorU32::greyscale(0x48)),
            );

            drawer.draw_label(
                &self.ui.theme.face(),
                &format!("Font size {:.2}", self.ui.theme.font_size),
                Rect {
                    pos: cursor,
                    size: [14.0 * em, 2.0 * em],
                },
                0,
                ColorU32::greyscale(0x00),
            );
            cursor[1] += 3.0 * em;

            if self.ui.button(
                drawer,
                ui::Button::with_label("Increase font size by 2").rect(Rect {
                    pos: cursor,
                    size: [20.0 * em, 1.5 * em],
                }),
            ) {
                self.ui.theme.font_size += 2.0;
            }
            cursor[1] += 3.0 * em;

            if self.ui.button(
                drawer,
                ui::Button::with_label("Decrease font size by 2").rect(Rect {
                    pos: cursor,
                    size: [20.0 * em, 1.5 * em],
                }),
            ) {
                self.ui.theme.font_size -= 2.0;
            }
            cursor[1] += 3.0 * em;
        }

        self.docking.end_docking(&mut self.ui, drawer);

        // -- Footer
        let footer_label = format!(
            "Focused: {:?} | Active: {:?}",
            self.ui.activation.focused, self.ui.activation.active
        );
        draw_area(
            &mut self.ui,
            drawer,
            footer_rect,
            ColorU32::from_f32(0.53, 0.13, 0.13, 1.0),
            Some(&footer_label),
        );

        // -- Fps histogram
        if self.show_fps {
            let histogram_rect = Rect {
                pos: [
                    fullscreen.pos[0] + fullscreen.size[0] - 250.0 - 1.0 * em,
                    1.0 * em,
                ],
                size: [250.0, 150.0],
            };
            custom_ui::widgets::histogram(
                &mut self.ui,
                drawer,
                custom_ui::widgets::FpsHistogram {
                    histogram: &self.fps_histogram,
                    rect: histogram_rect,
                },
            );
        }

        self.ui.end_frame();
    }

    pub fn draw_gpu(&mut self) -> VulkanResult<()> {
        self.renderer.render(Some(&self.drawer))
    }
}

fn main() -> Result<()> {
    profile::init();

    let mut asset_dir = PathBuf::from(concat!(env!("OUT_DIR"), "/"));
    asset_dir.push("dummy_file");

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Cripsy Waffle")
        .build(&event_loop)
        .unwrap();

    let ui_font = Font::from_file(
        asset_dir
            .with_file_name("iAWriterQuattroS-Regular.ttf")
            .to_str()
            .unwrap(),
        0,
    )
    .unwrap();

    let renderer = Renderer::new(&window)?;
    let drawer = Drawer::new(
        unsafe { &mut DRAWER_VERTEX_MEMORY },
        unsafe { &mut DRAWER_INDEX_MEMORY },
        [GLYPH_ATLAS_RESOLUTION, GLYPH_ATLAS_RESOLUTION],
        renderer.get_glyph_atlas_descriptor(),
    );

    let font_size = 18.0;
    let ui = ui::Ui::new(Rc::new(ui_font), font_size * (window.scale_factor() as f32));

    let mut app = App {
        renderer,
        drawer: Rc::new(drawer),
        ui,
        fps_histogram: custom_ui::FpsHistogram::new(),
        docking: ui_docking::Docking::new(),
        show_fps: true,
        font_size,
    };

    let now = Instant::now();
    let mut last_time = now.elapsed();

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
                event: WindowEvent::CursorMoved { position, .. },
                window_id,
            } if window_id == window.id() => {
                let mouse_position: winit::dpi::LogicalPosition<f32> = position.to_logical(1.0);
                app.ui
                    .set_mouse_position([mouse_position.x, mouse_position.y]);
            }

            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                window_id,
            } if window_id == window.id() => {
                if button == MouseButton::Left {
                    app.ui
                        .set_left_mouse_button_pressed(state == ElementState::Pressed);
                }
            }

            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { scale_factor, .. },
                window_id,
            } if window_id == window.id() => {
                app.ui.theme.font_size = app.font_size * (scale_factor as f32);
            }

            Event::RedrawRequested(window_id) if window_id == window.id() => {}

            Event::MainEventsCleared => {
                let window_size: winit::dpi::LogicalSize<f32> = window.inner_size().to_logical(1.0);

                let dt = now.elapsed() - last_time;
                last_time = now.elapsed();
                app.update(dt.as_secs_f32());

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
