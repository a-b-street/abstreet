use crate::game::Transition;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::freeform::freeform_controller;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx};

pub struct PlayScenario {
    top_center: WrappedComposite,
}

impl PlayScenario {
    pub fn new(name: &String, ctx: &mut EventCtx, ui: &UI) -> Box<dyn GameplayState> {
        Box::new(PlayScenario {
            top_center: freeform_controller(
                ctx,
                ui,
                GameplayMode::PlayScenario(name.to_string()),
                name,
            ),
        })
    }
}

impl GameplayState for PlayScenario {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        _: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        match self.top_center.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => {
                return (Some(t), false);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {}
        }

        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.top_center.draw(g);
    }
}
