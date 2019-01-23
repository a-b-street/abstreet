use crate::input::{ContextMenu, ModalMenuState};
use crate::{Canvas, Event, GfxCtx, ModalMenu, TopMenu, UserInput};
use abstutil::Timer;
use glium::glutin;
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
    fn event(&mut self, input: &mut UserInput) -> (EventLoopMode, T);
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
    screen_cap: Option<ScreenCaptureState>,
}

impl<T, G: GUI<T>> State<T, G> {
    fn event(mut self, ev: Event) -> (State<T, G>, EventLoopMode) {
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
        let (event_mode, data) =
            match panic::catch_unwind(panic::AssertUnwindSafe(|| gui.event(&mut input))) {
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

        // Don't constantly reset the events struct -- only when laziness changes.
        if let EventLoopMode::ScreenCaptureEverything { zoom, max_x, max_y } = event_mode {
            self.screen_cap = Some(ScreenCaptureState::new(
                self.gui.get_mut_canvas(),
                zoom,
                max_x,
                max_y,
            ));
        }

        (self, event_mode)
    }

    fn draw(&mut self, display: &glium::Display, program: &glium::Program) {
        let mut target = display.draw();
        let mut g = GfxCtx::new(self.gui.get_mut_canvas(), &display, &mut target, program);

        // If the very first event is render, then just wait.
        if let Some(ref data) = self.last_data {
            self.gui.get_mut_canvas().start_drawing();

            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                self.gui.new_draw(&mut g, data, self.screen_cap.is_some())
            })) {
                Ok(naming_hint) => {
                    if let Some(ref mut cap) = self.screen_cap {
                        cap.naming_hint = naming_hint;
                    }
                }
                Err(err) => {
                    self.gui.dump_before_abort();
                    panic::resume_unwind(err);
                }
            }

            if self.screen_cap.is_none() {
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
            }
        }

        target.finish().unwrap();
    }

    fn after_render(&mut self) {
        // Do this after we draw and flush to the screen.
        // TODO The very first time we grab is wrong. But waiting for one round of draw also didn't
        // seem to work...
        if let Some(ref mut cap) = self.screen_cap {
            cap.timer.next();
            let suffix = cap.naming_hint.take().unwrap_or_else(String::new);
            let filename = format!("{:02}x{:02}{}.png", cap.tile_x, cap.tile_y, suffix);
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
                self.screen_cap = None;
                return;
            }
            cap.filenames.push(filename);

            let canvas = self.gui.get_mut_canvas();
            cap.tile_x += 1;
            canvas.cam_x += canvas.window_width;
            if (canvas.cam_x + canvas.window_width) / canvas.cam_zoom >= cap.max_x {
                cap.tile_x = 1;
                canvas.cam_x = 0.0;
                cap.tile_y += 1;
                canvas.cam_y += canvas.window_height;
                if (canvas.cam_y + canvas.window_height) / canvas.cam_zoom >= cap.max_y {
                    let canvas = self.gui.get_mut_canvas();
                    canvas.cam_zoom = cap.orig_zoom;
                    canvas.cam_x = cap.orig_x;
                    canvas.cam_y = cap.orig_y;
                    self.screen_cap.take().unwrap().combine();
                }
            }
        }
    }
}

struct ScreenCaptureState {
    tile_x: usize,
    tile_y: usize,
    timer: Timer,
    naming_hint: Option<String>,
    filenames: Vec<String>,

    num_tiles_x: usize,
    num_tiles_y: usize,
    max_x: f64,
    max_y: f64,
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
            tile_x: 1,
            tile_y: 1,
            timer,
            naming_hint: None,
            filenames: Vec::new(),
            num_tiles_x,
            num_tiles_y,
            max_x,
            max_y,
            orig_zoom: canvas.cam_zoom,
            orig_x: canvas.cam_x,
            orig_y: canvas.cam_y,
        };
        canvas.cam_x = 0.0;
        canvas.cam_y = 0.0;
        canvas.cam_zoom = zoom;
        state
    }

    fn combine(self) {
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

pub fn run<T, G: GUI<T>>(mut gui: G, window_title: &str) {
    // DPI is broken on my system; force the old behavior.
    env::set_var("WINIT_HIDPI_FACTOR", "1.0");

    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title(window_title)
        .with_dimensions(glutin::dpi::LogicalSize::new(
            gui.get_mut_canvas().window_width,
            gui.get_mut_canvas().window_height,
        ));
    let context = glutin::ContextBuilder::new().with_depth_buffer(24);
    let display = glium::Display::new(window, context, &events_loop).unwrap();
    let program = glium::Program::from_source(
        &display,
        include_str!("vertex.glsl"),
        include_str!("fragment.glsl"),
        None,
    )
    .unwrap();

    let mut state = State {
        context_menu: ContextMenu::Inactive,
        top_menu: gui.top_menu(),
        modal_state: ModalMenuState::new(G::modal_menus()),
        last_data: None,
        screen_cap: None,
        gui,
    };

    let mut accumulator = Duration::new(0, 0);
    let mut previous_clock = Instant::now();
    let mut lazy_events = true;
    let mut redraw = false;
    loop {
        if redraw {
            state.draw(&display, &program);
            state.after_render();
            redraw = false;
        }

        let mut new_events: Vec<glutin::WindowEvent> = Vec::new();
        events_loop.poll_events(|event| {
            if let glutin::Event::WindowEvent { event, .. } = event {
                new_events.push(event);
            }
        });
        for event in new_events {
            if event == glutin::WindowEvent::CloseRequested {
                state.gui.before_quit();
                process::exit(0);
            }
            if state.screen_cap.is_none() {
                if let Some(ev) = Event::from_glutin_event(event) {
                    let (new_state, mode) = state.event(ev);
                    state = new_state;
                    lazy_events = mode == EventLoopMode::InputOnly;
                    redraw = true;
                }
            }
        }

        let now = Instant::now();
        accumulator += now - previous_clock;
        previous_clock = now;

        let fixed_time_stamp = Duration::new(0, 16_666_667);
        while accumulator >= fixed_time_stamp {
            accumulator -= fixed_time_stamp;
            // TODO send off an update event
        }

        thread::sleep(fixed_time_stamp - accumulator);
        if !redraw && !lazy_events {
            redraw = true;
        }
    }
}
