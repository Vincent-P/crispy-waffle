#ifndef RENDER_SAMPLE_SDF
#define RENDER_SAMPLE_SDF
#include "render/types.h"

f32 sdRoundedBox(float2 p, float2 b, f32 r)
{
	float2 q = abs(p) - b + r;
	return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

f32 sdBox(float2 p, float2 b)
{
	float2 d = abs(p) - b;
	return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

#endif
