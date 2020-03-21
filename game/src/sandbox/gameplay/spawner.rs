use crate::app::App;
use crate::colors;
use crate::common::{Colorer, CommonState};
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::sandbox::gameplay::freeform::Freeform;
use crate::sandbox::SandboxMode;
use abstutil::Timer;
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Text,
    TextExt, VerticalAlignment, Widget,
};
use geom::{Distance, Duration, PolyLine};
use map_model::{
    BuildingID, IntersectionID, LaneID, Map, PathConstraints, PathRequest, Position,
    NORMAL_LANE_THICKNESS,
};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use sim::{
    BorderSpawnOverTime, DrivingGoal, OriginDestination, Scenario, SidewalkSpot, Sim, TripSpawner,
    TripSpec,
};

const SMALL_DT: Duration = Duration::const_seconds(0.1);

// TODO So many problems here. One is using schedule_trip directly. But using a Scenario is weird
// because we need to keep amending it and re-instantiating it, and because picking specific
// starting positions for vehicles depends on randomized vehicle lengths...

pub struct AgentSpawner {
    composite: Composite,
    from: Source,
    maybe_goal: Option<(Goal, Option<PolyLine>)>,
    colorer: Colorer,
}

enum Source {
    WalkFromBldg(BuildingID),
    // Stash the driving Position here for convenience
    BikeFromBldg(BuildingID, Position),
    WalkFromBldgThenMaybeUseCar(BuildingID),
    WalkFromSidewalk(Position),
    Drive(Position),
}

#[derive(PartialEq)]
enum Goal {
    Building(BuildingID),
    Border(IntersectionID),
}

impl AgentSpawner {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Option<Box<dyn State>> {
        let map = &app.primary.map;
        let color = app.cs.get("selected");
        let mut c = Colorer::new(Text::from(Line("spawning agent")), vec![("start", color)]);

        match app.primary.current_selection {
            Some(ID::Building(id)) => {
                c.add_b(id, color);

                let spots = app.primary.sim.get_free_offstreet_spots(id);
                if !spots.is_empty() && app.per_obj.action(ctx, Key::F6, "seed a parked car here") {
                    let mut rng = app.primary.current_flags.sim_flags.make_rng();
                    app.primary.sim.seed_parked_car(
                        Scenario::rand_car(&mut rng),
                        spots[0],
                        Some(id),
                    );
                    return None;
                }
                if app.per_obj.action(ctx, Key::F3, "spawn a walking trip") {
                    return Some(Box::new(AgentSpawner {
                        composite: make_top_bar(
                            ctx,
                            "Spawning a pedestrian",
                            "Pick a building or border as a destination",
                        ),
                        from: Source::WalkFromBldg(id),
                        maybe_goal: None,
                        colorer: c.build(ctx, app),
                    }));
                }
                let parked = app.primary.sim.get_parked_cars_by_owner(id);
                // TODO Check if it's claimed... Haha if it is, MaybeUsingParkedCar still snags it!
                if !parked.is_empty()
                    && app.per_obj.action(
                        ctx,
                        Key::F5,
                        "spawn a pedestrian here using an owned parked car",
                    )
                {
                    return Some(Box::new(AgentSpawner {
                        composite: make_top_bar(
                            ctx,
                            "Spawning a walking trip using a parked car",
                            "Pick a building or border as a destination",
                        ),
                        from: Source::WalkFromBldgThenMaybeUseCar(id),
                        maybe_goal: None,
                        colorer: c.build(ctx, app),
                    }));
                }
                if let Some(pos) = Position::bldg_via_driving(id, map) {
                    if app
                        .per_obj
                        .action(ctx, Key::F4, "spawn a car starting here")
                    {
                        return Some(Box::new(AgentSpawner {
                            composite: make_top_bar(
                                ctx,
                                "Spawning a car",
                                "Pick a building or border as a destination",
                            ),
                            from: Source::Drive(pos),
                            maybe_goal: None,
                            colorer: c.build(ctx, app),
                        }));
                    }
                }
                if let Some(pos) = Position::bldg_via_biking(id, map) {
                    if app
                        .per_obj
                        .action(ctx, Key::F7, "spawn a bike starting here")
                    {
                        return Some(Box::new(AgentSpawner {
                            composite: make_top_bar(
                                ctx,
                                "Spawning a bike",
                                "Pick a building or border as a destination",
                            ),
                            from: Source::BikeFromBldg(id, pos),
                            maybe_goal: None,
                            colorer: c.build(ctx, app),
                        }));
                    }
                }
            }
            Some(ID::Lane(id)) => {
                c.add_l(id, color, map);

                if map.get_l(id).is_driving()
                    && app
                        .per_obj
                        .action(ctx, Key::F3, "spawn a car starting here")
                {
                    return Some(Box::new(AgentSpawner {
                        composite: make_top_bar(
                            ctx,
                            "Spawning a car",
                            "Pick a building or border as a destination",
                        ),
                        from: Source::Drive(Position::new(id, map.get_l(id).length() / 2.0)),
                        maybe_goal: None,
                        colorer: c.build(ctx, app),
                    }));
                } else if map.get_l(id).is_sidewalk()
                    && app
                        .per_obj
                        .action(ctx, Key::F3, "spawn a pedestrian starting here")
                {
                    return Some(Box::new(AgentSpawner {
                        composite: make_top_bar(
                            ctx,
                            "Spawning a pedestrian",
                            "Pick a building or border as a destination",
                        ),
                        from: Source::WalkFromSidewalk(Position::new(
                            id,
                            map.get_l(id).length() / 2.0,
                        )),
                        maybe_goal: None,
                        colorer: c.build(ctx, app),
                    }));
                }
            }
            Some(ID::Intersection(i)) => {
                if app.per_obj.action(ctx, Key::Z, "spawn agents here") {
                    spawn_agents_around(i, app);
                }
            }
            _ => {}
        }
        None
    }
}

