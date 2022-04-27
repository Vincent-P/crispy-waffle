use exo::pool::Handle;

use super::device::*;
use super::error::*;

use erupt::vk;
use std::path::PathBuf;

pub struct Shader {
    pub path: PathBuf,
    pub vkhandle: vk::ShaderModule,
    pub bytecode: Vec<u8>,
}

impl Device<'_> {
    pub fn create_shader(&mut self, path: PathBuf) -> VulkanResult<Handle<Shader>> {
        let bytecode = std::fs::read(&path).unwrap();

        let shader_info = vk::ShaderModuleCreateInfo {
            code_size: bytecode.len(),
            p_code: bytecode.as_ptr() as *const u32,
            ..Default::default()
        };

        let vkhandle = unsafe {
            self.device
                .create_shader_module(&shader_info, None)
                .result()?
        };

        let shader_handle = self.shaders.add(Shader {
            path,
            vkhandle,
            bytecode,
        });

        Ok(shader_handle)
    }

    pub fn destroy_shader(&mut self, shader_handle: Handle<Shader>) {
        let shader = self.shaders.get(shader_handle);
        unsafe {
            self.device.destroy_shader_module(shader.vkhandle, None);
        }
        self.shaders.remove(shader_handle);
    }
}
