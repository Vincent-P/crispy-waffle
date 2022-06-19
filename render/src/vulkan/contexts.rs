use super::buffer::*;
use super::context_pool::*;
use super::descriptor_set::*;
use super::device::*;
use super::error::*;
use super::fence::*;
use super::framebuffer::*;
use super::graphics_pipeline::*;
use super::image::*;
use super::queues;
use super::surface::*;
use erupt::vk;
use exo::{dynamic_array::DynamicArray, pool::Handle};

pub const MAX_SEMAPHORES: usize = 4;

pub struct BaseContext {
    pub cmd: vk::CommandBuffer,
    pub wait_fence_list: DynamicArray<Fence, MAX_SEMAPHORES>,
    pub wait_value_list: DynamicArray<u64, MAX_SEMAPHORES>,
    pub wait_stage_list: DynamicArray<vk::PipelineStageFlags, MAX_SEMAPHORES>,
    pub queue: vk::Queue,
    pub queue_type: usize,
    pub image_acquired_semaphore: Option<vk::Semaphore>,
    pub image_acquired_stage: Option<vk::PipelineStageFlags>,
    pub can_present_semaphore: Option<vk::Semaphore>,
}

impl BaseContext {
    pub fn begin(&self, device: &Device) -> VulkanResult<()> {
        let begin_info = vk::CommandBufferBeginInfoBuilder::new();
        unsafe {
            device
                .device
                .begin_command_buffer(self.cmd, &begin_info)
                .result()?;
        }

        let global_set = device.descriptors.bindless_set.vkset;
        if self.queue_type == queues::COMPUTE {
            unsafe {
                device.device.cmd_bind_descriptor_sets(
                    self.cmd,
                    vk::PipelineBindPoint::COMPUTE,
                    device.descriptors.pipeline_layout,
                    0,
                    &[global_set],
                    &[],
                );
            }
        } else if self.queue_type == queues::GRAPHICS {
            unsafe {
                device.device.cmd_bind_descriptor_sets(
                    self.cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    device.descriptors.pipeline_layout,
                    0,
                    &[global_set],
                    &[],
                );
            }
        }

        Ok(())
    }

    pub fn end(&self, device: &Device) -> VulkanResult<()> {
        unsafe {
            device.device.end_command_buffer(self.cmd).result()?;
        }
        Ok(())
    }

    pub fn wait_for_acquired(&mut self, surface: &Surface, stage_dst: vk::PipelineStageFlags) {
        self.image_acquired_semaphore =
            Some(surface.image_acquired_semaphores[surface.previous_image as usize]);
        self.image_acquired_stage = Some(stage_dst);
    }

    pub fn prepare_present(&mut self, surface: &Surface) {
        self.can_present_semaphore =
            Some(surface.can_present_semaphores[surface.current_image as usize]);
    }