impl State for AgentSpawner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        let map = &app.primary.map;

        let new_goal = match app.primary.current_selection {
            Some(ID::Building(b)) => Goal::Building(b),
            Some(ID::Intersection(i)) if map.get_i(i).is_border() => Goal::Border(i),
            _ => {
                self.maybe_goal = None;
                return Transition::Keep;
            }
        };

        let recalculate = match self.maybe_goal {
            Some((ref g, _)) => *g == new_goal,
            None => true,
        };

        if recalculate {
            let (start, constraints) = match self.from {
                Source::WalkFromBldg(b) => (
                    Position::bldg_via_walking(b, map),
                    PathConstraints::Pedestrian,
                ),
                Source::BikeFromBldg(_, pos) => (pos, PathConstraints::Bike),
                // TODO Find the driving lane in this case.
                Source::WalkFromBldgThenMaybeUseCar(b) => (
                    Position::bldg_via_walking(b, map),
                    PathConstraints::Pedestrian,
                ),
                Source::WalkFromSidewalk(pos) => (pos, PathConstraints::Pedestrian),
                Source::Drive(pos) => (pos, PathConstraints::Car),
            };
            let end = match new_goal {
                Goal::Building(to) => {
                    if constraints == PathConstraints::Pedestrian {
                        Position::bldg_via_walking(to, map)
                    } else {
                        DrivingGoal::ParkNear(to).goal_pos(constraints, map)
                    }
                }
                Goal::Border(to) => {
                    if let Some(g) = DrivingGoal::end_at_border(
                        map.get_i(to).some_incoming_road(map),
                        constraints,
                        map,
                    ) {
                        g.goal_pos(constraints, map)
                    } else {
                        self.maybe_goal = None;
                        return Transition::Keep;
                    }
                }
            };
            if start == end {
                self.maybe_goal = None;
            } else {
                if let Some(path) = map.pathfind(PathRequest {
                    start,
                    end,
                    constraints,
                }) {
                    self.maybe_goal = Some((new_goal, path.trace(map, start.dist_along(), None)));
                } else {
                    self.maybe_goal = None;
                }
            }
        }

        if self.maybe_goal.is_some() && app.per_obj.left_click(ctx, "end the agent here") {
            let mut rng = app.primary.current_flags.sim_flags.make_rng();
            let sim = &mut app.primary.sim;
            let mut spawner = sim.make_spawner();
            let err = schedule_trip(
                &self.from,
                self.maybe_goal.take().unwrap().0,
                map,
                sim,
                &mut spawner,
                &mut rng,
            );
            sim.flush_spawner(spawner, map, &mut Timer::new("spawn trip"), false);
            sim.normal_step(map, SMALL_DT);
            app.recalculate_current_selection(ctx);
            if let Some(e) = err {
                return Transition::Replace(msg("Spawning error", vec![e]));
            } else {
                return Transition::Pop;
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.colorer.draw(g);

        if let Some((_, Some(ref trace))) = self.maybe_goal {
            g.draw_polygon(
                app.cs.get("route"),
                &trace.make_polygons(NORMAL_LANE_THICKNESS),
            );
        }

        self.composite.draw(g);
        CommonState::draw_osd(g, app, &app.primary.current_selection);
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
                spawner.schedule_trip(
                    sim.random_person(),
                    sim.time(),
                    TripSpec::CarAppearing {
                        start_pos: Position::new(
                            lane.id,
                            Scenario::rand_dist(&mut rng, vehicle_spec.length, lane.length()),
                        ),
                        vehicle_spec,
                        goal: DrivingGoal::ParkNear(
                            map.all_buildings().choose(&mut rng).unwrap().id,
                        ),
                        ped_speed: Scenario::rand_ped_speed(&mut rng),
                    },
                    map,
                    sim,
                );
            }
        } else if lane.is_sidewalk() {
            for _ in 0..5 {
                spawner.schedule_trip(
                    sim.random_person(),
                    sim.time(),
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
                        ped_speed: Scenario::rand_ped_speed(&mut rng),
                    },
                    map,
                    sim,
                );
            }
        }
    }

    sim.flush_spawner(spawner, map, &mut timer, false);
    sim.normal_step(map, SMALL_DT);
}

