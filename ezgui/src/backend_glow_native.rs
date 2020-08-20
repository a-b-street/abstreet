pub use crate::backend_glow::Drawable;
use crate::backend_glow::{GfxCtxInnards, PrerenderInnards};
use crate::ScreenDims;
use glow::HasContext;

pub fn setup(window_title: &str) -> (PrerenderInnards, winit::event_loop::EventLoop<()>) {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title(window_title)
        .with_maximized(true);
    // TODO Need the same fallback as backend_glium
    // multisampling: 2 looks bad, 4 looks fine
    let context = glutin::ContextBuilder::new()
        .with_multisampling(4)
        .with_depth_buffer(2)
        .build_windowed(window, &event_loop)
        .unwrap();
    let windowed_context = unsafe { context.make_current().unwrap() };
    let gl =
        glow::Context::from_loader_function(|s| windowed_context.get_proc_address(s) as *const _);
    let program = unsafe { gl.create_program().expect("Cannot create program") };

    unsafe {
        let shaders = [
            (glow::VERTEX_SHADER, include_str!("shaders/vertex_140.glsl")),
            (
                glow::FRAGMENT_SHADER,
                include_str!("shaders/fragment_140.glsl"),
            ),
        ]
        .iter()
        .map(|(shader_type, source)| {
            let shader = gl
                .create_shader(*shader_type)
                .expect("Cannot create shader");
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                panic!(gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shader
        })
        .collect::<Vec<_>>();
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!(gl.get_program_info_log(program));
        }
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        gl.use_program(Some(program));

        gl.enable(glow::SCISSOR_TEST);

        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LEQUAL);

        gl.enable(glow::BLEND);
        gl.blend_func_separate(
            glow::SRC_ALPHA,
            glow::ONE_MINUS_SRC_ALPHA,
            glow::SRC_ALPHA,
            glow::ONE_MINUS_SRC_ALPHA,
        );
    }

    (
        PrerenderInnards::new(gl, program, WindowAdapter(windowed_context)),
        event_loop,
    )
}

pub(crate) struct VertexArray {
    pub(crate) id: u32,
    pub(crate) was_destroyed: bool,
}

pub(crate) struct Buffer {
    pub(crate) id: u32,
    pub(crate) was_destroyed: bool,
}

pub struct WindowAdapter(glutin::WindowedContext<glutin::PossiblyCurrent>);

impl WindowAdapter {
    pub fn window(&self) -> &winit::window::Window {
        &self.0.window()
    }

    pub fn window_resized(&self, new_size: ScreenDims, scale_factor: f64) {
        let physical_size = winit::dpi::LogicalSize::from(new_size).to_physical(scale_factor);
        self.0.resize(physical_size);
    }

    pub fn draw_finished(&self, _gfc_ctx_innards: GfxCtxInnards) {
        self.0.swap_buffers().unwrap();
    }
}
