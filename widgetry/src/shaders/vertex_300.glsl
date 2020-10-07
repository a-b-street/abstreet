#version 300 es

precision mediump float;
precision mediump sampler2DArray;

// (x offset, y offset, zoom)
uniform vec3 transform;
// (window width, window height, z value)
uniform vec3 window;
// textures grid
uniform sampler2DArray textures;

layout (location = 0) in vec3 position;
layout (location = 1) in vec4 color;
layout (location = 2) in float texture_index;

out vec4 fs_color;
out vec3 fs_texture_coord;
void main() {
    fs_color = color;

    float zoom = transform[2];

    // This is map_to_screen
    float screen_x = (position[0] * zoom) - transform[0];
    float screen_y = (position[1] * zoom) - transform[1];

    // Translate position to normalized device coordinates (NDC)
    float x = (screen_x / window[0] * 2.0) - 1.0;
    float y = (screen_y / window[1] * 2.0) - 1.0;

    // largest absolute value for layer base z values in drawing.rs (i.e.
    // MAPSPACE_Z vs. TOOLTIP_Z)
    float z_range = 2.0;
    // we increment z up to 1.0 to affect ordering within a layer, so consider
    // that when scaling z to NDC too
    float z_scale = z_range + 1.0;

    float z = (position[2] + window[2]) / z_scale;


    // Note the y inversion
    gl_Position = vec4(x, -y, z, 1.0);

    // An arbitrary factor to scale the textures we're using.
    //
    // The proper value depends on the design of the particular sprite sheet.
    // If we want to support multiple sprite sheets, this could become a
    // uniform, or a vertex attribute depending on how we expect it to change.
    float texture_scale = 16.0;

    float t_x = ((position[0] * zoom)) / texture_scale / zoom;
    float t_y = ((position[1] * zoom)) / texture_scale / zoom;
    fs_texture_coord = vec3(vec2(t_x, t_y), texture_index);
}
