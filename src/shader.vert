#version 450

out gl_PerVertex {
    vec4 gl_Position;
};

layout(location = 0) out vec4 outColor;

const vec2 positions[3] = vec2[3](
    vec2(0.0, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, 0.5)
);

const vec3 colors[3] = vec3[3](
    vec3(0.0, 1.0, 1.0),
    vec3(1.0, 0.0, 1.0),
    vec3(1.0, 1.0, 0.0)
);

void main() {
    outColor = vec4(colors[gl_VertexIndex], 1.0);;
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
}
