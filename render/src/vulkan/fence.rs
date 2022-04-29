use exo::dynamic_array::DynamicArray;

use erupt::{vk, ExtendableFrom};

use super::device::*;
use super::error::*;

pub struct Fence {
    pub timeline_semaphore: vk::Semaphore,
    pub value: u64,
}

impl Device<'_> {
    pub fn create_fence(&mut self) -> VulkanResult<Fence> {
        let value: u64 = 0;

        let mut timeline_info = vk::SemaphoreTypeCreateInfoBuilder::new()
            .semaphore_type(vk::SemaphoreType::TIMELINE_KHR)
            .initial_value(value);
        let semaphore_info = vk::SemaphoreCreateInfoBuilder::new().extend_from(&mut timeline_info);

        let timeline_semaphore = unsafe {
            self.device
                .create_semaphore(&semaphore_info.build_dangling(), None)
                .result()?
        };

        self.set_vk_name(
            timeline_semaphore.0,
            vk::ObjectType::SEMAPHORE,
            "timeline_semaphore",
        )?;

        Ok(Fence {
            timeline_semaphore,
            value,
        })
    }

    pub fn destroy_fence(&self, fence: Fence) {
        unsafe {
            self.device
                .destroy_semaphore(fence.timeline_semaphore, None);
        }
    }

    pub fn wait_for_fences(&self, fences: &[&Fence], wait_values: &[u64]) -> VulkanResult<()> {
        assert!(fences.len() == wait_values.len());

        let mut semaphores = DynamicArray::<vk::Semaphore, 4>::new();
        for fence in fences {
            semaphores.push(fence.timeline_semaphore);
        }

        let timeout: u64 = 10 * 1000 * 1000 * 1000;
        let wait_info = vk::SemaphoreWaitInfoBuilder::new()
            .semaphores(&semaphores)
            .values(wait_values);

        unsafe {
            self.device.wait_semaphores(&wait_info, timeout).result()?;
        }

        Ok(())
    }
}
