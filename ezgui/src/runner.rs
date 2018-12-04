use glutin_window::GlutinWindow;
use input::UserInput;
use opengl_graphics::{Filter, GlGraphics, GlyphCache, OpenGL, TextureSettings};
use piston::event_loop::{EventLoop, EventSettings, Events};
use piston::input::RenderEvent;
use piston::window::{Window, WindowSettings};
use {Canvas, GfxCtx};

pub trait GUI<T> {
    fn event(&mut self, input: UserInput) -> (EventLoopMode, T);
    fn get_mut_canvas(&mut self) -> &mut Canvas;
    fn draw(&self, g: &mut GfxCtx, data: T);
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

    let texture_settings = TextureSettings::new().filter(Filter::Nearest);
    let mut glyphs = GlyphCache::new(
        // TODO don't assume this exists!
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        (),
        texture_settings,
    ).expect("Could not load font");

    let mut last_event_mode = EventLoopMode::InputOnly;
    while let Some(ev) = events.next(&mut window) {
        let (new_event_mode, data) = gui.event(UserInput::new(ev.clone()));
        // Don't constantly reset the events struct -- only when laziness changes.
        if new_event_mode != last_event_mode {
            events.set_lazy(new_event_mode == EventLoopMode::InputOnly);
            last_event_mode = new_event_mode;
        }

        if let Some(args) = ev.render_args() {
            gl.draw(args.viewport(), |c, g| {
                let mut g = GfxCtx::new(&mut glyphs, g, c);
                gui.get_mut_canvas()
                    .start_drawing(&mut g, window.draw_size());
                gui.draw(&mut g, data);
            });
        }
    }
}
