#version 300 es

precision mediump float;
precision mediump sampler2DArray;

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, _)
uniform vec3 window;

in vec4 pass_style;
out vec4 f_color;

void main() {
    f_color = pass_style;
}
