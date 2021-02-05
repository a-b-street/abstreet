use glow::HasContext;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use winit::platform::web::WindowExtWebSys;

use abstutil::Timer;

use crate::backend_glow::{GfxCtxInnards, PrerenderInnards, SpriteTexture};
use crate::ScreenDims;

pub fn setup(
    window_title: &str,
    timer: &mut Timer,
) -> (PrerenderInnards, winit::event_loop::EventLoop<()>) {
    info!("Setting up widgetry");

    // This doesn't seem to work for the shader panics here, but later it does work. Huh.
    std::panic::set_hook(Box::new(|info| {
        error!("Panicked: {}", info);
    }));

    let event_loop = winit::event_loop::EventLoop::new();
    let get_full_size = || {
        // TODO Not sure how to get scrollbar dims
        let scrollbars = 30.0;
        let win = web_sys::window().unwrap();
        // `inner_width` corresponds to the browser's `self.innerWidth` function, which are in
        // Logical, not Physical, pixels
        winit::dpi::LogicalSize::new(
            win.inner_width().unwrap().as_f64().unwrap() - scrollbars,
            win.inner_height().unwrap().as_f64().unwrap() - scrollbars,
        )
    };
    let winit_window = winit::window::WindowBuilder::new()
        .with_title(window_title)
        .with_inner_size(get_full_size())
        .build(&event_loop)
        .unwrap();
    let canvas = winit_window.canvas();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    document.get_element_by_id("loading").unwrap().remove();
    let body = document.body().unwrap();
    body.append_child(&canvas)
        .expect("Append canvas to HTML body");

    let winit_window = Rc::new(winit_window);

    // resize of our winit::Window whenever the browser window changes size.
    {
        let winit_window = winit_window.clone();
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |e: web_sys::Event| {
            debug!("handling resize event: {:?}", e);
            let size = get_full_size();
            winit_window.set_inner_size(size)
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // First try WebGL 2.0 context.
    // WebGL 2.0 isn't supported by default on macOS Safari, or any iOS browser (which are all just
    // Safari wrappers).
    let (program, gl) = webgl2_program_context(&canvas)
        .or_else(|err| {
            warn!(
                "failed to build WebGL 2.0 context with error: \"{}\". Trying WebGL 1.0 instead...",
                err
            );
            webgl1_program_context(&canvas)
        })
        .unwrap();

    fn webgl2_program_context(
        canvas: &web_sys::HtmlCanvasElement,
    ) -> anyhow::Result<(glow::Program, glow::Context)> {
        let maybe_context: Option<_> = canvas
            .get_context("webgl2")
            .map_err(|err| anyhow!("error getting context for WebGL 2.0: {:?}", err))?;
        let js_webgl2_context =
            maybe_context.ok_or(anyhow!("Browser doesn't support WebGL 2.0"))?;
        let webgl2_context = js_webgl2_context
            .dyn_into::<web_sys::WebGl2RenderingContext>()
            .map_err(|err| anyhow!("unable to cast to WebGl2RenderingContext. error: {:?}", err))?;
        let gl = glow::Context::from_webgl2_context(webgl2_context);
        let shader_inputs = [
            (
                glow::VERTEX_SHADER,
                include_str!("../shaders/vertex_300.glsl"),
            ),
            (
                glow::FRAGMENT_SHADER,
                include_str!("../shaders/fragment_300.glsl"),
            ),
        ];
        let program = unsafe { build_program(&gl, &shader_inputs)? };

        info!("start load textures");
        let sprite_texture = SpriteTexture::new(
            include_bytes!("../textures/spritesheet.png").to_vec(),
            64,
            64,
        )
        .expect("failed to format texture sprite sheet");
        sprite_texture
            .upload_gl2(&gl)
            .expect("failed to upload textures");
        info!("stop load textures");

        Ok((program, gl))
    }

    fn webgl1_program_context(
        canvas: &web_sys::HtmlCanvasElement,
    ) -> anyhow::Result<(glow::Program, glow::Context)> {
        let maybe_context: Option<_> = canvas
            .get_context("webgl")
            .map_err(|err| anyhow!("error getting context for WebGL 1.0: {:?}", err))?;
        let js_webgl1_context =
            maybe_context.ok_or(anyhow!("Browser doesn't support WebGL 1.0"))?;
        let webgl1_context = js_webgl1_context
            .dyn_into::<web_sys::WebGlRenderingContext>()
            .map_err(|err| anyhow!("unable to cast to WebGlRenderingContext. error: {:?}", err))?;
        let gl = glow::Context::from_webgl1_context(webgl1_context);

        let shader_inputs = [
            (
                glow::VERTEX_SHADER,
                include_str!("../shaders/vertex_webgl1.glsl"),
            ),
            (
                glow::FRAGMENT_SHADER,
                include_str!("../shaders/fragment_webgl1.glsl"),
            ),
        ];
        let program = unsafe { build_program(&gl, &shader_inputs)? };
        Ok((program, gl))
    }

    // TODO: move this to backend_glow and share w/ native backend?
    /// shaders_input: (shader_type: u32, shader_src: &str)
    unsafe fn build_program(
        gl: &glow::Context,
        shaders_input: &[(u32, &str)],
    ) -> anyhow::Result<glow::Program> {
        let program = gl.create_program().expect("Cannot create program");

        let shaders = shaders_input
            .iter()
            .map(|(shader_type, source)| {
                let shader = gl
                    .create_shader(*shader_type)
                    .expect("Cannot create shader");
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    error!("Shader error: {}", gl.get_shader_info_log(shader));
                    panic!(gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shader
            })
            .collect::<Vec<_>>();

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            error!("Linking error: {}", gl.get_program_info_log(program));
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
            glow::ONE,
            glow::ONE_MINUS_SRC_ALPHA,
            glow::ONE_MINUS_DST_ALPHA,
            glow::ONE,
        );

        Ok(program)
    }

    (
        PrerenderInnards::new(gl, program, WindowAdapter(winit_window)),
        event_loop,
    )
}

pub struct WindowAdapter(Rc<winit::window::Window>);

impl WindowAdapter {
    pub fn window(&self) -> &winit::window::Window {
        &self.0
    }

    pub fn window_resized(&self, new_size: ScreenDims, scale_factor: f64) {
        debug!(
            "[window_resize] new_size: {:?}, scale_factor: {}",
            new_size, scale_factor
        );
    }

    pub fn draw_finished(&self, _gfc_ctx_innards: GfxCtxInnards) {}
}