// Returns optional error message
fn schedule_trip(
    src: &Source,
    raw_goal: Goal,
    map: &Map,
    sim: &mut Sim,
    spawner: &mut TripSpawner,
    rng: &mut XorShiftRng,
) -> Option<String> {
    match src {
        Source::WalkFromBldg(_) | Source::WalkFromSidewalk(_) => {
            let start = match src {
                Source::WalkFromBldg(b) => SidewalkSpot::building(*b, map),
                Source::WalkFromSidewalk(pos) => {
                    SidewalkSpot::suddenly_appear(pos.lane(), pos.dist_along(), map)
                }
                _ => unreachable!(),
            };
            let goal = match raw_goal {
                Goal::Building(to) => SidewalkSpot::building(to, map),
                Goal::Border(to) => {
                    if let Some(goal) = SidewalkSpot::end_at_border(to, map) {
                        goal
                    } else {
                        return Some(format!("Can't end a walking trip at {}; no sidewalks", to));
                    }
                }
            };
            let ped_speed = Scenario::rand_ped_speed(rng);
            if let Some((stop1, stop2, route)) =
                map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos)
            {
                println!("Using {} from {} to {}", route, stop1, stop2);
                spawner.schedule_trip(
                    sim.random_person(),
                    sim.time(),
                    TripSpec::UsingTransit {
                        start,
                        goal,
                        route,
                        stop1,
                        stop2,
                        ped_speed,
                    },
                    map,
                    sim,
                );
            } else {
                println!("Not using transit");
                spawner.schedule_trip(
                    sim.random_person(),
                    sim.time(),
                    TripSpec::JustWalking {
                        start,
                        goal,
                        ped_speed,
                    },
                    map,
                    sim,
                );
            }
        }
        Source::BikeFromBldg(b, _) => {
            let goal = match raw_goal {
                Goal::Building(to) => DrivingGoal::ParkNear(to),
                Goal::Border(to) => {
                    if let Some(g) = DrivingGoal::end_at_border(
                        map.get_i(to).some_incoming_road(map),
                        PathConstraints::Bike,
                        map,
                    ) {
                        g
                    } else {
                        return Some(format!("Can't end a bike trip at {}", to));
                    }
                }
            };
            spawner.schedule_trip(
                sim.random_person(),
                sim.time(),
                TripSpec::UsingBike {
                    start: SidewalkSpot::building(*b, map),
                    vehicle: Scenario::rand_bike(rng),
                    goal,
                    ped_speed: Scenario::rand_ped_speed(rng),
                },
                map,
                sim,
            );
        }
        _ => {
            // Driving
            let goal = match raw_goal {
                Goal::Building(to) => DrivingGoal::ParkNear(to),
                Goal::Border(to) => {
                    if let Some(g) = DrivingGoal::end_at_border(
                        map.get_i(to).some_incoming_road(map),
                        PathConstraints::Car,
                        map,
                    ) {
                        g
                    } else {
                        return Some(format!("Can't end a car trip at {}", to));
                    }
                }
            };
            match src {
                Source::Drive(from) => {
                    if let Some(start_pos) = TripSpec::spawn_car_at(*from, map) {
                        spawner.schedule_trip(
                            sim.random_person(),
                            sim.time(),
                            TripSpec::CarAppearing {
                                start_pos,
                                vehicle_spec: Scenario::rand_car(rng),
                                goal,
                                ped_speed: Scenario::rand_ped_speed(rng),
                            },
                            map,
                            sim,
                        );
                    } else {
                        return Some(format!("Can't make a car appear at {:?}", from));
                    }
                }
                Source::WalkFromBldgThenMaybeUseCar(b) => {
                    spawner.schedule_trip(
                        sim.random_person(),
                        sim.time(),
                        TripSpec::MaybeUsingParkedCar {
                            start_bldg: *b,
                            goal,
                            ped_speed: Scenario::rand_ped_speed(rng),
                        },
                        map,
                        sim,
                    );
                }
                _ => unreachable!(),
            }
        }
    }
    None
}

