#![cfg_attr(debug_assertions, windows_subsystem = "console")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod custom_render;
mod custom_ui;
mod simple_renderer;

use crate::simple_renderer::SimpleRenderer;
use drawer2d::{drawer::*, font::*, rect::*};
use raw_window_handle::HasRawWindowHandle;
use render::{render_graph, shader, vulkan, vulkan::error::VulkanResult};
use std::{cell::RefCell, rc::Rc, time::Instant};
use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

static mut DRAWER_VERTEX_MEMORY: [u8; 64 << 10] = [0; 64 << 10];
static mut DRAWER_INDEX_MEMORY: [u32; 8 << 10] = [0; 8 << 10];
const GLYPH_ATLAS_RESOLUTION: i32 = 4096;

struct Renderer {
    base: simple_renderer::SimpleRenderer,
    ui_node: custom_render::UiPass,
    demo_node: Rc<RefCell<custom_render::DemoNode>>,
}

impl Renderer {
    pub fn new<WindowHandle: HasRawWindowHandle>(
        window_handle: &WindowHandle,
        window_size: [i32; 2],
    ) -> vulkan::VulkanResult<Self> {
        let mut simple_renderer = SimpleRenderer::new(window_handle, window_size)?;
        let ui_node = custom_render::UiPass::new(
            &mut simple_renderer.device,
            [GLYPH_ATLAS_RESOLUTION, GLYPH_ATLAS_RESOLUTION],
        )?;
        let demo_node = Rc::new(RefCell::new(custom_render::DemoNode::new(
            &mut simple_renderer.device,
        )?));

        Ok(Self {
            base: simple_renderer,
            ui_node,
            demo_node,
        })
    }

    pub fn destroy(self) {
        self.base.destroy();
    }

    pub fn get_glyph_atlas_descriptor(&self) -> u32 {
        self.base
            .device
            .images
            .get(self.ui_node.glyph_atlas)
            .full_view
            .sampled_idx
    }

    pub fn render(
        &mut self,
        drawer: Option<&Rc<Drawer<'static>>>,
        demo_viewport: Option<[i32; 2]>,
        dt: f32,
    ) -> VulkanResult<()> {
        use render_graph::graph::{TextureDesc, TextureSize};

        profile::next_frame();
        profile::scope!("render");

        let intermediate_buffer = self.base.render_graph.output_image(TextureDesc::new(
            String::from("render buffer desc"),
            TextureSize::ScreenRelative([1.0, 1.0]),
        ));

        if let Some(viewport_size) = demo_viewport {
            let demo_buffer = self.base.render_graph.output_image(TextureDesc::new(
                String::from("demo viewport"),
                TextureSize::Absolute([viewport_size[0], viewport_size[1], 1]),
            ));

            custom_render::DemoNode::register_graph(
                &self.demo_node,
                &mut self.base.render_graph,
                demo_buffer,
                dt,
                self.base.time,
            );
        }

        if let Some(drawer) = drawer {
            self.ui_node
                .register_graph(&mut self.base.render_graph, intermediate_buffer, drawer);
        }

        self.base.render(intermediate_buffer, dt)?;

        Ok(())
    }
}

struct App {
    pub renderer: Renderer,
    pub drawer: Rc<Drawer<'static>>,
    pub ui: ui::Ui,
    pub window_size: [f32; 2],
    fps_histogram: custom_ui::FpsHistogram,
    docking: ui_docking::Docking,
    show_fps: bool,
    font_size: f32,
    demo_viewport: Option<[i32; 2]>,
}

impl App {
    pub fn update(&mut self, dt: f32) -> vulkan::VulkanResult<()> {
        self.fps_histogram.push_time(dt);
        self.draw_ui();
        self.renderer
            .render(Some(&self.drawer), self.demo_viewport, dt)
    }

    pub fn draw_ui(&mut self) {
        profile::scope!("ui draw");
        let viewport_size = self.window_size;
        let drawer = Rc::get_mut(&mut self.drawer).unwrap();

        drawer.clear();
        self.ui.new_frame();

        let em = self.ui.em();

        let fullscreen = Rect {
            pos: [0.0, 0.0],
            size: viewport_size,
        };

        // -- Content
        self.docking.begin_docking(&self.ui, fullscreen);

        if let Some(demo_rect) = self.docking.tabview("Demo") {
            self.demo_viewport = Some([demo_rect.size[0] as i32, demo_rect.size[1] as i32]);
            let texture_descriptor = self.renderer.demo_node.borrow().output_descriptor();
            drawer.draw_textured_rect(
                TexturedRect::new(demo_rect).texture_descriptor(texture_descriptor),
            );
        } else {
            self.demo_viewport = None;
        }

        if let Some(_options_rect) = self.docking.tabview("Options") {}

        self.docking.end_docking(&mut self.ui, drawer);

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
}

fn main() {
    profile::init();

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Editor")
        .build(&event_loop)
        .unwrap();

    let inner_size = {
        let window_size: winit::dpi::LogicalSize<f32> = window.inner_size().to_logical(1.0);
        [window_size.width as i32, window_size.height as i32]
    };

    let ui_font = Font::from_file(
        concat!(env!("OUT_DIR"), "/", "iAWriterQuattroS-Regular.ttf"),
        0,
    )
    .unwrap();

    let renderer = Renderer::new(&window, inner_size).unwrap();
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
        window_size: [inner_size[0] as f32, inner_size[1] as f32],
        demo_viewport: None,
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
                event: WindowEvent::Resized(physical_size),
                window_id,
            } if window_id == window.id() => {
                let window_size: winit::dpi::LogicalSize<f32> = physical_size.to_logical(1.0);
                let mut surface = &mut app.renderer.base.swapchain_node.borrow_mut().surface;
                surface.is_outdated = true;
                surface.size_requested =
                    Some([window_size.width as i32, window_size.height as i32]);
                app.window_size = [window_size.width, window_size.height];
            }

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

                app.window_size = [window_size.width, window_size.height];

                if let Err(e) = app.update(dt.as_secs_f32()) {
                    eprintln!("Renderer error: {:?}", e);
                    *control_flow = ControlFlow::Exit;
                }
            }

            _ => (),
        }
    });

    app.renderer.destroy();
}
