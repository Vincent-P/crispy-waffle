use super::device::*;
use super::error::*;
use erupt::vk;
use exo::pool::Handle;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Shader {
    pub path: PathBuf,
    pub vkhandle: vk::ShaderModule,
    pub bytecode: Vec<u8>,
}

impl Device {
    pub fn create_shader(&mut self, path: &str) -> VulkanResult<Handle<Shader>> {
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
            path: PathBuf::from(path),
            vkhandle,
            bytecode,
        });

        Ok(shader_handle)
    }

    pub fn update_shader_from_fs(&mut self, shader_handle: Handle<Shader>) -> VulkanResult<()> {
        let shader = self.shaders.get_mut(shader_handle);
        println!("reloading shader {:?}", &shader.path);

        let new_bytecode = std::fs::read(&shader.path).unwrap();
        let shader_info = vk::ShaderModuleCreateInfo {
            code_size: new_bytecode.len(),
            p_code: new_bytecode.as_ptr() as *const u32,
            ..Default::default()
        };

        unsafe {
            self.device.destroy_shader_module(shader.vkhandle, None);
            shader.vkhandle = self
                .device
                .create_shader_module(&shader_info, None)
                .result()?;
        }

        Ok(())
    }

    pub fn destroy_shader(&mut self, shader_handle: Handle<Shader>) {
        let shader = self.shaders.get(shader_handle);
        unsafe {
            self.device.destroy_shader_module(shader.vkhandle, None);
        }
        self.shaders.remove(shader_handle);
    }
}
