use crate::app::App;
use crate::common::Tab;
use crate::game::Transition;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_controller, FinalScore, GameplayMode, GameplayState};
use crate::sandbox::{SandboxControls, SandboxMode};
use ezgui::{Btn, EventCtx, GfxCtx};
use geom::{Duration, Time};
use sim::PersonID;
use std::collections::BTreeSet;

const GOAL: Duration = Duration::const_seconds(3.0 * 60.0);

pub struct OptimizeCommute {
    top_center: WrappedComposite,
    person: PersonID,
    time: Time,
}

impl OptimizeCommute {
    pub fn new(ctx: &mut EventCtx, app: &App, person: PersonID) -> Box<dyn GameplayState> {
        Box::new(OptimizeCommute {
            top_center: make_top_center(ctx, app, person),
            person,
            time: Time::START_OF_DAY,
        })
    }
}

impl GameplayState for OptimizeCommute {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        if self.time != app.primary.sim.time() {
            self.top_center = make_top_center(ctx, app, self.person);
            self.time = app.primary.sim.time();
        }

        match self.top_center.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => {
                return (Some(t), false);
            }
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "locate person" => {
                    let person = self.person;
                    return (
                        Some(Transition::KeepWithData(Box::new(
                            move |state, app, ctx| {
                                let mode = state.downcast_mut::<SandboxMode>().unwrap();
                                let mut actions = mode.contextual_actions();
                                mode.controls.common.as_mut().unwrap().launch_info_panel(
                                    ctx,
                                    app,
                                    Tab::PersonTrips(person, BTreeSet::new()),
                                    &mut actions,
                                );
                            },
                        ))),
                        false,
                    );
                }
                _ => unreachable!(),
            },
            None => {}
        }

        // TODO After all of the person's trips are done, we can actually end then
        if app.primary.sim.is_done() {
            let (verdict, success) = final_score(app);
            // TODO Plumb through a next stage here
            let next = None;
            return (
                Some(Transition::Push(FinalScore::new(
                    ctx,
                    app,
                    verdict,
                    GameplayMode::OptimizeCommute(self.person),
                    next,
                ))),
                false,
            );
        }

        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

fn make_top_center(ctx: &mut EventCtx, app: &App, person: PersonID) -> WrappedComposite {
    let mut rows = vec![
        Btn::svg_def("../data/system/assets/tools/location.svg").build(ctx, "locate person", None),
    ];

    challenge_controller(
        ctx,
        app,
        GameplayMode::OptimizeCommute(person),
        &format!("Optimize {}'s commute", person),
        rows,
    )
}

// True if the challenge is completed
fn final_score(app: &App) -> (String, bool) {
    (format!("TODO"), false)
}
