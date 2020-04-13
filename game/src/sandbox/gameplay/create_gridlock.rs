use crate::app::App;
use crate::game::Transition;
use crate::helpers::cmp_count_fewer;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_controller, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use abstutil::prettyprint_usize;
use ezgui::{EventCtx, GfxCtx, Line, Text};
use sim::TripMode;

pub struct CreateGridlock {
    top_center: WrappedComposite,
}

impl CreateGridlock {
    pub fn new(ctx: &mut EventCtx, app: &App, mode: GameplayMode) -> Box<dyn GameplayState> {
        Box::new(CreateGridlock {
            top_center: challenge_controller(ctx, app, mode, "Gridlock Challenge", Vec::new()),
        })
    }
}

impl GameplayState for CreateGridlock {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        match self.top_center.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => {
                return (Some(t), false);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {}
        }

        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

// TODO Revive this data in some form
#[allow(unused)]
fn gridlock_panel(app: &App) -> Text {
    let (after_all, _, after_per_mode) = app
        .primary
        .sim
        .get_analytics()
        .trip_times(app.primary.sim.time());
    let (before_all, _, before_per_mode) = app.prebaked().trip_times(app.primary.sim.time());

    let mut txt = Text::new();
    txt.add_appended(vec![
        Line(format!(
            "{} total trips (",
            prettyprint_usize(after_all.count())
        )),
        cmp_count_fewer(after_all.count(), before_all.count()),
        Line(")"),
    ]);

    for mode in TripMode::all() {
        let a = after_per_mode[&mode].count();
        let b = before_per_mode[&mode].count();
        txt.add_appended(vec![
            Line(format!(
                "  {}: {} (",
                mode.ongoing_verb(),
                prettyprint_usize(a)
            )),
            cmp_count_fewer(a, b),
            Line(")"),
        ]);
    }

    txt
}
