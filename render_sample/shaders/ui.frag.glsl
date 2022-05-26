#version 460
#pragma shader_stage(fragment)

// Types

#define float2 vec2
#define float3 vec3
#define float4 vec4
#define uint2 uvec2
#define uint3 uvec3
#define uint4 uvec4
#define int2 ivec2
#define int3 ivec3
#define int4 ivec4
#define float4x4 mat4
#define float3x3 mat3
#define float2x2 mat2
#define float2x3 mat3x2

#define u32 uint
#define i32 int

const u32 sizeof_float4 = 16;
#define NaN intBitsToFloat(0xffffffff)

// End Types

// Rects

struct Rect
{
	float2 position;
	float2 size;
};
const u32 sizeof_rect = sizeof_float4;

struct ColorRect
{
	Rect rect;
	u32 color;
	u32 i_clip_rect;
	u32 padding[2];
};

const u32 sizeof_color_rect = 2 * sizeof_float4;

struct TexturedRect
{
	Rect rect;
	Rect uv;
	u32 texture_descriptor;
	u32 i_clip_rect;
	u32 padding[2];
};
const u32 sizeof_textured_rect = 3 * sizeof_float4;

const u32 RectType_Color = 0;
const u32 RectType_Textured = 1;

// End Rects

// Bindless
#extension GL_EXT_nonuniform_qualifier : require

#define GLOBAL_SAMPLER_BINDING 0
#define GLOBAL_IMAGE_BINDING 1
#define GLOBAL_BUFFER_BINDING 2

#define GLOBAL_BINDLESS_SET 0
#define GLOBAL_UNIFORM_SET 1
#define SHADER_UNIFORM_SET 2

#define BINDLESS_BUFFER layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_BUFFER_BINDING) buffer

BINDLESS_BUFFER ColorRectBuffer    { ColorRect rects[];  } global_buffers_color_rects[];
BINDLESS_BUFFER TexturedRectBuffer	   { TexturedRect rects[];	 } global_buffers_textured_rects[];

layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_SAMPLER_BINDING) uniform sampler2D global_textures[];
layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_SAMPLER_BINDING) uniform usampler2D global_textures_uint[];
layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_SAMPLER_BINDING) uniform sampler3D global_textures_3d[];
layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_SAMPLER_BINDING) uniform usampler3D global_textures_3d_uint[];

layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_IMAGE_BINDING, rgba8) uniform image2D global_images_2d_rgba8[];
layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_IMAGE_BINDING, rgba16f) uniform image2D global_images_2d_rgba16f[];
layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_IMAGE_BINDING, rgba32f) uniform image2D global_images_2d_rgba32f[];
layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_IMAGE_BINDING, r32f) uniform image2D global_images_2d_r32f[];
// End Bindless

layout(set = SHADER_UNIFORM_SET, binding = 0) uniform Options {
    float2 scale;
    float2 translation;
    u32 vertices_descriptor_index;
    u32 primitive_bytes_offset;
};

bool is_in_rect(float2 pos, Rect rect)
{
    return !(
        pos.x < rect.position.x
     || pos.x > rect.position.x + rect.size.x
     || pos.y < rect.position.y
     || pos.y > rect.position.y + rect.size.y
        );
}

float4 color_rect(u32 i_primitive, u32 corner, float2 uv)
{
    u32 primitive_offset = primitive_bytes_offset / sizeof_color_rect;
    ColorRect rect = global_buffers_color_rects[vertices_descriptor_index].rects[primitive_offset + i_primitive];
    return unpackUnorm4x8(rect.color);
}

float4 textured_rect(u32 i_primitive, u32 corner, float2 uv)
{
    u32 primitive_offset = primitive_bytes_offset / sizeof_textured_rect;
    TexturedRect rect = global_buffers_textured_rects[vertices_descriptor_index].rects[primitive_offset + i_primitive];

	return texture(global_textures[nonuniformEXT(rect.texture_descriptor)], uv);
}

layout(location = 0) in float2 i_uv;
layout(location = 1) in flat u32 i_primitive_index;
layout(location = 0) out float4 o_color;
void main()
{
	u32 corner		   = (i_primitive_index & 0xc0000000u) >> 30;
	u32 primitive_type = (i_primitive_index & 0x3f000000u) >> 24;
	u32 i_primitive	= (i_primitive_index & 0x00ffffffu);

	if (primitive_type == RectType_Color)
	{
		o_color = color_rect(i_primitive, corner, i_uv);
	}
	else if (primitive_type == RectType_Textured)
	{
		o_color = textured_rect(i_primitive, corner, i_uv);
	}
	else
	{
		o_color = float4(1, 0, 1, 1);
	}
}
