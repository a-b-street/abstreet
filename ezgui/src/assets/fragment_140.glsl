#version 140

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, _)
uniform vec3 window;

uniform sampler2DArray tex0;
uniform sampler2DArray tex1;
uniform sampler2DArray tex2;
uniform sampler2DArray tex3;
uniform sampler2DArray tex4;
uniform sampler2DArray tex5;
uniform sampler2DArray tex6;
uniform sampler2DArray tex7;
uniform sampler2DArray tex8;
uniform sampler2DArray tex9;
uniform sampler2DArray tex10;
uniform sampler2DArray tex11;
uniform sampler2DArray tex12;
uniform sampler2DArray tex13;
uniform sampler2DArray tex14;

in vec4 pass_style;
out vec4 f_color;

void handle_texture(in sampler2DArray tex, in vec4 style, out vec4 color) {
    if (pass_style[3] >= 200.0) {
        color = texture(tex, vec3(pass_style[0], pass_style[1], pass_style[3] - 200.0));
        if (color[3] > 0.0) {
            color = vec4(1.0, 0.55, 0.0, 1.0);
        }
    } else {
        color = texture(tex, vec3(pass_style[0], pass_style[1], pass_style[3] - 100.0));
    }
}

void main() {
    // See actually_upload in drawing.rs to understand the different things encoded.
    if (pass_style[3] < 100.0) {
        f_color = pass_style;
    } else if (pass_style[2] == 0.0) {
        handle_texture(tex0, pass_style, f_color);
    } else if (pass_style[2] == 1.0) {
        handle_texture(tex1, pass_style, f_color);
    } else if (pass_style[2] == 2.0) {
        handle_texture(tex2, pass_style, f_color);
    } else if (pass_style[2] == 3.0) {
        handle_texture(tex3, pass_style, f_color);
    } else if (pass_style[2] == 4.0) {
        handle_texture(tex4, pass_style, f_color);
    } else if (pass_style[2] == 5.0) {
        handle_texture(tex5, pass_style, f_color);
    } else if (pass_style[2] == 6.0) {
        handle_texture(tex6, pass_style, f_color);
    } else if (pass_style[2] == 7.0) {
        handle_texture(tex7, pass_style, f_color);
    } else if (pass_style[2] == 8.0) {
        handle_texture(tex8, pass_style, f_color);
    } else if (pass_style[2] == 9.0) {
        handle_texture(tex9, pass_style, f_color);
    } else if (pass_style[2] == 10.0) {
        handle_texture(tex10, pass_style, f_color);
    } else if (pass_style[2] == 11.0) {
        handle_texture(tex11, pass_style, f_color);
    } else if (pass_style[2] == 12.0) {
        handle_texture(tex12, pass_style, f_color);
    } else if (pass_style[2] == 13.0) {
        handle_texture(tex13, pass_style, f_color);
    } else if (pass_style[2] == 14.0) {
        handle_texture(tex14, pass_style, f_color);
    } else if (pass_style[0] == 100.0) {
        // The hatching should be done in map-space, so panning/zooming doesn't move the stripes.
        // This is screen_to_map, also accounting for the y-inversion done by the vertex shader.
        float map_x = (gl_FragCoord.x + transform[0]) / transform[2];
        float map_y = (window[1] - gl_FragCoord.y + transform[1]) / transform[2];
        if (mod(map_x + map_y, 2.0) <= 0.1) {
            f_color = vec4(0.0, 1.0, 1.0, 1.0);
        } else if (mod(map_x - map_y, 2.0) <= 0.1) {
            f_color = vec4(0.0, 1.0, 1.0, 1.0);
        } else {
            // Let the polygon with its original colors show instead.
            discard;
	}
    } else if (pass_style[0] == 101.0) {
        float map_x = (gl_FragCoord.x + transform[0]) / transform[2];
        float map_y = (window[1] - gl_FragCoord.y + transform[1]) / transform[2];
        if (mod(map_x + map_y, 2.0) <= 0.5) {
            f_color = vec4(1.0, 1.0, 1.0, 1.0);
        } else {
            // Let the polygon with its original colors show instead.
            discard;
	}
    }
}
