use rand::seq::SliceRandom;
use rand::Rng;

use abstutil::Timer;
use geom::{Distance, Polygon};
use map_gui::tools::{nice_map_name, ChooseSomething, CityPicker, PopupMsg, PromptInput};
use map_gui::ID;
use map_model::{BuildingID, IntersectionID, Position, NORMAL_LANE_THICKNESS};
use sim::{IndividTrip, PersonSpec, Scenario, TripEndpoint, TripMode, TripPurpose};
use widgetry::{
    lctrl, Btn, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    ScreenRectangle, Spinner, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::CommonState;
use crate::edit::EditMode;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{Actions, SandboxControls, SandboxMode};

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    top_center: Panel,
}

impl Freeform {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn GameplayState> {
        Box::new(Freeform {
            top_center: Panel::empty(ctx),
        })
    }
}

impl GameplayState for Freeform {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
        _: &mut Actions,
    ) -> Option<Transition> {
        match self.top_center.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "change map" => Some(Transition::Push(CityPicker::new(
                    ctx,
                    app,
                    Box::new(|ctx, app| {
                        Transition::Multi(vec![
                            Transition::Pop,
                            Transition::Replace(SandboxMode::simple_new(
                                ctx,
                                app,
                                GameplayMode::Freeform(app.primary.map.get_name().clone()),
                            )),
                        ])
                    }),
                ))),
                "change traffic" => Some(Transition::Push(make_change_traffic(
                    ctx,
                    app,
                    self.top_center.rect_of("change traffic").clone(),
                    "none".to_string(),
                ))),
                "edit map" => Some(Transition::Push(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::Freeform(app.primary.map.get_name().clone()),
                ))),
                "Start a new trip" => Some(Transition::Push(AgentSpawner::new(ctx, None))),
                "Record trips as a scenario" => Some(Transition::Push(PromptInput::new(
                    ctx,
                    "Name this scenario",
                    Box::new(|name, ctx, app| {
                        if abstutil::file_exists(abstutil::path_scenario(
                            app.primary.map.get_name(),
                            &name,
                        )) {
                            Transition::Push(PopupMsg::new(
                                ctx,
                                "Error",
                                vec![format!(
                                    "A scenario called \"{}\" already exists, please pick another \
                                     name",
                                    name
                                )],
                            ))
                        } else {
                            app.primary
                                .sim
                                .generate_scenario(&app.primary.map, name)
                                .save();
                            Transition::Pop
                        }
                    }),
                ))),
                _ => unreachable!(),
            },
            _ => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }

    fn recreate_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        let rows = vec![
            Widget::row(vec![
                Line("Sandbox").small_heading().draw(ctx),
                Widget::vert_separator(ctx, 50.0),
                "Map:".draw_text(ctx),
                Btn::pop_up(ctx, Some(nice_map_name(app.primary.map.get_name()))).build(
                    ctx,
                    "change map",
                    lctrl(Key::L),
                ),
                "Traffic:".draw_text(ctx),
                Btn::pop_up(ctx, Some("none")).build(ctx, "change traffic", Key::S),
                Btn::svg_def("system/assets/tools/edit_map.svg").build(
                    ctx,
                    "edit map",
                    lctrl(Key::E),
                ),
            ])
            .centered(),
            Widget::row(vec![
                Btn::text_fg("Start a new trip").build_def(ctx, None),
                Btn::text_fg("Record trips as a scenario").build_def(ctx, None),
            ])
            .centered(),
            Text::from_all(vec![
                Line("Select an intersection and press "),
                Key::Z.txt(ctx),
                Line(" to start traffic nearby"),
            ])
            .draw(ctx),
        ];

        self.top_center = Panel::new(Widget::col(rows))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);
    }
}

pub fn make_change_traffic(
    ctx: &mut EventCtx,
    app: &App,
    btn: ScreenRectangle,
    current: String,
) -> Box<dyn State<App>> {
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
        "generate from census data",
        "census".to_string(),
    ));
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
            Transition::Multi(vec![
                Transition::Pop,
                Transition::Replace(SandboxMode::simple_new(
                    ctx,
                    app,
                    if scenario_name == "none" {
                        GameplayMode::Freeform(app.primary.map.get_name().clone())
                    } else {
                        GameplayMode::PlayScenario(
                            app.primary.map.get_name().clone(),
                            scenario_name,
                            Vec::new(),
                        )
                    },
                )),
            ])
        }),
    )
}

