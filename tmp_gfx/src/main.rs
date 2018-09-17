#[macro_use]
extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_window_glutin;
extern crate glutin;

use gfx::traits::{Device, FactoryExt};
use glutin::dpi::LogicalSize;
use glutin::GlContext;

type ColorFormat = gfx::format::Rgba8;
type DepthFormat = gfx::format::DepthStencil;

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

gfx_defines!{
    vertex GpuFillVertex {
        position: [f32; 2] = "a_position",
    }

    pipeline fill_pipeline {
        vbo: gfx::VertexBuffer<GpuFillVertex> = (),
        out_color: gfx::RenderTarget<ColorFormat> = "out_color",
    }
}

fn main() {
    let mut events_loop = glutin::EventsLoop::new();

    let (initial_width, initial_height) = (700.0, 700.0);

    let glutin_builder = glutin::WindowBuilder::new()
        .with_dimensions(LogicalSize::new(initial_width, initial_height))
        .with_decorations(true)
        .with_title("gfx playground".to_string());

    let context = glutin::ContextBuilder::new().with_vsync(true);

    let (window, mut device, mut factory, mut main_fbo, mut main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(glutin_builder, context, &events_loop);

    let shader = factory
        .link_program(VERTEX_SHADER.as_bytes(), FRAGMENT_SHADER.as_bytes())
        .unwrap();

    let pso = factory
        .create_pipeline_from_program(
            &shader,
            gfx::Primitive::TriangleList,
            gfx::state::Rasterizer::new_fill(),
            fill_pipeline::new(),
        ).unwrap();

    // The geometry!
    let vertices = vec![
        // 0 = Top-left
        GpuFillVertex {
            position: [-1.0, 0.7],
        },
        // 1 = Top-right
        GpuFillVertex {
            position: [1.0, 1.0],
        },
        // 2 = Bottom-left
        GpuFillVertex {
            position: [-1.0, -1.0],
        },
        // 3 = Bottom-right
        GpuFillVertex {
            position: [1.0, -1.0],
        },
    ];
    let indices: Vec<u16> = vec![0, 1, 2, 1, 2, 3];
    let (vbo, ibo) = factory.create_vertex_buffer_with_slice(&vertices, &indices[..]);

    let mut cmd_queue: gfx::Encoder<_, _> = factory.create_command_buffer().into();

    let mut cam = Camera {
        center_x: initial_width / 2.0,
        center_y: initial_height / 2.0,
        zoom: 1.0,
    };
    loop {
        if !handle_input(&mut events_loop, &mut cam) {
            break;
        }

        gfx_window_glutin::update_views(&window, &mut main_fbo, &mut main_depth);

        cmd_queue.clear(&main_fbo.clone(), BLACK);
        cmd_queue.draw(
            &ibo,
            &pso,
            &fill_pipeline::Data {
                vbo: vbo.clone(),
                out_color: main_fbo.clone(),
            },
        );
        cmd_queue.flush(&mut device);

        window.swap_buffers().unwrap();

        device.cleanup();
    }
}

struct Camera {
    // Center on some point
    center_x: f64,
    center_y: f64,
    zoom: f64,
}

fn handle_input(event_loop: &mut glutin::EventsLoop, cam: &mut Camera) -> bool {
    use glutin::ElementState::Pressed;
    use glutin::Event;
    use glutin::VirtualKeyCode;

    let mut keep_running = true;

    event_loop.poll_events(|event| match event {
        Event::WindowEvent {
            event: glutin::WindowEvent::CloseRequested,
            ..
        } => {
            println!("Window Closed!");
            keep_running = false;
        }
        Event::WindowEvent {
            event:
                glutin::WindowEvent::KeyboardInput {
                    input:
                        glutin::KeyboardInput {
                            state: Pressed,
                            virtual_keycode: Some(key),
                            ..
                        },
                    ..
                },
            ..
        } => match key {
            VirtualKeyCode::Escape => {
                keep_running = false;
            }
            VirtualKeyCode::Left => {
                cam.center_x -= 1.0;
            }
            VirtualKeyCode::Right => {
                cam.center_x += 1.0;
            }
            VirtualKeyCode::Up => {
                cam.center_y += 1.0;
            }
            VirtualKeyCode::Down => {
                cam.center_y -= 1.0;
            }
            _ => {}
        },
        _ => {}
    });

    keep_running
}

// Coordinate system is math-like -- Y increases up.

static VERTEX_SHADER: &'static str = "
    #version 140

    in vec2 a_position;
    out vec4 v_color;

    void main() {
        gl_Position = vec4(a_position, 0.0, 1.0);
        // gl_Position.y *= -1.0;
        v_color = vec4(1.0, 0.0, 0.0, 0.5);
    }
";

static FRAGMENT_SHADER: &'static str = "
    #version 140
    in vec4 v_color;
    out vec4 out_color;

    void main() {
        out_color = v_color;
    }
";
