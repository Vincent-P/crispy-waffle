#version 460
#pragma shader_stage(vertex)

const vec4 positions[] = {
    vec4(0, 0, 1, 1),
    vec4(1, 0, 1, 1),
    vec4(0, 1, 1, 1),
    vec4(1, 0, 1, 1),
    vec4(1, 1, 1, 1),
    vec4(0, 1, 1, 1),
};

void main()
{
    gl_Position = positions[gl_VertexIndex%6];
}