// New experiment, stop squeezing in all these options into one thing, specialize.
pub struct SpawnManyAgents {
    composite: Composite,
    from: LaneID,
    maybe_goal: Option<(LaneID, Option<PolyLine>)>,
    schedule: Option<(usize, Duration)>,
    colorer: Colorer,
}

impl SpawnManyAgents {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Option<Box<dyn State>> {
        if let Some(ID::Lane(l)) = app.primary.current_selection {
            if app.primary.map.get_l(l).is_driving()
                && app
                    .per_obj
                    .action(ctx, Key::F2, "spawn many cars starting here")
            {
                let color = app.cs.get("selected");
                let mut c = Colorer::new(
                    Text::from(Line("spawning many agents")),
                    vec![("start", color)],
                );
                c.add_l(l, color, &app.primary.map);

                return Some(Box::new(SpawnManyAgents {
                    composite: make_top_bar(
                        ctx,
                        "Spawning many agents",
                        "Pick a driving lane as a destination",
                    ),
                    from: l,
                    maybe_goal: None,
                    schedule: None,
                    colorer: c.build(ctx, app),
                }));
            }
        }
        None
    }
}

impl State for SpawnManyAgents {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // TODO Weird pattern for handling "return value" from the wizard we launched? Maybe
        // PopWithData is a weird pattern; we should have a resume() handler that handles the
        // context
        if let Some((count, duration)) = self.schedule {
            let dst_l = self.maybe_goal.take().unwrap().0;
            create_swarm(app, self.from, dst_l, count, duration);
            let src = app.primary.map.get_l(self.from).src_i;
            let dst = app.primary.map.get_l(dst_l).dst_i;
            return Transition::PopWithData(Box::new(move |state, _, _| {
                let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                let freeform = sandbox.gameplay.downcast_mut::<Freeform>().unwrap();
                freeform.spawn_pts.insert(src);
                freeform.spawn_pts.insert(dst);
            }));
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        let map = &app.primary.map;

        let new_goal = match app.primary.current_selection {
            Some(ID::Lane(l)) if map.get_l(l).is_driving() => l,
            _ => {
                self.maybe_goal = None;
                return Transition::Keep;
            }
        };

        let recalculate = match self.maybe_goal {
            Some((l, _)) => l != new_goal,
            None => true,
        };

        if recalculate {
            if let Some(path) = map.pathfind(PathRequest {
                start: Position::new(self.from, Distance::ZERO),
                end: Position::new(new_goal, map.get_l(new_goal).length()),
                constraints: PathConstraints::Car,
            }) {
                self.maybe_goal = Some((new_goal, path.trace(map, Distance::ZERO, None)));
            } else {
                self.maybe_goal = None;
            }
        }

        if self.maybe_goal.is_some()
            && self.schedule.is_none()
            && app.per_obj.left_click(ctx, "end the swarm here")
        {
            return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, _| {
                let mut wizard = wiz.wrap(ctx);
                let count =
                    wizard.input_usize_prefilled("How many cars to spawn?", "1000".to_string())?;
                let duration = Duration::seconds(wizard.input_usize_prefilled(
                    "How many seconds to spawn them over?",
                    "90".to_string(),
                )? as f64);
                // TODO Or call create_swarm here and pop twice. Nice to play with two patterns
                // though.
                // TODO Another alt is to extend the wizard pattern and have a sort of
                // general-purpose wizard block.
                Some(Transition::PopWithData(Box::new(move |state, _, _| {
                    let mut spawn = state.downcast_mut::<SpawnManyAgents>().unwrap();
                    spawn.schedule = Some((count, duration));
                })))
            })));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.colorer.draw(g);

        if let Some((_, Some(ref trace))) = self.maybe_goal {
            g.draw_polygon(
                app.cs.get("route"),
                &trace.make_polygons(NORMAL_LANE_THICKNESS),
            );
        }

        self.composite.draw(g);
        CommonState::draw_osd(g, app, &app.primary.current_selection);
    }
}

