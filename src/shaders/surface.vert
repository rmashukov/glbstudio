#version 120
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;
uniform mat4 Model;
uniform mat4 Projection;
varying vec2 uv;
varying vec4 color;
varying vec3 world_position;
varying vec3 world_normal;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    world_position = position;
    world_normal = normal.xyz;
    uv = texcoord;
    color = color0 / 255.0;
}
