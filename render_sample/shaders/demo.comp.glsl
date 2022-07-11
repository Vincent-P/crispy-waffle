#version 460
#pragma shader_stage(compute)

#include "render/types.h"
#include "render/bindless.h"
#include "render/maths.h"
#include "render/hash.h"
#include "render/pbr.h"
#include "render_sample/sdf.h"

layout(set = SHADER_UNIFORM_SET, binding = 0) uniform Options {
	u32 storage_output_frame;
	u32 i_frame;
	f32 dt;
	f32 t;
};

#define OUTPUT_IMAGE global_images_2d_rgba8[storage_output_frame]

float4x4 view_matrix(float3 eye, float3 at, float3 up, out float4x4 inv)
{
	float3 z_axis = normalize(at - eye);
	float3 x_axis = normalize(cross(z_axis, up));
	float3 y_axis = cross(x_axis, z_axis);


	float4x4 result = transpose(float4x4(
									x_axis.x  ,  x_axis.y,   x_axis.z,   -dot(eye,x_axis),
									y_axis.x  ,  y_axis.y,   y_axis.z,   -dot(eye,y_axis),
									-z_axis.x ,  -z_axis.y,  -z_axis.z,  dot(eye, z_axis),
									0.0      ,  0.0,       0.0,       1.0
									));

	inv = transpose(float4x4(
						x_axis.x ,  y_axis.x,  -z_axis.x,  eye.x,
						x_axis.y ,  y_axis.y,  -z_axis.y,  eye.y,
						x_axis.z ,  y_axis.z,  -z_axis.z,  eye.z,
						0.0     ,  0.0,      0.0,       1.0
						));

	return result;
}



float4x4 infinite_perspective(float fov, float aspect_ratio, float near_plane, out float4x4 inverse)
{
	float n = near_plane;

	float focal_length = 1.0f / tan(TO_RADIANS * fov * 0.5); // = 2n / (height)
	// aspect_ratio = width/height
	float x  =  focal_length / aspect_ratio; // (2n/height)*(height/width) => 2n/width
	float y  = -focal_length; // -2n/height

	float4x4 projection = transpose(float4x4(
										x,    0.0f, 0.0f,  0.0f,
										0.0f, y,    0.0f,  0.0f,
										0.0f, 0.0f,    0,     n,
										0.0f, 0.0f, -1.0f, 0.0f
										));

	inverse = transpose(float4x4(
							1/x,  0.0f, 0.0f, 0.0f,
							0.0f, 1/y,  0.0f, 0.0f,
							0.0f, 0.0f, 0.0f, -1.0f,
							0.0f, 0.0f, 1/n,  0.0f
							));

	return projection;
}


struct Ray
{
	float3 origin;
	float t_min;
	float3 direction;
	float t_max;
};

bool fast_box_intersection(float3 box_min, float3 box_max, Ray ray, float3 inv_ray_dir)
{
	float3 t0 = (box_min - ray.origin) * inv_ray_dir;
	float3 t1 = (box_max - ray.origin) * inv_ray_dir;
	float tmin = max(max3(min(t0,t1)), ray.t_min);
	float tmax = min(min3(max(t0,t1)), ray.t_max);
	return tmin <= tmax;
}

float sdf_scene(float3 pos)
{
	float3 sphere_center = float3(0.0, 0.5, 0.0);
	sphere_center.y = sin(0.5 * PI * t) + 0.75;
	float sphere_radius = 1.0;
	float sphere_dist = sdf_sphere(sdf_repeat_pos(pos, float3(20.0, 100.0, 20.0)), sphere_center, sphere_radius);
	float d = sphere_dist;

	sphere_center.xz = float2(2.0, 1.0 + 2.0 * cos(PI * t));
	sphere_center.y = 0.5 * cos(0.5 * PI * t) + 1.0;
	sphere_radius = 0.10;
	sphere_dist = sdf_sphere(sdf_repeat_pos(pos, float3(7.0, 100.0, 7.0)), sphere_center, sphere_radius);
	d = min(d, sphere_dist);

	sphere_center.xz = 2.0 * float2(cos(0.33 * PI * t), cos(0.5 * PI * t)) + float2(-0.1, -0.33);
	sphere_center.y = 0.5 * cos(0.5 * PI * t) + 1.0;
	sphere_radius = 0.33;
	sphere_dist = sdf_sphere(sdf_repeat_pos(pos, float3(10.0)), sphere_center, sphere_radius);
	d = min(d, sphere_dist);

	float4 floor_plane = float4(0.0, 1.0, 0.0, 0.0);
	float floor_dist = sdf_plane(pos, floor_plane);
	d = min(d, floor_dist);

	return d;
}

float3 sdf_scene_gradient(float3 p) {
	const float DD = 0.1;
	return normalize(float3(
		sdf_scene(float3(p.x + DD, p.y, p.z)) -  sdf_scene(float3(p.x - DD, p.y, p.z)),
		sdf_scene(float3(p.x, p.y + DD, p.z)) -  sdf_scene(float3(p.x, p.y - DD, p.z)),
		sdf_scene(float3(p.x, p.y, p.z  + DD)) - sdf_scene(float3(p.x, p.y, p.z - DD))
	));
}

