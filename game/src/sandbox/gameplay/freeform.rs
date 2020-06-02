use crate::app::{App, ShowEverything};
use crate::common::{CityPicker, CommonState};
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::{nice_map_name, ID};
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::sandbox::SandboxMode;
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, ScreenRectangle, Spinner, TextExt, VerticalAlignment, Widget,
};
use geom::{Distance, Duration, Polygon};
use map_model::{Map, PathConstraints, PathRequest, Position, NORMAL_LANE_THICKNESS};
use sim::{
    DontDrawAgents, IndividTrip, PersonID, PersonSpec, Scenario, SidewalkSpot, SpawnTrip,
    TripEndpoint, TripMode,
};

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    top_center: Composite,
}

impl Freeform {
    pub fn new(ctx: &mut EventCtx, app: &App, mode: GameplayMode) -> Box<dyn GameplayState> {
        Box::new(Freeform {
            top_center: freeform_controller(ctx, app, mode, "none"),
        })
    }
}

impl GameplayState for Freeform {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> Option<Transition> {
        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "change map" => {
                    Some(Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            // The map will be switched before this callback happens.
                            let path = abstutil::path_map(app.primary.map.get_name());
                            Transition::PopThenReplace(Box::new(SandboxMode::new(
                                ctx,
                                app,
                                GameplayMode::Freeform(path),
                            )))
                        }),
                    )))
                }
                "change traffic" => Some(Transition::Push(make_change_traffic(
                    self.top_center.rect_of("change traffic").clone(),
                ))),
                "edit map" => Some(Transition::Push(Box::new(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::Freeform(abstutil::path_map(app.primary.map.get_name())),
                )))),
                "Start a new trip" => Some(Transition::Push(AgentSpawner::new(ctx, app))),
                _ => unreachable!(),
            },
            None => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

pub fn freeform_controller(
    ctx: &mut EventCtx,
    app: &App,
    gameplay: GameplayMode,
    scenario_name: &str,
) -> Composite {
    let mut rows = vec![Widget::row(vec![
        Line("Sandbox").small_heading().draw(ctx).margin(5),
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
        )
        .margin(5),
        "Map:".draw_text(ctx).margin(5),
        Btn::text_fg(format!("{} ▼", nice_map_name(app.primary.map.get_name())))
            .build(ctx, "change map", lctrl(Key::L))
            .margin(5),
        "Traffic:".draw_text(ctx).margin(5),
        Btn::text_fg(format!("{} ▼", scenario_name))
            .build(ctx, "change traffic", hotkey(Key::S))
            .margin(5),
        Btn::svg_def("../data/system/assets/tools/edit_map.svg")
            .build(ctx, "edit map", lctrl(Key::E))
            .margin(5),
    ])
    .centered()];
    if let GameplayMode::Freeform(_) = gameplay {
        rows.push(
            Btn::text_fg("Start a new trip")
                .build_def(ctx, None)
                .centered_horiz(),
        );
    }

    Composite::new(Widget::col(rows).bg(app.cs.panel_bg).padding(10))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx)
}

