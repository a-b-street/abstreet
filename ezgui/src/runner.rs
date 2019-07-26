use crate::input::ContextMenu;
use crate::{widgets, Canvas, Event, EventCtx, GfxCtx, Prerender, UserInput};
use glium::glutin;
use glium_glyph::glyph_brush::rusttype::Font;
use glium_glyph::GlyphBrush;
use std::cell::Cell;
use std::time::{Duration, Instant};
use std::{panic, process, thread};

// 30fps is 1000 / 30
const SLEEP_BETWEEN_FRAMES: Duration = Duration::from_millis(33);

pub trait GUI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode;
    fn draw(&self, g: &mut GfxCtx);
    // Will be called if event or draw panics.
    fn dump_before_abort(&self, _canvas: &Canvas) {}
    // Only before a normal exit, like window close
    fn before_quit(&self, _canvas: &Canvas) {}

    fn profiling_enabled(&self) -> bool {
        false
    }
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
    context_menu: ContextMenu,
}

impl<G: GUI> State<G> {
    // The bool indicates if the input was actually used.
    fn event(
        mut self,
        ev: Event,
        prerender: &Prerender,
        program: &glium::Program,
    ) -> (State<G>, EventLoopMode, bool) {
        // Clear out the possible keys
        if let ContextMenu::Inactive(_) = self.context_menu {
            self.context_menu = ContextMenu::new();
        }

        // It's impossible / very unlikey we'll grab the cursor in map space before the very first
        // start_drawing call.
        let mut input = UserInput::new(ev, self.context_menu, &mut self.canvas);
        let mut gui = self.gui;
        let mut canvas = self.canvas;
        let event_mode = match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            gui.event(&mut EventCtx {
                input: &mut input,
                canvas: &mut canvas,
                prerender,
                program,
            })
        })) {
            Ok(pair) => pair,
            Err(err) => {
                gui.dump_before_abort(&canvas);
                panic::resume_unwind(err);
            }
        };
        self.gui = gui;
        self.canvas = canvas;
        // TODO We should always do has_been_consumed, but various hacks prevent this from being
        // true. For now, just avoid the specific annoying redraw case when a KeyRelease or Update
        // event is unused.
        let input_used = match ev {
            Event::KeyRelease(_) | Event::Update => input.has_been_consumed(),
            _ => true,
        };
        self.context_menu = input.context_menu.maybe_build(&self.canvas);

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
        let mut g = GfxCtx::new(
            &self.canvas,
            &prerender,
            display,
            &mut target,
            program,
            &self.context_menu,
            screenshot,
        );

        self.canvas.start_drawing();

        if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            self.gui.draw(&mut g);
        })) {
            self.gui.dump_before_abort(&self.canvas);
            panic::resume_unwind(err);
        }
        let naming_hint = g.naming_hint.take();

        // Always draw the menus last.
        if let ContextMenu::Displaying(ref menu) = self.context_menu {
            menu.draw(&mut g);
        }

        // Flush text just once, so that GlyphBrush's internal caching works. We have to assume
        // nothing will ever cover up text.
        self.canvas
            .glyphs
            .borrow_mut()
            .draw_queued(display, &mut target);

        target.finish().unwrap();
        naming_hint
    }
}

pub fn run<G: GUI, F: FnOnce(&mut EventCtx) -> G>(
    window_title: &str,
    initial_width: f64,
    initial_height: f64,
    make_gui: F,
) {
    let events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title(window_title)
        .with_dimensions(glutin::dpi::LogicalSize::from_physical(
            glutin::dpi::PhysicalSize::new(initial_width, initial_height),
            events_loop.get_primary_monitor().get_hidpi_factor(),
        ));
    // 2 looks bad, 4 looks fine
    let context = glutin::ContextBuilder::new().with_multisampling(4);
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    let (vertex_shader, fragment_shader) =
        if display.is_glsl_version_supported(&glium::Version(glium::Api::Gl, 1, 4)) {
            (
                include_str!("assets/vertex_140.glsl"),
                include_str!("assets/fragment_140.glsl"),
            )
        } else if display.is_glsl_version_supported(&glium::Version(glium::Api::Gl, 1, 1)) {
            (
                include_str!("assets/vertex_110.glsl"),
                include_str!("assets/fragment_110.glsl"),
            )
        } else {
            panic!(
                "GLSL 140 and 110 not supported. Try {:?} or {:?}",
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

    let dejavu: &[u8] = include_bytes!("assets/DejaVuSans.ttf");
    let glyphs = GlyphBrush::new(&display, vec![Font::from_bytes(dejavu).unwrap()]);

    let mut canvas = Canvas::new(initial_width, initial_height, glyphs);
    let prerender = Prerender {
        display: &display,
        num_uploads: Cell::new(0),
        total_bytes_uploaded: Cell::new(0),
    };

    let gui = make_gui(&mut EventCtx {
        input: &mut UserInput::new(Event::NoOp, ContextMenu::new(), &mut canvas),
        canvas: &mut canvas,
        prerender: &prerender,
        program: &program,
    });

    let state = State {
        canvas,
        context_menu: ContextMenu::new(),
        gui,
    };

    loop_forever(state, events_loop, program, prerender);
}

fn loop_forever<G: GUI>(
    mut state: State<G>,
    mut events_loop: glutin::EventsLoop,
    program: glium::Program,
    prerender: Prerender,
) {
    if state.gui.profiling_enabled() {
        #[cfg(target_os = "linux")]
        {
            cpuprofiler::PROFILER
                .lock()
                .unwrap()
                .start("./profile")
                .unwrap();
        }
    }

    let mut wait_for_events = false;

    let hidpi_factor = events_loop.get_primary_monitor().get_hidpi_factor();
    loop {
        let start_frame = Instant::now();

        let mut new_events: Vec<Event> = Vec::new();
        events_loop.poll_events(|event| {
            if let glutin::Event::WindowEvent { event, .. } = event {
                if event == glutin::WindowEvent::CloseRequested {
                    if state.gui.profiling_enabled() {
                        #[cfg(target_os = "linux")]
                        {
                            cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
                        }
                    }
                    state.gui.before_quit(&state.canvas);
                    process::exit(0);
                }
                if let Some(ev) = Event::from_glutin_event(event, hidpi_factor) {
                    new_events.push(ev);
                }
            }
        });
        if !wait_for_events {
            new_events.push(Event::Update);
        }

        let mut any_input_used = false;

        for event in new_events {
            let (new_state, mode, input_used) = state.event(event, &prerender, &program);
            if input_used {
                any_input_used = true;
            }
            state = new_state;
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
            if any_input_used {
                // But if the event caused a state-change, the drawing state might be different
                // too. Need to recalculate what menu entries and such are valid. So send through
                // a no-op event.
                let (new_state, _, _) = state.event(Event::NoOp, &prerender, &program);
                state = new_state;
            }

            state.draw(&prerender.display, &program, &prerender, false);
            prerender.num_uploads.set(0);
        }

        // Primitive event loop.
        // TODO Read http://gameprogrammingpatterns.com/game-loop.html carefully.
        let this_frame = Instant::now().duration_since(start_frame);
        if SLEEP_BETWEEN_FRAMES > this_frame {
            thread::sleep(SLEEP_BETWEEN_FRAMES - this_frame);
        }
    }
}
