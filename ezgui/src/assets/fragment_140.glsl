#version 140

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, hatching == 1.0)
uniform vec3 window;

uniform sampler2D tex0;
uniform sampler2D tex1;
uniform sampler2D tex2;
uniform sampler2D tex3;
uniform sampler2D tex4;
uniform sampler2D tex5;
uniform sampler2D tex6;
uniform sampler2D tex7;
uniform sampler2D tex8;
uniform sampler2D tex9;

in vec4 pass_color;
in vec3 pass_tex_coords;
out vec4 f_color;

void main() {
    if (pass_tex_coords[0] == 0.0) {
        f_color = pass_color;
    } else if (pass_tex_coords[0] == 1.0) {
        f_color = texture(tex0, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 2.0) {
        f_color = texture(tex1, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 3.0) {
        f_color = texture(tex2, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 4.0) {
        f_color = texture(tex3, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 5.0) {
        f_color = texture(tex4, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 6.0) {
        f_color = texture(tex5, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 7.0) {
        f_color = texture(tex6, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 8.0) {
        f_color = texture(tex7, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 9.0) {
        f_color = texture(tex8, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    } else if (pass_tex_coords[0] == 10.0) {
        f_color = texture(tex9, vec2(pass_tex_coords[1], pass_tex_coords[2]));
    }

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
