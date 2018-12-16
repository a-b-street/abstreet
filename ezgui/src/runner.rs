use crate::input::ContextMenu;
use crate::{Canvas, Event, GfxCtx, UserInput};
use glutin_window::GlutinWindow;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventLoop, EventSettings, Events};
use piston::window::{Window, WindowSettings};
use std::panic;

pub trait GUI<T> {
    fn event(&mut self, input: &mut UserInput) -> (EventLoopMode, T);
    fn get_mut_canvas(&mut self) -> &mut Canvas;
    fn draw(&self, g: &mut GfxCtx, data: &T);
    // Will be called if event or draw panics.
    fn dump_before_abort(&self) {}
}

#[derive(Clone, Copy, PartialEq)]
pub enum EventLoopMode {
    Animation,
    InputOnly,
}

pub fn run<T, G: GUI<T>>(mut gui: G, window_title: &str, initial_width: u32, initial_height: u32) {
    let opengl = OpenGL::V3_2;
    let settings = WindowSettings::new(window_title, [initial_width, initial_height])
        .opengl(opengl)
        .exit_on_esc(false)
        // TODO it'd be cool to dynamically tweak antialiasing settings as we zoom in
        .samples(2)
        .srgb(false);
    let mut window: GlutinWindow = settings.build().expect("Could not create window");
    let mut events = Events::new(EventSettings::new().lazy(true));
    let mut gl = GlGraphics::new(opengl);

    let mut last_event_mode = EventLoopMode::InputOnly;
    let mut context_menu = ContextMenu::Inactive;
    let mut last_data: Option<T> = None;
    while let Some(ev) = events.next(&mut window) {
        use piston::input::RenderEvent;
        if let Some(args) = ev.render_args() {
            // If the very first event is render, then just wait.
            if let Some(ref data) = last_data {
                gl.draw(args.viewport(), |c, g| {
                    let mut g = GfxCtx::new(g, c);
                    gui.get_mut_canvas()
                        .start_drawing(&mut g, window.draw_size());

                    if let Err(err) =
                        panic::catch_unwind(panic::AssertUnwindSafe(|| gui.draw(&mut g, data)))
                    {
                        gui.dump_before_abort();
                        panic::resume_unwind(err);
                    }

                    // Always draw the context-menu last.
                    if let ContextMenu::Displaying(ref menu) = context_menu {
                        menu.draw(&mut g, gui.get_mut_canvas());
                    }
                });
            }
        } else {
            // Skip some events.
            use piston::input::{
                AfterRenderEvent, CursorEvent, FocusEvent, MouseRelativeEvent, ResizeEvent,
                TextEvent,
            };
            if ev.resize_args().is_some()
                || ev.focus_args().is_some()
                || ev.cursor_args().is_some()
                || ev.mouse_relative_args().is_some()
                || ev.after_render_args().is_some()
                || ev.text_args().is_some()
            {
                continue;
            }

            // It's impossible / very unlikey we'll grab the cursor in map space before the very first
            // start_drawing call.
            let mut input = UserInput::new(
                Event::from_piston_event(ev),
                context_menu,
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

            // Don't constantly reset the events struct -- only when laziness changes.
            if new_event_mode != last_event_mode {
                events.set_lazy(new_event_mode == EventLoopMode::InputOnly);
                last_event_mode = new_event_mode;
            }
        }
    }
}
