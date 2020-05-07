use crate::app::App;
use crate::challenges::{Challenge, HighScore};
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
use sim::{OrigPersonID, PersonID, TripID};
use std::collections::BTreeMap;

// TODO A nice level to unlock: specifying your own commute, getting to work on it

pub struct OptimizeCommute {
    top_center: Composite,
    person: PersonID,
    mode: GameplayMode,
    goal: Duration,
    time: Time,

    // Cache here for convenience
    trips: Vec<TripID>,

    once: bool,
}

impl OptimizeCommute {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        orig_person: OrigPersonID,
        goal: Duration,
    ) -> Box<dyn GameplayState> {
        let person = app.primary.sim.find_person_by_orig_id(orig_person).unwrap();
        let trips = app.primary.sim.get_person(person).trips.clone();
        Box::new(OptimizeCommute {
            top_center: make_top_center(
                ctx,
                app,
                Duration::ZERO,
                Duration::ZERO,
                0,
                trips.len(),
                goal,
            ),
            person,
            mode: GameplayMode::OptimizeCommute(orig_person, goal),
            goal,
            time: Time::START_OF_DAY,
            trips,
            once: true,
        })
    }

    pub fn cutscene_pt1(ctx: &mut EventCtx, app: &App, mode: &GameplayMode) -> Box<dyn State> {
        let goal = match mode {
            GameplayMode::OptimizeCommute(_, d) => *d,
            _ => unreachable!(),
        };
        CutsceneBuilder::new()
            .boss("Listen up, I've got a special job for you today.")
            .player("What is it? The scooter coalition back with demands for more valet parking?")
            .boss("No, all the tax-funded valets are still busy the kayakers.")
            .boss(
                "I've got a... friend who's tired of getting stuck in traffic on Broadway. You've \
                 got to make their commute as fast as possible.",
            )
            .player(
                "Ah, it's about time we finally put in those new bike lanes along Broadway! I'll \
                 get right on --",
            )
            .boss("No! Just smooth things out for this one person.")
            .player("Uh, what's so special about them?")
            .boss(
                "That's none of your concern! I've anonymized their name, so don't even bother \
                 digging into what happened in Ballard --",
            )
            .boss("JUST GET TO WORK, KID!")
            .narrator(
                "Somebody's blackmailing the boss. Guess it's time to help this VIP (very \
                 impatient person).",
            )
            .narrator(
                "The drone has been programmed to find the anonymous VIP. Watch their daily \
                 route, figure out what's wrong, and fix it.",
            )
            .narrator(format!(
                "Ignore the damage done to everyone else. Just speed up the VIP's trips by a \
                 total of {}.",
                goal
            ))
            .build(ctx, app)
    }

    pub fn cutscene_pt2(ctx: &mut EventCtx, app: &App, mode: &GameplayMode) -> Box<dyn State> {
        let goal = match mode {
            GameplayMode::OptimizeCommute(_, d) => *d,
            _ => unreachable!(),
        };
        // TODO The person chosen for this currently has more of an issue needing PBLs, actually.
        CutsceneBuilder::new()
            .boss("I've got another, er, friend who's sick of this parking situation.")
            .player(
                "Yeah, why do we dedicate so much valuable land to storing unused cars? It's \
                 ridiculous!",
            )
            .boss(
                "No, I mean, they're tired of having to hunt for parking. You need to make it \
                 easier.",
            )
            .player(
                "What? We're trying to encourage people to be less car-dependent. Why's this \
                 \"friend\" more important than the city's carbon-neutral goals?",
            )
            .boss("Everyone's calling in favors these days. Just make it happen!")
            .narrator("Too many people have dirt on the boss. Guess we have another VIP to help.")
            .narrator(format!(
                "Once again, ignore the damage to everyone else, and just speed up the VIP's \
                 trips by a total of {}.",
                goal
            ))
            .build(ctx, app)
    }
}

