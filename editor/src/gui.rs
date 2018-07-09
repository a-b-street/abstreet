use ezgui::input::UserInput;
use ezgui::GfxCtx;
use piston::window::Size;

pub trait GUI {
    fn event(&mut self, input: &mut UserInput) -> EventLoopMode;

    // TODO just take OSD stuff, not all of the input
    fn draw(&mut self, g: &mut GfxCtx, input: UserInput, window_size: Size);
}

#[derive(PartialEq)]
pub enum EventLoopMode {
    Animation,
    InputOnly,
}
