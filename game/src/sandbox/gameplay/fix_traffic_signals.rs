use crate::app::{App, FindDelayedIntersections};
use crate::challenges::HighScore;
use crate::common::Warping;
use crate::cutscene::{CutsceneBuilder, FYI};
use crate::edit::EditMode;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::sandbox::gameplay::{challenge_header, FinalScore, GameplayMode, GameplayState};
use crate::sandbox::{SandboxControls, SandboxMode};
use ezgui::{
    Btn, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, RewriteColor,
    Text, VerticalAlignment, Widget,
};
use geom::{Duration, Time};
use map_model::IntersectionID;

const THRESHOLD: Duration = Duration::const_seconds(20.0 * 60.0);

pub struct FixTrafficSignals {
    top_center: Composite,
    time: Time,
    once: bool,
    done: bool,
    mode: GameplayMode,
}

impl FixTrafficSignals {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn GameplayState> {
        Box::new(FixTrafficSignals {
            top_center: make_top_center(ctx, app, None, None),
            time: Time::START_OF_DAY,
            once: true,
            done: false,
            mode: GameplayMode::FixTrafficSignals,
        })
    }

    pub fn cutscene_pt1(ctx: &mut EventCtx, app: &App, _: &GameplayMode) -> Box<dyn State> {
        CutsceneBuilder::new("Traffic signal survivor")
            .boss("I hope you've had your coffee. There's a huge mess downtown.")
            .player("Did two buses get tangled together again?")
            .boss("Worse. SCOOT along Mercer is going haywire.")
            .player("SCOOT?")
            .boss(
                "You know, Split Cycle Offset Optimization Technique, the traffic signal \
                 coordination system? Did you sleep through college or what?",
            )
            .boss(
                "It's offline. All the traffic signals look like they've been reset to industry \
                 defaults.",
            )
            .player("Uh oh. Too much scooter traffic overwhelm it? Eh? EHH?")
            .boss("...")
            .boss("You know, not every problem you will face in life is caused by a pun.")
            .boss(
                "Most, in fact, will be caused by me ruining your life because you won't take \
                 your job seriously.",
            )
            .player("Sorry, boss.")
            .boss(
                "Oh no... reports are coming in, ALL of the traffic signals downtown are screwed \
                 up!",
            )
            .boss(
                "You need to go fix all of them. But listen, you haven't got much time. Focus on \
                 the worst problems first.",
            )
            .player("Sigh... it's going to be a long day.")
            .build(ctx, app, Box::new(cutscene_pt1_task))
    }
}

impl GameplayState for FixTrafficSignals {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> Option<Transition> {
        if self.once {
            self.once = false;
            assert!(app.primary.sim_cb.is_none());
            app.primary.sim_cb = Some(Box::new(FindDelayedIntersections {
                halt_limit: THRESHOLD,
                report_limit: Duration::minutes(1),
                currently_delayed: Vec::new(),
            }));
            app.primary.sim.set_periodic_callback(Duration::minutes(1));
        }

