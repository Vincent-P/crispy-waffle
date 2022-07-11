#ifndef RENDER_TYPES
#define RENDER_TYPES

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
#define f32 float

const u32 sizeof_float4 = 16;
#define NaN intBitsToFloat(0xffffffff)

const float PI  = 3.1415926538;
const float TO_RADIANS = PI / 180.0;

#endif
