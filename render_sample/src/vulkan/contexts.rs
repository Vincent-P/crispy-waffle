use super::context_pool::*;
use super::device::*;
use super::error::*;
use super::fence::*;
use super::image::*;
use super::queues;
use super::surface::*;

use exo::pool::Handle;

use arrayvec::ArrayVec;
use erupt::vk;

pub const MAX_SEMAPHORES: usize = 4;

pub struct BaseContext<'a> {
    pub device: &'a Device<'a>,
    pub cmd: vk::CommandBuffer,
    pub wait_fence_list: ArrayVec<Fence, MAX_SEMAPHORES>,
    pub wait_value_list: ArrayVec<u64, MAX_SEMAPHORES>,
    pub wait_stage_list: ArrayVec<vk::PipelineStageFlags, MAX_SEMAPHORES>,
    pub queue: vk::Queue,
    pub queue_type: usize,
    pub image_acquired_semaphore: Option<vk::Semaphore>,
    pub image_acquired_stage: Option<vk::PipelineStageFlags>,
    pub can_present_semaphore: Option<vk::Semaphore>,
}

pub struct TransferContext<'a> {
    base: BaseContext<'a>,
}

pub struct ComputeContext<'a> {
    base: BaseContext<'a>,
}

pub struct GraphicsContext<'a> {
    base: BaseContext<'a>,
}

impl<'a> Device<'a> {
    pub fn get_base_context(
        &self,
        context_pool: &mut ContextPool,
        i_queue: usize,
    ) -> VulkanResult<BaseContext> {
        assert!(i_queue < queues::COUNT);
        let i_cmd = context_pool.command_buffers_is_used[i_queue]
            .iter()
            .position(|is_used| *is_used == false);

        let cmd = if let Some(i_cmd) = i_cmd {
            context_pool.command_buffers_is_used[i_queue][i_cmd] = true;
            context_pool.command_buffers[i_queue][i_cmd]
        } else {
            let allocate_info = vk::CommandBufferAllocateInfoBuilder::new()
                .command_pool(context_pool.command_pools[i_queue])
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let cmd = unsafe {
                *self
                    .device
                    .allocate_command_buffers(&allocate_info)
                    .result()?
                    .get_unchecked(0)
            };

            context_pool.command_buffers[i_queue].push(cmd);
            context_pool.command_buffers_is_used[i_queue].push(true);
            cmd
        };

        let queue_family_idx = match i_queue {
            queues::GRAPHICS => self.graphics_family_idx,
            queues::COMPUTE => self.compute_family_idx,
            queues::TRANSFER => self.transfer_family_idx,
            _ => unreachable!(),
        };

        let queue = unsafe { self.device.get_device_queue(queue_family_idx, 0) };

        Ok(BaseContext {
            device: self,
            cmd,
            wait_fence_list: Default::default(),
            wait_value_list: Default::default(),
            wait_stage_list: Default::default(),
            queue,
            queue_type: i_queue,
            image_acquired_semaphore: None,
            image_acquired_stage: None,
            can_present_semaphore: None,
        })
    }

    pub fn get_graphics_context(
        &self,
        context_pool: &mut ContextPool,
    ) -> VulkanResult<GraphicsContext> {
        let base = self.get_base_context(context_pool, queues::GRAPHICS)?;
        Ok(GraphicsContext { base })
    }

    pub fn get_compute_context(
        &self,
        context_pool: &mut ContextPool,
    ) -> VulkanResult<ComputeContext> {
        let base = self.get_base_context(context_pool, queues::GRAPHICS)?;
        Ok(ComputeContext { base })
    }

    pub fn get_transfer_context(
        &self,
        context_pool: &mut ContextPool,
    ) -> VulkanResult<TransferContext> {
        let base = self.get_base_context(context_pool, queues::TRANSFER)?;
        Ok(TransferContext { base })
    }
}

// -- Actual commands implementation

pub trait HasBaseContext {
    fn base_context(&self) -> &BaseContext;
    fn base_context_mut(&mut self) -> &mut BaseContext;
}

pub trait TransferContextMethods: HasBaseContext {
    fn begin(&self) -> VulkanResult<()> {
        let base_context = self.base_context();
        let device = &base_context.device.device;

        let begin_info = vk::CommandBufferBeginInfoBuilder::new();
        unsafe {
            device
                .begin_command_buffer(base_context.cmd, &begin_info)
                .result()?;
        }

        Ok(())
    }

    fn end(&self) -> VulkanResult<()> {
        let base_context = self.base_context();
        let device = &base_context.device.device;

        unsafe {
            device.end_command_buffer(base_context.cmd).result()?;
        }
        Ok(())
    }

    fn wait_for_acquired(&mut self, surface: &Surface, stage_dst: vk::PipelineStageFlags) {
        let base_context = self.base_context_mut();
        base_context.image_acquired_semaphore =
            Some(surface.image_acquired_semaphores[surface.previous_image as usize]);
        base_context.image_acquired_stage = Some(stage_dst);
    }

    fn prepare_present(&mut self, surface: &Surface) {
        let base_context = self.base_context_mut();
        base_context.can_present_semaphore =
            Some(surface.can_present_semaphores[surface.current_image as usize]);
    }

    fn barrier(&self, image_handle: Handle<Image>, state_dst: ImageState) {
        let base_context = self.base_context();
        let device = &base_context.device.device;
        let image = base_context.device.images.get(image_handle);

        let src_access = image.state.get_src_access();
        let dst_access = state_dst.get_dst_access();

        const QUEUE_FAMILY_IGNORED: u32 = !0u32;
        let barrier = vk::ImageMemoryBarrierBuilder::new()
            .old_layout(src_access.layout)
            .new_layout(dst_access.layout)
            .src_access_mask(src_access.access)
            .dst_access_mask(dst_access.access)
            .src_queue_family_index(QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(QUEUE_FAMILY_IGNORED)
            .image(image.vkhandle)
            .subresource_range(image.full_view.range);

        unsafe {
            device.cmd_pipeline_barrier(
                base_context.cmd,
                src_access.stage,
                dst_access.stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }
}
// clear_barrier
// begin_pass
// end_pass
// barrier

pub trait ComputeContextMethods: TransferContextMethods {}
pub trait GraphicsContextMethods: ComputeContextMethods {}

impl<T: ComputeContextMethods> TransferContextMethods for T {}
impl<T: GraphicsContextMethods> ComputeContextMethods for T {}

impl<'a> HasBaseContext for TransferContext<'a> {
    fn base_context(&self) -> &BaseContext {
        &self.base
    }

    fn base_context_mut(&mut self) -> &'a mut BaseContext {
        &mut self.base
    }
}

impl<'a> HasBaseContext for ComputeContext<'a> {
    fn base_context(&self) -> &'a BaseContext {
        &self.base
    }

    fn base_context_mut(&mut self) -> &'a mut BaseContext {
        &mut self.base
    }
}

impl<'a> HasBaseContext for GraphicsContext<'a> {
    fn base_context(&self) -> &'a BaseContext {
        &self.base
    }

    fn base_context_mut(&mut self) -> &'a mut BaseContext {
        &mut self.base
    }
}

impl<'a> TransferContextMethods for TransferContext<'a> {}
impl<'a> ComputeContextMethods for ComputeContext<'a> {}
impl<'a> GraphicsContextMethods for GraphicsContext<'a> {}
