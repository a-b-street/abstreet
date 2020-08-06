use crate::app::{App, ShowEverything};
use crate::common::{CityPicker, CommonState};
use crate::edit::EditMode;
use crate::game::{ChooseSomething, State, Transition};
use crate::helpers::{nice_map_name, ID};
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::sandbox::SandboxMode;
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, ScreenRectangle, Spinner, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Distance, Polygon};
use map_model::{
    BuildingID, IntersectionID, Map, PathConstraints, PathRequest, Position, NORMAL_LANE_THICKNESS,
};
use rand::seq::SliceRandom;
use rand::Rng;
use sim::{
    DontDrawAgents, DrivingGoal, IndividTrip, PersonID, PersonSpec, Scenario, SidewalkSpot,
    SpawnTrip, TripEndpoint, TripMode, TripSpec,
};

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    top_center: Composite,
}

impl Freeform {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn GameplayState> {
        Box::new(Freeform {
            top_center: make_top_center(ctx, app),
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
            Outcome::Clicked(x) => match x.as_ref() {
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
                    ctx,
                    app,
                    self.top_center.rect_of("change traffic").clone(),
                    "none".to_string(),
                ))),
                "edit map" => Some(Transition::Push(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::Freeform(abstutil::path_map(app.primary.map.get_name())),
                ))),
                "Start a new trip" => Some(Transition::Push(AgentSpawner::new(ctx, None))),
                _ => unreachable!(),
            },
            _ => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

fn make_top_center(ctx: &mut EventCtx, app: &App) -> Composite {
    let rows = vec![
        Widget::row(vec![
            Line("Sandbox").small_heading().draw(ctx),
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
            ),
            "Map:".draw_text(ctx),
            Btn::text_fg(format!("{} ↓", nice_map_name(app.primary.map.get_name()))).build(
                ctx,
                "change map",
                lctrl(Key::L),
            ),
            "Traffic:".draw_text(ctx),
            Btn::text_fg("none ↓").build(ctx, "change traffic", hotkey(Key::S)),
            Btn::svg_def("system/assets/tools/edit_map.svg").build(ctx, "edit map", lctrl(Key::E)),
        ])
        .centered(),
        Btn::text_fg("Start a new trip")
            .build_def(ctx, None)
            .centered_horiz(),
        Text::from_all(vec![
            Line("Select an intersection and press "),
            Line(Key::Z.describe()).fg(ctx.style().hotkey_color),
            Line(" to start traffic nearby"),
        ])
        .draw(ctx),
    ];

    Composite::new(Widget::col(rows))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx)
}

pub fn make_change_traffic(
    ctx: &mut EventCtx,
    app: &App,
    btn: ScreenRectangle,
    current: String,
) -> Box<dyn State> {
    let mut choices = Vec::new();
    for name in abstutil::list_all_objects(abstutil::path_all_scenarios(app.primary.map.get_name()))
    {
        if name == "weekday" {
            choices.push(Choice::new("realistic weekday traffic", name).tooltip(
                "Trips will begin throughout the entire day. Midnight is usually quiet, so you \
                 may need to fast-forward to morning rush hour. Data comes from Puget Sound \
                 Regional Council's Soundcast model.",
            ));
        } else {
            choices.push(Choice::new(name.clone(), name));
        }
    }
    choices.push(
        Choice::new("trips between home and work", "home_to_work".to_string()).tooltip(
            "Randomized people will leave homes in the morning, go to work, then return in the \
             afternoon. It'll be very quiet before 7am and between 10am to 5pm.",
        ),
    );
    choices.push(
        Choice::new("random unrealistic trips", "random".to_string()).tooltip(
            "Lots of trips will start at midnight, but not constantly appear through the day.",
        ),
    );
    choices.push(Choice::new(
        "none, except for buses -- you manually spawn traffic",
        "none".to_string(),
    ));
    let choices = choices
        .into_iter()
        .map(|c| {
            if c.data == current {
                c.active(false)
            } else {
                c
            }
        })
        .collect();

    ChooseSomething::new_below(
        ctx,
        &btn,
        choices,
        Box::new(|scenario_name, ctx, app| {
            let map_path = abstutil::path_map(app.primary.map.get_name());
            Transition::PopThenReplace(Box::new(SandboxMode::new(
                ctx,
                app,
                if scenario_name == "none" {
                    GameplayMode::Freeform(map_path)
                } else {
                    GameplayMode::PlayScenario(map_path, scenario_name, Vec::new())
                },
            )))
        }),
    )
}

