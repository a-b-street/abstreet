use geom::{Duration, Time};
use map_gui::ID;
use map_model::IntersectionID;
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Image, Key, Line, Outcome, Panel, State, Text,
    VerticalAlignment, Widget,
};

use crate::app::Transition;
use crate::app::{App, FindDelayedIntersections};
use crate::challenges::cutscene::{CutsceneBuilder, FYI};
use crate::challenges::HighScore;
use crate::common::Warping;
use crate::edit::EditMode;
use crate::sandbox::gameplay::{challenge_header, FinalScore, GameplayMode, GameplayState};
use crate::sandbox::{Actions, SandboxControls, SandboxMode};

const THRESHOLD: Duration = Duration::const_seconds(20.0 * 60.0);

pub struct FixTrafficSignals {
    top_right: Panel,
    time: Time,
    worst: Option<(IntersectionID, Duration)>,
    done_at: Option<Time>,
    mode: GameplayMode,
}

impl FixTrafficSignals {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn GameplayState> {
        Box::new(FixTrafficSignals {
            top_right: Panel::empty(ctx),
            time: Time::START_OF_DAY,
            worst: None,
            done_at: None,
            mode: GameplayMode::FixTrafficSignals,
        })
    }

    pub fn cutscene_pt1(ctx: &mut EventCtx, _: &App, _: &GameplayMode) -> Box<dyn State<App>> {
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
            .extra(
                "parents.svg.gz",
                0.6,
                "Hi, er, we're calling from Lower Queen Anne. What's going on?!",
            )
            .extra(
                "parents.svg.gz",
                0.6,
                "We just missed a VERY important appointment. Nobody's moving an inch!",
            )
            .boss(
                "Oh no... reports are coming in, ALL of the traffic signals downtown are screwed \
                 up!",
            )
            .boss(
                "You need to go fix all of them. But listen, you haven't got much time. Focus on \
                 the worst problems first.",
            )
            .player("Sigh... it's going to be a long day.")
            .build(ctx, Box::new(cutscene_pt1_task))
    }
}

