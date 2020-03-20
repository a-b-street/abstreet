use crate::app::App;
use crate::colors;
use crate::common::{tool_panel, Minimap, Warping};
use crate::edit::EditMode;
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::managed::WrappedComposite;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{
    spawn_agents_around, AgentMeter, SandboxControls, SandboxMode, ScoreCard, SpeedControls,
    TimePanel,
};
use abstutil::Timer;
use ezgui::{
    hotkey, hotkeys, lctrl, Btn, Button, Color, Composite, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, RewriteColor, ScreenPt, Text, TextExt,
    VerticalAlignment,
};
use geom::{Distance, Duration, PolyLine, Polygon, Pt2D, Statistic, Time};
use map_model::{BuildingID, IntersectionID, IntersectionType, LaneType, Map, RoadID};
use sim::{
    AgentID, Analytics, BorderSpawnOverTime, CarID, OriginDestination, Scenario, VehicleType,
};
use std::collections::BTreeSet;

pub struct Tutorial {
    top_center: Composite,
    last_finished_task: Task,

    msg_panel: Option<Composite>,
    warped: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub struct TutorialPointer {
    pub stage: usize,
    // Index into messages. messages.len() means the actual task.
    pub part: usize,
}

impl TutorialPointer {
    pub fn new(stage: usize, part: usize) -> TutorialPointer {
        TutorialPointer { stage, part }
    }

    fn max(self, other: TutorialPointer) -> TutorialPointer {
        if self.stage > other.stage {
            self
        } else if other.stage > self.stage {
            other
        } else {
            TutorialPointer::new(self.stage, self.part.max(other.part))
        }
    }
}

impl Tutorial {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        current: TutorialPointer,
    ) -> Box<dyn GameplayState> {
        if app.session.tutorial.is_none() {
            app.session.tutorial = Some(TutorialState::new(ctx, app));
        }
        let mut tut = app.session.tutorial.take().unwrap();
        tut.current = current;
        tut.latest = tut.latest.max(current);
        let state = tut.make_state(ctx, app);
        app.session.tutorial = Some(tut);
        state
    }
}

impl GameplayState for Tutorial {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        let mut tut = app.session.tutorial.as_mut().unwrap();

