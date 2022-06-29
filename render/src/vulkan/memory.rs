#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Lifetime {
    Buffer,
    Image,
}

impl vk_alloc::Lifetime for Lifetime {}

pub type Allocation = vk_alloc::Allocation<Lifetime>;
pub type Allocator = vk_alloc::Allocator<Lifetime>;
