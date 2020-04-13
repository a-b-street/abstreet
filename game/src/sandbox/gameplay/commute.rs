use crate::app::App;
use crate::common::{ContextualActions, Tab};
use crate::cutscene::CutsceneBuilder;
use crate::edit::EditMode;
use crate::game::{State, Transition};
use crate::helpers::cmp_duration_shorter;
use crate::helpers::ID;
use crate::sandbox::gameplay::{challenge_header, FinalScore, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use ezgui::{
    Btn, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Text, TextExt,
    VerticalAlignment, Widget,
};
use geom::{Duration, Time};
use sim::{PersonID, TripID};
use std::collections::BTreeMap;
use std::fmt::Write;

// TODO A nice level to unlock: specifying your own commute, getting to work on it

const GOAL: Duration = Duration::const_seconds(3.0 * 60.0);

pub struct OptimizeCommute {
    top_center: Composite,
    person: PersonID,
    time: Time,

    // Cache here for convenience
    trips: Vec<TripID>,

    once: bool,
}

impl OptimizeCommute {
    pub fn new(ctx: &mut EventCtx, app: &App, person: PersonID) -> Box<dyn GameplayState> {
        let trips = app.primary.sim.get_person(person).trips.clone();
        Box::new(OptimizeCommute {
            top_center: make_top_center(ctx, app, &trips),
            person,
            time: Time::START_OF_DAY,
            trips,
            once: true,
        })
    }

    pub fn cutscene(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        CutsceneBuilder::new()
            .scene("boss", "Listen up, I've got a special job for you today.")
            .scene(
                "player",
                "What is it? The scooter coalition back with demands for more valet parking?",
            )
            .scene(
                "boss",
                "No, all the tax-funded valets are still busy the kayakers.",
            )
            .scene(
                "boss",
                "I've got a... friend who's tired of getting stuck in traffic on Broadway. You've \
                 got to make their commute as fast as possible.",
            )
            .scene(
                "player",
                "Ah, it's about time we finally put in those new bike lanes along Broadway! I'll \
                 get right on --",
            )
            .scene("boss", "No! Just smooth things out for this one person.")
            .scene("player", "Uh, what's so special about them?")
            .scene(
                "boss",
                "That's none of your concern! I've anonymized their name, so don't even bother \
                 digging into what happened at dinn --",
            )
            .scene("boss", "JUST GET TO WORK, KID!")
            .narrator(
                "Somebody's blackmailing the boss. Guess it's time to help this VIP (very \
                 impatient person).",
            )
            .narrator(
                "The drone has been programmed to find the anonymous VIP. Watch their daily \
                 route, figure out what's wrong, and fix it.",
            )
            .narrator(
                "Ignore the damage done to everyone else. Just speed up the VIP's trips by a \
                 total of 3 minutes.",
            )
            .build(ctx, app)
    }
}

impl GameplayState for OptimizeCommute {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        if self.once {
            self.once = false;
            controls.common.as_mut().unwrap().launch_info_panel(
                ctx,
                app,
                Tab::PersonTrips(self.person, BTreeMap::new()),
                &mut Actions {
                    paused: controls.speed.as_ref().unwrap().is_paused(),
                },
            );
        }

        if self.time != app.primary.sim.time() {
            self.top_center = make_top_center(ctx, app, &self.trips);
            self.time = app.primary.sim.time();
        }

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "edit map" => {
                    return (
                        Some(Transition::Push(Box::new(EditMode::new(
                            ctx,
                            app,
                            GameplayMode::OptimizeCommute(self.person),
                        )))),
                        false,
                    );
                }
                "instructions" => {
                    return (
                        Some(Transition::Push(OptimizeCommute::cutscene(ctx, app))),
                        false,
                    );
                }
                "locate VIP" => {
                    controls.common.as_mut().unwrap().launch_info_panel(
                        ctx,
                        app,
                        Tab::PersonTrips(self.person, BTreeMap::new()),
                        &mut Actions {
                            paused: controls.speed.as_ref().unwrap().is_paused(),
                        },
                    );
                }
                _ => unreachable!(),
            },
            None => {}
        }

        // TODO After all of the person's trips are done, we can actually end then
        if app.primary.sim.is_done() {
            let (verdict, _success) = final_score(app, &self.trips);
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

fn make_top_center(ctx: &mut EventCtx, app: &App, trips: &Vec<TripID>) -> Composite {
    let mut done = 0;
    let mut before_time = Duration::ZERO;
    let mut after_time = Duration::ZERO;
    for t in trips {
        if let Some((total, _)) = app.primary.sim.finished_trip_time(*t) {
            done += 1;
            after_time += total;
            before_time += app.prebaked().finished_trip_time(*t).unwrap();
        }
    }

    let mut txt = Text::from(Line(format!("Total trip time: {} (", after_time)));
    txt.append_all(cmp_duration_shorter(after_time, before_time));
    txt.append(Line(")"));

    Composite::new(
        Widget::col(vec![
            challenge_header(ctx, "Optimize the VIP's commute"),
            Widget::row(vec![
                Btn::svg_def("../data/system/assets/tools/location.svg")
                    .build(ctx, "locate VIP", None)
                    .margin_right(10),
                format!("{}/{} trips done", done, trips.len())
                    .draw_text(ctx)
                    .margin_right(20),
                txt.draw(ctx).margin_right(20),
                format!("Goal: {} faster", GOAL).draw_text(ctx),
            ]),
        ])
        .bg(app.cs.panel_bg),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

// True if the challenge is completed
fn final_score(app: &App, trips: &Vec<TripID>) -> (String, bool) {
    let mut done = 0;
    let mut before_time = Duration::ZERO;
    let mut after_time = Duration::ZERO;
    for t in trips {
        if let Some((total, _)) = app.primary.sim.finished_trip_time(*t) {
            done += 1;
            after_time += total;
            before_time += app.prebaked().finished_trip_time(*t).unwrap();
        }
    }

    // TODO Needs work
    let mut verdict = format!(
        "Originally, total commute time was {}. Now it's {}.",
        before_time, after_time
    );
    write!(
        &mut verdict,
        " The goal is {} faster. You've done {}.",
        GOAL,
        before_time - after_time
    )
    .unwrap();
    if done != trips.len() {
        write!(&mut verdict, " Not all trips are done yet. Wait longer.").unwrap();
    }

    (
        verdict,
        done == trips.len() && before_time - after_time >= GOAL,
    )
}

// TODO Probably refactor this for most challenge modes, or have SandboxMode pass in Actions
struct Actions {
    paused: bool,
}

impl ContextualActions for Actions {
    fn actions(&self, _: &App, _: ID) -> Vec<(Key, String)> {
        Vec::new()
    }
    fn execute(
        &mut self,
        _: &mut EventCtx,
        _: &mut App,
        _: ID,
        _: String,
        _: &mut bool,
    ) -> Transition {
        unreachable!()
    }
    fn is_paused(&self) -> bool {
        self.paused
    }
}
