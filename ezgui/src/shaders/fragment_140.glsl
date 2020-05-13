#version 140

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, _)
uniform vec3 window;

in vec4 pass_style;
out vec4 f_color;

void main() {
    // See actually_upload in drawing.rs to understand the different things encoded.
    if (pass_style[0] == 100.0) {
        float map_x = (gl_FragCoord.x + transform[0]) / transform[2];
        float map_y = (window[1] - gl_FragCoord.y + transform[1]) / transform[2];
        if (mod(map_x + map_y, 2.0) <= 0.5) {
            f_color = vec4(1.0, 1.0, 1.0, 1.0);
        } else {
            // Let the polygon with its original colors show instead.
            discard;
	}
    } else {
        // https://en.wikipedia.org/wiki/Grayscale#Luma_coding_in_video_systems
        //float gray = dot(pass_style.rgb, vec3(0.299, 0.587, 0.114));
        //f_color = vec4(vec3(gray), pass_style.a);
        f_color = pass_style;
    }
}
