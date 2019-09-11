#version 140

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, 0.0=mapspace and 1.0=screenspace)
uniform vec3 window;

in vec2 position;
in vec4 style;
out vec4 pass_style;

void main() {
    pass_style = style;

    // This is map_to_screen
    float screen_x = (position[0] * transform[2]) - transform[0];
    float screen_y = (position[1] * transform[2]) - transform[1];
    // Translate that to clip-space or whatever it's called
    float x = (screen_x / window[0] * 2.0) - 1.0;
    float y = (screen_y / window[1] * 2.0) - 1.0;

    // Screen-space is at z=0.0
    float z = 0.5;
    if (window[2] == 1.0) {
        z = 0.0;
    }

    // Note the y inversion
    gl_Position = vec4(x, -y, z, 1.0);
}