struct AgentSpawner {
    composite: Composite,
    source: Option<TripEndpoint>,
    goal: Option<(TripEndpoint, Option<Polygon>)>,
    confirmed: bool,
}

impl AgentSpawner {
    fn new(ctx: &mut EventCtx, start: Option<BuildingID>) -> Box<dyn State> {
        let mut spawner = AgentSpawner {
            source: None,
            goal: None,
            confirmed: false,
            composite: Composite::new(Widget::col(vec![
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
                    "Type of trip:".draw_text(ctx),
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
                    "Number of trips:".draw_text(ctx),
                    Spinner::new(ctx, (1, 1000), 1).named("number"),
                ]),
                Btn::text_fg("Confirm").inactive(ctx).named("Confirm"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        };
        if let Some(b) = start {
            spawner.source = Some(TripEndpoint::Bldg(b));
            spawner.composite.replace(
                ctx,
                "instructions",
                "Click a building or border to specify end"
                    .draw_text(ctx)
                    .named("instructions"),
            );
        }
        Box::new(spawner)
    }
}

impl State for AgentSpawner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Confirm" => {
                    let map = &app.primary.map;
                    let mut scenario = Scenario::empty(map, "one-shot");
                    let from = self.source.take().unwrap();
                    let to = self.goal.take().unwrap().0;
                    for i in 0..self.composite.spinner("number") as usize {
                        if let Some(trip) = SpawnTrip::new(
                            from.clone(),
                            to.clone(),
                            self.composite.dropdown_value("mode"),
                            map,
                        ) {
                            scenario.people.push(PersonSpec {
                                id: PersonID(app.primary.sim.get_all_people().len() + i),
                                orig_id: None,
                                trips: vec![IndividTrip::new(app.primary.sim.time(), trip)],
                            });
                        }
                    }
                    let mut rng = app.primary.current_flags.sim_flags.make_rng();
                    scenario.instantiate(
                        &mut app.primary.sim,
                        map,
                        &mut rng,
                        &mut Timer::new("spawn trip"),
                    );
                    app.primary.sim.tiny_step(map, &mut app.primary.sim_cb);
                    app.recalculate_current_selection(ctx);
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                // We need to recalculate the path to see if this is sane. Otherwise we could trick
                // a pedestrian into wandering on/off a highway border.
                if self.goal.is_some() {
                    let to = self.goal.as_ref().unwrap().0.clone();
                    if let Some(path) = path_request(
                        self.source.clone().unwrap(),
                        to.clone(),
                        self.composite.dropdown_value("mode"),
                        &app.primary.map,
                    )
                    .and_then(|req| app.primary.map.pathfind(req))
                    {
                        self.goal = Some((
                            to,
                            path.trace(&app.primary.map, Distance::ZERO, None)
                                .map(|pl| pl.make_polygons(NORMAL_LANE_THICKNESS)),
                        ));
                    } else {
                        self.goal = None;
                        self.confirmed = false;
                        self.composite.replace(
                            ctx,
                            "instructions",
                            "Click a building or border to specify end"
                                .draw_text(ctx)
                                .named("instructions"),
                        );
                        self.composite.replace(
                            ctx,
                            "Confirm",
                            Btn::text_fg("Confirm").inactive(ctx).named("Confirm"),
                        );
                    }
                }
            }
            _ => {}
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
        CommonState::draw_osd(g, app);

