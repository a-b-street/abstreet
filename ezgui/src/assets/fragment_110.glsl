#version 110

varying vec4 pass_color;
varying float pass_hatching;

void main() {
    gl_FragColor = pass_color;

    if (pass_hatching == 1.0 && mod(gl_FragCoord.x + gl_FragCoord.y, 20.0) <= 5.0) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
    }
}
