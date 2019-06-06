#version 110

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, hatching == 1.0)
uniform vec3 window;

varying vec4 pass_color;
varying vec4 f_color;

void main() {
    gl_FragColor = pass_color;

    if (window[2] == 1.0) {
        // The hatching should be done in map-space, so panning/zooming doesn't move the stripes.
        // This is screen_to_map, also accounting for the y-inversion done by the vertex shader.
        float map_x = (gl_FragCoord.x + transform[0]) / transform[2];
        float map_y = (window[1] - gl_FragCoord.y + transform[1]) / transform[2];
        if (mod(map_x + map_y, 2.0) <= 0.1) {
            f_color = vec4(0.0, 1.0, 1.0, 1.0);
        }
        if (mod(map_x - map_y, 2.0) <= 0.1) {
            f_color = vec4(0.0, 1.0, 1.0, 1.0);
        }
    }
}