struct AgentSpawner {
    panel: Panel,
    source: Option<TripEndpoint>,
    goal: Option<(TripEndpoint, Option<Polygon>)>,
    confirmed: bool,
}

impl AgentSpawner {
    fn new(ctx: &mut EventCtx, start: Option<BuildingID>) -> Box<dyn State<App>> {
        let mut spawner = AgentSpawner {
            source: None,
            goal: None,
            confirmed: false,
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("New trip").small_heading().draw(ctx),
                    Btn::close(ctx),
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
                Btn::text_fg("Confirm").inactive(ctx),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        };
        if let Some(b) = start {
            spawner.source = Some(TripEndpoint::Bldg(b));
            spawner.panel.replace(
                ctx,
                "instructions",
                "Click a building or border to specify end".draw_text(ctx),
            );
        }
        Box::new(spawner)
    }
}

impl State<App> for AgentSpawner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Confirm" => {
                    let map = &app.primary.map;
                    let mut scenario = Scenario::empty(map, "one-shot");
                    let from = self.source.take().unwrap();
                    let to = self.goal.take().unwrap().0;
                    for _ in 0..self.panel.spinner("number") as usize {
                        scenario.people.push(PersonSpec {
                            orig_id: None,
                            origin: from.clone(),
                            trips: vec![IndividTrip::new(
                                app.primary.sim.time(),
                                TripPurpose::Shopping,
                                to.clone(),
                                self.panel.dropdown_value("mode"),
                            )],
                        });
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
                    if let Some(path) = TripEndpoint::path_req(
                        self.source.clone().unwrap(),
                        to.clone(),
                        self.panel.dropdown_value("mode"),
                        &app.primary.map,
                    )
                    .and_then(|req| app.primary.map.pathfind(req).ok())
                    {
                        self.goal = Some((
                            to,
                            path.trace(&app.primary.map)
                                .map(|pl| pl.make_polygons(NORMAL_LANE_THICKNESS)),
                        ));
                    } else {
                        self.goal = None;
                        self.confirmed = false;
                        self.panel.replace(
                            ctx,
                            "instructions",
                            "Click a building or border to specify end".draw_text(ctx),
                        );
                        self.panel
                            .replace(ctx, "Confirm", Btn::text_fg("Confirm").inactive(ctx));
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
            app.primary.current_selection = app.mouseover_unzoomed_everything(ctx);
            if match app.primary.current_selection {
                Some(ID::Intersection(i)) => !app.primary.map.get_i(i).is_border(),
                Some(ID::Building(_)) => false,
                _ => true,
            } {
                app.primary.current_selection = None;
            }
        }
        if let Some(hovering) = match app.primary.current_selection {
            Some(ID::Intersection(i)) => Some(TripEndpoint::Border(i)),
            Some(ID::Building(b)) => Some(TripEndpoint::Bldg(b)),
            None => None,
            _ => unreachable!(),
        } {
            if self.source.is_none() && app.per_obj.left_click(ctx, "start here") {
                self.source = Some(hovering);
                self.panel.replace(
                    ctx,
                    "instructions",
                    "Click a building or border to specify end".draw_text(ctx),
                );
            } else if self.source.is_some() && self.source != Some(hovering.clone()) {
                if self
                    .goal
                    .as_ref()
                    .map(|(to, _)| to != &hovering)
                    .unwrap_or(true)
                {
                    if let Some(path) = TripEndpoint::path_req(
                        self.source.clone().unwrap(),
                        hovering.clone(),
                        self.panel.dropdown_value("mode"),
                        &app.primary.map,
                    )
                    .and_then(|req| app.primary.map.pathfind(req).ok())
                    {
                        self.goal = Some((
                            hovering,
                            path.trace(&app.primary.map)
                                .map(|pl| pl.make_polygons(NORMAL_LANE_THICKNESS)),
                        ));
                    } else {
                        self.goal = None;
                    }
                }

                if self.goal.is_some() && app.per_obj.left_click(ctx, "end here") {
                    app.primary.current_selection = None;
                    self.confirmed = true;
                    self.panel.replace(
                        ctx,
                        "instructions",
                        "Confirm the trip settings".draw_text(ctx),
                    );
                    self.panel.replace(
                        ctx,
                        "Confirm",
                        Btn::text_fg("Confirm").build_def(ctx, Key::Enter),
                    );
                }
            }
        } else {
            self.goal = None;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);

        if let Some(ref endpt) = self.source {
            g.draw_polygon(
                Color::BLUE.alpha(0.8),
                match endpt {
                    TripEndpoint::Border(i) => app.primary.map.get_i(*i).polygon.clone(),
                    TripEndpoint::Bldg(b) => app.primary.map.get_b(*b).polygon.clone(),
                    TripEndpoint::SuddenlyAppear(_) => unreachable!(),
                },
            );
        }
        if let Some((ref endpt, ref poly)) = self.goal {
            g.draw_polygon(
                Color::GREEN.alpha(0.8),
                match endpt {
                    TripEndpoint::Border(i) => app.primary.map.get_i(*i).polygon.clone(),
                    TripEndpoint::Bldg(b) => app.primary.map.get_b(*b).polygon.clone(),
                    TripEndpoint::SuddenlyAppear(_) => unreachable!(),
                },
            );
            if let Some(p) = poly {
                g.draw_polygon(Color::PURPLE, p.clone());
            }
        }
    }
}

