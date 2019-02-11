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
}

#[derive(Clone, Copy, PartialEq)]
pub enum EventLoopMode {
    Animation,
    InputOnly,
    ScreenCaptureEverything { zoom: f64, max_x: f64, max_y: f64 },
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
    fn event(mut self, ev: Event, prerender: &Prerender) -> (State<T, G>, EventLoopMode) {
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

        (self, event_mode)
    }

    // Returns naming hint. Logically consumes the number of uploads.
    pub(crate) fn draw(
        &mut self,
        display: &glium::Display,
        program: &glium::Program,
        screenshot: bool,
        uploads_so_far: usize,
        bytes_uploaded_so_far: usize,
    ) -> Option<String> {
        let mut target = display.draw();
        let prerender = Prerender {
            display,
            num_uploads: Cell::new(uploads_so_far),
            total_bytes_uploaded: Cell::new(bytes_uploaded_so_far),
        };
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
    let program = glium::Program::new(
        &display,
        glium::program::ProgramCreationInput::SourceCode {
            vertex_shader: include_str!("assets/vertex.glsl"),
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            geometry_shader: None,
            fragment_shader: include_str!("assets/fragment.glsl"),
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

    let num_uploads = prerender.num_uploads.get();
    let total_bytes_uploaded = prerender.total_bytes_uploaded.get();
    loop_forever(
        state,
        events_loop,
        display,
        program,
        num_uploads,
        total_bytes_uploaded,
    );
}

fn loop_forever<T, G: GUI<T>>(
    mut state: State<T, G>,
    mut events_loop: glutin::EventsLoop,
    display: glium::Display,
    program: glium::Program,
    mut uploads_so_far: usize,
    mut bytes_uploaded_so_far: usize,
) {
    let mut wait_for_events = false;
    loop {
        let start_frame = Instant::now();

        let mut new_events: Vec<Event> = Vec::new();
        events_loop.poll_events(|event| {
            if let glutin::Event::WindowEvent { event, .. } = event {
                if event == glutin::WindowEvent::CloseRequested {
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

        let any_new_events = !new_events.is_empty();

        for event in new_events {
            let prerender = Prerender {
                display: &display,
                num_uploads: Cell::new(uploads_so_far),
                total_bytes_uploaded: Cell::new(bytes_uploaded_so_far),
            };
            let (new_state, mode) = state.event(event, &prerender);
            state = new_state;
            wait_for_events = mode == EventLoopMode::InputOnly;
            uploads_so_far = prerender.num_uploads.get();
            bytes_uploaded_so_far = prerender.total_bytes_uploaded.get();
            if let EventLoopMode::ScreenCaptureEverything { zoom, max_x, max_y } = mode {
                state =
                    widgets::screenshot_everything(state, &display, &program, zoom, max_x, max_y);
            }
        }

        // TODO Every time we press and release a single key, we draw twice. Ideally we'd batch
        // those events before drawing or somehow know that the release event was ignored and we
        // don't need to redraw.
        if any_new_events {
            state.draw(
                &display,
                &program,
                false,
                uploads_so_far,
                bytes_uploaded_so_far,
            );
            uploads_so_far = 0;
        }

        // Primitive event loop.
        // TODO Read http://gameprogrammingpatterns.com/game-loop.html carefully.
        let this_frame = Instant::now().duration_since(start_frame);
        if SLEEP_BETWEEN_FRAMES > this_frame {
            thread::sleep(SLEEP_BETWEEN_FRAMES - this_frame);
        }
    }
}
