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

    let glutin_builder = glutin::WindowBuilder::new()
        .with_dimensions(LogicalSize::new(700.0, 700.0))
        .with_decorations(true)
        .with_title("Simple tessellation".to_string());

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
        GpuFillVertex {
            position: [-1.0, -1.0],
        },
        GpuFillVertex {
            position: [1.0, -1.0],
        },
        GpuFillVertex {
            position: [-1.0, 1.0],
        },
        GpuFillVertex {
            position: [1.0, 1.0],
        },
    ];
    let indices: Vec<u16> = vec![0, 1, 2, 2, 3, 0];
    let (vbo, ibo) = factory.create_vertex_buffer_with_slice(&vertices, &indices[..]);

    let mut cmd_queue: gfx::Encoder<_, _> = factory.create_command_buffer().into();

    loop {
        if !update_inputs(&mut events_loop) {
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

fn update_inputs(event_loop: &mut glutin::EventsLoop) -> bool {
    use glutin::ElementState::Pressed;
    use glutin::Event;
    use glutin::VirtualKeyCode;

    let mut status = true;

    event_loop.poll_events(|event| match event {
        Event::WindowEvent {
            event: glutin::WindowEvent::CloseRequested,
            ..
        } => {
            println!("Window Closed!");
            status = false;
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
                status = false;
            }
            _key => {}
        },
        _ => {}
    });

    status
}

static VERTEX_SHADER: &'static str = "
    #version 140

    in vec2 a_position;
    out vec4 v_color;

    void main() {
        gl_Position = vec4(a_position, 0.0, 1.0);
        gl_Position.y *= -1.0;
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
