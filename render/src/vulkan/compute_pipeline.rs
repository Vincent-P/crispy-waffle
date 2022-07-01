use super::device::*;
use super::error::*;
use super::shader::*;
use erupt::vk;
use exo::pool::Handle;
use std::ffi::CString;

#[derive(Debug)]
pub struct ComputeProgram {
    pub name: String,
    pub pipeline: vk::Pipeline,
    pub shader: Handle<Shader>,
}

impl Device {
    pub fn create_compute_program(
        &mut self,
        name: String,
        shader_handle: Handle<Shader>,
    ) -> VulkanResult<Handle<ComputeProgram>> {
        let res = self.compute_programs.add(ComputeProgram {
            name,
            pipeline: create_compute_pipeline(self, shader_handle)?,
            shader: shader_handle,
        });

        Ok(res)
    }

    pub fn compile_compute_program(
        &mut self,
        program_handle: Handle<ComputeProgram>,
    ) -> VulkanResult<()> {
        let program = self.compute_programs.get(program_handle);
        let new_pipeline = create_compute_pipeline(self, program.shader)?;
        if !program.pipeline.is_null() {
            unsafe {
                self.device.destroy_pipeline(program.pipeline, None);
            }
        }

        let program = self.compute_programs.get_mut(program_handle);
        program.pipeline = new_pipeline;

        Ok(())
    }
}

fn create_compute_pipeline(
    device: &Device,
    shader_handle: Handle<Shader>,
) -> VulkanResult<vk::Pipeline> {
    let shader = device.shaders.get(shader_handle);
    let entrypoint = CString::new("main").unwrap();
    let shader_info = vk::PipelineShaderStageCreateInfoBuilder::new()
        .stage(vk::ShaderStageFlagBits::COMPUTE)
        .module(shader.vkhandle)
        .name(&entrypoint);

    let pipeline_info = vk::ComputePipelineCreateInfoBuilder::new()
        .stage(*shader_info)
        .layout(device.descriptors.pipeline_layout);

    let vkpipeline = unsafe {
        device
            .device
            .create_compute_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
            .result()?[0]
    };

    Ok(vkpipeline)
}