pub fn make_change_traffic(btn: ScreenRectangle) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, app| {
        let (_, scenario_name) = wiz.wrap(ctx).choose_exact(
            (
                HorizontalAlignment::Centered(btn.center().x),
                VerticalAlignment::Below(btn.y2 + 15.0),
            ),
            None,
            || {
                let mut list = Vec::new();
                for name in abstutil::list_all_objects(abstutil::path_all_scenarios(
                    app.primary.map.get_name(),
                )) {
                    if name == "weekday" {
                        list.push(Choice::new("realistic weekday traffic", name).tooltip(
                            "Trips will begin throughout the entire day. Midnight is usually \
                             quiet, so you may need to fast-forward to morning rush hour. Data \
                             comes from Puget Sound Regional Council's Soundcast model.",
                        ));
                        list.push(
                            Choice::new("5 weekdays repeated", "5 weekdays repeated".to_string())
                                .tooltip(
                                    "Same as the weekday traffic pattern, but blindly repeated 5 \
                                     times. This isn't realistic; people don't take exactly the \
                                     same trips every day.",
                                ),
                        );
                    } else {
                        list.push(Choice::new(name.clone(), name));
                    }
                }
                list.push(
                    Choice::new("random unrealistic trips", "random".to_string()).tooltip(
                        "Lots of trips will start at midnight, but not constantly appear through \
                         the day.",
                    ),
                );
                list.push(Choice::new("just buses", "just buses".to_string()));
                list.push(Choice::new(
                    "none (you manually spawn traffic)",
                    "empty".to_string(),
                ));
                list
            },
        )?;
        let map_path = abstutil::path_map(app.primary.map.get_name());
        Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
            ctx,
            app,
            if scenario_name == "empty" {
                GameplayMode::Freeform(map_path)
            } else {
                GameplayMode::PlayScenario(map_path, scenario_name)
            },
        ))))
    }))
}

// TODO Maybe move all this to the other module

const SMALL_DT: Duration = Duration::const_seconds(0.1);

struct AgentSpawner {
    composite: Composite,
    source: Option<TripEndpoint>,
    goal: Option<(TripEndpoint, Option<Polygon>)>,
    confirmed: bool,
}

