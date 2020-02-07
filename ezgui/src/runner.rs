use crate::assets::Assets;
use crate::{widgets, Canvas, Event, EventCtx, GfxCtx, Key, Prerender, UserInput};
use geom::Duration;
use glium::glutin;
use std::cell::Cell;
use std::time::Instant;
use std::{panic, process, thread};

// 30fps is 1000 / 30
const SLEEP_BETWEEN_FRAMES: std::time::Duration = std::time::Duration::from_millis(33);

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
        mut self,
        ev: Event,
        prerender: &Prerender,
        program: &glium::Program,
    ) -> (State<G>, EventLoopMode, bool) {
        // It's impossible / very unlikey we'll grab the cursor in map space before the very first
        // start_drawing call.
        let input = UserInput::new(ev, &self.canvas);
        let mut gui = self.gui;
        let mut canvas = self.canvas;

        // Update some ezgui state that's stashed in Canvas for sad reasons.
        {
            canvas.button_tooltip = None;

            if let Event::WindowResized(width, height) = input.event {
                canvas.window_width = width;
                canvas.window_height = height;
            }

            if input.event == Event::KeyPress(Key::LeftControl) {
                canvas.lctrl_held = true;
            }
            if input.event == Event::KeyRelease(Key::LeftControl) {
                canvas.lctrl_held = false;
            }

            if let Some(pt) = input.get_moved_mouse() {
                canvas.cursor_x = pt.x;
                canvas.cursor_y = pt.y;
            }

            if input.event == Event::WindowGainedCursor {
                canvas.window_has_cursor = true;
            }
            if input.window_lost_cursor() {
                canvas.window_has_cursor = false;
            }
        }

        let mut ctx = EventCtx {
            fake_mouseover: false,
            input: input,
            canvas: &mut canvas,
            prerender,
            program,
        };
        let event_mode = match panic::catch_unwind(panic::AssertUnwindSafe(|| gui.event(&mut ctx)))
        {
            Ok(pair) => pair,
            Err(err) => {
                gui.dump_before_abort(&canvas);
                panic::resume_unwind(err);
            }
        };
        // TODO We should always do has_been_consumed, but various hacks prevent this from being
        // true. For now, just avoid the specific annoying redraw case when a KeyRelease event is
        // unused.
        let input_used = match ev {
            Event::KeyRelease(_) => ctx.input.has_been_consumed(),
            _ => true,
        };
        self.gui = gui;
        self.canvas = canvas;

        (self, event_mode, input_used)
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

        target.finish().unwrap();
        naming_hint
    }
}

pub struct Settings {
    window_title: String,
    profiling_enabled: bool,
    default_font_size: usize,
    override_hidpi_factor: Option<f64>,
    dump_raw_events: bool,
}

