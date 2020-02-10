use crate::assets::Assets;
use crate::{widgets, Canvas, Event, EventCtx, GfxCtx, Key, Prerender, UserInput};
use geom::Duration;
use std::cell::Cell;
use std::panic;
use std::time::Instant;

const UPDATE_FREQUENCY: std::time::Duration = std::time::Duration::from_millis(1000 / 30);

pub trait GUI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode;
    fn draw(&self, g: &mut GfxCtx);
    // Will be called if event or draw panics.
    fn dump_before_abort(&self, _canvas: &Canvas) {}
    // Only before a normal exit, like window close
    fn before_quit(&self, _canvas: &Canvas) {}
}

#[derive(Clone, PartialEq)]
pub enum EventLoopMode {
    Animation,
    InputOnly,
    ScreenCaptureEverything {
        dir: String,
        zoom: f64,
        max_x: f64,
        max_y: f64,
    },
    ScreenCaptureCurrentShot,
}

pub(crate) struct State<G: GUI> {
    pub(crate) gui: G,
    pub(crate) canvas: Canvas,
}

impl<G: GUI> State<G> {
    // The bool indicates if the input was actually used.
    fn event(
        &mut self,
        ev: Event,
        prerender: &Prerender,
        program: &glium::Program,
    ) -> (EventLoopMode, bool) {
        // It's impossible / very unlikey we'll grab the cursor in map space before the very first
        // start_drawing call.
        let input = UserInput::new(ev, &self.canvas);

        // Update some ezgui state that's stashed in Canvas for sad reasons.
        {
            self.canvas.button_tooltip = None;

            if let Event::WindowResized(width, height) = input.event {
                self.canvas.window_width = width;
                self.canvas.window_height = height;
            }

            if input.event == Event::KeyPress(Key::LeftControl) {
                self.canvas.lctrl_held = true;
            }
            if input.event == Event::KeyRelease(Key::LeftControl) {
                self.canvas.lctrl_held = false;
            }

            if let Some(pt) = input.get_moved_mouse() {
                self.canvas.cursor_x = pt.x;
                self.canvas.cursor_y = pt.y;
            }

            if input.event == Event::WindowGainedCursor {
                self.canvas.window_has_cursor = true;
            }
            if input.window_lost_cursor() {
                self.canvas.window_has_cursor = false;
            }
        }

        match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let mut ctx = EventCtx {
                fake_mouseover: false,
                input: input,
                canvas: &mut self.canvas,
                prerender,
                program,
            };
            let evloop = self.gui.event(&mut ctx);
            // TODO We should always do has_been_consumed, but various hacks prevent this from being
            // true. For now, just avoid the specific annoying redraw case when a KeyRelease event
            // is unused.
            let input_used = match ev {
                Event::KeyRelease(_) => ctx.input.has_been_consumed(),
                _ => true,
            };
            (evloop, input_used)
        })) {
            Ok(pair) => pair,
            Err(err) => {
                self.gui.dump_before_abort(&self.canvas);
                panic::resume_unwind(err);
            }
        }
    }

    // Returns naming hint. Logically consumes the number of uploads.
    pub(crate) fn draw(
        &mut self,
        display: &glium::Display,
        program: &glium::Program,
        prerender: &Prerender,
        screenshot: bool,
    ) -> Option<String> {
        let mut target = display.draw();
        let mut g = GfxCtx::new(&self.canvas, &prerender, &mut target, program, screenshot);

        self.canvas.start_drawing();

        if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            self.gui.draw(&mut g);
        })) {
            self.gui.dump_before_abort(&self.canvas);
            panic::resume_unwind(err);
        }
        let naming_hint = g.naming_hint.take();

        if false {
            println!(
                "----- {} uploads, {} draw calls, {} forks -----",
                g.get_num_uploads(),
                g.num_draw_calls,
                g.num_forks
            );
        }

        target.finish().unwrap();
        naming_hint
    }
}

pub struct Settings {
    window_title: String,
    font_dir: String,
    profiling_enabled: bool,
    default_font_size: usize,
    dump_raw_events: bool,
}

impl Settings {
    pub fn new(window_title: &str, font_dir: &str) -> Settings {
        Settings {
            window_title: window_title.to_string(),
            font_dir: font_dir.to_string(),
            profiling_enabled: false,
            default_font_size: 30,
            dump_raw_events: false,
        }
    }

    pub fn enable_profiling(&mut self) {
        assert!(!self.profiling_enabled);
        self.profiling_enabled = true;
    }

    pub fn dump_raw_events(&mut self) {
        assert!(!self.dump_raw_events);
        self.dump_raw_events = true;
    }

    pub fn default_font_size(&mut self, size: usize) {
        self.default_font_size = size;
    }
}