impl AgentSpawner {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(AgentSpawner {
            source: None,
            goal: None,
            confirmed: false,
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("New trip").small_heading().draw(ctx),
                        Btn::plaintext("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    "Click a building or border to specify start"
                        .draw_text(ctx)
                        .named("instructions"),
                    Widget::row(vec![
                        "Type of trip:".draw_text(ctx).margin_right(10),
                        Widget::dropdown(
                            ctx,
                            "mode",
                            TripMode::Drive,
                            TripMode::all()
                                .into_iter()
                                .map(|m| Choice::new(m.ongoing_verb(), m))
                                .collect(),
                        ),
                    ]),
                    Widget::row(vec![
                        "Number of trips:".draw_text(ctx).margin_right(10),
                        Spinner::new(ctx, (1, 1000), 1).named("number"),
                    ]),
                    Btn::text_fg("Confirm").inactive(ctx).named("Confirm"),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State for AgentSpawner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // Arguably we should invalidate the goal when mode changes, since the path could change,
        // but meh.
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Confirm" => {
                    let map = &app.primary.map;
                    let mut scenario = Scenario::empty(map, "one-shot");
                    let from = self.source.take().unwrap();
                    let to = self.goal.take().unwrap().0;
                    for i in 0..self.composite.spinner("number") {
                        scenario.people.push(PersonSpec {
                            id: PersonID(app.primary.sim.get_all_people().len() + i),
                            orig_id: None,
                            trips: vec![IndividTrip {
                                depart: app.primary.sim.time(),
                                trip: SpawnTrip::new(
                                    from.clone(),
                                    to.clone(),
                                    self.composite.dropdown_value("mode"),
                                    map,
                                ),
                            }],
                        });
                    }
                    let mut rng = app.primary.current_flags.sim_flags.make_rng();
                    scenario.instantiate(
                        &mut app.primary.sim,
                        map,
                        &mut rng,
                        &mut Timer::new("spawn trip"),
                    );
                    app.primary.sim.normal_step(map, SMALL_DT);
                    app.recalculate_current_selection(ctx);
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        ctx.canvas_movement();

        if self.confirmed {
            return Transition::Keep;
        }

        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                true,
            );
            if let Some(ID::Intersection(i)) = app.primary.current_selection {
                if !app.primary.map.get_i(i).is_border() {
                    app.primary.current_selection = None;
                }
            } else if let Some(ID::Building(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
            }
        }
        if let Some(hovering) = match app.primary.current_selection {
            Some(ID::Intersection(i)) => Some(TripEndpoint::Border(i, None)),
            Some(ID::Building(b)) => Some(TripEndpoint::Bldg(b)),
            None => None,
            _ => unreachable!(),
        } {
            if self.source.is_none() && app.per_obj.left_click(ctx, "start here") {
                self.source = Some(hovering);
                self.composite.replace(
                    ctx,
                    "instructions",
                    "Click a building or border to specify end"
                        .draw_text(ctx)
                        .named("instructions"),
                );
            } else if self.source.is_some() && self.source != Some(hovering.clone()) {
                if self
                    .goal
                    .as_ref()
                    .map(|(to, _)| to != &hovering)
                    .unwrap_or(true)
                {
                    if let Some(path) = path_request(
                        self.source.clone().unwrap(),
                        hovering.clone(),
                        self.composite.dropdown_value("mode"),
                        &app.primary.map,
                    )
                    .and_then(|req| app.primary.map.pathfind(req))
                    {
                        self.goal = Some((
                            hovering,
                            path.trace(&app.primary.map, Distance::ZERO, None)
                                .map(|pl| pl.make_polygons(NORMAL_LANE_THICKNESS)),
                        ));
                    } else {
                        self.goal = None;
                    }
                }

                if self.goal.is_some() && app.per_obj.left_click(ctx, "end here") {
                    app.primary.current_selection = None;
                    self.confirmed = true;
                    self.composite.replace(
                        ctx,
                        "instructions",
                        "Confirm the trip settings"
                            .draw_text(ctx)
                            .named("instructions"),
                    );
                    self.composite.replace(
                        ctx,
                        "Confirm",
                        Btn::text_fg("Confirm").build_def(ctx, hotkey(Key::Enter)),
                    );
                }
            }
        } else {
            self.goal = None;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        CommonState::draw_osd(g, app, &app.primary.current_selection);

        if let Some(ref endpt) = self.source {
            g.draw_polygon(
                Color::BLUE.alpha(0.8),
                match endpt {
                    TripEndpoint::Border(i, _) => &app.primary.map.get_i(*i).polygon,
                    TripEndpoint::Bldg(b) => &app.primary.map.get_b(*b).polygon,
                },
            );
        }
        if let Some((ref endpt, ref poly)) = self.goal {
            g.draw_polygon(
                Color::GREEN.alpha(0.8),
                match endpt {
                    TripEndpoint::Border(i, _) => &app.primary.map.get_i(*i).polygon,
                    TripEndpoint::Bldg(b) => &app.primary.map.get_b(*b).polygon,
                },
            );
            if let Some(p) = poly {
                g.draw_polygon(Color::PURPLE, p);
            }
        }
    }
}

// TODO This exists in a few other places, in less clear forms...
fn path_request(
    from: TripEndpoint,
    to: TripEndpoint,
    mode: TripMode,
    map: &Map,
) -> Option<PathRequest> {
    Some(PathRequest {
        start: pos(from, mode, true, map)?,
        end: pos(to, mode, false, map)?,
        constraints: match mode {
            TripMode::Walk | TripMode::Transit => PathConstraints::Pedestrian,
            TripMode::Drive => PathConstraints::Car,
            TripMode::Bike => PathConstraints::Bike,
        },
    })
}

// TODO Bugs!
fn pos(endpt: TripEndpoint, mode: TripMode, from: bool, map: &Map) -> Option<Position> {
    match endpt {
        TripEndpoint::Bldg(b) => match mode {
            TripMode::Walk | TripMode::Transit => Some(map.get_b(b).front_path.sidewalk),
            // TODO nope! use parking blckhole
            TripMode::Bike | TripMode::Drive => Position::bldg_via_driving(b, map),
        },
        TripEndpoint::Border(i, _) => match mode {
            TripMode::Walk | TripMode::Transit => if from {
                SidewalkSpot::start_at_border(i, None, map)
            } else {
                SidewalkSpot::end_at_border(i, None, map)
            }
            .map(|spot| spot.sidewalk_pos),
            // TODO
            TripMode::Bike | TripMode::Drive => None,
        },
    }
}