        if let Some(ref endpt) = self.source {
            g.draw_polygon(
                Color::BLUE.alpha(0.8),
                match endpt {
                    TripEndpoint::Border(i, _) => app.primary.map.get_i(*i).polygon.clone(),
                    TripEndpoint::Bldg(b) => app.primary.map.get_b(*b).polygon.clone(),
                },
            );
        }
        if let Some((ref endpt, ref poly)) = self.goal {
            g.draw_polygon(
                Color::GREEN.alpha(0.8),
                match endpt {
                    TripEndpoint::Border(i, _) => app.primary.map.get_i(*i).polygon.clone(),
                    TripEndpoint::Bldg(b) => app.primary.map.get_b(*b).polygon.clone(),
                },
            );
            if let Some(p) = poly {
                g.draw_polygon(Color::PURPLE, p.clone());
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

fn pos(endpt: TripEndpoint, mode: TripMode, from: bool, map: &Map) -> Option<Position> {
    match endpt {
        TripEndpoint::Bldg(b) => match mode {
            TripMode::Walk | TripMode::Transit => Some(map.get_b(b).sidewalk_pos),
            TripMode::Bike => Some(DrivingGoal::ParkNear(b).goal_pos(PathConstraints::Bike, map)?),
            TripMode::Drive => Some(
                DrivingGoal::ParkNear(b)
                    .goal_pos(PathConstraints::Car, map)
                    .unwrap(),
            ),
        },
        TripEndpoint::Border(i, _) => match mode {
            TripMode::Walk | TripMode::Transit => if from {
                SidewalkSpot::start_at_border(i, None, map)
            } else {
                SidewalkSpot::end_at_border(i, None, map)
            }
            .map(|spot| spot.sidewalk_pos),
            TripMode::Bike | TripMode::Drive => (if from {
                map.get_i(i).some_outgoing_road(map)
            } else {
                map.get_i(i).some_incoming_road(map)
            })
            .and_then(|dr| {
                dr.lanes(
                    if mode == TripMode::Bike {
                        PathConstraints::Bike
                    } else {
                        PathConstraints::Car
                    },
                    map,
                )
                .get(0)
                .map(|l| Position::start(*l))
            }),
        },
    }
}

pub fn spawn_agents_around(i: IntersectionID, app: &mut App) {
    let map = &app.primary.map;
    let sim = &mut app.primary.sim;
    let mut rng = app.primary.current_flags.sim_flags.make_rng();
    let mut spawner = sim.make_spawner();

    if map.all_buildings().is_empty() {
        println!("No buildings, can't pick destinations");
        return;
    }

    let mut timer = Timer::new(format!(
        "spawning agents around {} (rng seed {:?})",
        i, app.primary.current_flags.sim_flags.rng_seed
    ));

    let now = sim.time();
    for l in &map.get_i(i).incoming_lanes {
        let lane = map.get_l(*l);
        if lane.is_driving() || lane.is_biking() {
            for _ in 0..10 {
                let vehicle_spec = if rng.gen_bool(0.7) && lane.is_driving() {
                    Scenario::rand_car(&mut rng)
                } else {
                    Scenario::rand_bike(&mut rng)
                };
                if vehicle_spec.length > lane.length() {
                    continue;
                }
                let person = sim.random_person(
                    Scenario::rand_ped_speed(&mut rng),
                    vec![vehicle_spec.clone()],
                );
                spawner.schedule_trip(
                    person,
                    now,
                    TripSpec::VehicleAppearing {
                        start_pos: Position::new(
                            lane.id,
                            Scenario::rand_dist(&mut rng, vehicle_spec.length, lane.length()),
                        ),
                        goal: DrivingGoal::ParkNear(
                            map.all_buildings().choose(&mut rng).unwrap().id,
                        ),
                        use_vehicle: person.vehicles[0].id,
                        retry_if_no_room: false,
                        origin: None,
                    },
                    TripEndpoint::Border(lane.src_i, None),
                    false,
                    false,
                    map,
                );
            }
        } else if lane.is_walkable() {
            for _ in 0..5 {
                spawner.schedule_trip(
                    sim.random_person(Scenario::rand_ped_speed(&mut rng), Vec::new()),
                    now,
                    TripSpec::JustWalking {
                        start: SidewalkSpot::suddenly_appear(
                            lane.id,
                            Scenario::rand_dist(&mut rng, 0.1 * lane.length(), 0.9 * lane.length()),
                            map,
                        ),
                        goal: SidewalkSpot::building(
                            map.all_buildings().choose(&mut rng).unwrap().id,
                            map,
                        ),
                    },
                    TripEndpoint::Border(lane.src_i, None),
                    false,
                    false,
                    map,
                );
            }
        }
    }

    sim.flush_spawner(spawner, map, &mut timer);
    sim.tiny_step(map, &mut app.primary.sim_cb);
}

pub fn actions(_: &App, id: ID) -> Vec<(Key, String)> {
    match id {
        ID::Building(_) => vec![(Key::Z, "start a trip here".to_string())],
        ID::Intersection(_) => vec![(Key::Z, "spawn agents here".to_string())],
        _ => Vec::new(),
    }
}

pub fn execute(ctx: &mut EventCtx, app: &mut App, id: ID, action: String) -> Transition {
    match (id, action.as_ref()) {
        (ID::Building(b), "start a trip here") => Transition::Push(AgentSpawner::new(ctx, Some(b))),
        (ID::Intersection(id), "spawn agents here") => {
            spawn_agents_around(id, app);
            Transition::Keep
        }
        _ => unreachable!(),
    }
}
