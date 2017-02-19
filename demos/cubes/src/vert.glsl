#version 150 core

vec3 rotate_vector(vec4 quat, vec3 vec) {
    return vec + 2.0 * cross(cross(vec, quat.xyz) - quat.w * vec, quat.xyz);
}

in vec4 a_Pos;
in ivec4 a_Normal;
in vec4 a_OffsetScale;
in vec4 a_Rotation;
in vec4 a_Color;

out vec4 v_Color;
out vec3 v_Normal;
out vec3 v_HalfDir;

uniform b_Globals {
    mat4 u_Projection;
    vec4 u_CameraPos;
    vec4 u_LightPos;
    vec4 u_LightColor;
};

void main() {
    v_Color = a_Color * u_LightColor;
    v_Normal = rotate_vector(a_Rotation, vec3(a_Normal.xyz));
    vec3 world_pos = rotate_vector(a_Rotation, a_Pos.xyz) * a_OffsetScale.w + a_OffsetScale.xyz;
    vec3 light_dir = normalize(u_LightPos.xyz - world_pos);
    vec3 camera_dir = normalize(u_CameraPos.xyz - world_pos);
    v_HalfDir = normalize(light_dir + camera_dir);
    gl_Position = u_Projection * vec4(world_pos, 1.0);
}
