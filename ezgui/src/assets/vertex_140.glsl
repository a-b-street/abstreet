#version 300 es

precision mediump float;

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, z value)
uniform vec3 window;

layout (location = 0) in vec2 position;
layout (location = 1) in vec4 style;
out vec4 pass_style;

void main() {
    pass_style = style;

    // This is map_to_screen
    float screen_x = (position[0] * transform[2]) - transform[0];
    float screen_y = (position[1] * transform[2]) - transform[1];
    // Translate that to clip-space or whatever it's called
    float x = (screen_x / window[0] * 2.0) - 1.0;
    float y = (screen_y / window[1] * 2.0) - 1.0;

    // Note the y inversion
    gl_Position = vec4(x, -y, window[2], 1.0);
}
