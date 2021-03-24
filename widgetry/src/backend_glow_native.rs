use abstutil::Timer;

use crate::backend_glow::{build_program, GfxCtxInnards, PrerenderInnards, SpriteTexture};
use crate::{ScreenDims, Settings};

pub fn setup(
    settings: &Settings,
    timer: &mut Timer,
) -> (PrerenderInnards, winit::event_loop::EventLoop<()>) {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title(&settings.window_title)
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
        panic!("Your videocard doesn't support the OpenGL mode requested. This is a common issue when running inside a virtual machine; please run natively if possible. See https://github.com/a-b-street/abstreet/issues/103 for more info, and feel free to ask for help using that issue.\n\nError: {:?}", err);
    });

    let windowed_context = unsafe { context.make_current().unwrap() };
    let gl = unsafe {
        glow::Context::from_loader_function(|s| windowed_context.get_proc_address(s) as *const _)
    };

    let program = unsafe {
        build_program(
            &gl,
            include_str!("../shaders/vertex_140.glsl"),
            include_str!("../shaders/fragment_140.glsl"),
        )
        .or_else(|err| {
            warn!(
                "unable to build program with default shaderrs, falling back to v300. error: {:?}",
                err
            );
            build_program(
                &gl,
                include_str!("../shaders/vertex_300.glsl"),
                include_str!("../shaders/fragment_300.glsl"),
            )
        })
        .unwrap_or_else(|err| {
            panic!("error building program: {:?}", err);
        })
    };

    timer.start("load textures");
    let sprite_texture = SpriteTexture::new(
        include_bytes!("../textures/spritesheet.png").to_vec(),
        64,
        64,
    )
    .expect("failed to format texture sprite sheet");
    sprite_texture
        .upload_gl2(&gl)
        .expect("failed to upload textures");
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