impl GameplayState for OptimizeCommute {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
    ) -> Option<Transition> {
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
            self.time = app.primary.sim.time();

            let (before, after, done) = get_score(app, &self.trips);
            self.top_center =
                make_top_center(ctx, app, before, after, done, self.trips.len(), self.goal);

            if done == self.trips.len() {
                return Some(Transition::Push(final_score(
                    ctx,
                    app,
                    self.mode.clone(),
                    before,
                    after,
                    self.goal,
                )));
            }
        }

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "edit map" => {
                    return Some(Transition::Push(Box::new(EditMode::new(
                        ctx,
                        app,
                        self.mode.clone(),
                    ))));
                }
                "instructions" => {
                    return Some(Transition::Push((Challenge::find(&self.mode)
                        .0
                        .cutscene
                        .unwrap())(
                        ctx, app, &self.mode
                    )));
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

        None
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

// Returns (before, after, number of trips done)
fn get_score(app: &App, trips: &Vec<TripID>) -> (Duration, Duration, usize) {
    let mut done = 0;
    let mut before = Duration::ZERO;
    let mut after = Duration::ZERO;
    for t in trips {
        if let Some((total, _)) = app.primary.sim.finished_trip_time(*t) {
            done += 1;
            after += total;
            // Assume all trips completed before changes
            before += app.prebaked().finished_trip_time(*t).unwrap();
        }
    }
    (before, after, done)
}

fn make_top_center(
    ctx: &mut EventCtx,
    app: &App,
    before: Duration,
    after: Duration,
    done: usize,
    trips: usize,
    goal: Duration,
) -> Composite {
    let mut txt = Text::from(Line(format!("Total trip time: {} (", after)));
    txt.append_all(cmp_duration_shorter(after, before));
    txt.append(Line(")"));
    let sentiment = if before - after >= goal {
        "../data/system/assets/tools/happy.svg"
    } else {
        "../data/system/assets/tools/sad.svg"
    };

    Composite::new(
        Widget::col(vec![
            challenge_header(ctx, "Optimize the VIP's commute"),
            Widget::row(vec![
                Btn::svg_def("../data/system/assets/tools/location.svg")
                    .build(ctx, "locate VIP", None)
                    .margin_right(10),
                format!("{}/{} trips done", done, trips)
                    .draw_text(ctx)
                    .margin_right(20),
                txt.draw(ctx).margin_right(20),
                format!("Goal: {} faster", goal)
                    .draw_text(ctx)
                    .margin_right(5),
                Widget::draw_svg(ctx, sentiment).centered_vert(),
            ]),
        ])
        .bg(app.cs.panel_bg)
        .padding(5),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn final_score(
    ctx: &mut EventCtx,
    app: &mut App,
    mode: GameplayMode,
    before: Duration,
    after: Duration,
    goal: Duration,
) -> Box<dyn State> {
    let mut next_mode: Option<GameplayMode> = None;

    let msg = if before == after {
        format!(
            "The VIP's commute still takes a total of {}. Were you asleep on the job? Try \
             changing something!",
            before
        )
    } else if after > before {
        // TODO mad lib insults
        format!(
            "The VIP's commute went from {} total to {}. You utter dunce! Are you trying to screw \
             me over?!",
            before, after
        )
    } else if before - after < goal {
        format!(
            "The VIP's commute went from {} total to {}. Hmm... that's {} faster. But didn't I \
             tell you to speed things up by {} at least?",
            before,
            after,
            before - after,
            goal
        )
    } else {
        HighScore {
            goal: format!("make VIP's commute at least {} faster", goal),
            score: before - after,
            edits_name: app.primary.map.get_edits().edits_name.clone(),
        }
        .record(app, mode.clone());

        next_mode = Challenge::find(&mode).1.map(|c| c.gameplay);

        format!(
            "Alright, you somehow managed to shave {} down from the VIP's original commute of {}. \
             I guess that'll do. Maybe you're not totally useless after all.",
            before - after,
            before
        )
    };

    FinalScore::new(ctx, app, msg, mode, next_mode)
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
