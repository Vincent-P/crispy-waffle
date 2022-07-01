#version 460
#pragma shader_stage(compute)

#include "render/types.h"
#include "render/bindless.h"

layout(set = SHADER_UNIFORM_SET, binding = 0) uniform Options {
	u32 storage_output_frame;
	u32 i_frame;
};

#define OUTPUT_IMAGE global_images_2d_rgba8[storage_output_frame]

layout(local_size_x = 16, local_size_y = 16, local_size_z = 1) in;
void main()
{
    uint local_idx   = gl_LocalInvocationIndex;
    uint3 global_idx = gl_GlobalInvocationID;
    uint3 group_idx  = gl_WorkGroupID;

    int2 pixel_pos = int2(global_idx.xy);
    int2 output_size = imageSize(OUTPUT_IMAGE);

    if (any(greaterThan(pixel_pos, output_size)))
    {
        return;
    }

	float4 output_color = float4(1.0);
	output_color.r = f32((global_idx + i_frame) / 256);
	output_color.g = f32((global_idx + i_frame) % 256);
	output_color.b = 0.0;
	output_color.a = 1.0;

    imageStore(OUTPUT_IMAGE, pixel_pos, output_color);

}
