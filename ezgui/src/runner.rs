use crate::input::{ContextMenu, ModalMenuState};
use crate::{Canvas, Event, GfxCtx, ModalMenu, TopMenu, UserInput};
use glutin_window::GlutinWindow;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventLoop, EventSettings, Events};
use piston::window::WindowSettings;
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
    fn draw(&self, g: &mut GfxCtx, data: &T);
    fn draw_screengrab(&self, g: &mut GfxCtx, data: &T) {
        self.draw(g, data);
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

    // TODO Probably time to bundle this state up. :)
    let mut last_event_mode = EventLoopMode::InputOnly;
    let mut context_menu = ContextMenu::Inactive;
    let mut top_menu = gui.top_menu();
    let mut modal_state = ModalMenuState::new(G::modal_menus());
    let mut last_data: Option<T> = None;
    let mut screen_cap: Option<ScreenCaptureState> = None;

    while let Some(ev) = events.next(&mut window) {
        use piston::input::{CloseEvent, RenderEvent};
        if let Some(args) = ev.render_args() {
            // If the very first event is render, then just wait.
            if let Some(ref data) = last_data {
                gl.draw(args.viewport(), |c, g| {
                    let mut g = GfxCtx::new(g, c);
                    gui.get_mut_canvas().start_drawing(&mut g);

                    if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                        if screen_cap.is_some() {
                            gui.draw_screengrab(&mut g, data);
                        } else {
                            gui.draw(&mut g, data);
                        }
                    })) {
                        gui.dump_before_abort();
                        panic::resume_unwind(err);
                    }

                    if screen_cap.is_none() {
                        // Always draw the menus last.
                        if let Some(ref menu) = top_menu {
                            menu.draw(&mut g, gui.get_mut_canvas());
                        }
                        for (_, ref menu) in &modal_state.active {
                            menu.draw(&mut g, gui.get_mut_canvas());
                        }
                        if let ContextMenu::Displaying(ref menu) = context_menu {
                            menu.draw(&mut g, gui.get_mut_canvas());
                        }
                    }
                });
            }
        } else if ev.close_args().is_some() {
            gui.before_quit();
            process::exit(0);
        } else {
            // Skip some events.
            use piston::input::{
                AfterRenderEvent, FocusEvent, IdleEvent, MouseRelativeEvent, TextEvent,
            };
            if ev.after_render_args().is_some() {
                // Do this after we draw and flush to the screen.
                // TODO The very first time we grab is wrong. But waiting for one round of draw
                // also didn't seem to work...
                if let Some(ref mut cap) = screen_cap {
                    let filename = format!("screen{:02}x{:02}.png", cap.tile_x, cap.tile_y);
                    println!(
                        "Grabbing {} (of {}, {} total)",
                        filename, cap.num_tiles_x, cap.num_tiles_y
                    );
                    if !process::Command::new("scrot")
                        .args(&["--quality", "100", "--focused", "--silent", &filename])
                        .status()
                        .unwrap()
                        .success()
                    {
                        println!("scrot failed; aborting");
                        screen_cap = None;
                        continue;
                    }

                    let canvas = gui.get_mut_canvas();
                    cap.tile_x += 1;
                    canvas.cam_x += canvas.window_width;
                    if (canvas.cam_x + canvas.window_width) / canvas.cam_zoom >= cap.max_x {
                        cap.tile_x = 1;
                        canvas.cam_x = 0.0;
                        cap.tile_y += 1;
                        canvas.cam_y += canvas.window_height;
                        if (canvas.cam_y + canvas.window_height) / canvas.cam_zoom >= cap.max_y {
                            cap.combine();
                            let canvas = gui.get_mut_canvas();
                            canvas.cam_zoom = cap.orig_zoom;
                            canvas.cam_x = cap.orig_x;
                            canvas.cam_y = cap.orig_y;
                            screen_cap = None;
                        }
                    }
                }
                continue;
            }
            if screen_cap.is_some() {
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

            // It's impossible / very unlikey we'll grab the cursor in map space before the very first
            // start_drawing call.
            let mut input = UserInput::new(
                Event::from_piston_event(ev),
                context_menu,
                top_menu,
                modal_state,
                gui.get_mut_canvas(),
            );
            let (new_event_mode, data) =
                match panic::catch_unwind(panic::AssertUnwindSafe(|| gui.event(&mut input))) {
                    Ok(pair) => pair,
                    Err(err) => {
                        gui.dump_before_abort();
                        panic::resume_unwind(err);
                    }
                };
            last_data = Some(data);
            context_menu = input.context_menu.maybe_build(gui.get_mut_canvas());
            top_menu = input.top_menu;
            modal_state = input.modal_state;
            if let Some(action) = input.chosen_action {
                panic!(
                    "\"{}\" chosen from the top or modal menu, but nothing consumed it",
                    action
                );
            }
            let mut still_active = Vec::new();
            for (mode, menu) in modal_state.active.into_iter() {
                if input.set_mode_called.contains(&mode) {
                    still_active.push((mode, menu));
                }
            }
            modal_state.active = still_active;

            // Don't constantly reset the events struct -- only when laziness changes.
            if new_event_mode != last_event_mode {
                events.set_lazy(new_event_mode == EventLoopMode::InputOnly);
                last_event_mode = new_event_mode;

                if let EventLoopMode::ScreenCaptureEverything { zoom, max_x, max_y } =
                    new_event_mode
                {
                    println!("Starting to capturing screenshots");
                    let canvas = gui.get_mut_canvas();
                    screen_cap = Some(ScreenCaptureState {
                        tile_x: 1,
                        tile_y: 1,
                        num_tiles_x: (max_x * zoom / canvas.window_width).floor() as usize,
                        num_tiles_y: (max_y * zoom / canvas.window_height).floor() as usize,
                        max_x,
                        max_y,
                        orig_zoom: canvas.cam_zoom,
                        orig_x: canvas.cam_x,
                        orig_y: canvas.cam_y,
                    });
                    canvas.cam_x = 0.0;
                    canvas.cam_y = 0.0;
                    canvas.cam_zoom = zoom;
                    events.set_lazy(false);
                }
            }
        }
    }
}

struct ScreenCaptureState {
    tile_x: usize,
    tile_y: usize,

    num_tiles_x: usize,
    num_tiles_y: usize,
    max_x: f64,
    max_y: f64,
    orig_zoom: f64,
    orig_x: f64,
    orig_y: f64,
}

impl ScreenCaptureState {
    fn combine(&self) {
        println!("Combining {} tiles...", self.num_tiles_x * self.num_tiles_y);
        let mut args = Vec::new();
        for y in 1..=self.num_tiles_y {
            for x in 1..=self.num_tiles_x {
                args.push(format!("screen{:02}x{:02}.png", x, y));
            }
        }
        args.push("-mode".to_string());
        args.push("Concatenate".to_string());
        args.push("-tile".to_string());
        args.push(format!("{}x{}", self.num_tiles_x, self.num_tiles_y));
        args.push("screencap.png".to_string());
        assert!(process::Command::new("montage")
            .args(&args)
            .status()
            .unwrap()
            .success());

        for x in 1..=self.num_tiles_x {
            for y in 1..=self.num_tiles_y {
                fs::remove_file(format!("screen{:02}x{:02}.png", x, y)).unwrap();
            }
        }

        println!("Produced screencap.png!");
    }
}
