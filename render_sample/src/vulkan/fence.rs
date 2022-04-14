use erupt::vk;

pub struct Fence {
    timeline_semaphore: vk::Semaphore,
    value: u64,
}
