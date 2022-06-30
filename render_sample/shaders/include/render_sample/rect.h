#ifndef RENDER_SAMPLE_RECT
#define RENDER_SAMPLE_RECT

#include "render/types.h"
#include "render/bindless.h"

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
	f32 border_radius;
	u32 padding;
};

const u32 sizeof_color_rect = 2 * sizeof_float4;

struct TexturedRect
{
	Rect rect;
	Rect uv;
	u32 texture_descriptor;
	u32 i_clip_rect;
	f32 border_radius;
	u32 base_color;
};
const u32 sizeof_textured_rect = 3 * sizeof_float4;

const u32 RectType_Color = 0;
const u32 RectType_Textured = 1;

BINDLESS_BUFFER ColorRectBuffer      { ColorRect rects[];  }   global_buffers_color_rects[];
BINDLESS_BUFFER TexturedRectBuffer   { TexturedRect rects[]; } global_buffers_textured_rects[];
#endif
