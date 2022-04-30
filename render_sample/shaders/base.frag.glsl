#version 460
#pragma shader_stage(fragment)

layout(set = 2, binding = 0) uniform Options {
	vec4 color;
};

layout(location = 0) out vec4 o_color;
void main()
{
    o_color = vec4(color);
}
