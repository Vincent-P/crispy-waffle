use super::ring_buffer::*;
use super::vulkan::contexts::*;
use super::vulkan::device::*;
use super::vulkan::error::*;
use std::mem::size_of;

pub fn bind_shader_options<Context: AsRef<ComputeContext>>(
    device: &mut Device,
    ring_buffer: &mut RingBuffer,
    ctx: &Context,
    options_len: usize,
) -> VulkanResult<*mut [u8]> {
    let (slice, offset) = ring_buffer.allocate(options_len, 0x40); // 0x10 enough on AMD, should probably check device features
    let i_descriptor = device.find_or_create_uniform_descriptor(ring_buffer.buffer, options_len)?;
    let descriptor = &device.descriptors.uniform_descriptor_sets[i_descriptor];
    ctx.as_ref().bind_uniform_set(device, descriptor, offset, 2);
    Ok(slice)
}

pub fn bind_and_copy_shader_options<Context: AsRef<ComputeContext>, T>(
    device: &mut Device,
    ring_buffer: &mut RingBuffer,
    ctx: &Context,
    data: T,
) -> VulkanResult<()> {
    let options = bind_shader_options(device, ring_buffer, &ctx, size_of::<T>()).unwrap();
    unsafe {
        let p_options = std::slice::from_raw_parts_mut((*options).as_ptr() as *mut T, 1);
        p_options[0] = data;
    }

    Ok(())
}
