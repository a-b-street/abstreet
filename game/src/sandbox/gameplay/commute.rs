use crate::app::App;
use crate::challenges::{Challenge, HighScore};
use crate::common::{ContextualActions, Tab};
use crate::cutscene::{CutsceneBuilder, FYI};
use crate::edit::EditMode;
use crate::game::{State, Transition};
use crate::helpers::cmp_duration_shorter;
use crate::helpers::ID;
use crate::sandbox::gameplay::{challenge_header, FinalScore, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use ezgui::{
    Btn, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    RewriteColor, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Duration, Polygon, Time};
use sim::{OrigPersonID, PersonID, TripID};
use std::collections::BTreeMap;

// TODO Avoid hack entirely, or tune appearance
const METER_HACK: f64 = -15.0;

// TODO A nice level to unlock: specifying your own commute, getting to work on it

pub struct OptimizeCommute {
    top_center: Composite,
    meter: Composite,
    person: PersonID,
    mode: GameplayMode,
    goal: Duration,
    time: Time,
    done: bool,

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
            top_center: Composite::new(Widget::col(vec![
                challenge_header(ctx, "Optimize the VIP's commute"),
                Widget::row(vec![
                    format!("Speed up the VIP's trips by {}", goal)
                        .draw_text(ctx)
                        .centered_vert(),
                    Btn::svg(
                        "system/assets/tools/hint.svg",
                        RewriteColor::Change(Color::WHITE, app.cs.hovering),
                    )
                    .build(ctx, "hint", None)
                    .align_right(),
                ]),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            meter: make_meter(ctx, Duration::ZERO, Duration::ZERO, 0, trips.len()),
            person,
            mode: GameplayMode::OptimizeCommute(orig_person, goal),
            goal,
            time: Time::START_OF_DAY,
            done: false,
            trips,
            once: true,
        })
    }

    pub fn cutscene_pt1(ctx: &mut EventCtx, app: &App, mode: &GameplayMode) -> Box<dyn State> {
        CutsceneBuilder::new("Optimize one commute: part 1")
            .boss("Listen up, I've got a special job for you today.")
            .player("What is it? The scooter coalition back with demands for more valet parking?")
            .boss("No, all the tax-funded valets are still busy with the kayakers.")
            .boss(
                "I've got a... friend who's tired of getting stuck in traffic. You've got to make \
                 their commute as fast as possible.",
            )
            .player("Uh, what's so special about them?")
            .boss(
                "That's none of your concern! I've anonymized their name, so don't even bother \
                 digging into what happened in Ballard --",
            )
            .boss("JUST GET TO WORK, KID!")
            .player(
                "(Somebody's blackmailing the boss. Guess it's time to help this Very Impatient \
                 Person.)",
            )
            .build(ctx, app, cutscene_task(mode))
    }

    pub fn cutscene_pt2(ctx: &mut EventCtx, app: &App, mode: &GameplayMode) -> Box<dyn State> {
        // TODO The person chosen for this currently has more of an issue needing PBLs, actually.
        CutsceneBuilder::new("Optimize one commute: part 2")
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
            .player("(Too many people have dirt on the boss. Guess we have another VIP to help.)")
            .build(ctx, app, cutscene_task(mode))
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

        self.meter.align_below(
            ctx,
            &controls.agent_meter.as_ref().unwrap().composite,
            METER_HACK,
        );

        if self.time != app.primary.sim.time() && !self.done {
            self.time = app.primary.sim.time();

            let (before, after, done) = get_score(app, &self.trips);
            self.meter = make_meter(ctx, before, after, done, self.trips.len());
            self.meter.align_below(
                ctx,
                &controls.agent_meter.as_ref().unwrap().composite,
                METER_HACK,
            );

            if done == self.trips.len() {
                self.done = true;
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
            Outcome::Clicked(x) => match x.as_ref() {
                "edit map" => {
                    return Some(Transition::Push(Box::new(EditMode::new(
                        ctx,
                        app,
                        self.mode.clone(),
                    ))));
                }
                "instructions" => {
                    let contents = (cutscene_task(&self.mode))(ctx);
                    return Some(Transition::Push(FYI::new(ctx, contents, Color::WHITE)));
                }
                "hint" => {
                    // TODO Multiple hints. Point to follow button.
                    let mut txt = Text::from(Line("Hints"));
                    txt.add(Line(""));
                    txt.add(Line("Use the locator at the top right to find the VIP."));
                    txt.add(Line("You can wait for one of their trips to begin or end."));
                    txt.add(Line("Focus on trips spent mostly waiting"));
                    let contents = txt.draw(ctx);
                    return Some(Transition::Push(FYI::new(ctx, contents, app.cs.panel_bg)));
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        match self.meter.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
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
            _ => {}
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
        self.meter.draw(g);
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

fn make_meter(
    ctx: &mut EventCtx,
    before: Duration,
    after: Duration,
    done: usize,
    trips: usize,
) -> Composite {
    let mut txt = Text::from(Line(format!("Total time: {} (", after)));
    txt.append_all(cmp_duration_shorter(after, before));
    txt.append(Line(")"));

    Composite::new(Widget::col(vec![
        // Separator
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(
                Color::WHITE,
                Polygon::rectangle(0.2 * ctx.canvas.window_width / ctx.get_scale_factor(), 2.0),
            )]),
        )
        .centered_horiz(),
        Widget::row(vec![
            Btn::svg_def("system/assets/tools/location.svg").build(ctx, "locate VIP", None),
            format!("{}/{} trips done", done, trips).draw_text(ctx),
            txt.draw(ctx),
        ]),
    ]))
    .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
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

fn cutscene_task(mode: &GameplayMode) -> Box<dyn Fn(&mut EventCtx) -> Widget> {
    let goal = match mode {
        GameplayMode::OptimizeCommute(_, d) => *d,
        _ => unreachable!(),
    };

    Box::new(move |ctx| {
        Widget::custom_col(vec![
            Text::from_multiline(vec![
                Line(format!("Speed up the VIP's trips by a total of {}", goal)).fg(Color::BLACK),
                Line("Ignore the damage done to everyone else.").fg(Color::BLACK),
            ])
            .draw(ctx)
            .margin_below(30),
            Widget::row(vec![
                Widget::col(vec![
                    Line("Time").fg(Color::BLACK).draw(ctx),
                    Widget::draw_svg_transform(
                        ctx,
                        "system/assets/tools/time.svg",
                        RewriteColor::ChangeAll(Color::BLACK),
                    ),
                    Text::from_multiline(vec![
                        Line("Until the VIP's").fg(Color::BLACK),
                        Line("last trip is done").fg(Color::BLACK),
                    ])
                    .draw(ctx),
                ]),
                Widget::col(vec![
                    Line("Goal").fg(Color::BLACK).draw(ctx),
                    Widget::draw_svg_transform(
                        ctx,
                        "system/assets/tools/location.svg",
                        RewriteColor::ChangeAll(Color::BLACK),
                    ),
                    Text::from_multiline(vec![
                        Line("Speed up the VIP's trips").fg(Color::BLACK),
                        Line(format!("by at least {}", goal)).fg(Color::BLACK),
                    ])
                    .draw(ctx),
                ]),
                Widget::col(vec![
                    Line("Score").fg(Color::BLACK).draw(ctx),
                    Widget::draw_svg_transform(
                        ctx,
                        "system/assets/tools/star.svg",
                        RewriteColor::ChangeAll(Color::BLACK),
                    ),
                    Text::from_multiline(vec![
                        Line("How much time").fg(Color::BLACK),
                        Line("the VIP saves").fg(Color::BLACK),
                    ])
                    .draw(ctx),
                ]),
            ])
            .evenly_spaced(),
        ])
    })
}