    pub fn barrier(&self, device: &mut Device, image_handle: Handle<Image>, state_dst: ImageState) {
        let image = device.images.get_mut(image_handle);

        let src_access = image.state.get_src_access();
        let dst_access = state_dst.get_dst_access();

        image.state = state_dst;

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
            device.device.cmd_pipeline_barrier(
                self.cmd,
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

pub struct TransferContext {
    base: BaseContext,
}

impl TransferContext {
    pub fn base_context(&self) -> &BaseContext {
        &self.base
    }

    pub fn base_context_mut(&mut self) -> &mut BaseContext {
        &mut self.base
    }
}

#[derive(Debug)]
pub struct BufferImageCopy {
    pub buffer_offset: u64,
    pub buffer_size: u32,
    pub image_offset: [i32; 3],
    pub image_extent: [u32; 3],
}

impl TransferContext {
    pub fn copy_buffer_to_image(
        &mut self,
        device: &Device,
        buffer: Handle<Buffer>,
        image: Handle<Image>,
        copies: &[BufferImageCopy],
    ) {
        let buffer = device.buffers.get(buffer);
        let image = device.images.get(image);

        let regions: Vec<_> = copies
            .iter()
            .map(|copy| {
                vk::BufferImageCopyBuilder::new()
                    .image_subresource(
                        *vk::ImageSubresourceLayersBuilder::new()
                            .aspect_mask(image.full_view.range.aspect_mask)
                            .mip_level(0)
                            .base_array_layer(0)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: copy.image_extent[0],
                        height: copy.image_extent[1],
                        depth: copy.image_extent[2],
                    })
                    .image_offset(vk::Offset3D {
                        x: copy.image_offset[0],
                        y: copy.image_offset[1],
                        z: copy.image_offset[2],
                    })
                    .buffer_offset(copy.buffer_offset)
            })
            .collect();

        unsafe {
            device.device.cmd_copy_buffer_to_image(
                self.base.cmd,
                buffer.vkhandle,
                image.vkhandle,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            );
        }
    }

    pub fn clear_image(&self, device: &Device, image: Handle<Image>, clear_color: ClearColorValue) {
        let image = device.images.get(image);
        let range = image.full_view.range;

        let clear_color = clear_color.to_vk();

        let ranges = [vk::ImageSubresourceRangeBuilder::new()
            .aspect_mask(range.aspect_mask)
            .base_mip_level(range.base_mip_level)
            .level_count(range.level_count)
            .base_array_layer(range.base_array_layer)
            .layer_count(range.layer_count)];

        unsafe {
            device.device.cmd_clear_color_image(
                self.base.cmd,
                image.vkhandle,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &clear_color,
                &ranges,
            );
        }
    }
}

pub struct ComputeContext {
    base: TransferContext,
}

impl AsRef<TransferContext> for ComputeContext {
    fn as_ref(&self) -> &TransferContext {
        &self.base
    }
}

impl AsMut<TransferContext> for ComputeContext {
    fn as_mut(&mut self) -> &mut TransferContext {
        &mut self.base
    }
}

impl AsMut<ComputeContext> for ComputeContext {
    fn as_mut(&mut self) -> &mut ComputeContext {
        self
    }
}

impl ComputeContext {
    pub fn base_context(&self) -> &BaseContext {
        &self.base.base
    }

    pub fn base_context_mut(&mut self) -> &mut BaseContext {
        &mut self.base.base
    }

    pub fn transfer(&self) -> &TransferContext {
        &self.base
    }

    pub fn transfer_mut(&mut self) -> &mut TransferContext {
        &mut self.base
    }

    pub fn bind_uniform_set(
        &self,
        device: &Device,
        descriptor: &DynamicBufferDescriptor,
        offset: u32,
        i_set: u32,
    ) {
        assert!(i_set == 1 || i_set == 2);
        let base_context = self.base_context();

        unsafe {
            device.device.cmd_bind_descriptor_sets(
                base_context.cmd,
                vk::PipelineBindPoint::COMPUTE,
                device.descriptors.pipeline_layout,
                i_set,
                &[descriptor.vkset],
                &[offset],
            );
        }

        if base_context.queue_type == queues::GRAPHICS {
            unsafe {
                device.device.cmd_bind_descriptor_sets(
                    base_context.cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    device.descriptors.pipeline_layout,
                    i_set,
                    &[descriptor.vkset],
                    &[offset],
                );
            }
        }
    }
}

pub struct GraphicsContext {
    base: ComputeContext,
}

impl AsMut<GraphicsContext> for GraphicsContext {
    fn as_mut(&mut self) -> &mut GraphicsContext {
        self
    }
}

impl AsRef<ComputeContext> for GraphicsContext {
    fn as_ref(&self) -> &ComputeContext {
        &self.base
    }
}

impl AsMut<ComputeContext> for GraphicsContext {
    fn as_mut(&mut self) -> &mut ComputeContext {
        &mut self.base
    }
}

impl AsRef<TransferContext> for GraphicsContext {
    fn as_ref(&self) -> &TransferContext {
        &self.base.base
    }
}

impl AsMut<TransferContext> for GraphicsContext {
    fn as_mut(&mut self) -> &mut TransferContext {
        &mut self.base.base
    }
}

impl GraphicsContext {
    pub fn base_context(&self) -> &BaseContext {
        &self.base.base.base
    }

    pub fn base_context_mut(&mut self) -> &mut BaseContext {
        &mut self.base.base.base
    }

    pub fn base(&self) -> &BaseContext {
        &self.base.base.base
    }

    pub fn base_mut(&mut self) -> &mut BaseContext {
        &mut self.base.base.base
    }

    pub fn transfer(&self) -> &TransferContext {
        &self.base.base
    }

    pub fn transfer_mut(&mut self) -> &mut TransferContext {
        &mut self.base.base
    }

    pub fn begin_pass(
        &mut self,
        device: &mut Device,
        framebuffer_handle: Handle<Framebuffer>,
        load_ops: &[LoadOp],
    ) -> VulkanResult<()> {
        let base_context = self.base_context_mut();

        let (framebuffer, renderpass) =
            device.find_framebuffer_renderpass(framebuffer_handle, load_ops)?;

        let mut clear_values = DynamicArray::<vk::ClearValue, MAX_ATTACHMENTS>::new();
        for load_op in load_ops {
            clear_values.push(load_op.clear_value());
        }

        let begin_info = vk::RenderPassBeginInfoBuilder::new()
            .render_pass(renderpass.vkhandle)
            .framebuffer(framebuffer.vkhandle)
            .render_area(vk::Rect2D {
                extent: vk::Extent2D {
                    width: framebuffer.format.size[0] as u32,
                    height: framebuffer.format.size[1] as u32,
                },
                ..Default::default()
            })
            .clear_values(&clear_values);

        unsafe {
            device.device.cmd_begin_render_pass(
                base_context.cmd,
                &begin_info,
                vk::SubpassContents::INLINE,
            );
        }

        Ok(())
    }

    pub fn end_pass(&self, device: &Device) {
        let base_context = self.base_context();
        unsafe {
            device.device.cmd_end_render_pass(base_context.cmd);
        }
    }

    pub fn bind_graphics_pipeline(
        &self,
        device: &Device,
        program_handle: Handle<GraphicsProgram>,
        index: usize,
    ) {
        let base_context = self.base_context();
        let program = device.graphics_programs.get(program_handle);
        let pipeline = program.pipelines[index];
        unsafe {
            device.device.cmd_bind_pipeline(
                base_context.cmd,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline,
            );
        }
    }

    pub fn set_viewport(&self, device: &Device, viewport: vk::ViewportBuilder) {
        let base_context = self.base_context();
        let viewports = [viewport];
        unsafe {
            device
                .device
                .cmd_set_viewport(base_context.cmd, 0, &viewports);
        }
    }

    pub fn set_scissor(&self, device: &Device, scissor: vk::Rect2DBuilder) {
        let base_context = self.base_context();
        let scissors = [scissor];
        unsafe {
            device
                .device
                .cmd_set_scissor(base_context.cmd, 0, &scissors);
        }
    }

    pub fn bind_index_buffer(
        &self,
        device: &Device,
        buffer_handle: Handle<Buffer>,
        index_type: vk::IndexType,
        offset: usize,
    ) {
        let base_context = self.base_context();
        let buffer = device.buffers.get(buffer_handle);
        unsafe {
            device.device.cmd_bind_index_buffer(
                base_context.cmd,
                buffer.vkhandle,
                offset as u64,
                index_type,
            );
        }
    }
}

pub struct DrawOptions {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub vertex_offset: u32,
    pub instance_offset: u32,
}

impl Default for DrawOptions {
    fn default() -> Self {
        Self {
            vertex_count: 0,
            instance_count: 1,
            vertex_offset: 0,
            instance_offset: 0,
        }
    }
}

impl GraphicsContext {
    pub fn draw(&self, device: &Device, draw_options: DrawOptions) {
        let base_context = self.base_context();
        unsafe {
            device.device.cmd_draw(
                base_context.cmd,
                draw_options.vertex_count,
                draw_options.instance_count,
                draw_options.vertex_offset,
                draw_options.instance_offset,
            );
        }
    }
}

pub struct DrawIndexedOptions {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub index_offset: u32,
    pub vertex_offset: u32,
    pub instance_offset: u32,
}

impl Default for DrawIndexedOptions {
    fn default() -> Self {
        Self {
            vertex_count: 0,
            instance_count: 1,
            index_offset: 0,
            vertex_offset: 0,
            instance_offset: 0,
        }
    }
}

impl GraphicsContext {
    pub fn draw_indexed(&self, device: &Device, draw_options: DrawIndexedOptions) {
        let base_context = self.base_context();
        unsafe {
            device.device.cmd_draw_indexed(
                base_context.cmd,
                draw_options.vertex_count,
                draw_options.instance_count,
                draw_options.index_offset,
                (draw_options.vertex_offset).try_into().unwrap(),
                draw_options.instance_offset,
            );
        }
    }
}

impl Device {
    fn get_base_context(
        &self,
        context_pool: &mut ContextPool,
        i_queue: usize,
    ) -> VulkanResult<BaseContext> {
        assert!(i_queue < queues::COUNT);
        let i_cmd = context_pool.command_buffers_is_used[i_queue]
            .iter()
            .position(|is_used| !(*is_used));

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
            cmd,
            wait_fence_list: DynamicArray::new(),
            wait_value_list: DynamicArray::new(),
            wait_stage_list: DynamicArray::new(),
            queue,
            queue_type: i_queue,
            image_acquired_semaphore: None,
            image_acquired_stage: None,
            can_present_semaphore: None,
        })
    }

    pub fn get_transfer_context(
        &self,
        context_pool: &mut ContextPool,
    ) -> VulkanResult<TransferContext> {
        Ok(TransferContext {
            base: self.get_base_context(context_pool, queues::GRAPHICS)?,
        })
    }

    pub fn get_compute_context(
        &self,
        context_pool: &mut ContextPool,
    ) -> VulkanResult<ComputeContext> {
        let base = TransferContext {
            base: self.get_base_context(context_pool, queues::COMPUTE)?,
        };
        Ok(ComputeContext { base })
    }

    pub fn get_graphics_context(
        &self,
        context_pool: &mut ContextPool,
    ) -> VulkanResult<GraphicsContext> {
        let base = TransferContext {
            base: self.get_base_context(context_pool, queues::GRAPHICS)?,
        };
        let base = ComputeContext { base };
        Ok(GraphicsContext { base })
    }
}