        // First of all, might need to initiate warping
        if !self.warped {
            if let Some((ref id, zoom)) = tut.stage().warp_to {
                self.warped = true;
                return (
                    Some(Transition::Push(Warping::new(
                        ctx,
                        id.canonical_point(&app.primary).unwrap(),
                        Some(zoom),
                        None,
                        &mut app.primary,
                    ))),
                    false,
                );
            }
        }

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit" => {
                    return (None, true);
                }
                "previous tutorial" => {
                    tut.current = TutorialPointer::new(tut.current.stage - 1, 0);
                    return (Some(transition(ctx, app)), false);
                }
                "next tutorial" => {
                    tut.current = TutorialPointer::new(tut.current.stage + 1, 0);
                    return (Some(transition(ctx, app)), false);
                }
                "instructions" => {
                    tut.current = TutorialPointer::new(tut.current.stage, 0);
                    return (Some(transition(ctx, app)), false);
                }
                "edit map" => {
                    // TODO Ideally this would be an inactive button in message states
                    if self.msg_panel.is_none() {
                        let mode = GameplayMode::Tutorial(tut.current);
                        return (
                            Some(Transition::Push(Box::new(EditMode::new(ctx, app, mode)))),
                            false,
                        );
                    }
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if let Some(ref mut msg) = self.msg_panel {
            match msg.event(ctx) {
                Some(Outcome::Clicked(x)) => match x.as_ref() {
                    "previous message" => {
                        tut.prev();
                        return (Some(transition(ctx, app)), false);
                    }
                    "next message" | "Try it" => {
                        tut.next();
                        return (Some(transition(ctx, app)), false);
                    }
                    _ => unreachable!(),
                },
                None => {
                    // Don't allow other interactions
                    return (Some(Transition::Keep), false);
                }
            }
        }

        // Interaction things
        if tut.interaction() == Task::Camera {
            if app.primary.current_selection == Some(ID::Building(BuildingID(9)))
                && app.per_obj.left_click(ctx, "put out the... fire?")
            {
                tut.next();
                return (Some(transition(ctx, app)), false);
            }
        } else if tut.interaction() == Task::InspectObjects {
            match app.primary.current_selection {
                Some(ID::Lane(l)) => {
                    if app.per_obj.action(ctx, Key::I, "inspect the lane") {
                        tut.inspected_lane = true;
                        self.top_center = tut.make_top_center(ctx, false);
                        return (
                            Some(Transition::Push(msg(
                                "Inspection",
                                match app.primary.map.get_l(l).lane_type {
                                    LaneType::Driving => vec![
                                        "This is a regular lane for driving.",
                                        "Cars, bikes, and buses all share it.",
                                    ],
                                    LaneType::Parking => vec!["This is an on-street parking lane."],
                                    LaneType::Sidewalk => {
                                        vec!["This is a sidewalk. Only pedestrians can use it."]
                                    }
                                    LaneType::Biking => vec!["This is a bike-only lane."],
                                    LaneType::Bus => {
                                        vec!["This is a bus lane. Bikes may also use it."]
                                    }
                                    LaneType::SharedLeftTurn => vec![
                                        "This is a lane where either direction of traffic can \
                                         turn left.",
                                    ],
                                    LaneType::Construction => {
                                        vec!["This lane is currently closed for construction."]
                                    }
                                },
                            ))),
                            false,
                        );
                    }
                }
                Some(ID::Building(_)) => {
                    if app.per_obj.action(ctx, Key::I, "inspect the building") {
                        tut.inspected_building = true;
                        self.top_center = tut.make_top_center(ctx, false);
                        return (
                            Some(Transition::Push(msg(
                                "Inspection",
                                vec![
                                    "Knock knock, anyone home?",
                                    "Did you know: most trips begin and end at a building.",
                                ],
                            ))),
                            false,
                        );
                    }
                }
                Some(ID::Intersection(i)) => {
                    if app.per_obj.action(ctx, Key::I, "inspect the intersection") {
                        match app.primary.map.get_i(i).intersection_type {
                            IntersectionType::StopSign => {
                                tut.inspected_stop_sign = true;
                                self.top_center = tut.make_top_center(ctx, false);
                                return (
                                    Some(Transition::Push(msg(
                                        "Inspection",
                                        vec!["Most intersections are regulated by stop signs."],
                                    ))),
                                    false,
                                );
                            }
                            IntersectionType::TrafficSignal => {
                                return (
                                    Some(Transition::Push(msg(
                                        "Inspection",
                                        vec![
                                            "This intersection is controlled by a traffic signal. \
                                             You'll learn more about these soon.",
                                        ],
                                    ))),
                                    false,
                                );
                            }
                            IntersectionType::Border => {
                                tut.inspected_border = true;
                                self.top_center = tut.make_top_center(ctx, false);
                                return (
                                    Some(Transition::Push(msg(
                                        "Inspection",
                                        vec![
                                            "This is a border of the map. Vehicles appear and \
                                             disappear here.",
                                        ],
                                    ))),
                                    false,
                                );
                            }
                            IntersectionType::Construction => {
                                return (
                                    Some(Transition::Push(msg(
                                        "Inspection",
                                        vec![
                                            "This intersection is currently closed for \
                                             construction.",
                                        ],
                                    ))),
                                    false,
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
            if tut.inspected_lane
                && tut.inspected_building
                && tut.inspected_stop_sign
                && tut.inspected_border
            {
                tut.next();
                return (Some(transition(ctx, app)), false);
            }
        } else if tut.interaction() == Task::TimeControls {
            if app.primary.sim.time() >= Time::START_OF_DAY + Duration::hours(17) {
                tut.next();
                return (Some(transition(ctx, app)), false);
            }
        } else if tut.interaction() == Task::PauseResume {
            let is_paused = controls.speed.as_ref().unwrap().is_paused();
            if tut.was_paused && !is_paused {
                tut.was_paused = false;
            }
            if !tut.was_paused && is_paused {
                tut.num_pauses += 1;
                tut.was_paused = true;
                self.top_center = tut.make_top_center(ctx, false);
            }
            if tut.num_pauses == 3 {
                tut.next();
                return (Some(transition(ctx, app)), false);
            }
        } else if tut.interaction() == Task::Escort {
            let target = CarID(30, VehicleType::Car);
            let is_parked = app
                .primary
                .sim
                .agent_to_trip(AgentID::Car(target))
                .is_none();
            if !tut.car_parked && is_parked && tut.following_car {
                tut.car_parked = true;
                self.top_center = tut.make_top_center(ctx, false);
            }

            if controls.common.as_ref().unwrap().info_panel_open() == Some(ID::Car(target)) {
                if !tut.following_car {
                    // TODO There's a delay of one event before the checklist updates, because the
                    // info panel opening happens at the end of the event. Not a big deal.
                    tut.following_car = true;
                    self.top_center = tut.make_top_center(ctx, false);
                }
            }

            if let Some(ID::Car(c)) = app.primary.current_selection {
                if app.per_obj.action(ctx, Key::C, "draw WASH ME") {
                    if c == target {
                        if is_parked {
                            tut.next();
                            return (Some(transition(ctx, app)), false);
                        } else {
                            return (
                                Some(Transition::Push(msg(
                                    "Not yet!",
                                    vec![
                                        "You're going to run up to an occupied car and draw on \
                                         their windows?",
                                        "Sounds like we should be friends.",
                                        "But, er, wait for the car to park. (You can speed up \
                                         time!)",
                                    ],
                                ))),
                                false,
                            );
                        }
                    } else if c.1 == VehicleType::Bike {
                        return (
                            Some(Transition::Push(msg(
                                "That's a bike",
                                vec![
                                    "Achievement unlocked: You attempted to draw WASH ME on a \
                                     cyclist.",
                                    "This game is PG-13 or something, so I can't really describe \
                                     what happens next.",
                                    "But uh, don't try this at home.",
                                ],
                            ))),
                            false,
                        );
                    } else {
                        return (
                            Some(Transition::Push(msg(
                                "Wrong car",
                                vec![
                                    "You're looking at the wrong car.",
                                    "Use the 'reset to midnight' (key binding 'X') to start over, \
                                     if you lost the car to follow.",
                                ],
                            ))),
                            false,
                        );
                    }
                }
            }
        } else if tut.interaction() == Task::LowParking {
            if let Some(ID::Lane(l)) = app.primary.current_selection {
                if app
                    .per_obj
                    .action(ctx, Key::C, "check the parking availability")
                {
                    let lane = app.primary.map.get_l(l);
                    if !lane.is_parking() {
                        return (
                            Some(Transition::Push(msg(
                                "Uhh..",
                                vec!["That's not even a parking lane"],
                            ))),
                            false,
                        );
                    }
                    let percent = (app.primary.sim.get_free_spots(l).len() as f64)
                        / (lane.number_parking_spots() as f64);
                    if percent > 0.1 {
                        return (
                            Some(Transition::Push(msg(
                                "Not quite",
                                vec![
                                    format!("This lane has {:.0}% spots free", percent * 100.0),
                                    "Try using the 'parking availability' layer from the minimap \
                                     controls"
                                        .to_string(),
                                ],
                            ))),
                            false,
                        );
                    }
                    tut.next();
                    return (Some(transition(ctx, app)), false);
                }
            }
        } else if tut.interaction() == Task::WatchBikes {
            if app.primary.sim.time() >= Time::START_OF_DAY + Duration::minutes(2) {
                tut.next();
                return (Some(transition(ctx, app)), false);
            }
        } else if tut.interaction() == Task::FixBikes {
            if app.primary.sim.is_done() {
                let (all, _, _) = app
                    .primary
                    .sim
                    .get_analytics()
                    .trip_times(app.primary.sim.time());
                let max = all.select(Statistic::Max);

                if !tut.score_delivered {
                    tut.score_delivered = true;
                    if app.primary.map.get_edits().commands.is_empty() {
                        return (
                            Some(Transition::Push(msg(
                                "All trips completed",
                                vec![
                                    "You didn't change anything!",
                                    "Try editing the map to create some bike lanes.",
                                ],
                            ))),
                            false,
                        );
                    }
                    // TODO Prebake results and use the normal differential stuff
                    let baseline = Duration::minutes(7) + Duration::seconds(15.0);
                    if max > baseline {
                        return (
                            Some(Transition::Push(msg(
                                "All trips completed",
                                vec![
                                    "Your changes made things worse!".to_string(),
                                    format!(
                                        "The slowest trip originally took {}, but now it took {}",
                                        baseline, max
                                    ),
                                    "".to_string(),
                                    "Try again!".to_string(),
                                ],
                            ))),
                            false,
                        );
                    }
                    // TODO Tune. The real solution doesn't work because of sim bugs.
                    if max > Duration::minutes(6) + Duration::seconds(40.0) {
                        return (
                            Some(Transition::Push(msg(
                                "All trips completed",
                                vec![
                                    "Nice, you helped things a bit!".to_string(),
                                    format!(
                                        "The slowest trip originally took {}, but now it took {}",
                                        baseline, max
                                    ),
                                    "".to_string(),
                                    "See if you can do a little better though.".to_string(),
                                ],
                            ))),
                            false,
                        );
                    }
                    return (
                        Some(Transition::Push(msg(
                            "All trips completed",
                            vec![format!(
                                "Awesome! The slowest trip originally took {}, but now it only \
                                 took {}",
                                baseline, max
                            )],
                        ))),
                        false,
                    );
                }
                if max <= Duration::minutes(6) + Duration::seconds(30.0) {
                    tut.next();
                }
                return (Some(transition(ctx, app)), false);
            }
        } else if tut.interaction() == Task::WatchBuses {
            if app.primary.sim.time() >= Time::START_OF_DAY + Duration::minutes(5) {
                tut.next();
                return (Some(transition(ctx, app)), false);
            }
        } else if tut.interaction() == Task::Done {
            // If the player chooses to stay here, at least go back to the message panel.
            tut.prev();
            return (None, true);
        }

        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let tut = app.session.tutorial.as_ref().unwrap();

        if self.msg_panel.is_some() {
            State::grey_out_map(g);
        }

        self.top_center.draw(g);

        if let Some(ref msg) = self.msg_panel {
            // Arrows underneath the message panel, but on top of other panels
            if let Some((_, Some(fxn))) = tut.lines() {
                let pt = (fxn)(g, app);
                g.fork_screenspace();
                g.draw_polygon(
                    Color::RED,
                    &PolyLine::new(vec![
                        self.msg_panel
                            .as_ref()
                            .unwrap()
                            .center_of("next message")
                            .to_pt(),
                        pt,
                    ])
                    .make_arrow(Distance::meters(20.0))
                    .unwrap(),
                );
                g.unfork();
            }

            msg.draw(g);
        }

        // Special things
        if tut.interaction() == Task::Camera {
            g.draw_polygon(
                Color::hex("#e25822"),
                &app.primary.map.get_b(BuildingID(9)).polygon,
            );
        }
    }

    fn can_move_canvas(&self) -> bool {
        self.msg_panel.is_none()
    }
    fn can_examine_objects(&self) -> bool {
        self.last_finished_task >= Task::WatchBikes
    }
    fn has_common(&self) -> bool {
        self.last_finished_task >= Task::Camera
    }
    fn has_tool_panel(&self) -> bool {
        self.last_finished_task >= Task::Camera
    }
    fn has_time_panel(&self) -> bool {
        self.last_finished_task >= Task::InspectObjects
    }
    fn has_speed(&self) -> bool {
        self.last_finished_task >= Task::InspectObjects
    }
    fn get_agent_meter_params(&self) -> Option<Option<ScoreCard>> {
        if self.last_finished_task >= Task::PauseResume {
            if self.last_finished_task == Task::WatchBikes {
                Some(Some(ScoreCard {
                    stat: Statistic::Max,
                    goal: Duration::seconds(45.0),
                }))
            } else {
                Some(None)
            }
        } else {
            None
        }
    }
    fn has_minimap(&self) -> bool {
        self.last_finished_task >= Task::Escort
    }
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
enum Task {
    Nil,
    Camera,
    InspectObjects,
    TimeControls,
    PauseResume,
    Escort,
    LowParking,
    WatchBikes,
    FixBikes,
    WatchBuses,
    FixBuses,
    Done,
}

impl Task {
    fn top_txt(self, ctx: &EventCtx, state: &TutorialState) -> Text {
        let simple = match self {
            Task::Nil => unreachable!(),
            Task::Camera => "Put out the fire at the Montlake Market",
            Task::InspectObjects => {
                let mut txt = Text::from(Line("Click and inspect one of each:"));
                if state.inspected_lane {
                    txt.add(Line("☑ lane").fg(Color::GREEN));
                } else {
                    txt.add(Line("☐ lane"));
                }
                if state.inspected_building {
                    txt.add(Line("☑ building").fg(Color::GREEN));
                } else {
                    txt.add(Line("☐ building"));
                }
                if state.inspected_stop_sign {
                    txt.add(Line("☑ intersection with stop sign").fg(Color::GREEN));
                } else {
                    txt.add(Line("☐ intersection with stop sign"));
                }
                if state.inspected_border {
                    txt.add(Line("☑ intersection on the map border").fg(Color::GREEN));
                } else {
                    txt.add(Line("☐ intersection on the map border"));
                }
                return txt;
            }
            Task::TimeControls => "Simulate until after 5pm",
            Task::PauseResume => {
                let mut txt = Text::from(Line("☐ Pause/resume "));
                txt.append(Line(format!("{} times", 3 - state.num_pauses)).fg(Color::GREEN));
                return txt;
            }
            Task::Escort => {
                // Inspect the target car, wait for them to park, draw WASH ME on the window
                let mut txt = Text::new();
                if state.following_car {
                    txt.add(Line("☑ follow the target car").fg(Color::GREEN));
                } else {
                    txt.add(Line("☐ follow the target car"));
                }
                if state.car_parked {
                    txt.add(Line("☑ wait for them to park").fg(Color::GREEN));
                } else {
                    txt.add(Line("☐ wait for them to park"));
                }
                if state.inspected_building {
                    txt.add(Line("☑ draw WASH ME on the window").fg(Color::GREEN));
                } else {
                    txt.add(Line("☐ draw WASH ME on the window"));
                }
                return txt;
            }
            Task::LowParking => "Find a road with almost no parking spots available",
            Task::WatchBikes => "Simulate 2 minutes",
            Task::FixBikes => "Speed up the slowest trip by 45s",
            Task::WatchBuses => "Simulate 5 minutes and watch the buses",
            Task::FixBuses => "Speed up bus 43 and 48",
            Task::Done => "Tutorial complete!",
        };

        let mut txt = Text::new();
        txt.add_wrapped(format!("☐ {}", simple), 0.6 * ctx.canvas.window_width);
        txt
    }

    fn label(self) -> &'static str {
        match self {
            Task::Nil => unreachable!(),
            Task::Camera => "Moving the camera",
            Task::InspectObjects => "Interacting with objects",
            Task::TimeControls => "Controlling time",
            Task::PauseResume => "Pausing/resuming",
            Task::Escort => "Following agents",
            Task::LowParking => "Using extra data layers",
            Task::WatchBikes => "Observing a problem",
            Task::FixBikes => "Editing lanes",
            Task::WatchBuses => "Observing buses",
            Task::FixBuses => "Speeding up buses",
            Task::Done => "Tutorial complete!",
        }
    }
}

struct Stage {
    messages: Vec<(
        Vec<&'static str>,
        Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>>,
    )>,
    task: Task,
    warp_to: Option<(ID, f64)>,
    spawn: Option<Box<dyn Fn(&mut App)>>,
}

fn arrow(pt: ScreenPt) -> Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>> {
    Some(Box::new(move |_, _| pt.to_pt()))
}

impl Stage {
    fn new(task: Task) -> Stage {
        Stage {
            messages: Vec::new(),
            task,
            warp_to: None,
            spawn: None,
        }
    }

    fn msg(
        mut self,
        lines: Vec<&'static str>,
        point_to: Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>>,
    ) -> Stage {
        self.messages.push((lines, point_to));
        self
    }

    fn warp_to(mut self, id: ID, zoom: Option<f64>) -> Stage {
        assert!(self.warp_to.is_none());
        self.warp_to = Some((id, zoom.unwrap_or(4.0)));
        self
    }

    fn spawn(mut self, cb: Box<dyn Fn(&mut App)>) -> Stage {
        assert!(self.spawn.is_none());
        self.spawn = Some(cb);
        self
    }

    fn spawn_around(self, i: IntersectionID) -> Stage {
        self.spawn(Box::new(move |app| spawn_agents_around(i, app)))
    }

    fn spawn_randomly(self) -> Stage {
        self.spawn(Box::new(|app| {
            Scenario::small_run(&app.primary.map).instantiate(
                &mut app.primary.sim,
                &app.primary.map,
                &mut app.primary.current_flags.sim_flags.make_rng(),
                &mut Timer::throwaway(),
            )
        }))
    }

    fn spawn_scenario(self, scenario: Scenario) -> Stage {
        self.spawn(Box::new(move |app| {
            let mut timer = Timer::new("spawn scenario with prebaked results");
            scenario.instantiate(
                &mut app.primary.sim,
                &app.primary.map,
                &mut app.primary.current_flags.sim_flags.make_rng(),
                &mut timer,
            );

            let prebaked: Analytics = abstutil::read_binary(
                abstutil::path_prebaked_results(&scenario.map_name, &scenario.scenario_name),
                &mut timer,
            );
            app.set_prebaked(Some((
                scenario.map_name.clone(),
                scenario.scenario_name.clone(),
                prebaked,
            )));
        }))
    }
}

pub struct TutorialState {
    stages: Vec<Stage>,
    latest: TutorialPointer,
    pub current: TutorialPointer,

    // Goofy state for just some stages.
    inspected_lane: bool,
    inspected_building: bool,
    inspected_stop_sign: bool,
    inspected_border: bool,

    was_paused: bool,
    num_pauses: usize,

    following_car: bool,
    car_parked: bool,

    score_delivered: bool,
}

fn make_bike_lane_scenario(map: &Map) -> Scenario {
    let mut s = Scenario::empty(map, "car vs bike contention");
    s.border_spawn_over_time.push(BorderSpawnOverTime {
        num_peds: 0,
        num_cars: 10,
        num_bikes: 10,
        percent_use_transit: 0.0,
        start_time: Time::START_OF_DAY,
        stop_time: Time::START_OF_DAY + Duration::seconds(10.0),
        start_from_border: RoadID(303).backwards(),
        goal: OriginDestination::GotoBldg(BuildingID(3)),
    });
    s
}

fn make_bus_lane_scenario(map: &Map) -> Scenario {
    let mut s = Scenario::empty(map, "car vs bus contention");
    let mut routes = BTreeSet::new();
    routes.insert("43".to_string());
    routes.insert("48".to_string());
    s.only_seed_buses = Some(routes);
    for src in vec![
        RoadID(61).backwards(),
        RoadID(240).forwards(),
        RoadID(56).forwards(),
    ] {
        s.border_spawn_over_time.push(BorderSpawnOverTime {
            num_peds: 100,
            num_cars: 100,
            num_bikes: 0,
            percent_use_transit: 1.0,
            start_time: Time::START_OF_DAY,
            stop_time: Time::START_OF_DAY + Duration::seconds(10.0),
            start_from_border: src,
            goal: OriginDestination::EndOfRoad(RoadID(0).forwards()),
        });
    }
    s
}

fn transition(ctx: &mut EventCtx, app: &mut App) -> Transition {
    let tut = app.session.tutorial.as_mut().unwrap();
    tut.reset_state();
    let mode = GameplayMode::Tutorial(tut.current);
    Transition::Replace(Box::new(SandboxMode::new(ctx, app, mode)))
}

impl TutorialState {
    // These're mutex to each state, but still important to reset. Otherwise if you go back to a
    // previous interaction stage, it'll just be automatically marked done.
    fn reset_state(&mut self) {
        self.inspected_lane = false;
        self.inspected_building = false;
        self.inspected_stop_sign = false;
        self.inspected_border = false;
        self.was_paused = true;
        self.num_pauses = 0;
        self.score_delivered = false;
        self.following_car = false;
        self.car_parked = false;
    }

    fn stage(&self) -> &Stage {
        &self.stages[self.current.stage]
    }

    fn interaction(&self) -> Task {
        let stage = self.stage();
        if self.current.part == stage.messages.len() {
            stage.task
        } else {
            Task::Nil
        }
    }
    fn lines(
        &self,
    ) -> Option<&(
        Vec<&'static str>,
        Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>>,
    )> {
        let stage = self.stage();
        if self.current.part == stage.messages.len() {
            None
        } else {
            Some(&stage.messages[self.current.part])
        }
    }

    fn next(&mut self) {
        self.current.part += 1;
        if self.current.part == self.stage().messages.len() + 1 {
            self.current = TutorialPointer::new(self.current.stage + 1, 0);
        }
        self.latest = self.latest.max(self.current);
    }
    fn prev(&mut self) {
        if self.current.part == 0 {
            self.current = TutorialPointer::new(
                self.current.stage - 1,
                self.stages[self.current.stage - 1].messages.len(),
            );
        } else {
            self.current.part -= 1;
        }
    }

    fn make_top_center(&self, ctx: &mut EventCtx, edit_map: bool) -> Composite {
        let mut col = vec![ManagedWidget::row(vec![
            Line("Tutorial").size(26).draw(ctx).margin(5),
            ManagedWidget::draw_batch(
                ctx,
                GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
            )
            .margin(5),
            if self.current.stage == 0 {
                Button::inactive_button(ctx, "<")
            } else {
                WrappedComposite::nice_text_button(
                    ctx,
                    Text::from(Line("<")),
                    None,
                    "previous tutorial",
                )
            }
            .margin(5),
            {
                let mut txt = Text::from(Line(format!("Task {}", self.current.stage + 1)));
                // TODO Smaller font and use alpha for the "/9" part
                txt.append(Line(format!("/{}", self.stages.len())).fg(Color::grey(0.7)));
                txt.draw(ctx).margin(5)
            },
            if self.current.stage == self.latest.stage {
                Button::inactive_button(ctx, ">")
            } else {
                WrappedComposite::nice_text_button(
                    ctx,
                    Text::from(Line(">")),
                    None,
                    "next tutorial",
                )
            }
            .margin(5),
            WrappedComposite::text_button(ctx, "Quit", None).margin(5),
        ])
        .centered()];
        {
            let task = self.interaction();
            if task != Task::Nil {
                col.push(ManagedWidget::row(vec![
                    Text::from(
                        Line(format!(
                            "Task {}: {}",
                            self.current.stage + 1,
                            self.stage().task.label()
                        ))
                        .roboto_bold(),
                    )
                    .draw(ctx),
                    // TODO also text saying "instructions"... can we layout two things easily to
                    // make a button?
                    Btn::svg_def("../data/system/assets/tools/info.svg")
                        .build(ctx, "instructions", None)
                        .centered_vert()
                        .align_right(),
                ]));
                col.push(task.top_txt(ctx, self).draw(ctx).margin(5));
            }
        }
        if edit_map {
            col.push(
                Btn::svg_def("../data/system/assets/tools/edit_map.svg")
                    .build(ctx, "edit map", lctrl(Key::E))
                    .margin(5),
            );
        }

        Composite::new(ManagedWidget::col(col).bg(colors::PANEL_BG))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx)
    }

    fn make_state(&self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn GameplayState> {
        if self.interaction() == Task::Nil {
            app.primary.current_selection = None;
        }

        // TODO Should some of this always happen?
        app.primary.clear_sim();
        if let Some(ref cb) = self.stage().spawn {
            let old = app.primary.current_flags.sim_flags.rng_seed;
            app.primary.current_flags.sim_flags.rng_seed = Some(42);
            (cb)(app);
            app.primary.current_flags.sim_flags.rng_seed = old;
            app.primary
                .sim
                .normal_step(&app.primary.map, Duration::seconds(0.1));
        }

        let last_finished_task = if self.current.stage == 0 {
            Task::Nil
        } else {
            self.stages[self.current.stage - 1].task
        };

        Box::new(Tutorial {
            top_center: self.make_top_center(ctx, last_finished_task >= Task::WatchBikes),
            last_finished_task,

            msg_panel: if let Some((ref lines, _)) = self.lines() {
                let mut col = vec![
                    {
                        let mut txt = Text::new();
                        txt.add(Line(self.stage().task.label()).roboto_bold());
                        txt.add(Line(""));

                        for l in lines {
                            txt.add(Line(*l));
                        }
                        txt.draw(ctx)
                    },
                    ManagedWidget::row(vec![
                        if self.current.part > 0 {
                            Btn::svg(
                                "../data/system/assets/tools/prev.svg",
                                RewriteColor::Change(Color::WHITE, colors::HOVERING),
                            )
                            .build(ctx, "previous message", hotkey(Key::LeftArrow))
                            .margin(5)
                        } else {
                            ManagedWidget::draw_svg_transform(
                                ctx,
                                "../data/system/assets/tools/prev.svg",
                                RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                            )
                        },
                        format!("{}/{}", self.current.part + 1, self.stage().messages.len())
                            .draw_text(ctx)
                            .centered_vert()
                            .margin(5),
                        if self.current.part == self.stage().messages.len() - 1 {
                            ManagedWidget::draw_svg_transform(
                                ctx,
                                "../data/system/assets/tools/next.svg",
                                RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                            )
                            .named("next message")
                        } else {
                            Btn::svg(
                                "../data/system/assets/tools/next.svg",
                                RewriteColor::Change(Color::WHITE, colors::HOVERING),
                            )
                            .build(
                                ctx,
                                "next message",
                                hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter]),
                            )
                        }
                        .margin(5),
                    ]),
                ];
                if self.current.part == self.stage().messages.len() - 1 {
                    col.push(WrappedComposite::text_bg_button(
                        ctx,
                        "Try it",
                        hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter]),
                    ));
                }

                Some(
                    Composite::new(
                        ManagedWidget::col(col)
                            .centered()
                            .bg(colors::PANEL_BG)
                            .outline(5.0, Color::WHITE)
                            .padding(5),
                    )
                    .build(ctx),
                )
            } else {
                None
            },
            warped: false,
        })
    }

    fn new(ctx: &mut EventCtx, app: &mut App) -> TutorialState {
        let mut state = TutorialState {
            stages: Vec::new(),
            latest: TutorialPointer::new(0, 0),
            current: TutorialPointer::new(0, 0),

            inspected_lane: false,
            inspected_building: false,
            inspected_stop_sign: false,
            inspected_border: false,
            was_paused: true,
            num_pauses: 0,
            following_car: false,
            car_parked: false,
            score_delivered: false,
        };

        let tool_panel = tool_panel(ctx);
        let time = TimePanel::new(ctx, app);
        let speed = SpeedControls::new(ctx);
        let agent_meter = AgentMeter::new(ctx, app, None);
        // The minimap is hidden at low zoom levels
        let orig_zoom = ctx.canvas.cam_zoom;
        ctx.canvas.cam_zoom = 100.0;
        let minimap = Minimap::new(ctx, app);
        ctx.canvas.cam_zoom = orig_zoom;

        let osd = ScreenPt::new(
            0.1 * ctx.canvas.window_width,
            0.97 * ctx.canvas.window_height,
        );

        state.stages.push(
            Stage::new(Task::Camera)
                .warp_to(ID::Intersection(IntersectionID(141)), None)
                .msg(
                    vec![
                        "Welcome to your first day as a contract traffic engineer --",
                        "like a paid assassin, but capable of making WAY more people cry.",
                        "Seattle is a fast-growing city, and nobody can decide how to fix the \
                         traffic.",
                    ],
                    None,
                )
                // TODO Point out settings and the alternative?
                .msg(
                    vec![
                        "Let's start with the controls.",
                        "Click and drag to pan around the map, and use your scroll",
                        "wheel or touchpad to zoom in and out.",
                    ],
                    None,
                )
                .msg(
                    vec![
                        "Let's try that ou--",
                        "WHOA THE MONTLAKE MARKET IS ON FIRE!",
                        "GO CLICK ON IT, QUICK!",
                    ],
                    None,
                )
                .msg(
                    vec!["(Hint: Look around for an unusually red building)"],
                    None,
                ),
        );

        state.stages.push(
            Stage::new(Task::InspectObjects)
                .msg(
                    vec![
                        "Er, sorry about that.",
                        "Just a little joke we like to play on the new recruits.",
                    ],
                    None,
                )
                .msg(
                    vec![
                        "If you're going to storm out of here, you can always go back towards the \
                         main screen using this button",
                        "(But please continue with the training.)",
                    ],
                    arrow(tool_panel.inner.center_of("back")),
                )
                .msg(
                    vec![
                        "Now, let's learn how to inspect and interact with objects in the map.",
                        "Select something with your mouse, then click on it.",
                    ],
                    None,
                )
                .msg(
                    vec![
                        "(By the way, the bottom of the screen shows keyboard shortcuts",
                        "for whatever you're selecting; you don't have to click an object first.)",
                    ],
                    arrow(osd),
                )
                .msg(
                    vec![
                        "I wonder what kind of information is available for different objects?",
                        "Let's find out! Click each object to open more details, then use the \
                         inspect action.",
                    ],
                    None,
                ),
        );

        state.stages.push(
            Stage::new(Task::TimeControls)
                .warp_to(ID::Intersection(IntersectionID(64)), None)
                .msg(
                    vec![
                        "Inspection complete!",
                        "",
                        "You'll work day and night, watching traffic patterns unfold.",
                    ],
                    arrow(time.composite.center_of_panel()),
                )
                .msg(
                    vec!["You can pause or resume time"],
                    arrow(speed.composite.inner.center_of("pause")),
                )
                .msg(
                    vec!["Speed things up"],
                    arrow(speed.composite.inner.center_of("30x speed")),
                )
                .msg(
                    vec!["Advance time by certain amounts"],
                    arrow(speed.composite.inner.center_of("step forwards 1 hour")),
                )
                .msg(
                    vec!["And reset to the beginning of the day"],
                    arrow(speed.composite.inner.center_of("reset to midnight")),
                )
                .msg(
                    vec!["Let's try these controls out. Run the simulation until 5pm or later."],
                    None,
                ),
        );

        state.stages.push(
            Stage::new(Task::PauseResume)
                .msg(
                    vec!["Whew, that took a while! (Hopefully not though...)"],
                    None,
                )
                .msg(
                    vec![
                        "You might've figured it out already,",
                        "But you'll be pausing/resuming time VERY frequently",
                    ],
                    arrow(speed.composite.inner.center_of("pause")),
                )
                .msg(
                    vec![
                        "Again, most controls have a key binding shown at the bottom of the \
                         screen.",
                        "Press SPACE to pause/resume time.",
                    ],
                    arrow(osd),
                )
                .msg(
                    vec!["Just reassure me and pause/resume time a few times, alright?"],
                    None,
                ),
        );

        state.stages.push(
            Stage::new(Task::Escort)
                // Don't center on where the agents are, be a little offset
                .warp_to(ID::Building(BuildingID(813)), Some(10.0))
                .spawn_around(IntersectionID(247))
                .msg(
                    vec!["Alright alright, no need to wear out your spacebar."],
                    None,
                )
                .msg(
                    vec![
                        "Oh look, some people appeared!",
                        "We've got pedestrians, bikes, and cars moving around now.",
                    ],
                    None,
                )
                .msg(
                    vec!["You can see the number of them here."],
                    arrow(agent_meter.composite.center_of_panel()),
                )
                .msg(
                    vec![
                        "Why don't you follow this car to their destination,",
                        "see where they park, and then play a little... prank?",
                    ],
                    Some(Box::new(|g, app| {
                        g.canvas
                            .map_to_screen(
                                app.primary
                                    .sim
                                    .canonical_pt_for_agent(
                                        AgentID::Car(CarID(30, VehicleType::Car)),
                                        &app.primary.map,
                                    )
                                    .unwrap(),
                            )
                            .to_pt()
                    })),
                )
                .msg(
                    vec![
                        "You don't have to manually chase them; just click to follow.",
                        "(If you do lose track of them, just reset)",
                    ],
                    arrow(speed.composite.inner.center_of("reset to midnight")),
                ),
        );

        state.stages.push(
            Stage::new(Task::LowParking)
                .spawn_randomly()
                .msg(
                    vec![
                        "What an immature prank. You should re-evaluate your life decisions.",
                        "",
                        "The map is quite large, so to help you orient",
                        "the minimap shows you an overview of all activity.",
                        "You can click and drag it just like the normal map.",
                    ],
                    arrow(minimap.composite.center_of("minimap")),
                )
                .msg(
                    vec!["Find addresses here"],
                    arrow(minimap.composite.center_of("search")),
                )
                .msg(
                    vec!["Set up shortcuts to favorite areas"],
                    arrow(minimap.composite.center_of("shortcuts")),
                )
                .msg(
                    vec![
                        "Apply different heatmap layers to the map, to find data such as:",
                        "- roads with high traffic",
                        "- bus stops",
                        "- current parking",
                    ],
                    arrow(minimap.composite.center_of("change overlay")),
                )
                .msg(
                    vec![
                        "Let's try these out.",
                        "There are lots of cars parked everywhere.",
                        "Can you find a road that's almost out of parking spots?",
                    ],
                    None,
                ),
        );

        let bike_lane_scenario = make_bike_lane_scenario(&app.primary.map);

        state.stages.push(
            Stage::new(Task::WatchBikes)
                .warp_to(ID::Building(BuildingID(543)), None)
                .spawn_scenario(bike_lane_scenario.clone())
                .msg(
                    vec![
                        "Well done!",
                        "",
                        "Let's see what's happening over here.",
                        "(Just watch for a moment at whatever speed you like.)",
                    ],
                    None,
                ),
        );

        let top_center = state.make_top_center(ctx, true);
        state.stages.push(
            Stage::new(Task::FixBikes)
                .spawn_scenario(bike_lane_scenario)
                .warp_to(ID::Building(BuildingID(543)), None)
                .msg(
                    vec![
                        "Looks like lots of cars and bikes trying to go to the playfield.",
                        "When lots of cars and bikes share the same lane,",
                        "cars are delayed (assuming there's no room to pass) and",
                        "the cyclist probably feels unsafe too.",
                    ],
                    None,
                )
                .msg(
                    vec![
                        "Luckily, you have the power to modify lanes!",
                        "What if you could transform the parking lanes that aren't being used much",
                        "into a protected bike lane?",
                    ],
                    None,
                )
                .msg(
                    vec!["To edit lanes, click 'edit map' and then select a lane."],
                    arrow(top_center.center_of("edit map")),
                )
                .msg(
                    vec![
                        "Some changes you make can't take effect until the next day;",
                        "like what if you removed a parking lane while there are cars on it?",
                        "So when you leave edit mode, the day will always reset to midnight.",
                        "",
                        "People are on fixed schedules: every day, everybody leaves at exactly \
                         the same time,",
                        "making the same decision to drive, walk, bike, or take a bus.",
                        "",
                        "All you can influence is how their experience will be in the short term.",
                    ],
                    None,
                )
                .msg(
                    vec![
                        "So adjust lanes and speed up the slowest trip by at least 45s.",
                        "When all trips are done, you'll get your final score.",
                    ],
                    arrow(agent_meter.composite.center_of_panel()),
                ),
        );

        if false {
            let bus_lane_scenario = make_bus_lane_scenario(&app.primary.map);
            // TODO There's no clear measurement for how well the buses are doing.
            // TODO Probably want a steady stream of the cars appearing

            state.stages.push(
                Stage::new(Task::WatchBuses)
                    .warp_to(ID::Building(BuildingID(1979)), Some(0.5))
                    .spawn_scenario(bus_lane_scenario.clone())
                    .msg(
                        vec![
                            "Alright, now it's a game day at the University of Washington.",
                            "Everyone's heading north across the bridge.",
                            "Watch what happens to the bus 43 and 48.",
                        ],
                        None,
                    ),
            );

            state.stages.push(
                Stage::new(Task::FixBuses)
                    .warp_to(ID::Building(BuildingID(1979)), Some(0.5))
                    .spawn_scenario(bus_lane_scenario.clone())
                    .msg(
                        vec!["Let's speed up the poor bus! Why not dedicate some bus lanes to it?"],
                        None,
                    ),
            );
        }

        state.stages.push(Stage::new(Task::Done).msg(
            vec![
                "Training complete!",
                "Use sandbox mode to explore larger areas of Seattle and try out any ideas you \
                 have.",
                "Or try your skills at a particular challenge!",
                "",
                "Go have the appropriate amount of fun.",
            ],
            None,
        ));

        // For my debugging sanity
        if app.opts.dev {
            state.latest = TutorialPointer::new(
                state.stages.len() - 1,
                state.stages.last().as_ref().unwrap().messages.len(),
            );
        }

        state

        // TODO Multi-modal trips -- including parking. (Cars per bldg, ownership)
        // TODO Explain the finished trip data
        // The city is in total crisis. You've only got 10 days to do something before all hell
        // breaks loose and people start kayaking / ziplining / crab-walking / cartwheeling / to
        // work.
    }

    // TODO Weird hack to prebake.
    pub fn scenarios_to_prebake(map: &Map) -> Vec<Scenario> {
        vec![make_bike_lane_scenario(map), make_bus_lane_scenario(map)]
    }
}
