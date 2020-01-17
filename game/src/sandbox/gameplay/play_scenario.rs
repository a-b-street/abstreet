use crate::game::Transition;
use crate::managed::Composite;
use crate::sandbox::gameplay::freeform::freeform_controller;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx};

pub struct PlayScenario;

impl PlayScenario {
    pub fn new(name: &String, ctx: &mut EventCtx, ui: &UI) -> (Composite, Box<dyn GameplayState>) {
        (
            freeform_controller(ctx, ui, GameplayMode::PlayScenario(name.to_string()), name),
            Box::new(PlayScenario),
        )
    }
}

impl GameplayState for PlayScenario {
    fn event(&mut self, _: &mut EventCtx, _: &mut UI, _: &mut Overlays) -> Option<Transition> {
        None
    }

    fn draw(&self, _: &mut GfxCtx, _: &UI) {}
}
