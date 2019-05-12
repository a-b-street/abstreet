#version 140

in vec4 pass_color;
in float pass_hatching;
out vec4 f_color;

void main() {
    f_color = pass_color;

    if (pass_hatching == 1.0 && mod(gl_FragCoord.x + gl_FragCoord.y, 20.0) <= 5.0) {
        f_color = vec4(0.0, 0.0, 0.0, 1.0);
    }
}
