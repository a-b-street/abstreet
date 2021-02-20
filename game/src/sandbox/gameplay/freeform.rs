use rand::seq::SliceRandom;
use rand::Rng;

use abstutil::Timer;
use geom::{Distance, Duration, Polygon};
use map_gui::tools::{
    grey_out_map, nice_map_name, open_browser, CityPicker, PopupMsg, PromptInput,
};
use map_gui::ID;
use map_model::{BuildingID, IntersectionID, Position, NORMAL_LANE_THICKNESS};
use sim::{IndividTrip, PersonSpec, Scenario, TripEndpoint, TripMode, TripPurpose};
use widgetry::{
    lctrl, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    SimpleState, Spinner, State, StyledButtons, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::{jump_to_time_upon_startup, update_url_free_param, CommonState};
use crate::edit::EditMode;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{Actions, SandboxControls, SandboxMode};

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    top_center: Panel,
}

impl Freeform {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn GameplayState> {
        if let Err(err) = update_url_free_param(
            app.primary
                .map
                .get_name()
                .path()
                .strip_prefix(&abstio::path(""))
                .unwrap()
                .to_string(),
        ) {
            warn!("Couldn't update URL: {}", err);
        }

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
                    Box::new(|_, app| {
                        let sandbox = if app.opts.dev {
                            SandboxMode::async_new(
                                app,
                                GameplayMode::Freeform(app.primary.map.get_name().clone()),
                                jump_to_time_upon_startup(Duration::hours(6)),
                            )
                        } else {
                            SandboxMode::simple_new(
                                app,
                                GameplayMode::Freeform(app.primary.map.get_name().clone()),
                            )
                        };
                        Transition::Multi(vec![Transition::Pop, Transition::Replace(sandbox)])
                    }),
                ))),
                "change scenario" => Some(Transition::Push(ChangeScenario::new(ctx, app, "none"))),
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
                        if abstio::file_exists(abstio::path_scenario(
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
                ctx.style()
                    .btn_light_popup_icon_text(
                        "system/assets/tools/map.svg",
                        nice_map_name(app.primary.map.get_name()),
                    )
                    .hotkey(lctrl(Key::L))
                    .build_widget(ctx, "change map"),
                ctx.style()
                    .btn_light_popup_icon_text("system/assets/tools/calendar.svg", "none")
                    .hotkey(Key::S)
                    .build_widget(ctx, "change scenario"),
                ctx.style()
                    .btn_outline_light_icon_text("system/assets/tools/pencil.svg", "Edit map")
                    .hotkey(lctrl(Key::E))
                    .build_widget(ctx, "edit map"),
            ])
            .centered(),
            Widget::row(vec![
                ctx.style()
                    .btn_outline_light_text("Start a new trip")
                    .build_def(ctx),
                ctx.style()
                    .btn_outline_light_text("Record trips as a scenario")
                    .build_def(ctx),
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

pub struct ChangeScenario;

impl ChangeScenario {
    pub fn new(ctx: &mut EventCtx, app: &App, current_scenario: &str) -> Box<dyn State<App>> {
        // (Button action, label, full description)
        let mut choices = Vec::new();
        for name in abstio::list_all_objects(abstio::path_all_scenarios(app.primary.map.get_name()))
        {
            if name == "weekday" {
                choices.push((
                    name,
                    "realistic weekday traffic".to_string(),
                    "Trips will begin throughout the entire day. Midnight is usually quiet, so \
                     you may need to fast-forward to morning rush hour. Data comes from Puget \
                     Sound Regional Council's Soundcast model from 2014.",
                ));
            } else {
                choices.push((
                    name.clone(),
                    name,
                    "This is custom scenario data for this map",
                ));
            }
        }
        choices.push((
            "home_to_work".to_string(),
            "trips between home and work".to_string(),
            "Randomized people will leave homes in the morning, go to work, then return in the \
             afternoon. It'll be very quiet before 7am and between 10am to 5pm. The population \
             size and location of homes and workplaces is all guessed just from OpenStreetMap \
             tags.",
        ));
        choices.push((
            "random".to_string(),
            "random unrealistic trips".to_string(),
            "A fixed number of trips will start at midnight, but not constantly appear through \
             the day.",
        ));
        choices.push((
            "census".to_string(),
            "generate from US census data".to_string(),
            "A population from 2010 US census data will travel between home and workplaces. This \
             option will only work for maps in the US, and generating it will take a few moments \
             as some data is downloaded for this map.",
        ));
        choices.push((
            "none".to_string(),
            "none, except for buses".to_string(),
            "You can manually spawn traffic around a single intersection or by using the tool in \
             the top panel to start individual trips.",
        ));

        let mut col = vec![
            Widget::row(vec![
                Line("Pick your scenario").small_heading().draw(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Line("Each scenario determines what people live and travel around this map").draw(ctx),
        ];
        for (name, label, description) in choices {
            let btn = ctx
                .style()
                .btn_solid_dark_text(&label)
                .disabled(name == current_scenario);
            col.push(
                Widget::row(vec![
                    btn.build_widget(ctx, &name),
                    Text::from(Line(description).secondary())
                        .wrap_to_pct(ctx, 40)
                        .draw(ctx)
                        .align_right(),
                ])
                .margin_above(30),
            );
        }
        col.push(
            ctx.style()
                .btn_outline_light_text("Import your own data")
                .build_def(ctx),
        );

        SimpleState::new(
            Panel::new(Widget::col(col)).build(ctx),
            Box::new(ChangeScenario),
        )
    }
}

impl SimpleState<App> for ChangeScenario {
    fn on_click(&mut self, _: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        if x == "close" {
            Transition::Pop
        } else if x == "Import your own data" {
            open_browser(
                "https://a-b-street.github.io/docs/trafficsim/travel_demand.html#custom-import",
            );
            Transition::Keep
        } else {
            Transition::Multi(vec![
                Transition::Pop,
                Transition::Replace(SandboxMode::simple_new(
                    app,
                    if x == "none" {
                        GameplayMode::Freeform(app.primary.map.get_name().clone())
                    } else {
                        GameplayMode::PlayScenario(
                            app.primary.map.get_name().clone(),
                            x.to_string(),
                            Vec::new(),
                        )
                    },
                )),
            ])
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }
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
                    ctx.style().btn_close_widget(ctx),
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
                ctx.style()
                    .btn_outline_light_text("Confirm")
                    .disabled(true)
                    .build_def(ctx),
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
                        self.panel.replace(
                            ctx,
                            "Confirm",
                            ctx.style()
                                .btn_outline_light_text("Confirm")
                                .disabled(true)
                                .build_def(ctx),
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
                        ctx.style()
                            .btn_outline_light_text("Confirm")
                            .hotkey(Key::Enter)
                            .build_def(ctx),
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
