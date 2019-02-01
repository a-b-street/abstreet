use crate::input::{ContextMenu, ModalMenuState};
use crate::{
    screenshot, text, Canvas, Event, EventCtx, GfxCtx, ModalMenu, Prerender, TopMenu, UserInput,
};
use glium::glutin;
use glium_glyph::glyph_brush::rusttype::Font;
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::GlyphBrush;
use std::time::{Duration, Instant};
use std::{env, panic, process, thread};

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
    fn event(mut self, ev: Event, display: &glium::Display) -> (State<T, G>, EventLoopMode) {
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
                prerender: &Prerender { display },
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

    // Returns naming hint.
    pub(crate) fn draw(
        &mut self,
        display: &glium::Display,
        program: &glium::Program,
        screenshot: bool,
    ) -> Option<String> {
        let mut target = display.draw();
        let mut g = GfxCtx::new(&self.canvas, &display, &mut target, program);
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

    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title(window_title)
        .with_dimensions(glutin::dpi::LogicalSize::new(initial_width, initial_height));
    // 2 looks bad, 4 looks fine
    let context = glutin::ContextBuilder::new().with_multisampling(4);
    let display = glium::Display::new(window, context, &events_loop).unwrap();
    let program = glium::Program::new(
        &display,
        glium::program::ProgramCreationInput::SourceCode {
            vertex_shader: include_str!("vertex.glsl"),
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            geometry_shader: None,
            fragment_shader: include_str!("fragment.glsl"),
            transform_feedback_varyings: None,
            // Without this, SRGB gets enabled and post-processes the color from the fragment
            // shader.
            outputs_srgb: true,
            uses_point_size: false,
        },
    )
    .unwrap();

    let dejavu: &[u8] = include_bytes!("DejaVuSans.ttf");
    let fonts = vec![Font::from_bytes(dejavu).unwrap()];
    let vmetrics = fonts[0].v_metrics(Scale::uniform(text::FONT_SIZE));
    // TODO This works for this font, but could be more paranoid with abs()
    let line_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);
    let glyphs = GlyphBrush::new(&display, fonts);

    let mut canvas = Canvas::new(initial_width, initial_height, glyphs, line_height);
    let gui = make_gui(&mut canvas, &Prerender { display: &display });

    let mut state = State {
        top_menu: gui.top_menu(&canvas),
        canvas,
        context_menu: ContextMenu::Inactive,
        modal_state: ModalMenuState::new(G::modal_menus()),
        last_data: None,
        gui,
    };

    let mut accumulator = Duration::new(0, 0);
    let mut previous_clock = Instant::now();
    let mut wait_for_events = false;
    loop {
        let mut new_events: Vec<glutin::WindowEvent> = Vec::new();
        events_loop.poll_events(|event| {
            if let glutin::Event::WindowEvent { event, .. } = event {
                new_events.push(event);
            }
        });
        let any_new_events = !new_events.is_empty();
        for event in new_events {
            if event == glutin::WindowEvent::CloseRequested {
                state.gui.before_quit(&state.canvas);
                process::exit(0);
            }
            if let Some(ev) = Event::from_glutin_event(event) {
                let (new_state, mode) = state.event(ev, &display);
                state = new_state;
                wait_for_events = mode == EventLoopMode::InputOnly;
                if let EventLoopMode::ScreenCaptureEverything { zoom, max_x, max_y } = mode {
                    state = screenshot::screenshot_everything(
                        state, &display, &program, zoom, max_x, max_y,
                    );
                }
            }
        }

        if any_new_events || !wait_for_events {
            state.draw(&display, &program, false);
        }

        if !wait_for_events {
            let (new_state, mode) = state.event(Event::Update, &display);
            state = new_state;
            wait_for_events = mode == EventLoopMode::InputOnly;
        }

        // TODO This isn't right at all... sleep only if nothing happened.
        if !any_new_events && wait_for_events {
            let now = Instant::now();
            accumulator += now - previous_clock;
            previous_clock = now;
            let fixed_time_stamp = Duration::new(0, 16_666_667);
            while accumulator >= fixed_time_stamp {
                accumulator -= fixed_time_stamp;
            }
            thread::sleep(fixed_time_stamp - accumulator);
        }
    }
}
