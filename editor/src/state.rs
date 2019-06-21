use crate::ui::UI;
use ezgui::{EventCtx, EventLoopMode, GfxCtx};

pub trait State {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode);
    fn draw(&self, g: &mut GfxCtx, ui: &UI);
    fn draw_default_ui(&self) -> bool {
        true
    }
}

pub enum Transition {
    Keep,
    Pop,
    Push(Box<State>),
    Replace(Box<State>),
}
