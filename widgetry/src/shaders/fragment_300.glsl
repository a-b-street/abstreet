#version 300 es

precision mediump float;
precision mediump sampler2DArray;

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, z value)
uniform vec3 window;
// textures grid
uniform sampler2DArray textures;

in vec4 fs_color;
in vec3 fs_texture_coord;

out vec4 out_color;

void main() {
    out_color = fs_color * texture(textures, fs_texture_coord);
}
