#version 460
#pragma shader_stage(vertex)

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

BINDLESS_BUFFER ColorRectBuffer	   { ColorRect rects[];	 } global_buffers_color_rects[];
BINDLESS_BUFFER TexturedRectBuffer	   { TexturedRect rects[];	 } global_buffers_textured_rects[];
// End Bindless

layout(set = SHADER_UNIFORM_SET, binding = 0) uniform Options {
	float2 scale;
	float2 translation;
	u32 vertices_descriptor_index;
	u32 primitive_bytes_offset;
};

void color_rect(out float2 o_position, out float2 o_uv, u32 i_primitive, u32 corner)
{
	u32 primitive_offset = primitive_bytes_offset / sizeof_color_rect;
	ColorRect rect = global_buffers_color_rects[vertices_descriptor_index].rects[primitive_offset + i_primitive];

	o_position = rect.rect.position;
	o_uv = float2(0.0);

	// 0 - 3
	// |   |
	// 1 - 2
	if (corner == 1)
	{
		o_position.y += rect.rect.size.y;
		o_uv.y = 1.0f;
	}
	else if (corner == 2)
	{
		o_position += rect.rect.size;
		o_uv = float2(1.0f);
	}
	else if (corner == 3)
	{
		o_position.x += rect.rect.size.x;
		o_uv.x = 1.0f;
	}
}

void textured_rect(out float2 o_position, out float2 o_uv, u32 i_primitive, u32 corner)
{
	u32 primitive_offset = primitive_bytes_offset / sizeof_textured_rect;
	TexturedRect rect = global_buffers_textured_rects[vertices_descriptor_index].rects[primitive_offset + i_primitive];

	o_position = rect.rect.position;
	o_uv = rect.uv.position;

	// 0 - 3
	// |   |
	// 1 - 2
	if (corner == 1)
	{
		o_position.y += rect.rect.size.y;
		o_uv.y += rect.uv.size.y;
	}
	else if (corner == 2)
	{
		o_position += rect.rect.size;
		o_uv += rect.uv.size;
	}
	else if (corner == 3)
	{
		o_position.x += rect.rect.size.x;
		o_uv.x = rect.uv.size.x;
	}
}

layout(location = 0) out float2 o_uv;
layout(location = 1) out flat u32 o_primitive_index;
layout(location = 2) out flat u32 o_corner;
layout(location = 3) out flat u32 o_primitive_type;
layout(location = 4) out flat u32 o_i_index;
void main()
{
	u32 corner		   = (gl_VertexIndex & 0xc0000000u) >> 30;
	u32 primitive_type = (gl_VertexIndex & 0x3f000000u) >> 24;
	u32 i_primitive	   = (gl_VertexIndex & 0x00ffffffu);

	float2 position = float2(0.0);
	float2 uv = float2(0.0);

	if (primitive_type == RectType_Color)
	{
		color_rect(position, uv, i_primitive, corner);
	}
	else if (primitive_type == RectType_Textured)
	{
		textured_rect(position, uv, i_primitive, corner);
	}
	else
	{
		gl_Position = float4(NaN);
		return;
	}

	gl_Position = float4(position * scale + translation, 0.0, 1.0);
	o_uv = uv;
	o_primitive_index = gl_VertexIndex;

	o_corner = corner;
	o_primitive_type = primitive_type;
	o_i_index = i_primitive;
}
