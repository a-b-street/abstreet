#version 410

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
    vec4 x = fs_color * texture(textures, fs_texture_coord);
    out_color = vec4(x.a * x.r, x.a * x.g, x.a * x.b, x.a);

    // Enable this to quickly see everything in greyscale.
    // https://en.wikipedia.org/wiki/Grayscale#Luma_coding_in_video_systems
    //float gray = dot(fs_color.rgb, vec3(0.299, 0.587, 0.114));
    //out_color = vec4(vec3(gray), fs_color.a);
}