impl Settings {
    pub fn new(window_title: &str) -> Settings {
        Settings {
            window_title: window_title.to_string(),
            profiling_enabled: false,
            default_font_size: 30,
            override_hidpi_factor: None,
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

    pub fn override_hidpi_factor(&mut self, override_hidpi_factor: f64) {
        self.override_hidpi_factor = Some(override_hidpi_factor);
    }
}

pub fn run<G: GUI, F: FnOnce(&mut EventCtx) -> G>(settings: Settings, make_gui: F) {
    let events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title(settings.window_title)
        .with_maximized(true);
    // multisampling: 2 looks bad, 4 looks fine
    //
    // The Z values are very simple:
    // 1.0: The buffer is reset every frame
    // 0.5: Map-space geometry and text
    // 0.1: Screen-space text
    // 0.0: Screen-space geometry
    // Had weird issues with Z buffering not working as intended, so this is slightly more
    // complicated than necessary to work.
    let context = glutin::ContextBuilder::new()
        .with_multisampling(4)
        .with_depth_buffer(2);
    let display = glium::Display::new(window, context, &events_loop).unwrap();

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

    let mut hidpi_factor = events_loop.get_primary_monitor().get_hidpi_factor();
    println!("HiDPI factor is purportedly {}", hidpi_factor);
    if let Some(x) = settings.override_hidpi_factor {
        println!("... but overriding it to {} by flag", x);
        hidpi_factor = x;
    }
    let window_size = events_loop.get_primary_monitor().get_dimensions();
    let mut canvas = Canvas::new(window_size.width, window_size.height, hidpi_factor);
    let prerender = Prerender {
        assets: Assets::new(settings.default_font_size),
        display: &display,
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

    let state = State { canvas, gui };

    loop_forever(
        state,
        events_loop,
        program,
        prerender,
        settings.profiling_enabled,
        settings.dump_raw_events,
    );
}

fn loop_forever<G: GUI>(
    mut state: State<G>,
    mut events_loop: glutin::EventsLoop,
    program: glium::Program,
    prerender: Prerender,
    profiling_enabled: bool,
    dump_raw_events: bool,
) {
    if profiling_enabled {
        #[cfg(feature = "profiler")]
        {
            cpuprofiler::PROFILER
                .lock()
                .unwrap()
                .start("./profile")
                .unwrap();
        }
    }

    let mut wait_for_events = false;
    let mut prev_frame = Instant::now();

    loop {
        let start_frame = Instant::now();

        let mut new_events: Vec<Event> = Vec::new();
        events_loop.poll_events(|event| {
            if let glutin::Event::WindowEvent { event, .. } = event {
                if event == glutin::WindowEvent::CloseRequested {
                    if profiling_enabled {
                        #[cfg(feature = "profiler")]
                        {
                            cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
                        }
                    }
                    state.gui.before_quit(&state.canvas);
                    process::exit(0);
                }
                if let glutin::WindowEvent::HiDpiFactorChanged(hidpi_factor) = event {
                    println!(
                        "HiDPI factor changed from {} to {}",
                        state.canvas.hidpi_factor, hidpi_factor
                    );
                    state.canvas.hidpi_factor = hidpi_factor;
                }
                if dump_raw_events {
                    println!("Event: {:?}", event);
                }
                if let Some(ev) = Event::from_glutin_event(event, state.canvas.hidpi_factor) {
                    new_events.push(ev);
                }
            }
        });
        if !wait_for_events {
            new_events.push(Event::Update(Duration::realtime_elapsed(prev_frame)));
            prev_frame = Instant::now();
        }

        let mut any_input_used = false;

        for event in new_events {
            let (new_state, mode, input_used) = state.event(event, &prerender, &program);
            if input_used {
                any_input_used = true;
            }
            state = new_state;
            // If we just unpaused, then don't act as if lots of time has passed.
            if wait_for_events && mode == EventLoopMode::Animation {
                prev_frame = Instant::now();
            }
            wait_for_events = mode == EventLoopMode::InputOnly;
            match mode {
                EventLoopMode::ScreenCaptureEverything {
                    dir,
                    zoom,
                    max_x,
                    max_y,
                } => {
                    state = widgets::screenshot_everything(
                        &dir,
                        state,
                        &prerender.display,
                        &program,
                        &prerender,
                        zoom,
                        max_x,
                        max_y,
                    );
                }
                EventLoopMode::ScreenCaptureCurrentShot => {
                    widgets::screenshot_current(
                        &mut state,
                        &prerender.display,
                        &program,
                        &prerender,
                    );
                }
                _ => {}
            };
        }

        // Don't draw if an event was ignored and we're not in Animation mode. Every keypress also
        // fires a release event, most of which are ignored.
        if any_input_used || !wait_for_events {
            state.draw(&prerender.display, &program, &prerender, false);
            prerender.num_uploads.set(0);
        }

        let this_frame = Instant::now().duration_since(start_frame);
        if SLEEP_BETWEEN_FRAMES > this_frame {
            thread::sleep(SLEEP_BETWEEN_FRAMES - this_frame);
        }
    }
}
