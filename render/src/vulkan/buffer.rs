use exo::pool::Handle;

use super::device::*;
use super::error::*;

use erupt::vk;
use gpu_alloc::{Request, UsageFlags};
use gpu_alloc_erupt::EruptMemoryDevice;

pub struct BufferSpec {
    pub size: usize,
    pub usages: vk::BufferUsageFlags,
    pub memory_usage: UsageFlags,
}

impl Default for BufferSpec {
    fn default() -> Self {
        Self {
            size: 0,
            usages: vk::BufferUsageFlags::STORAGE_BUFFER,
            memory_usage: UsageFlags::FAST_DEVICE_ACCESS,
        }
    }
}

pub struct Buffer {
    pub vkhandle: vk::Buffer,
    pub memory_block: Option<gpu_alloc::MemoryBlock<vk::DeviceMemory>>,
    pub spec: BufferSpec,
    pub mapped_ptr: *mut u8,
    pub storage_idx: u32,
}

impl Device {
    pub fn create_buffer(&mut self, spec: BufferSpec) -> VulkanResult<Handle<Buffer>> {
        let is_storage = spec.usages.contains(vk::BufferUsageFlags::STORAGE_BUFFER);

        let buffer_info = vk::BufferCreateInfoBuilder::new()
            .usage(spec.usages)
            .size(spec.size as u64)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let vkbuffer = unsafe { self.device.create_buffer(&buffer_info, None).result()? };

        let mem_requirements = unsafe { self.device.get_buffer_memory_requirements(vkbuffer) };
        let memory_block = unsafe {
            self.allocator.alloc(
                EruptMemoryDevice::wrap(&self.device),
                Request {
                    size: mem_requirements.size,
                    align_mask: 1,
                    usage: spec.memory_usage,
                    memory_types: !0,
                },
            )
        }?;

        unsafe {
            self.device
                .bind_buffer_memory(vkbuffer, *memory_block.memory(), 0)
                .result()?;
        }

        let buffer_handle = self.buffers.add(Buffer {
            vkhandle: vkbuffer,
            memory_block: Some(memory_block),
            spec,
            mapped_ptr: std::ptr::null_mut(),
            storage_idx: 0,
        });

        if is_storage {
            self.buffers.get_mut(buffer_handle).storage_idx =
                self.descriptors
                    .bindless_set
                    .bind_storage_buffer(buffer_handle) as u32;
        }

        Ok(buffer_handle)
    }

    pub fn map_buffer(&mut self, buffer_handle: Handle<Buffer>) -> *mut [u8] {
        let buffer = self.buffers.get_mut(buffer_handle);
        if buffer.mapped_ptr.is_null() {
            let mapped = unsafe {
                buffer
                    .memory_block
                    .as_mut()
                    .unwrap()
                    .map(EruptMemoryDevice::wrap(&self.device), 0, buffer.spec.size)
                    .unwrap()
                    .as_ptr()
            };
            assert!(!mapped.is_null());
            buffer.mapped_ptr = mapped;
        }
        std::ptr::slice_from_raw_parts_mut(buffer.mapped_ptr, buffer.spec.size)
    }
}
