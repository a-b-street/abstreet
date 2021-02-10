#version 100

precision mediump float;
precision mediump sampler2D;

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, z value)
uniform vec3 window;

// in
varying vec4 fs_color;
varying vec3 fs_texture_coord;

void main() {
    // FIXME: Texture loading not working with WebGL 1.0
    //
    // Since we originally targeted WebGL 2.0, our texture handling was based on `sampler2DArray`.
    // When we added fallback support for WebGL 1.0, since `sampler2DArray` isn't supported, we could not use the same
    // machinery. For now, since we're not using the textured style, WebGL 1.0 doesn't support textures.
    //
    // Until this is fixed, users will need a WebGL 2.0 browser to use textures. WebGL 2.0 seems to be supported by all
    // modern browsers, but Safari on macOS and any iOS browser (which are all just Safari wrappers) need to have the
    // user toggle the experimental WebGL 2.0 feature for textures to work.
    // vec4 tex_color = texture2D(textures, tex_coord);
    // Hardcode a no-op (white) texture until such a time as texture loading works for WebGL 1.0
    vec4 tex_color = vec4(1.0, 1.0, 1.0, 1.0);

    vec4 x = fs_color * tex_color;
    vec4 out_color = vec4(x.a * x.r, x.a * x.g, x.a * x.b, x.a);
    gl_FragColor = out_color;
}
