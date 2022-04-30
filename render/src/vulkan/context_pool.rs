use super::device::*;
use super::error::*;
use super::queues;

use erupt::vk;

#[derive(Debug)]
pub struct ContextPool {
    pub command_pools: [vk::CommandPool; queues::COUNT],
    pub command_buffers: [Vec<vk::CommandBuffer>; queues::COUNT],
    pub command_buffers_is_used: [Vec<bool>; queues::COUNT],
}

impl Device<'_> {
    pub fn create_context_pool(&self) -> VulkanResult<ContextPool> {
        let pool_info =
            vk::CommandPoolCreateInfoBuilder::new().queue_family_index(self.graphics_family_idx);
        let graphics_pool = unsafe { self.device.create_command_pool(&pool_info, None).result()? };

        let pool_info = pool_info.queue_family_index(self.compute_family_idx);
        let compute_pool = unsafe { self.device.create_command_pool(&pool_info, None).result()? };

        let pool_info = pool_info.queue_family_index(self.transfer_family_idx);
        let transfer_pool = unsafe { self.device.create_command_pool(&pool_info, None).result()? };

        Ok(ContextPool {
            command_pools: [transfer_pool, compute_pool, graphics_pool],
            command_buffers: Default::default(),
            command_buffers_is_used: Default::default(),
        })
    }

    pub fn reset_context_pool(&self, context_pool: &mut ContextPool) -> VulkanResult<()> {
        // TODO: Validate that all command buffers are recorded?
        for i_queue in 0..queues::COUNT {
            for is_used in &mut context_pool.command_buffers_is_used[i_queue] {
                *is_used = false;
            }

            unsafe {
                self.device
                    .reset_command_pool(
                        context_pool.command_pools[i_queue],
                        vk::CommandPoolResetFlags::RELEASE_RESOURCES,
                    )
                    .result()?
            };
        }
        Ok(())
    }

    pub fn destroy_context_pool(&self, context_pool: ContextPool) {
        for i_queue in 0..queues::COUNT {
            unsafe {
                self.device
                    .destroy_command_pool(context_pool.command_pools[i_queue], None);
            }
        }
    }
}
