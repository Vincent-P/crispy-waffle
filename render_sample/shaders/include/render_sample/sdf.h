#ifndef RENDER_SAMPLE_SDF
#define RENDER_SAMPLE_SDF
#include "render/types.h"

f32 sdf_rounded_box_2d(float2 p, float2 b, f32 r)
{
	float2 q = abs(p) - b + r;
	return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

f32 sdf_box_2d(float2 p, float2 b)
{
	float2 d = abs(p) - b;
	return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

float sdf_sphere(float3 pos, float3 center, float radius)
{
	return length(pos - center) - radius;
}

// n = (normal, origin dist)
float sdf_plane(vec3 pos, vec4 n)
{
  // n must be normalized
  return dot(pos, n.xyz) + n.w;
}

float3 sdf_repeat_pos(float3 pos, float3 c)
{
    return mod(pos + 0.5 * c, c) - 0.5 * c;
}

#endif
