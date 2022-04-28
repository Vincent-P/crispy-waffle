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
    vec4 pos = positions[gl_VertexIndex%6];
    pos.xy = pos.xy * 2.0 - 1.0;
    pos.xy *= 0.5;
    gl_Position = pos;
}
