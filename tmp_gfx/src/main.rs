use glium::{glutin, implement_vertex, uniform, Surface};
use std::thread;
use std::time::{Duration, Instant};
use std::{env, process};

mod camera;

fn main() {
    // DPI is broken on my system; force the old behavior.
    env::set_var("WINIT_HIDPI_FACTOR", "1.0");

    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title("testing glium")
        .with_dimensions(glutin::dpi::LogicalSize::new(1024.0, 768.0));
    let context = glutin::ContextBuilder::new().with_depth_buffer(24);
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    let red_triangle = {
        let red = [1.0, 0.0, 0.0, 1.0];
        let vb = glium::VertexBuffer::new(
            &display,
            &[
                Vertex {
                    position: [0.0, 0.5],
                    color: red,
                },
                Vertex {
                    position: [0.0, 0.0],
                    color: red,
                },
                Vertex {
                    position: [0.5, 0.0],
                    color: red,
                },
            ],
        )
        .unwrap();
        Drawable {
            vb,
            indices: glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
        }
    };
    let green_triangle = {
        let green = [0.0, 1.0, 0.0, 1.0];
        let vb = glium::VertexBuffer::new(
            &display,
            &[
                Vertex {
                    position: [-0.5, 0.0],
                    color: green,
                },
                Vertex {
                    position: [0.0, 0.0],
                    color: green,
                },
                Vertex {
                    position: [0.0, -0.5],
                    color: green,
                },
            ],
        )
        .unwrap();
        Drawable {
            vb,
            indices: glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
        }
    };

    let program = glium::Program::from_source(
        &display,
        include_str!("vertex.glsl"),
        include_str!("fragment.glsl"),
        None,
    )
    .unwrap();

    let mut camera = camera::CameraState::new();

    let mut accumulator = Duration::new(0, 0);
    let mut previous_clock = Instant::now();

    loop {
        draw(
            &camera,
            &display,
            &program,
            vec![&red_triangle, &green_triangle],
        );
        handle_events(&mut camera, &mut events_loop);

        let now = Instant::now();
        accumulator += now - previous_clock;
        previous_clock = now;

        let fixed_time_stamp = Duration::new(0, 16_666_667);
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
    stuff: Vec<&Drawable>,
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
    target.clear_color_and_depth((1.0, 1.0, 1.0, 0.0), 1.0);
    for triangle in stuff {
        target
            .draw(
                &triangle.vb,
                &triangle.indices,
                &program,
                &uniforms,
                &params,
            )
            .unwrap();
    }
    target.finish().unwrap();
}

fn handle_events(camera: &mut camera::CameraState, events_loop: &mut glutin::EventsLoop) {
    events_loop.poll_events(|event| {
        if let glutin::Event::WindowEvent { event, .. } = event {
            match event {
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
            };
        }
    });
}

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    // TODO Maybe pass color as a uniform instead
    color: [f32; 4],
}

implement_vertex!(Vertex, position, color);

struct Drawable {
    vb: glium::VertexBuffer<Vertex>,
    indices: glium::index::NoIndices,
}
