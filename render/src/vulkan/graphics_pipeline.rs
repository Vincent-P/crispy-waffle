use super::framebuffer::*;
use super::shader::*;

use exo::pool::Handle;

use arrayvec::ArrayVec;
use erupt::vk;

pub const MAX_RENDER_STATES: usize = 4;

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