float sdf_trace_shadows(float3 p, float3 light_dir, float light_size, float tmin, float tmax)
{
	float visibility = 1.0;
	float d = tmin;
	float previous_d = 1e20;
	while (d < tmax)
	{
		float dist = sdf_scene(p + d * light_dir);
		if (dist < 0.001)
		{
			return 0.0;
		}

		float dist_2 = dist*dist;
        float intersection_d = dist_2 / (2.0*previous_d);
        float closest = sqrt(dist_2 - intersection_d * intersection_d);
        visibility = min(visibility, light_size * closest / max(0.0, d - intersection_d));

        previous_d = d;
		d += dist;
	}
	return visibility;
}


vec3 TurboColormap(in float x) {
	const vec4 kRedVec4 = vec4(0.13572138, 4.61539260, -42.66032258, 132.13108234);
	const vec4 kGreenVec4 = vec4(0.09140261, 2.19418839, 4.84296658, -14.18503333);
	const vec4 kBlueVec4 = vec4(0.10667330, 12.64194608, -60.58204836, 110.36276771);
	const vec2 kRedVec2 = vec2(-152.94239396, 59.28637943);
	const vec2 kGreenVec2 = vec2(4.27729857, 2.82956604);
	const vec2 kBlueVec2 = vec2(-89.90310912, 27.34824973);

	x = saturate(x);
	vec4 v4 = vec4( 1.0, x, x * x, x * x * x);
	vec2 v2 = v4.zw * v4.z;
	return vec3(
		dot(v4, kRedVec4)   + dot(v2, kRedVec2),
		dot(v4, kGreenVec4) + dot(v2, kGreenVec2),
		dot(v4, kBlueVec4)  + dot(v2, kBlueVec2)
	);
}

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

	float2 uv = float2(pixel_pos) / float2(output_size);
	float4 clip_space = float4(uv * float2(2.0) - float2(1.0), 1.0, 1.0);

	float2 neighbor_uv = (float2(pixel_pos) + float2(1.0)) / float2(output_size);
	float4 neighbor_clip_space = float4(neighbor_uv * float2(2.0) - float2(1.0), 1.0, 1.0);

	const float ORBIT_RADIUS = 5.0;
	const float ROT = TO_RADIANS * 33.0;
	float3 camera_pos = float3(5.0);
	camera_pos.x = ORBIT_RADIUS * cos(ROT * t);
	camera_pos.z = ORBIT_RADIUS * sin(ROT * t);
	camera_pos.y = 2.5;

	float3 target_pos = float3(0.0);

	const float FOV = 60.0;
	const float NEAR_PLANE = 0.1;
	float aspect_ratio = float(output_size.x) / float(output_size.y);
	float4x4 invproj = float4x4(1.0);
	float4x4 proj = infinite_perspective(FOV, aspect_ratio, NEAR_PLANE, invproj);
	float4x4 invview = float4x4(1.0);
	float4x4 view = view_matrix(camera_pos, target_pos, float3(0.0, 1.0, 0.0), invview);

	// world pos in near plane
	float4 pixel_world_pos = invview * (invproj * clip_space);
	pixel_world_pos /= pixel_world_pos.w;

	float4 neighbor_pixel_world_pos = invview * (invproj * neighbor_clip_space);
	neighbor_pixel_world_pos /= neighbor_pixel_world_pos.w;

	float3 pixel_size_ws = abs(neighbor_pixel_world_pos.xyz - pixel_world_pos.xyz);
	float pixel_radius_ws = max3(0.5 * pixel_size_ws);

	float3 ro = camera_pos;
	float3 rd = normalize(pixel_world_pos.xyz - camera_pos);

	const uint MAX_SAMPLES = 2;
	const float MAX_DIST = 100.0;

	float d = 0.0;
	float inter_steps = 0.0;
	float A = 1.0 / (1.0 + pixel_radius_ws);
	// intersect scene
	while (d < MAX_DIST && inter_steps < 500.0)
	{
		float3 p = ro + d * rd;
		float dist = sdf_scene(p);
		if (dist < pixel_radius_ws * d)
		{
			break;
		}
		d = d + dist;
		inter_steps += 1.0;
	}

	float3 R = float3(0.0);
	if (d < MAX_DIST)
	{
		uint3 seed = uint3(pixel_pos.x, pixel_pos.y, i_frame);
		uint3 hash = hash3(seed);
		float3 rng = hash_to_float3(hash);

		float3 p = ro + d * rd;

		float3 N = sdf_scene_gradient(p);
		float3 albedo = float3(1.0, 1.0, 1.0);
		float roughness = 0.5;
		float metallic = 0.5;
		float3 emissive = float3(0.0);

		float aperture = 60.0 * TO_RADIANS;
		float3 cone_dir = N;
		float C = sqrt(aperture*aperture + 1.0);
		float A = C / (C - aperture);

		float t = 0.2;
		while (t < 20.0)
		{
			float dist = sdf_scene(p + t * cone_dir);
			if (dist < 0.01) {
				break;
			}
			t = (t + dist) * A;
		}
		R = float3(t / 20.0);
		R = float3(N * 0.5 + 0.5);
		R = fract(p);
		R = TurboColormap(inter_steps / 200.0);
		R = float3(pixel_radius_ws * 10000.0);
		R = pixel_size_ws * 10000.0;
	}

	float4 output_color = float4(1.0);
	output_color.rgb = float3(R);
	output_color.a = 1.0;

	imageStore(OUTPUT_IMAGE, pixel_pos, output_color);

}
