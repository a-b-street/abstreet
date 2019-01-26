use crate::input::{ContextMenu, ModalMenuState};
use crate::{text, Canvas, Event, GfxCtx, ModalMenu, Prerender, TopMenu, UserInput};
use abstutil::Timer;
use glium::glutin;
use glium_glyph::glyph_brush::rusttype::Font;
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::GlyphBrush;
use std::io::Write;
use std::time::{Duration, Instant};
use std::{env, fs, panic, process, thread};

pub trait GUI<T> {
    // Called once
    fn top_menu(&self) -> Option<TopMenu> {
        None
    }
    fn modal_menus() -> Vec<ModalMenu> {
        Vec::new()
    }
    fn event(&mut self, input: &mut UserInput, prerender: &Prerender) -> (EventLoopMode, T);
    fn get_mut_canvas(&mut self) -> &mut Canvas;
    // TODO Migrate all callers
    fn draw(&self, g: &mut GfxCtx, data: &T);
    // Return optional naming hint for screencap. TODO This API is getting gross.
    fn new_draw(&self, g: &mut GfxCtx, data: &T, _screencap: bool) -> Option<String> {
        self.draw(g, data);
        None
    }
    // Will be called if event or draw panics.
    fn dump_before_abort(&self) {}
    // Only before a normal exit, like window close
    fn before_quit(&self) {}
}

#[derive(Clone, Copy, PartialEq)]
pub enum EventLoopMode {
    Animation,
    InputOnly,
    ScreenCaptureEverything { zoom: f64, max_x: f64, max_y: f64 },
}

struct State<T, G: GUI<T>> {
    gui: G,
    context_menu: ContextMenu,
    top_menu: Option<TopMenu>,
    modal_state: ModalMenuState,
    last_data: Option<T>,
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
            self.gui.get_mut_canvas(),
        );
        let mut gui = self.gui;
        let (event_mode, data) = match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            gui.event(&mut input, &Prerender { display })
        })) {
            Ok(pair) => pair,
            Err(err) => {
                gui.dump_before_abort();
                panic::resume_unwind(err);
            }
        };
        self.gui = gui;
        self.last_data = Some(data);
        self.context_menu = input.context_menu.maybe_build(self.gui.get_mut_canvas());
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

    fn draw(&mut self, display: &glium::Display, program: &glium::Program) {
        let mut target = display.draw();
        let mut g = GfxCtx::new(self.gui.get_mut_canvas(), &display, &mut target, program);

        // If the very first event is render, then just wait.
        if let Some(ref data) = self.last_data {
            self.gui.get_mut_canvas().start_drawing();

            if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                self.gui.new_draw(&mut g, data, false)
            })) {
                self.gui.dump_before_abort();
                panic::resume_unwind(err);
            }

            // Always draw the menus last.
            if let Some(ref menu) = self.top_menu {
                menu.draw(&mut g, self.gui.get_mut_canvas());
            }
            for (_, ref menu) in &self.modal_state.active {
                menu.draw(&mut g, self.gui.get_mut_canvas());
            }
            if let ContextMenu::Displaying(ref menu) = self.context_menu {
                menu.draw(&mut g, self.gui.get_mut_canvas());
            }

            // Always draw text last
            self.gui
                .get_mut_canvas()
                .glyphs
                .borrow_mut()
                .draw_queued(display, &mut target);
        }

        target.finish().unwrap();
    }
}

