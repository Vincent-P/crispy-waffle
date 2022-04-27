use erupt::vk;

pub struct Shader {
    filename: String,
    vkhandle: vk::ShaderModule,
    bytecode: Vec<u8>,
}
