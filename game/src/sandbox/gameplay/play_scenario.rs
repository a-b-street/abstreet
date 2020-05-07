use crate::app::App;
use crate::game::Transition;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::freeform::freeform_controller;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use ezgui::{EventCtx, GfxCtx};

pub struct PlayScenario {
    top_center: WrappedComposite,
}

impl PlayScenario {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        name: &String,
        mode: GameplayMode,
    ) -> Box<dyn GameplayState> {
        Box::new(PlayScenario {
            top_center: freeform_controller(ctx, app, mode, name),
        })
    }
}

impl GameplayState for PlayScenario {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> Option<Transition> {
        match self.top_center.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => {
                return Some(t);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}
