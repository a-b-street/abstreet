#version 140

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, _)
uniform vec3 window;

in vec4 pass_style;
out vec4 f_color;

void main() {
    // https://en.wikipedia.org/wiki/Grayscale#Luma_coding_in_video_systems
    //float gray = dot(pass_style.rgb, vec3(0.299, 0.587, 0.114));
    //f_color = vec4(vec3(gray), pass_style.a);

    //f_color = vec4(1.0 - pass_style.r, 1.0 - pass_style.g, 1.0 - pass_style.b, pass_style.a);

    f_color = pass_style;
}
