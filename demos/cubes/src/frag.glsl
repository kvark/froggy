#version 150 core

in vec4 v_Color;
in vec3 v_Normal;
in vec3 v_HalfDir;

out vec4 Target0;

void main() {
    float diffuse = max(0.0, dot(normalize(v_Normal), normalize(v_HalfDir)));
    Target0 = diffuse * v_Color;
}
