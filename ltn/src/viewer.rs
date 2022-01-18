use widgetry::{EventCtx, GfxCtx, State, Transition};

use crate::App;

// TODO Placeholder
pub struct Viewer;

impl Viewer {
    pub fn new_state(_: &mut EventCtx, _: &App) -> Box<dyn State<App>> {
        Box::new(Viewer)
    }
}

impl State<App> for Viewer {
    fn event(&mut self, _: &mut EventCtx, _: &mut App) -> Transition<App> {
        Transition::Keep
    }

    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}