        if self.time != app.primary.sim.time() && !self.done {
            self.time = app.primary.sim.time();

            if let Some((i, t)) = app
                .primary
                .sim_cb
                .as_mut()
                .unwrap()
                .downcast_mut::<FindDelayedIntersections>()
                .unwrap()
                .currently_delayed
                .get(0)
                .cloned()
            {
                let dt = app.primary.sim.time() - t;
                if dt >= THRESHOLD {
                    self.done = true;
                    self.top_center =
                        make_top_center(ctx, app, Some((i, dt)), Some(app.primary.sim.time()));
                    return Some(Transition::PushTwice(
                        final_score(ctx, app, self.mode.clone(), true),
                        Warping::new(
                            ctx,
                            ID::Intersection(i).canonical_point(&app.primary).unwrap(),
                            Some(10.0),
                            None,
                            &mut app.primary,
                        ),
                    ));
                } else {
                    self.top_center = make_top_center(ctx, app, Some((i, dt)), None);
                }
            } else {
                self.top_center = make_top_center(ctx, app, None, None);
            }

            if app.primary.sim.is_done() {
                self.done = true;
                // TODO The score is up to 1 min (report_limit) off.
                return Some(Transition::Push(final_score(
                    ctx,
                    app,
                    self.mode.clone(),
                    false,
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
                    let contents = cutscene_pt1_task(ctx);
                    return Some(Transition::Push(FYI::new(ctx, contents, Color::WHITE)));
                }
                "hint" => {
                    // TODO Multiple hints. Point to layers.
                    let mut txt = Text::from(Line("Hint"));
                    txt.add(Line(""));
                    txt.add_appended(vec![
                        Line("Press "),
                        Line(Key::L.describe()).fg(ctx.style().hotkey_color),
                        Line(" to open layers. Try "),
                        Line(Key::D.describe()).fg(ctx.style().hotkey_color),
                        Line("elay or worst traffic "),
                        Line(Key::J.describe()).fg(ctx.style().hotkey_color),
                        Line("ams"),
                    ]);
                    let contents = txt.draw(ctx);
                    return Some(Transition::Push(FYI::new(ctx, contents, app.cs.panel_bg)));
                }
                "try again" => {
                    return Some(Transition::Replace(Box::new(SandboxMode::new(
                        ctx,
                        app,
                        self.mode.clone(),
                    ))));
                }
                "go to slowest intersection" => {
                    let i = app
                        .primary
                        .sim_cb
                        .as_ref()
                        .unwrap()
                        .downcast_ref::<FindDelayedIntersections>()
                        .unwrap()
                        .currently_delayed[0]
                        .0;
                    return Some(Transition::Push(Warping::new(
                        ctx,
                        ID::Intersection(i).canonical_point(&app.primary).unwrap(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    )));
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

    fn on_destroy(&self, app: &mut App) {
        assert!(app.primary.sim_cb.is_some());
        app.primary.sim_cb = None;
        app.primary.sim.unset_periodic_callback();
    }
}

fn make_top_center(
    ctx: &mut EventCtx,
    app: &App,
    worst: Option<(IntersectionID, Duration)>,
    failed_at: Option<Time>,
) -> Composite {
    Composite::new(
        Widget::col(vec![
            challenge_header(ctx, "Traffic signal survivor"),
            if let Some((_, delay)) = worst {
                Widget::row(vec![
                    Text::from_all(vec![
                        Line("Worst delay: "),
                        Line(delay.to_string()).fg(if delay < Duration::minutes(5) {
                            Color::hex("#F9EC51")
                        } else if delay < Duration::minutes(15) {
                            Color::hex("#EE702E")
                        } else {
                            Color::hex("#EB3223")
                        }),
                    ])
                    .draw(ctx),
                    Btn::svg_def("../data/system/assets/tools/location.svg")
                        .build(ctx, "go to slowest intersection", None)
                        .align_right(),
                ])
            } else {
                Widget::row(vec![
                    Text::from_all(vec![Line("Worst delay: "), Line("none!").secondary()])
                        .draw(ctx),
                    Widget::draw_svg_transform(
                        ctx,
                        "../data/system/assets/tools/location.svg",
                        RewriteColor::ChangeAlpha(0.5),
                    )
                    .align_right(),
                ])
            },
            if let Some(t) = failed_at {
                Widget::row(vec![
                    Line(format!("Delay exceeded {} at {}", THRESHOLD, t))
                        .fg(Color::RED)
                        .draw(ctx)
                        .centered_vert()
                        .margin_right(10),
                    Btn::text_fg("try again").build_def(ctx, None),
                ])
            } else {
                Widget::row(vec![
                    Line(format!("Keep delay under {}", THRESHOLD))
                        .secondary()
                        .draw(ctx),
                    Btn::svg(
                        "../data/system/assets/tools/hint.svg",
                        RewriteColor::Change(Color::WHITE, app.cs.hovering),
                    )
                    .build(ctx, "hint", None)
                    .align_right(),
                ])
            },
        ])
        .bg(app.cs.panel_bg)
        .padding(16),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn final_score(
    ctx: &mut EventCtx,
    app: &mut App,
    mode: GameplayMode,
    failed: bool,
) -> Box<dyn State> {
    let score = app.primary.sim.time() - Time::START_OF_DAY;
    HighScore {
        goal: format!(
            "make it {} without delay exceeding {}",
            app.primary.sim.get_end_of_day() - Time::START_OF_DAY,
            THRESHOLD
        ),
        score,
        edits_name: app.primary.map.get_edits().edits_name.clone(),
    }
    .record(app, mode.clone());

    let msg = if failed {
        format!(
            "You only made it {} before the traffic signals caused a jam. Lame!",
            score
        )
    } else {
        "Wow, you managed to fix the signals. Great job!".to_string()
    };
    FinalScore::new(ctx, app, msg, mode, None)
}

// TODO Can we automatically transform text and SVG colors?
fn cutscene_pt1_task(ctx: &mut EventCtx) -> Widget {
    Widget::col(vec![
        Text::from_multiline(vec![
            Line(format!(
                "Don't let anyone be delayed by one traffic signal more than {}!",
                THRESHOLD
            ))
            .fg(Color::BLACK),
            Line("Survive as long as possible through 24 hours of a busy weekday.")
                .fg(Color::BLACK),
        ])
        .draw(ctx)
        .margin_below(30),
        Widget::row(vec![
            Widget::col(vec![
                Line("Time").fg(Color::BLACK).draw(ctx),
                Widget::draw_svg_transform(
                    ctx,
                    "../data/system/assets/tools/time.svg",
                    RewriteColor::ChangeAll(Color::BLACK),
                )
                .margin_below(5)
                .margin_above(5),
                Line("24 hours").fg(Color::BLACK).draw(ctx),
            ]),
            Widget::col(vec![
                Line("Goal").fg(Color::BLACK).draw(ctx),
                Widget::draw_svg_transform(
                    ctx,
                    "../data/system/assets/tools/location.svg",
                    RewriteColor::ChangeAll(Color::BLACK),
                )
                .margin_below(5)
                .margin_above(5),
                Text::from_multiline(vec![
                    Line("Keep delay at all intersections").fg(Color::BLACK),
                    Line(format!("under {}", THRESHOLD)).fg(Color::BLACK),
                ])
                .draw(ctx),
            ]),
            Widget::col(vec![
                Line("Score").fg(Color::BLACK).draw(ctx),
                Widget::draw_svg_transform(
                    ctx,
                    "../data/system/assets/tools/star.svg",
                    RewriteColor::ChangeAll(Color::BLACK),
                )
                .margin_below(5)
                .margin_above(5),
                Line("How long you survive").fg(Color::BLACK).draw(ctx),
            ]),
        ])
        .evenly_spaced(),
    ])
}
