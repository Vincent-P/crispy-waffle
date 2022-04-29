use super::device::*;
use super::error::*;
use super::framebuffer::*;
use super::shader::*;

use exo::pool::Handle;

use arrayvec::ArrayVec;
use erupt::vk;
use std::ffi::CString;

pub const MAX_RENDER_STATES: usize = 4;

#[derive(Copy, Clone)]
pub enum PrimitiveTopology {
    TriangleList,
    PointList,
}

pub struct DepthState {
    pub test: Option<vk::CompareOp>,
    pub enable_write: bool,
    pub bias: f32,
}

pub struct RasterizationState {
    pub enable_conservative_rasterization: bool,
    pub culling: bool,
}

pub struct InputAssemblyState {
    pub topology: PrimitiveTopology,
}

pub struct RenderState {
    pub depth: DepthState,
    pub rasterization: RasterizationState,
    pub input_assembly: InputAssemblyState,
    pub alpha_blending: bool,
}

pub struct GraphicsState {
    pub vertex_shader: Handle<Shader>,
    pub fragment_shader: Handle<Shader>,
    pub attachments_format: FramebufferFormat,
}

pub struct GraphicsProgram {
    pub name: String,
    pub graphics_state: GraphicsState,
    pub render_states: ArrayVec<RenderState, MAX_RENDER_STATES>,
    pub pipelines: ArrayVec<vk::Pipeline, MAX_RENDER_STATES>,
    pub cache: vk::PipelineCache,
    pub renderpass: vk::RenderPass,
}

