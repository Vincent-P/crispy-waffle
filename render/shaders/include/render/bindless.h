#ifndef RENDER_BINDLESS
#define RENDER_BINDLESS

#extension GL_EXT_nonuniform_qualifier : require

#define GLOBAL_SAMPLER_BINDING 0
#define GLOBAL_IMAGE_BINDING 1
#define GLOBAL_BUFFER_BINDING 2

#define GLOBAL_BINDLESS_SET 0
#define GLOBAL_UNIFORM_SET 1
#define SHADER_UNIFORM_SET 2

#define BINDLESS_BUFFER layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_BUFFER_BINDING) buffer
#define BINDLESS_SAMPLER layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_SAMPLER_BINDING) uniform
#define BINDLESS_IMAGE(format) layout(set = GLOBAL_BINDLESS_SET, binding = GLOBAL_IMAGE_BINDING, format) uniform

BINDLESS_SAMPLER sampler2D global_textures[];
BINDLESS_SAMPLER usampler2D global_textures_uint[];
BINDLESS_SAMPLER sampler3D global_textures_3d[];
BINDLESS_SAMPLER usampler3D global_textures_3d_uint[];

BINDLESS_IMAGE(rgba8)   image2D global_images_2d_rgba8[];
BINDLESS_IMAGE(rgba16f) image2D global_images_2d_rgba16f[];
BINDLESS_IMAGE(rgba32f) image2D global_images_2d_rgba32f[];
BINDLESS_IMAGE(r32f)    image2D global_images_2d_r32f[];
#endif