pub fn spawn_agents_around(i: IntersectionID, app: &mut App) {
    let map = &app.primary.map;
    let mut rng = app.primary.current_flags.sim_flags.make_rng();
    let mut scenario = Scenario::empty(map, "one-shot");

    if map.all_buildings().is_empty() {
        println!("No buildings, can't pick destinations");
        return;
    }

    let mut timer = Timer::new(format!(
        "spawning agents around {} (rng seed {:?})",
        i, app.primary.current_flags.sim_flags.rng_seed
    ));

    for l in &map.get_i(i).incoming_lanes {
        let lane = map.get_l(*l);
        if lane.is_driving() || lane.is_biking() {
            for _ in 0..10 {
                let mode = if rng.gen_bool(0.7) && lane.is_driving() {
                    TripMode::Drive
                } else {
                    TripMode::Bike
                };
                scenario.people.push(PersonSpec {
                    orig_id: None,
                    origin: TripEndpoint::SuddenlyAppear(Position::new(
                        lane.id,
                        Scenario::rand_dist(&mut rng, Distance::ZERO, lane.length()),
                    )),
                    trips: vec![IndividTrip::new(
                        app.primary.sim.time(),
                        TripPurpose::Shopping,
                        TripEndpoint::Bldg(map.all_buildings().choose(&mut rng).unwrap().id),
                        mode,
                    )],
                });
            }
        } else if lane.is_walkable() {
            for _ in 0..5 {
                scenario.people.push(PersonSpec {
                    orig_id: None,
                    origin: TripEndpoint::SuddenlyAppear(Position::new(
                        lane.id,
                        Scenario::rand_dist(&mut rng, 0.1 * lane.length(), 0.9 * lane.length()),
                    )),
                    trips: vec![IndividTrip::new(
                        app.primary.sim.time(),
                        TripPurpose::Shopping,
                        TripEndpoint::Bldg(map.all_buildings().choose(&mut rng).unwrap().id),
                        TripMode::Walk,
                    )],
                });
            }
        }
    }

    let retry_if_no_room = false;
    scenario.instantiate_without_retries(
        &mut app.primary.sim,
        map,
        &mut rng,
        retry_if_no_room,
        &mut timer,
    );
    app.primary.sim.tiny_step(map, &mut app.primary.sim_cb);
}

pub fn actions(_: &App, id: ID) -> Vec<(Key, String)> {
    match id {
        ID::Building(_) => vec![(Key::Z, "start a trip here".to_string())],
        ID::Intersection(_) => vec![(Key::Z, "spawn agents here".to_string())],
        _ => Vec::new(),
    }
}

pub fn execute(ctx: &mut EventCtx, app: &mut App, id: ID, action: &str) -> Transition {
    match (id, action.as_ref()) {
        (ID::Building(b), "start a trip here") => Transition::Push(AgentSpawner::new(ctx, Some(b))),
        (ID::Intersection(id), "spawn agents here") => {
            spawn_agents_around(id, app);
            Transition::Keep
        }
        _ => unreachable!(),
    }
}