pub fn run<G: 'static + GUI, F: FnOnce(&mut EventCtx) -> G>(settings: Settings, make_gui: F) -> ! {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title(settings.window_title)
        .with_maximized(true);
    // multisampling: 2 looks bad, 4 looks fine
    let context = glutin::ContextBuilder::new()
        .with_multisampling(4)
        .with_depth_buffer(2);
    // TODO This step got slow
    println!("Initializing OpenGL window");
    let display = glium::Display::new(window, context, &event_loop).unwrap();

    let (vertex_shader, fragment_shader) =
        if display.is_glsl_version_supported(&glium::Version(glium::Api::Gl, 1, 4)) {
            (
                include_str!("assets/vertex_140.glsl"),
                include_str!("assets/fragment_140.glsl"),
            )
        } else {
            panic!(
                "GLSL 140 not supported. Try {:?} or {:?}",
                display.get_opengl_version(),
                display.get_supported_glsl_version()
            );
        };

    // To quickly iterate on shaders without recompiling...
    /*let mut vert = String::new();
    let mut frag = String::new();
    let (vertex_shader, fragment_shader) = {
        use std::io::Read;

        let mut f1 = std::fs::File:: open("../ezgui/src/assets/vertex_140.glsl").unwrap();
        f1.read_to_string(&mut vert).unwrap();

        let mut f2 = std::fs::File:: open("../ezgui/src/assets/fragment_140.glsl").unwrap();
        f2.read_to_string(&mut frag).unwrap();

        (&vert, &frag)
    };*/

    let program = glium::Program::new(
        &display,
        glium::program::ProgramCreationInput::SourceCode {
            vertex_shader,
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            geometry_shader: None,
            fragment_shader,
            transform_feedback_varyings: None,
            // Without this, SRGB gets enabled and post-processes the color from the fragment
            // shader.
            outputs_srgb: true,
            uses_point_size: false,
        },
    )
    .unwrap();

    let window_size = event_loop.primary_monitor().size();
    let mut canvas = Canvas::new(window_size.width.into(), window_size.height.into());
    let prerender = Prerender {
        assets: Assets::new(settings.default_font_size, settings.font_dir),
        display,
        num_uploads: Cell::new(0),
        total_bytes_uploaded: Cell::new(0),
    };

    let gui = make_gui(&mut EventCtx {
        fake_mouseover: true,
        input: UserInput::new(Event::NoOp, &canvas),
        canvas: &mut canvas,
        prerender: &prerender,
        program: &program,
    });

    let mut state = State { canvas, gui };

    if settings.profiling_enabled {
        #[cfg(feature = "profiler")]
        {
            cpuprofiler::PROFILER
                .lock()
                .unwrap()
                .start("./profile")
                .unwrap();
        }
    }

    let profiling_enabled = settings.profiling_enabled;
    let dump_raw_events = settings.dump_raw_events;

    let mut running = true;
    let mut last_update = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        if dump_raw_events {
            println!("Event: {:?}", event);
        }
        let ev = match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                // ControlFlow::Exit cleanly shuts things down, meaning on larger maps, lots of
                // GPU stuff is dropped. Better to just abort violently and let the OS clean
                // up.
                if profiling_enabled {
                    #[cfg(feature = "profiler")]
                    {
                        cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
                    }
                }
                state.gui.before_quit(&state.canvas);
                std::process::exit(0);
            }
            winit::event::Event::WindowEvent { event, .. } => {
                if let Some(ev) = Event::from_winit_event(event) {
                    ev
                } else {
                    // Don't touch control_flow if we got an irrelevant event
                    return;
                }
            }
            winit::event::Event::RedrawRequested(_) => {
                state.draw(&prerender.display, &program, &prerender, false);
                prerender.num_uploads.set(0);
                return;
            }
            winit::event::Event::MainEventsCleared => {
                // We might've switched to InputOnly after the WaitUntil was requested.
                if running {
                    Event::Update(Duration::realtime_elapsed(last_update))
                } else {
                    return;
                }
            }
            _ => {
                return;
            }
        };

        // We want a max of UPDATE_FREQUENCY between updates, so measure the update time before
        // doing the work (which takes time).
        if let Event::Update(_) = ev {
            last_update = Instant::now();
            *control_flow =
                winit::event_loop::ControlFlow::WaitUntil(Instant::now() + UPDATE_FREQUENCY);
        }

        let (mode, input_used) = state.event(ev, &prerender, &program);
        if input_used {
            prerender.display.gl_window().window().request_redraw();
        }

        match mode {
            EventLoopMode::InputOnly => {
                running = false;
                *control_flow = winit::event_loop::ControlFlow::Wait;
            }
            EventLoopMode::Animation => {
                // If we just unpaused, then don't act as if lots of time has passed.
                if !running {
                    last_update = Instant::now();
                    *control_flow = winit::event_loop::ControlFlow::WaitUntil(
                        Instant::now() + UPDATE_FREQUENCY,
                    );
                }

                running = true;
            }
            EventLoopMode::ScreenCaptureEverything {
                dir,
                zoom,
                max_x,
                max_y,
            } => {
                widgets::screenshot_everything(
                    &mut state,
                    &dir,
                    &prerender.display,
                    &program,
                    &prerender,
                    zoom,
                    max_x,
                    max_y,
                );
            }
            EventLoopMode::ScreenCaptureCurrentShot => {
                widgets::screenshot_current(&mut state, &prerender.display, &program, &prerender);
            }
        }
    });
}
