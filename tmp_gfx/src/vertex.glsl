#version 140

uniform mat4 persp_matrix;
uniform mat4 view_matrix;

in vec2 position;
in vec4 color;
out vec4 pass_color;

void main() {
    pass_color = color;

    gl_Position = vec4(position, 0.0, 1.0);
    gl_Position = persp_matrix * view_matrix * vec4(position, 0.0, 1.0);
}
