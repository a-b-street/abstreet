use std::cell::Cell;
use std::rc::Rc;

use anyhow::Result;
use wasm_bindgen::JsCast;
use winit::platform::web::WindowExtWebSys;

use abstutil::Timer;

use crate::assets::Assets;
use crate::backend_glow::{build_program, GfxCtxInnards, PrerenderInnards, SpriteTexture};
use crate::{Canvas, Event, EventCtx, GfxCtx, Prerender, ScreenDims, Settings, Style, UserInput};

pub fn setup(
    settings: &Settings,
    timer: &mut Timer,
) -> (PrerenderInnards, winit::event_loop::EventLoop<()>) {
    info!("Setting up widgetry");

    // This doesn't seem to work for the shader panics here, but later it does work. Huh.
    std::panic::set_hook(Box::new(|info| {
        error!("Panicked: {}", info);
    }));

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let root_element = document
        .get_element_by_id(&settings.root_dom_element_id)
        .expect("failed to find root widgetry element");

    // Clear out any loading messages
    root_element.set_inner_html("");

    let root_element_size = {
        let root_element = root_element.clone();
        move || {
            winit::dpi::LogicalSize::new(root_element.client_width(), root_element.client_height())
        }
    };

    let event_loop = winit::event_loop::EventLoop::new();
    let winit_window = winit::window::WindowBuilder::new()
        .with_title(&settings.window_title)
        .with_inner_size(root_element_size())
        .build(&event_loop)
        .unwrap();
    let canvas = winit_window.canvas();
    root_element
        .append_child(&canvas)
        .expect("failed to append canvas to widgetry root element");

    let winit_window = Rc::new(winit_window);

    // resize of our winit::Window whenever the browser window changes size.
    {
        let winit_window = winit_window.clone();
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |e: web_sys::Event| {
            debug!("handling resize event: {:?}", e);
            winit_window.set_inner_size(root_element_size());
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // First try WebGL 2.0 context.
    // WebGL 2.0 isn't supported by default on macOS Safari, or any iOS browser (which are all just
    // Safari wrappers).
    let (gl, program) = webgl2_glow_context(&canvas)
        .and_then(|gl| webgl2_program(gl, timer))
        .or_else(|err| {
            warn!(
                "failed to build WebGL 2.0 context with error: \"{}\". Trying WebGL 1.0 instead...",
                err
            );
            webgl1_glow_context(&canvas).and_then(|gl| webgl1_program(gl, timer))
        })
        .unwrap();

    fn webgl2_glow_context(canvas: &web_sys::HtmlCanvasElement) -> Result<glow::Context> {
        let maybe_context: Option<_> = canvas
            .get_context("webgl2")
            .map_err(|err| anyhow!("error getting context for WebGL 2.0: {:?}", err))?;
        let js_webgl2_context =
            maybe_context.ok_or(anyhow!("Browser doesn't support WebGL 2.0"))?;
        let webgl2_context = js_webgl2_context
            .dyn_into::<web_sys::WebGl2RenderingContext>()
            .map_err(|err| anyhow!("unable to cast to WebGl2RenderingContext. error: {:?}", err))?;
        Ok(glow::Context::from_webgl2_context(webgl2_context))
    }

    fn webgl1_glow_context(canvas: &web_sys::HtmlCanvasElement) -> Result<glow::Context> {
        let maybe_context: Option<_> = canvas
            .get_context("webgl")
            .map_err(|err| anyhow!("error getting context for WebGL 1.0: {:?}", err))?;
        let js_webgl1_context =
            maybe_context.ok_or(anyhow!("Browser doesn't support WebGL 1.0"))?;
        let webgl1_context = js_webgl1_context
            .dyn_into::<web_sys::WebGlRenderingContext>()
            .map_err(|err| anyhow!("unable to cast to WebGlRenderingContext. error: {:?}", err))?;
        Ok(glow::Context::from_webgl1_context(webgl1_context))
    }

    (
        PrerenderInnards::new(gl, program, Some(WindowAdapter(winit_window))),
        event_loop,
    )
}

fn webgl2_program(gl: glow::Context, timer: &mut Timer) -> Result<(glow::Context, glow::Program)> {
    let program = unsafe {
        build_program(
            &gl,
            include_str!("../shaders/vertex_300.glsl"),
            include_str!("../shaders/fragment_300.glsl"),
        )?
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

    Ok((gl, program))
}

fn webgl1_program(gl: glow::Context, timer: &mut Timer) -> Result<(glow::Context, glow::Program)> {
    let program = unsafe {
        build_program(
            &gl,
            include_str!("../shaders/vertex_webgl1.glsl"),
            include_str!("../shaders/fragment_webgl1.glsl"),
        )?
    };

    timer.start("load textures");
    let sprite_texture = SpriteTexture::new(
        include_bytes!("../textures/spritesheet.png").to_vec(),
        64,
        64,
    )
    .expect("failed to format texture sprite sheet");
    sprite_texture
        .upload_webgl1(&gl)
        .expect("failed to upload textures");
    timer.stop("load textures");

    Ok((gl, program))
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

/// Sets up widgetry in a mode where it just draws to a WebGL context and doesn't handle events or
/// interactions at all.
pub struct RenderOnly {
    prerender: Prerender,
    style: Style,
    canvas: Canvas,
}

impl RenderOnly {
    pub fn new(raw_gl: web_sys::WebGlRenderingContext, settings: Settings) -> RenderOnly {
        std::panic::set_hook(Box::new(|info| {
            error!("Panicked: {}", info);
        }));

        info!("Setting up widgetry in render-only mode");
        let mut timer = Timer::new("setup render-only");
        let initial_size = ScreenDims::new(
            raw_gl.drawing_buffer_width().into(),
            raw_gl.drawing_buffer_height().into(),
        );
        // Mapbox always seems to hand us WebGL1
        let (gl, program) =
            webgl1_program(glow::Context::from_webgl1_context(raw_gl), &mut timer).unwrap();
        let prerender_innards = PrerenderInnards::new(gl, program, None);

        let style = Style::light_bg();
        let prerender = Prerender {
            assets: Assets::new(
                style.clone(),
                settings.assets_base_url,
                settings.assets_are_gzipped,
                settings.read_svg,
            ),
            num_uploads: Cell::new(0),
            inner: prerender_innards,
            scale_factor: settings.scale_factor.unwrap_or(1.0),
        };
        let canvas = Canvas::new(initial_size, settings.canvas_settings);

        RenderOnly {
            prerender,
            style,
            canvas,
        }
    }

    /// Creates a no-op `EventCtx`, just for client code that needs this interface to upload
    /// geometry. There's no actual event.
    pub fn event_ctx(&mut self) -> EventCtx {
        EventCtx {
            fake_mouseover: true,
            input: UserInput::new(Event::NoOp, &self.canvas),
            canvas: &mut self.canvas,
            prerender: &self.prerender,
            style: &mut self.style,
            updates_requested: vec![],
            canvas_movement_called: false,
            focus_owned_by: None,
            next_focus_owned_by: None,
        }
    }

    /// Creates a `GfxCtx`, allowing things to be drawn.
    pub fn gfx_ctx(&self) -> GfxCtx {
        self.prerender.inner.use_program_for_renderonly();
        let screenshot = false;
        GfxCtx::new(&self.prerender, &self.canvas, &self.style, screenshot)
    }
}
