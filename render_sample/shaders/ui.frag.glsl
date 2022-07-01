#version 460
#pragma shader_stage(fragment)

#include "render/types.h"
#include "render_sample/rect.h"
#include "render_sample/sdf.h"

layout(set = SHADER_UNIFORM_SET, binding = 0) uniform Options {
	float2 scale;
	float2 translation;
	u32 vertices_descriptor_index;
	u32 primitive_bytes_offset;
	u32 glyph_atlas_descriptor;
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

	float sd = sdRoundedBox((uv - float2(0.5)) * rect.rect.size, float2(0.5) * rect.rect.size, rect.border_radius);
	float alpha = clamp(0.5 - sd, 0.0, 1.0);
	float4 color = unpackUnorm4x8(rect.color);
	return color * alpha;
}

float4 textured_rect(u32 i_primitive, u32 corner, float2 uv)
{
	u32 primitive_offset = primitive_bytes_offset / sizeof_textured_rect;
	TexturedRect rect = global_buffers_textured_rects[vertices_descriptor_index].rects[primitive_offset + i_primitive];

	float4 color = float4(1.0);
	if (rect.texture_descriptor == glyph_atlas_descriptor)
	{
		float alpha_mask = texture(global_textures[glyph_atlas_descriptor], uv).r;
		float4 base_color = unpackUnorm4x8(rect.base_color);
		color = base_color;
		color.a = alpha_mask;
		color.rgb *= color.a;
	}
	else
	{
		color = texture(global_textures[nonuniformEXT(rect.texture_descriptor)], uv);
	}

	return color;
}

layout(location = 0) in float2 i_uv;
layout(location = 1) in flat u32 i_primitive_index;
layout(location = 0) out float4 o_color;
void main()
{
	u32 corner         = (i_primitive_index & 0xc0000000u) >> 30;
	u32 primitive_type = (i_primitive_index & 0x3f000000u) >> 24;
	u32 i_primitive    = (i_primitive_index & 0x00ffffffu);

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
