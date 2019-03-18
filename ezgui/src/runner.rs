use crate::input::{ContextMenu, ModalMenuState};
use crate::{
    text, widgets, Canvas, Event, EventCtx, GfxCtx, ModalMenu, Prerender, TopMenu, UserInput,
};
use glium::glutin;
use glium_glyph::glyph_brush::rusttype::Font;
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::GlyphBrush;
use std::cell::Cell;
use std::time::{Duration, Instant};
use std::{env, panic, process, thread};

// 30fps is 1000 / 30
const SLEEP_BETWEEN_FRAMES: Duration = Duration::from_millis(33);

pub trait GUI<T> {
    // Called once
    fn top_menu(&self, _canvas: &Canvas) -> Option<TopMenu> {
        None
    }
    fn modal_menus() -> Vec<ModalMenu> {
        Vec::new()
    }
    fn event(&mut self, ctx: EventCtx) -> (EventLoopMode, T);
    // TODO Migrate all callers
    fn draw(&self, g: &mut GfxCtx, data: &T);
    // Return optional naming hint for screencap. TODO This API is getting gross.
    fn new_draw(&self, g: &mut GfxCtx, data: &T, _screencap: bool) -> Option<String> {
        self.draw(g, data);
        None
    }
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
}

pub(crate) struct State<T, G: GUI<T>> {
    pub(crate) gui: G,
    pub(crate) canvas: Canvas,
    context_menu: ContextMenu,
    top_menu: Option<TopMenu>,
    modal_state: ModalMenuState,
    pub(crate) last_data: Option<T>,
}

impl<T, G: GUI<T>> State<T, G> {
    // The bool indicates if the input was actually used.
    fn event(mut self, ev: Event, prerender: &Prerender) -> (State<T, G>, EventLoopMode, bool) {
        // It's impossible / very unlikey we'll grab the cursor in map space before the very first
        // start_drawing call.
        let mut input = UserInput::new(
            ev,
            self.context_menu,
            self.top_menu,
            self.modal_state,
            &mut self.canvas,
        );
        let mut gui = self.gui;
        let mut canvas = self.canvas;
        let (event_mode, data) = match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            gui.event(EventCtx {
                input: &mut input,
                canvas: &mut canvas,
                prerender,
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
        self.last_data = Some(data);
        // TODO We should always do has_been_consumed, but various hacks prevent this from being
        // true. For now, just avoid the specific annoying redraw case when a KeyRelease or Update
        // event is unused.
        let input_used = match ev {
            Event::KeyRelease(_) | Event::Update => input.has_been_consumed(),
            _ => true,
        };
        self.context_menu = input.context_menu.maybe_build(&self.canvas);
        self.top_menu = input.top_menu;
        self.modal_state = input.modal_state;
        if let Some(action) = input.chosen_action {
            panic!(
                "\"{}\" chosen from the top or modal menu, but nothing consumed it",
                action
            );
        }
        let mut still_active = Vec::new();
        for (mode, menu) in self.modal_state.active.into_iter() {
            if input.set_mode_called.contains(&mode) {
                still_active.push((mode, menu));
            }
        }
        self.modal_state.active = still_active;

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
        let mut g = GfxCtx::new(&self.canvas, &prerender, &mut target, program);
        let mut naming_hint: Option<String> = None;

        // If the very first event is render, then just wait.
        if let Some(ref data) = self.last_data {
            self.canvas.start_drawing();

            if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                naming_hint = self.gui.new_draw(&mut g, data, screenshot);
            })) {
                self.gui.dump_before_abort(&self.canvas);
                panic::resume_unwind(err);
            }

            if !screenshot {
                // Always draw the menus last.
                if let Some(ref menu) = self.top_menu {
                    menu.draw(&mut g);
                }
                for (_, ref menu) in &self.modal_state.active {
                    menu.draw(&mut g);
                }
                if let ContextMenu::Displaying(ref menu) = self.context_menu {
                    menu.draw(&mut g);
                }

                // Always draw text last
                self.canvas
                    .glyphs
                    .borrow_mut()
                    .draw_queued(display, &mut target);
            }
        }

        target.finish().unwrap();
        naming_hint
    }
}

pub fn run<T, G: GUI<T>, F: FnOnce(&mut Canvas, &Prerender) -> G>(
    window_title: &str,
    initial_width: f64,
    initial_height: f64,
    make_gui: F,
) {
    // DPI is broken on my system; force the old behavior.
    env::set_var("WINIT_HIDPI_FACTOR", "1.0");

    let events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title(window_title)
        .with_dimensions(glutin::dpi::LogicalSize::new(initial_width, initial_height));
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
    let fonts = vec![Font::from_bytes(dejavu).unwrap()];
    let vmetrics = fonts[0].v_metrics(Scale::uniform(text::FONT_SIZE));
    // TODO This works for this font, but could be more paranoid with abs()
    let line_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);
    let glyphs = GlyphBrush::new(&display, fonts);

    let mut canvas = Canvas::new(initial_width, initial_height, glyphs, line_height);
    let prerender = Prerender {
        display: &display,
        num_uploads: Cell::new(0),
        total_bytes_uploaded: Cell::new(0),
    };
    let gui = make_gui(&mut canvas, &prerender);

    let state = State {
        top_menu: gui.top_menu(&canvas),
        canvas,
        context_menu: ContextMenu::Inactive,
        modal_state: ModalMenuState::new(G::modal_menus()),
        last_data: None,
        gui,
    };

    loop_forever(state, events_loop, program, prerender);
}

fn loop_forever<T, G: GUI<T>>(
    mut state: State<T, G>,
    mut events_loop: glutin::EventsLoop,
    program: glium::Program,
    prerender: Prerender,
) {
    if state.gui.profiling_enabled() {
        #[cfg(unix)]
        {
            cpuprofiler::PROFILER
                .lock()
                .unwrap()
                .start("./profile")
                .unwrap();
        }
    }

    let mut wait_for_events = false;

    loop {
        let start_frame = Instant::now();

        let mut new_events: Vec<Event> = Vec::new();
        events_loop.poll_events(|event| {
            if let glutin::Event::WindowEvent { event, .. } = event {
                if event == glutin::WindowEvent::CloseRequested {
                    if state.gui.profiling_enabled() {
                        #[cfg(unix)]
                        {
                            cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
                        }
                    }
                    state.gui.before_quit(&state.canvas);
                    process::exit(0);
                }
                if let Some(ev) = Event::from_glutin_event(event) {
                    new_events.push(ev);
                }
            }
        });
        if !wait_for_events {
            new_events.push(Event::Update);
        }

        let mut any_input_used = false;

        for event in new_events {
            let (new_state, mode, input_used) = state.event(event, &prerender);
            if input_used {
                any_input_used = true;
            }
            state = new_state;
            wait_for_events = mode == EventLoopMode::InputOnly;
            if let EventLoopMode::ScreenCaptureEverything {
                dir,
                zoom,
                max_x,
                max_y,
            } = mode
            {
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
        }

        // Don't draw if an event was ignored. Every keypress also fires a release event, most of
        // which are ignored.
        if any_input_used {
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
