use super::ring_buffer::*;
use super::vulkan::contexts::*;
use super::vulkan::device::*;
use super::vulkan::error::*;

pub fn bind_shader_options<Context: ComputeContextMethods>(
    device: &mut Device,
    ring_buffer: &mut RingBuffer,
    ctx: &Context,
    options_len: usize,
) -> VulkanResult<*mut [u8]> {
    let (slice, offset) = ring_buffer.allocate(options_len, 0x40); // 0x10 enough on AMD, should probably check device features
    let i_descriptor = device.find_or_create_uniform_descriptor(ring_buffer.buffer, options_len)?;
    let descriptor = &device.descriptors.uniform_descriptor_sets[i_descriptor];
    ctx.bind_uniform_set(device, descriptor, offset, 2);
    Ok(slice)
}
