use glow::HasContext;

use abstutil::Timer;

use crate::backend_glow::{GfxCtxInnards, PrerenderInnards};
use crate::ScreenDims;

pub fn setup(
    window_title: &str,
    timer: &mut Timer,
) -> (PrerenderInnards, winit::event_loop::EventLoop<()>) {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title(window_title)
        .with_maximized(true);
    // TODO If people are hitting problems with context not matching what their GPU provides, dig up
    // backend_glium.rs from git and bring the fallback behavior here. (Ideally, there'd be
    // something in glutin to directly express this.) multisampling: 2 looks bad, 4 looks fine
    let context = glutin::ContextBuilder::new()
        .with_multisampling(4)
        .with_depth_buffer(2)
        .build_windowed(window.clone(), &event_loop)
        .or_else(|err| {
            warn!("Trying default graphics context after standard graphics context failed with error: {:?}",  err);
            glutin::ContextBuilder::new().build_windowed(window.clone(), &event_loop)
        })
        .or_else(|err| {
            warn!("Trying graphics context with vsync after default graphics context failed with error: {:?}", err);
            glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_windowed(window.clone(), &event_loop)
        }).unwrap_or_else(|err| {
        panic!("Your videocard doesn't support the OpenGL mode requested. This is a common issue when running inside a virtual machine; please run natively if possible. See https://github.com/dabreegster/abstreet/issues/103 for more info, and feel free to ask for help using that issue.\n\nError: {:?}", err);
    });

    let windowed_context = unsafe { context.make_current().unwrap() };
    let gl = unsafe {
        glow::Context::from_loader_function(|s| windowed_context.get_proc_address(s) as *const _)
    };
    let program = unsafe { gl.create_program().expect("Cannot create program") };

    unsafe {
        let shaders = compile_shaders(
            &gl,
            include_str!("../shaders/vertex_140.glsl"),
            include_str!("../shaders/fragment_140.glsl"),
        )
        .or_else(|err| {
            warn!(
                "unable to compile default shaderrs, falling back to v300. error: {:?}",
                err
            );
            compile_shaders(
                &gl,
                include_str!("../shaders/vertex_300.glsl"),
                include_str!("../shaders/fragment_300.glsl"),
            )
        })
        .unwrap_or_else(|err| {
            panic!("error building shader: {:?}", err);
        });

        for shader in &shaders {
            gl.attach_shader(program, *shader);
        }

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!(gl.get_program_info_log(program));
        }
        for shader in &shaders {
            gl.detach_shader(program, *shader);
            gl.delete_shader(*shader);
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

    timer.start("load textures");
    crate::backend_glow::load_textures(
        &gl,
        include_bytes!("../textures/spritesheet.png").to_vec(),
        64,
    )
    .unwrap();
    timer.stop("load textures");

    (
        PrerenderInnards::new(gl, program, WindowAdapter(windowed_context)),
        event_loop,
    )
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

unsafe fn compile_shaders(
    gl: &glow::Context,
    vertex_source: &str,
    fragment_source: &str,
) -> Result<[u32; 2], Box<dyn std::error::Error>> {
    unsafe fn compile_shader(
        gl: &glow::Context,
        shader_type: u32,
        shader_source: &str,
    ) -> Result<u32, Box<dyn std::error::Error>> {
        let shader = gl.create_shader(shader_type)?;
        gl.shader_source(shader, shader_source);
        gl.compile_shader(shader);
        if gl.get_shader_compile_status(shader) {
            Ok(shader)
        } else {
            Err(format!("error compiling shader: {}", gl.get_shader_info_log(shader)).into())
        }
    };

    Ok([
        compile_shader(gl, glow::VERTEX_SHADER, vertex_source)?,
        compile_shader(gl, glow::FRAGMENT_SHADER, fragment_source)?,
    ])
}