pub fn run<T, G: GUI<T>, F: FnOnce(Canvas, &Prerender) -> G>(
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

    // TODO Maybe we should own the Canvas too. Why make them store it? Or even know about it? Let
    // them borrow stuff during event() and during draw().
    let canvas = Canvas::new(initial_width, initial_height, glyphs, line_height);
    let gui = make_gui(canvas, &Prerender { display: &display });

    let mut state = State {
        context_menu: ContextMenu::Inactive,
        top_menu: gui.top_menu(),
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
                state.gui.before_quit();
                process::exit(0);
            }
            if let Some(ev) = Event::from_glutin_event(event) {
                let (new_state, mode) = state.event(ev, &display);
                state = new_state;
                wait_for_events = mode == EventLoopMode::InputOnly;
                if let EventLoopMode::ScreenCaptureEverything { zoom, max_x, max_y } = mode {
                    ScreenCaptureState::new(state.gui.get_mut_canvas(), zoom, max_x, max_y)
                        .run(&mut state, &display, &program);
                }
            }
        }

        if any_new_events || !wait_for_events {
            state.draw(&display, &program);
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

struct ScreenCaptureState {
    timer: Timer,
    filenames: Vec<String>,

    num_tiles_x: usize,
    num_tiles_y: usize,
    orig_zoom: f64,
    orig_x: f64,
    orig_y: f64,
}

impl ScreenCaptureState {
    fn new(canvas: &mut Canvas, zoom: f64, max_x: f64, max_y: f64) -> ScreenCaptureState {
        let num_tiles_x = (max_x * zoom / canvas.window_width).floor() as usize;
        let num_tiles_y = (max_y * zoom / canvas.window_height).floor() as usize;
        let mut timer = Timer::new("capturing screen");
        timer.start_iter("capturing images", num_tiles_x * num_tiles_y);
        fs::create_dir("screencap").unwrap();
        let state = ScreenCaptureState {
            timer,
            filenames: Vec::new(),
            num_tiles_x,
            num_tiles_y,
            orig_zoom: canvas.cam_zoom,
            orig_x: canvas.cam_x,
            orig_y: canvas.cam_y,
        };
        canvas.cam_zoom = zoom;
        state
    }

    fn run<T, G: GUI<T>>(
        mut self,
        state: &mut State<T, G>,
        display: &glium::Display,
        program: &glium::Program,
    ) {
        let last_data = state.last_data.as_ref().unwrap();

        for tile_y in 0..self.num_tiles_y {
            for tile_x in 0..self.num_tiles_x {
                self.timer.next();
                let canvas = state.gui.get_mut_canvas();
                canvas.cam_x = (tile_x as f64) * canvas.window_width;
                canvas.cam_y = (tile_y as f64) * canvas.window_height;

                let mut target = display.draw();
                let mut g = GfxCtx::new(canvas, &display, &mut target, program);

                let naming_hint = match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    state.gui.new_draw(&mut g, last_data, true)
                })) {
                    Ok(naming_hint) => naming_hint,
                    Err(err) => {
                        state.gui.dump_before_abort();
                        panic::resume_unwind(err);
                    }
                };
                target.finish().unwrap();

                if !self.screencap(tile_x, tile_y, naming_hint) {
                    return;
                }
            }
        }

        let canvas = state.gui.get_mut_canvas();
        canvas.cam_zoom = self.orig_zoom;
        canvas.cam_x = self.orig_x;
        canvas.cam_y = self.orig_y;
        self.finish();
    }

    fn screencap(&mut self, tile_x: usize, tile_y: usize, mut naming_hint: Option<String>) -> bool {
        let suffix = naming_hint.take().unwrap_or_else(String::new);
        let filename = format!("{:02}x{:02}{}.png", tile_x + 1, tile_y + 1, suffix);
        if !process::Command::new("scrot")
            .args(&[
                "--quality",
                "100",
                "--focused",
                "--silent",
                &format!("screencap/{}", filename),
            ])
            .status()
            .unwrap()
            .success()
        {
            println!("scrot failed; aborting");
            return false;
        }
        self.filenames.push(filename);
        true
    }

    fn finish(self) {
        let mut args = self.filenames;
        args.push("-mode".to_string());
        args.push("Concatenate".to_string());
        args.push("-tile".to_string());
        args.push(format!("{}x{}", self.num_tiles_x, self.num_tiles_y));
        args.push("full.png".to_string());

        let mut file = fs::File::create("screencap/combine.sh").unwrap();
        writeln!(file, "#!/bin/bash\n").unwrap();
        writeln!(file, "montage {}", args.join(" ")).unwrap();
        writeln!(file, "rm -f combine.sh").unwrap();
    }
}
