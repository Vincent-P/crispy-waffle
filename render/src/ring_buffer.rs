use exo::{dynamic_array::DynamicArray, pool::Handle};

use super::vulkan::{buffer::*, device::*, error::*};

use erupt::vk;
use gpu_alloc::UsageFlags;

pub struct RingBufferSpec {
    pub usages: vk::BufferUsageFlags,
    pub memory_usage: MemoryUsageFlags,
    pub frame_queue_length: usize,
    pub buffer_size: usize,
}

pub struct RingBuffer {
    spec: RingBufferSpec,
    pub buffer: Handle<Buffer>,
    memory_buffer: *mut [u8],
    cursor: usize,
    i_frame: usize,
    start_per_frame: DynamicArray<Option<usize>, 8>,
}

impl RingBuffer {
    pub fn new(device: &mut Device, spec: RingBufferSpec) -> VulkanResult<Self> {
        let mut start_per_frame = DynamicArray::<Option<usize>, 8>::new();
        start_per_frame.resize(spec.frame_queue_length, None);

        let buffer = device.create_buffer(BufferSpec {
            size: spec.buffer_size,
            usages: spec.usages,
            memory_usage: spec.memory_usage.union(UsageFlags::HOST_ACCESS),
        })?;

        Ok(Self {
            spec,
            buffer,
            memory_buffer: device.map_buffer(buffer),
            cursor: 0,
            i_frame: 0,
            start_per_frame,
        })
    }

    pub fn start_frame(&mut self) {
        self.i_frame += 1;
        let i_start = self.i_frame % self.start_per_frame.len();
        self.start_per_frame[i_start] = Some(self.cursor);
    }

    pub fn allocate(&mut self, size: usize, alignment: usize) -> (*mut [u8], u32) {
        let dist = self.cursor % alignment;
        if dist != 0 {
            self.cursor += alignment - dist;
            assert!(self.cursor % alignment == 0);
        }

        if self.cursor + size > unsafe { (*self.memory_buffer).len() } {
            self.cursor = 0;
        }

        let frame_size = self.start_per_frame.len();
        let previous_frame_start =
            self.start_per_frame[(self.i_frame + frame_size - 1) % frame_size];

        if previous_frame_start.is_some()
            && self.cursor < previous_frame_start.unwrap()
            && self.cursor + size > previous_frame_start.unwrap()
        {
            panic!("Not enough space in the ring buffer");
        }

        let offset = self.cursor;
        self.cursor += size;

        let res = unsafe { &mut (*self.memory_buffer)[offset..offset + size] as *mut [u8] };

        assert!(unsafe { (*res).len() } == size);
        assert!(offset < std::u32::MAX as usize);
        (res, offset as u32)
    }
}
