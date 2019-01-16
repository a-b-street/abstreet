use crate::input::{ContextMenu, ModalMenuState};
use crate::{Canvas, Event, GfxCtx, ModalMenu, TopMenu, UserInput};
use abstutil::Timer;
use glutin_window::GlutinWindow;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventLoop, EventSettings, Events};
use piston::window::WindowSettings;
use std::io::Write;
use std::{env, fs, panic, process};

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

pub fn run<T, G: GUI<T>>(mut gui: G, window_title: &str) {
    // DPI is broken on my system; force the old behavior.
    env::set_var("WINIT_HIDPI_FACTOR", "1.0");

    let opengl = OpenGL::V3_2;
    let settings = WindowSettings::new(
        window_title,
        [
            gui.get_mut_canvas().window_width as u32,
            gui.get_mut_canvas().window_height as u32,
        ],
    )
    .opengl(opengl)
    .exit_on_esc(false)
    // TODO it'd be cool to dynamically tweak antialiasing settings as we zoom in
    .samples(2)
    .srgb(false);
    let mut window: GlutinWindow = settings.build().expect("Could not create window");
    let mut events = Events::new(EventSettings::new().lazy(true));
    let mut gl = GlGraphics::new(opengl);

    let mut state = State {
        last_event_mode: EventLoopMode::InputOnly,
        context_menu: ContextMenu::Inactive,
        top_menu: gui.top_menu(),
        modal_state: ModalMenuState::new(G::modal_menus()),
        last_data: None,
        screen_cap: None,
        gui,
    };

    while let Some(ev) = events.next(&mut window) {
        use piston::input::{CloseEvent, RenderEvent};
        if let Some(args) = ev.render_args() {
            gl.draw(args.viewport(), |c, g| {
                state.draw(&mut GfxCtx::new(g, c));
            });
        } else if ev.close_args().is_some() {
            state.gui.before_quit();
            process::exit(0);
        } else {
            // Skip some events.
            use piston::input::{
                AfterRenderEvent, FocusEvent, IdleEvent, MouseRelativeEvent, TextEvent,
            };
            if ev.after_render_args().is_some() {
                state.after_render();
                continue;
            }
            if state.screen_cap.is_some() {
                continue;
            }
            if ev.after_render_args().is_some()
                || ev.focus_args().is_some()
                || ev.idle_args().is_some()
                || ev.mouse_relative_args().is_some()
                || ev.text_args().is_some()
            {
                continue;
            }

            state = state.event(ev, &mut events);
        }
    }
}

struct State<T, G: GUI<T>> {
    gui: G,
    last_event_mode: EventLoopMode,
    context_menu: ContextMenu,
    top_menu: Option<TopMenu>,
    modal_state: ModalMenuState,
    last_data: Option<T>,
    screen_cap: Option<ScreenCaptureState>,
}

impl<T, G: GUI<T>> State<T, G> {
    fn event(mut self, ev: piston::input::Event, events: &mut Events) -> State<T, G> {
        // It's impossible / very unlikey we'll grab the cursor in map space before the very first
        // start_drawing call.
        let mut input = UserInput::new(
            Event::from_piston_event(ev),
            self.context_menu,
            self.top_menu,
            self.modal_state,
            self.gui.get_mut_canvas(),
        );
        let mut gui = self.gui;
        let (new_event_mode, data) =
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
        if new_event_mode != self.last_event_mode {
            events.set_lazy(new_event_mode == EventLoopMode::InputOnly);
            self.last_event_mode = new_event_mode;

            if let EventLoopMode::ScreenCaptureEverything { zoom, max_x, max_y } = new_event_mode {
                self.screen_cap = Some(ScreenCaptureState::new(
                    self.gui.get_mut_canvas(),
                    zoom,
                    max_x,
                    max_y,
                ));
                events.set_lazy(false);
            }
        }

        self
    }

    fn draw(&mut self, g: &mut GfxCtx) {
        // If the very first event is render, then just wait.
        if let Some(ref data) = self.last_data {
            self.gui.get_mut_canvas().start_drawing(g);

            match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                self.gui.new_draw(g, data, self.screen_cap.is_some())
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
                    menu.draw(g, self.gui.get_mut_canvas());
                }
                for (_, ref menu) in &self.modal_state.active {
                    menu.draw(g, self.gui.get_mut_canvas());
                }
                if let ContextMenu::Displaying(ref menu) = self.context_menu {
                    menu.draw(g, self.gui.get_mut_canvas());
                }
            }
        }
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
        write!(file, "#!/bin/bash\n\n").unwrap();
        write!(file, "montage {}\n", args.join(" ")).unwrap();
        write!(file, "rm -f combine.sh\n").unwrap();
    }
}