fn create_swarm(app: &mut App, from: LaneID, to: LaneID, count: usize, duration: Duration) {
    let mut scenario = Scenario::empty(&app.primary.map, "swarm");
    scenario.border_spawn_over_time.push(BorderSpawnOverTime {
        num_peds: 0,
        num_cars: count,
        num_bikes: 0,
        start_time: app.primary.sim.time() + SMALL_DT,
        stop_time: app.primary.sim.time() + SMALL_DT + duration,
        start_from_border: app
            .primary
            .map
            .get_l(from)
            .get_directed_parent(&app.primary.map),
        goal: OriginDestination::EndOfRoad(
            app.primary
                .map
                .get_l(to)
                .get_directed_parent(&app.primary.map),
        ),
        percent_use_transit: 0.0,
    });
    let mut rng = app.primary.current_flags.sim_flags.make_rng();
    scenario.instantiate(
        &mut app.primary.sim,
        &app.primary.map,
        &mut rng,
        &mut Timer::throwaway(),
    );
}

fn make_top_bar(ctx: &mut EventCtx, title: &str, howto: &str) -> Composite {
    Composite::new(
        Widget::col(vec![
            Widget::row(vec![
                Line(title).roboto_bold().draw(ctx),
                Btn::text_fg("X")
                    .build_def(ctx, hotkey(Key::Escape))
                    .align_right(),
            ]),
            howto.draw_text(ctx),
        ])
        .padding(10)
        .bg(colors::PANEL_BG),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}