impl Device<'_> {
    pub fn create_graphics_program(
        &mut self,
        graphics_state: GraphicsState,
    ) -> VulkanResult<Handle<GraphicsProgram>> {
        let mut load_ops = ArrayVec::<LoadOp, MAX_ATTACHMENTS>::new();
        for i in 0..graphics_state.attachments_format.attachment_formats.len() {
            load_ops.push(LoadOp::Ignore);
        }

        let renderpass = super::framebuffer::create_renderpass(
            &self.device,
            &graphics_state.attachments_format,
            &load_ops,
        )?
        .vkhandle;

        let handle = self.graphics_programs.add(GraphicsProgram {
            name: String::new(),
            graphics_state,
            render_states: ArrayVec::new(),
            pipelines: ArrayVec::new(),
            cache: vk::PipelineCache::null(),
            renderpass,
        });

        Ok(handle)
    }

    pub fn destroy_program(&mut self, program_handle: Handle<GraphicsProgram>) {
        let program = self.graphics_programs.get(program_handle);
        for pipeline in program.pipelines.iter() {
            unsafe {
                self.device.destroy_pipeline(*pipeline, None);
            }
        }
        unsafe {
            self.device.destroy_pipeline_cache(program.cache, None);
            self.device.destroy_render_pass(program.renderpass, None);
        }
        self.graphics_programs.remove(program_handle);
    }

    pub fn compile_graphics_program(
        &mut self,
        program_handle: Handle<GraphicsProgram>,
        render_state: RenderState,
    ) -> VulkanResult<usize> {
        let program = self.graphics_programs.get_mut(program_handle);

        let mut dynamic_states = ArrayVec::<vk::DynamicState, 4>::new();
        dynamic_states.push(vk::DynamicState::VIEWPORT);
        dynamic_states.push(vk::DynamicState::SCISSOR);

        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfoBuilder::new().dynamic_states(&dynamic_states);

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfoBuilder::new();

        let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfoBuilder::new()
            .topology(render_state.input_assembly.topology.to_vk())
            .primitive_restart_enable(false);

        let rasterization_info = vk::PipelineRasterizationStateCreateInfoBuilder::new()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(if render_state.rasterization.culling {
                vk::CullModeFlags::BACK
            } else {
                vk::CullModeFlags::NONE
            })
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(render_state.depth.bias != 0.0)
            .depth_bias_constant_factor(render_state.depth.bias)
            .depth_bias_clamp(0.0)
            .depth_bias_slope_factor(0.0)
            .line_width(1.0);

        let mut attachment_blend_states =
            ArrayVec::<vk::PipelineColorBlendAttachmentStateBuilder, MAX_ATTACHMENTS>::new();

        for _color_attachment in &program.graphics_state.attachments_format.attachment_formats {
            let mut state = vk::PipelineColorBlendAttachmentStateBuilder::new()
                .color_write_mask(
                    vk::ColorComponentFlags::R
                        | vk::ColorComponentFlags::G
                        | vk::ColorComponentFlags::B
                        | vk::ColorComponentFlags::A,
                )
                .blend_enable(render_state.alpha_blending);

            if render_state.alpha_blending {
                state = state
                    .src_color_blend_factor(vk::BlendFactor::ONE)
                    .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                    .color_blend_op(vk::BlendOp::ADD)
                    .src_alpha_blend_factor(vk::BlendFactor::ONE)
                    .dst_alpha_blend_factor(vk::BlendFactor::ONE)
                    .alpha_blend_op(vk::BlendOp::ADD);
            }

            attachment_blend_states.push(state);
        }

        let color_blend_state_info = vk::PipelineColorBlendStateCreateInfoBuilder::new()
            .attachments(&attachment_blend_states)
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .blend_constants([0.0, 0.0, 0.0, 0.0]);

        let viewport_info = vk::PipelineViewportStateCreateInfoBuilder::new()
            .viewport_count(1)
            .scissor_count(1);

        let stencil_info = vk::StencilOpStateBuilder::new()
            .fail_op(vk::StencilOp::KEEP)
            .pass_op(vk::StencilOp::KEEP)
            .compare_op(vk::CompareOp::ALWAYS)
            .compare_mask(0)
            .reference(0)
            .depth_fail_op(vk::StencilOp::KEEP)
            .write_mask(0);

        let depth_stencil_info = vk::PipelineDepthStencilStateCreateInfoBuilder::new()
            .depth_test_enable(render_state.depth.test.is_some())
            .depth_write_enable(render_state.depth.enable_write)
            .depth_compare_op(render_state.depth.test.unwrap_or(vk::CompareOp::NEVER))
            .depth_bounds_test_enable(false)
            .min_depth_bounds(0.0)
            .max_depth_bounds(0.0)
            .stencil_test_enable(false)
            .back(*stencil_info)
            .front(*stencil_info);

        let multisample_info = vk::PipelineMultisampleStateCreateInfoBuilder::new()
            .rasterization_samples(vk::SampleCountFlagBits::_1);

        let entrypoint = CString::new("main").unwrap();
        let module_name = &entrypoint;
        let mut shader_stages = ArrayVec::<vk::PipelineShaderStageCreateInfoBuilder, 3>::new();

        if program.graphics_state.vertex_shader.is_valid() {
            let shader = self.shaders.get(program.graphics_state.vertex_shader);
            let shader_info = vk::PipelineShaderStageCreateInfoBuilder::new()
                .stage(vk::ShaderStageFlagBits::VERTEX)
                .module(shader.vkhandle)
                .name(module_name);
            shader_stages.push(shader_info);
        }

        if program.graphics_state.fragment_shader.is_valid() {
            let shader = self.shaders.get(program.graphics_state.fragment_shader);
            let shader_info = vk::PipelineShaderStageCreateInfoBuilder::new()
                .stage(vk::ShaderStageFlagBits::FRAGMENT)
                .module(shader.vkhandle)
                .name(module_name);
            shader_stages.push(shader_info);
        }

        let pipeline_info = vk::GraphicsPipelineCreateInfoBuilder::new()
            .layout(self.descriptors.pipeline_layout)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly_info)
            .rasterization_state(&rasterization_info)
            .color_blend_state(&color_blend_state_info)
            .multisample_state(&multisample_info)
            .dynamic_state(&dynamic_state_info)
            .viewport_state(&viewport_info)
            .depth_stencil_state(&depth_stencil_info)
            .stages(&shader_stages)
            .render_pass(program.renderpass)
            .subpass(0);

        let pipeline = unsafe {
            self.device
                .create_graphics_pipelines(program.cache, &[pipeline_info], None)
                .result()?[0]
        };

        let index = program.pipelines.len();
        program.pipelines.push(pipeline);
        program.render_states.push(render_state);

        Ok(index)
    }
}

impl PrimitiveTopology {
    pub fn to_vk(self) -> vk::PrimitiveTopology {
        match self {
            PrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
            PrimitiveTopology::PointList => vk::PrimitiveTopology::POINT_LIST,
        }
    }
}
