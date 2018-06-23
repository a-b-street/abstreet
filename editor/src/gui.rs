use animation;
use ezgui::GfxCtx;
use ezgui::input::UserInput;
use piston::window::Size;
use std;

pub trait GUI {
    fn event(self, input: &mut UserInput, window_size: &Size) -> (Self, animation::EventLoopMode)
    where
        Self: std::marker::Sized;

    // TODO just take OSD stuff, not all of the input
    fn draw(&self, g: &mut GfxCtx, input: UserInput);
}
