use glium::vertex::VertexBufferAny;
use glium::{glutin, program, uniform, Surface};
use std::thread;
use std::time::{Duration, Instant};
use std::{env, process};

mod camera;
mod support;

fn main() {
    // DPI is broken on my system; force the old behavior.
    env::set_var("WINIT_HIDPI_FACTOR", "1.0");

    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title("testing glium")
        .with_dimensions(glutin::dpi::LogicalSize::new(1024.0, 768.0));
    let context = glutin::ContextBuilder::new().with_depth_buffer(24);
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    // TODO The geometry...
    let vertex_buffer = support::load_wavefront(&display, include_bytes!("teapot.obj"));

    let program = program!(&display,
        140 => {
            vertex: "
                #version 140

                uniform mat4 persp_matrix;
                uniform mat4 view_matrix;

                in vec3 position;
                in vec3 normal;
                out vec3 v_position;
                out vec3 v_normal;

                void main() {
                    v_position = position;
                    v_normal = normal;
                    gl_Position = persp_matrix * view_matrix * vec4(v_position * 0.005, 1.0);
                }
            ",

            fragment: "
                #version 140

                in vec3 v_normal;
                out vec4 f_color;

                const vec3 LIGHT = vec3(-0.2, 0.8, 0.1);

                void main() {
                    float lum = max(dot(normalize(v_normal), normalize(LIGHT)), 0.0);
                    vec3 color = (0.3 + 0.7 * lum) * vec3(1.0, 1.0, 1.0);
                    f_color = vec4(color, 1.0);
                }
            ",
        },
    )
    .unwrap();

    let mut camera = camera::CameraState::new();

    let mut accumulator = Duration::new(0, 0);
    let mut previous_clock = Instant::now();

    loop {
        draw(&camera, &display, &program, &vertex_buffer);
        handle_events(&mut camera, &mut events_loop);

        let now = Instant::now();
        accumulator += now - previous_clock;
        previous_clock = now;

        let fixed_time_stamp = Duration::new(0, 16666667);
        while accumulator >= fixed_time_stamp {
            accumulator -= fixed_time_stamp;
            // TODO send off an update event
        }

        thread::sleep(fixed_time_stamp - accumulator);
    }
}

fn draw(
    camera: &camera::CameraState,
    display: &glium::Display,
    program: &glium::Program,
    vertex_buffer: &VertexBufferAny,
) {
    let uniforms = uniform! {
        persp_matrix: camera.get_perspective(),
        view_matrix: camera.get_view(),
    };

    let params = glium::DrawParameters {
        depth: glium::Depth {
            test: glium::DepthTest::IfLess,
            write: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut target = display.draw();
    target.clear_color_and_depth((0.0, 0.0, 0.0, 0.0), 1.0);
    target
        .draw(
            vertex_buffer,
            &glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
            &program,
            &uniforms,
            &params,
        )
        .unwrap();
    target.finish().unwrap();
}

fn handle_events(camera: &mut camera::CameraState, events_loop: &mut glutin::EventsLoop) {
    events_loop.poll_events(|event| match event {
        glutin::Event::WindowEvent { event, .. } => match event {
            glutin::WindowEvent::CloseRequested => {
                process::exit(0);
            }
            glutin::WindowEvent::KeyboardInput { input, .. } => {
                if input.virtual_keycode == Some(glutin::VirtualKeyCode::Escape) {
                    process::exit(0);
                }

                camera.process_input(input);
            }
            _ => {}
        },
        _ => {}
    });
}