impl GameplayState for FixTrafficSignals {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
        _: &mut Actions,
    ) -> Option<Transition> {
        // Normally we just do this once at the beginning, but because there are other paths to
        // reseting (like jump-to-time), it's safest just to do this.
        if app.primary.sim_cb.is_none() {
            app.primary.sim_cb = Some(Box::new(FindDelayedIntersections {
                halt_limit: THRESHOLD,
                report_limit: Duration::minutes(1),
                currently_delayed: Vec::new(),
            }));
            app.primary.sim.set_periodic_callback(Duration::minutes(1));
        }

        if self.time != app.primary.sim.time() && self.done_at.is_none() {
            self.time = app.primary.sim.time();

            self.worst = None;
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
                self.worst = Some((i, app.primary.sim.time() - t));
            }

            if self
                .worst
                .map(|(_, delay)| delay >= THRESHOLD)
                .unwrap_or(false)
            {
                self.done_at = Some(app.primary.sim.time());
                self.recreate_panels(ctx, app);

                return Some(Transition::Multi(vec![
                    Transition::Push(final_score(ctx, app, self.mode.clone(), true)),
                    Transition::Push(Warping::new(
                        ctx,
                        app.primary
                            .canonical_point(ID::Intersection(self.worst.unwrap().0))
                            .unwrap(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    )),
                ]));
            } else {
                self.recreate_panels(ctx, app);
            }

            if app.primary.sim.is_done() {
                self.done_at = Some(app.primary.sim.time());
                // TODO The score is up to 1 min (report_limit) off.
                return Some(Transition::Push(final_score(
                    ctx,
                    app,
                    self.mode.clone(),
                    false,
                )));
            }
        }

        match self.top_right.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "edit map" => {
                    return Some(Transition::Push(EditMode::new(ctx, app, self.mode.clone())));
                }
                "instructions" => {
                    let contents = cutscene_pt1_task(ctx);
                    return Some(Transition::Push(FYI::new(ctx, contents, Color::WHITE)));
                }
                "hint" => {
                    // TODO Multiple hints. Point to layers.
                    let mut txt = Text::from("Hint");
                    txt.add_line("");
                    txt.add_appended(vec![
                        Line("Press "),
                        Key::L.txt(ctx),
                        Line(" to open layers. Try "),
                        Key::D.txt(ctx),
                        Line("elay or worst traffic "),
                        Key::J.txt(ctx),
                        Line("ams"),
                    ]);
                    let contents = txt.into_widget(ctx);
                    return Some(Transition::Push(FYI::new(ctx, contents, app.cs.panel_bg)));
                }
                "try again" => {
                    return Some(Transition::Replace(SandboxMode::simple_new(
                        app,
                        self.mode.clone(),
                    )));
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
                        app.primary.canonical_point(ID::Intersection(i)).unwrap(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    )));
                }
                "explain score" => {
                    // TODO Adjust wording
                    return Some(Transition::Push(FYI::new(
                        ctx,
                        Text::from_multiline(vec![
                            Line("You changed some traffic signals in the middle of the day."),
                            Line(
                                "First see if you can survive for a full day, making changes \
                                 along the way.",
                            ),
                            Line("Then you should check if your changes work from midnight."),
                        ])
                        .into_widget(ctx),
                        app.cs.panel_bg,
                    )));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_right.draw(g);
    }

    fn recreate_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        if let Some(time) = self.done_at {
            self.top_right = Panel::new(Widget::col(vec![
                challenge_header(ctx, "Traffic signal survivor"),
                Widget::row(vec![
                    Line(format!("Delay exceeded {} at {}", THRESHOLD, time))
                        .fg(Color::RED)
                        .into_widget(ctx)
                        .centered_vert(),
                    ctx.style().btn_outline.text("try again").build_def(ctx),
                ]),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx);
        } else {
            let meter = Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/location.svg")
                    .disabled(self.worst.is_none())
                    .build_widget(ctx, "go to slowest intersection"),
                Text::from_all(vec![
                    Line("Worst delay: "),
                    if let Some((_, delay)) = self.worst {
                        Line(delay.to_string(&app.opts.units)).fg(if delay < Duration::minutes(5) {
                            Color::hex("#F9EC51")
                        } else if delay < Duration::minutes(15) {
                            Color::hex("#EE702E")
                        } else {
                            Color::hex("#EB3223")
                        })
                    } else {
                        Line("none!").secondary()
                    },
                ])
                .into_widget(ctx)
                .centered_vert(),
                if app.primary.dirty_from_edits {
                    ctx.style()
                        .btn_plain
                        .icon("system/assets/tools/info.svg")
                        .build_widget(ctx, "explain score")
                        .align_right()
                } else {
                    Widget::nothing()
                },
            ]);

            self.top_right = Panel::new(Widget::col(vec![
                challenge_header(ctx, "Traffic signal survivor"),
                Widget::row(vec![
                    Line(format!(
                        "Keep delay at all intersections under {}",
                        THRESHOLD
                    ))
                    .into_widget(ctx),
                    ctx.style()
                        .btn_plain
                        .icon_text("system/assets/tools/lightbulb.svg", "Hint")
                        .build_widget(ctx, "hint")
                        .align_right(),
                ]),
                meter,
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx);
        }
    }

    fn on_destroy(&self, app: &mut App) {
        assert!(app.primary.sim_cb.is_some());
        app.primary.sim_cb = None;
        app.primary.sim.unset_periodic_callback();
    }
}

fn final_score(
    ctx: &mut EventCtx,
    app: &mut App,
    mode: GameplayMode,
    failed: bool,
) -> Box<dyn State<App>> {
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
    let icon_builder = Image::empty().color(Color::BLACK).dims(50.0);
    Widget::custom_col(vec![
        Text::from_multiline(vec![
            Line(format!(
                "Don't let anyone be delayed by one traffic signal more than {}!",
                THRESHOLD
            ))
            .fg(Color::BLACK),
            Line("Survive as long as possible through 24 hours of a busy weekday.")
                .fg(Color::BLACK),
        ])
        .into_widget(ctx)
        .margin_below(30),
        Widget::custom_row(vec![
            Widget::col(vec![
                Line("Time").fg(Color::BLACK).into_widget(ctx),
                icon_builder
                    .clone()
                    .source_path("system/assets/tools/time.svg")
                    .into_widget(ctx),
                Line("24 hours").fg(Color::BLACK).into_widget(ctx),
            ]),
            Widget::col(vec![
                Line("Goal").fg(Color::BLACK).into_widget(ctx),
                icon_builder
                    .clone()
                    .source_path("system/assets/tools/location.svg")
                    .into_widget(ctx),
                Text::from_multiline(vec![
                    Line("Keep delay at all intersections").fg(Color::BLACK),
                    Line(format!("under {}", THRESHOLD)).fg(Color::BLACK),
                ])
                .into_widget(ctx),
            ]),
            Widget::col(vec![
                Line("Score").fg(Color::BLACK).into_widget(ctx),
                icon_builder
                    .source_path("system/assets/tools/star.svg")
                    .into_widget(ctx),
                Line("How long you survive")
                    .fg(Color::BLACK)
                    .into_widget(ctx),
            ]),
        ])
        .evenly_spaced(),
    ])
}
