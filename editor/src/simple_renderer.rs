use exo::dynamic_array::DynamicArray;
use exo::pool::Handle;
use raw_window_handle::HasRawWindowHandle;
use render::{render_graph, ring_buffer::*, shader, vk, vulkan, vulkan::error::VulkanResult};
use render_graph::{builtins, graph::TextureDesc};
use std::{cell::RefCell, ffi::CStr, os::raw::c_char, rc::Rc};

const FRAME_QUEUE_LENGTH: usize = 2;

pub struct SimpleRenderer {
    pub instance: vulkan::Instance,
    pub physical_devices: DynamicArray<vulkan::PhysicalDevice, { vulkan::MAX_PHYSICAL_DEVICES }>,
    pub device: vulkan::Device,
    pub i_device: usize,
    pub context_pools: [vulkan::ContextPool; FRAME_QUEUE_LENGTH],
    pub uniform_buffer: RingBuffer,
    pub dynamic_vertex_buffer: RingBuffer,
    pub dynamic_index_buffer: RingBuffer,
    pub upload_buffer: RingBuffer,
    pub render_graph: render_graph::graph::RenderGraph,
    pub swapchain_node: Rc<RefCell<render_graph::builtins::SwapchainPass>>,
    pub frame_count: usize,
    pub time: f32,
    pub shader_watcher: shader::ShaderWatcher,
}

impl SimpleRenderer {
    pub fn new<WindowHandle: HasRawWindowHandle>(
        window_handle: &WindowHandle,
        window_size: [i32; 2],
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

        let surface = vulkan::Surface::new(
            &instance,
            &mut device,
            physical_device,
            window_handle,
            Some(window_size),
        )?;
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
                memory_usage: vulkan::buffer::MemoryUsageFlags::CpuToGpu,
                frame_queue_length: FRAME_QUEUE_LENGTH,
                buffer_size: 1024,
            },
        )?;

        let dynamic_vertex_buffer = RingBuffer::new(
            &mut device,
            RingBufferSpec {
                usages: vk::BufferUsageFlags::STORAGE_BUFFER,
                memory_usage: vulkan::buffer::MemoryUsageFlags::CpuToGpu,
                frame_queue_length: FRAME_QUEUE_LENGTH,
                buffer_size: 128 << 10,
            },
        )?;

        let dynamic_index_buffer = RingBuffer::new(
            &mut device,
            RingBufferSpec {
                usages: vk::BufferUsageFlags::INDEX_BUFFER,
                memory_usage: vulkan::buffer::MemoryUsageFlags::CpuToGpu,
                frame_queue_length: FRAME_QUEUE_LENGTH,
                buffer_size: 128 << 10,
            },
        )?;

        let upload_buffer = RingBuffer::new(
            &mut device,
            RingBufferSpec {
                usages: vk::BufferUsageFlags::TRANSFER_SRC,
                memory_usage: vulkan::buffer::MemoryUsageFlags::CpuToGpu,
                frame_queue_length: FRAME_QUEUE_LENGTH,
                buffer_size: 32 << 20,
            },
        )?;

        let render_graph = render_graph::graph::RenderGraph::new();

        let mut shader_watcher = shader::ShaderWatcher::new();
        render::watch_crate_shaders!(shader_watcher);

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
            render_graph,
            swapchain_node,
            frame_count: 0,
            time: 0.0,
            shader_watcher,
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

    pub fn render(&mut self, output: Handle<TextureDesc>, dt: f32) -> VulkanResult<()> {
        profile::scope!("simple_renderer render");

        let i_frame = {
            let b = self.swapchain_node.borrow();
            b.i_frame
        };

        let swapchain_output = builtins::SwapchainPass::acquire_next_image(
            &self.swapchain_node,
            &mut self.render_graph,
        );

        builtins::blit_image(&mut self.render_graph, output, swapchain_output);

        builtins::SwapchainPass::present(
            &self.swapchain_node,
            &mut self.render_graph,
            (i_frame + FRAME_QUEUE_LENGTH) as u64,
        );

        let current_frame = i_frame % FRAME_QUEUE_LENGTH;
        let context_pool = &mut self.context_pools[current_frame];

        let wait_value: u64 = i_frame as u64;
        {
            let fence = &self.swapchain_node.borrow().fence;
            let wait_values = [wait_value];
            self.device.wait_for_fences(&[fence], &wait_values)?;
        }

        self.device.reset_context_pool(context_pool)?;

        let reloaded_shader = self.shader_watcher.update(|watch_event| {
            if let render::shader::DebouncedEvent::Write(path) = watch_event {
                self.device
                    .shaders
                    .iter()
                    .find(|(_handle, shader)| shader.path == path)
                    .map(|(handle, _shader)| handle)
            } else {
                None
            }
        });

        if let Some(reloaded_shader) = reloaded_shader {
            self.device.wait_idle().unwrap();

            self.device.update_shader_from_fs(reloaded_shader)?;

            let graphics_programs_to_reload: Vec<_> = self
                .device
                .graphics_programs
                .iter()
                .filter(|(_handle, program)| {
                    program.graphics_state.vertex_shader == reloaded_shader
                        || program.graphics_state.fragment_shader == reloaded_shader
                })
                .map(|(handle, _program)| handle)
                .collect();

            for program_handle in graphics_programs_to_reload {
                let pipeline_count = self
                    .device
                    .graphics_programs
                    .get(program_handle)
                    .pipelines
                    .len();

                for i_pipeline in 0..pipeline_count {
                    self.device
                        .compile_graphics_program_pipeline(program_handle, i_pipeline)?;
                }
            }

            let compute_programs_to_reload: Vec<_> = self
                .device
                .compute_programs
                .iter()
                .filter(|(_handle, program)| program.shader == reloaded_shader)
                .map(|(handle, _program)| handle)
                .collect();

            for program_handle in compute_programs_to_reload {
                self.device.compile_compute_program(program_handle)?;
            }
        }

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
        self.time += dt;

        Ok(())
    }
}
