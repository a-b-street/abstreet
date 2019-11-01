use crate::common::CommonState;
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{hotkey, EventCtx, GfxCtx, Key, ModalMenu};
use geom::{Duration, PolyLine};
use map_model::{BuildingID, IntersectionID, LaneType, Map, PathRequest, Position, LANE_THICKNESS};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use sim::{DrivingGoal, Scenario, SidewalkSpot, Sim, TripSpec};

const SMALL_DT: Duration = Duration::const_seconds(0.1);

pub struct AgentSpawner {
    menu: ModalMenu,
    from: Source,
    maybe_goal: Option<(Goal, Option<PolyLine>)>,
}

enum Source {
    WalkFromBldg(BuildingID),
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
    pub fn new(ctx: &mut EventCtx, ui: &mut UI) -> Option<Box<dyn State>> {
        let menu = ModalMenu::new("Agent Spawner", vec![(hotkey(Key::Escape), "quit")], ctx);
        let map = &ui.primary.map;
        match ui.primary.current_selection {
            Some(ID::Building(id)) => {
                let spots = ui.primary.sim.get_free_offstreet_spots(id);
                if !spots.is_empty()
                    && ctx
                        .input
                        .contextual_action(Key::F6, "seed a parked car here")
                {
                    let mut rng = ui.primary.current_flags.sim_flags.make_rng();
                    ui.primary.sim.seed_parked_car(
                        Scenario::rand_car(&mut rng),
                        spots[0],
                        Some(id),
                    );
                    return None;
                }
                if ctx
                    .input
                    .contextual_action(Key::F3, "spawn a pedestrian starting here just walking")
                {
                    return Some(Box::new(AgentSpawner {
                        menu,
                        from: Source::WalkFromBldg(id),
                        maybe_goal: None,
                    }));
                }
                let parked = ui.primary.sim.get_parked_cars_by_owner(id);
                // TODO Check if it's claimed... Haha if it is, MaybeUsingParkedCar still snags it!
                if !parked.is_empty()
                    && ctx.input.contextual_action(
                        Key::F5,
                        "spawn a pedestrian here using an owned parked car",
                    )
                {
                    return Some(Box::new(AgentSpawner {
                        menu,
                        from: Source::WalkFromBldgThenMaybeUseCar(id),
                        maybe_goal: None,
                    }));
                }
                if let Some(pos) = Position::bldg_via_driving(id, map) {
                    if ctx
                        .input
                        .contextual_action(Key::F4, "spawn a car starting here")
                    {
                        return Some(Box::new(AgentSpawner {
                            menu,
                            from: Source::Drive(pos),
                            maybe_goal: None,
                        }));
                    }
                }
            }
            Some(ID::Lane(id)) => {
                if map.get_l(id).is_driving()
                    && ctx
                        .input
                        .contextual_action(Key::F3, "spawn a car starting here")
                {
                    return Some(Box::new(AgentSpawner {
                        menu,
                        from: Source::Drive(Position::new(id, map.get_l(id).length() / 2.0)),
                        maybe_goal: None,
                    }));
                } else if map.get_l(id).is_sidewalk()
                    && ctx
                        .input
                        .contextual_action(Key::F3, "spawn a pedestrian starting here")
                {
                    return Some(Box::new(AgentSpawner {
                        menu,
                        from: Source::WalkFromSidewalk(Position::new(
                            id,
                            map.get_l(id).length() / 2.0,
                        )),
                        maybe_goal: None,
                    }));
                }
            }
            Some(ID::Intersection(i)) => {
                if ctx
                    .input
                    .contextual_action(Key::Z, "spawn agents around this intersection")
                {
                    spawn_agents_around(i, ui, ctx);
                }
            }
            _ => {}
        }
        None
    }
}

impl State for AgentSpawner {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // TODO Instructions to select target building/lane
        self.menu.event(ctx);
        if self.menu.action("quit") {
            return Transition::Pop;
        }

        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }

        let map = &ui.primary.map;

        let new_goal = match ui.primary.current_selection {
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
            let (start, driving) = match self.from {
                Source::WalkFromBldg(b) => (Position::bldg_via_walking(b, map), false),
                // TODO Find the driving lane in this case.
                Source::WalkFromBldgThenMaybeUseCar(b) => {
                    (Position::bldg_via_walking(b, map), false)
                }
                Source::WalkFromSidewalk(pos) => (pos, false),
                Source::Drive(pos) => (pos, true),
            };
            let end = match new_goal {
                Goal::Building(to) => {
                    if driving {
                        let end = map.find_driving_lane_near_building(to);
                        Position::new(end, map.get_l(end).length())
                    } else {
                        Position::bldg_via_walking(to, map)
                    }
                }
                Goal::Border(to) => {
                    let lanes = map.get_i(to).get_incoming_lanes(
                        map,
                        if driving {
                            LaneType::Driving
                        } else {
                            LaneType::Sidewalk
                        },
                    );
                    if lanes.is_empty() {
                        self.maybe_goal = None;
                        return Transition::Keep;
                    }
                    Position::new(lanes[0], map.get_l(lanes[0]).length())
                }
            };
            if start == end {
                self.maybe_goal = None;
            } else {
                if let Some(path) = map.pathfind(PathRequest {
                    start,
                    end,
                    can_use_bike_lanes: false,
                    can_use_bus_lanes: false,
                }) {
                    self.maybe_goal = Some((new_goal, path.trace(map, start.dist_along(), None)));
                } else {
                    self.maybe_goal = None;
                }
            }
        }

        if self.maybe_goal.is_some() && ctx.input.contextual_action(Key::F3, "end the agent here") {
            let mut rng = ui.primary.current_flags.sim_flags.make_rng();
            let sim = &mut ui.primary.sim;
            let err = schedule_trip(
                &self.from,
                self.maybe_goal.take().unwrap().0,
                map,
                sim,
                &mut rng,
            );
            sim.spawn_all_trips(map, &mut Timer::new("spawn trip"), false);
            sim.step(map, SMALL_DT);
            ui.recalculate_current_selection(ctx);
            if let Some(e) = err {
                return Transition::Replace(msg("Spawning error", vec![e]));
            } else {
                return Transition::Pop;
            }
        }

        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let src = match self.from {
            Source::WalkFromBldg(b) | Source::WalkFromBldgThenMaybeUseCar(b) => ID::Building(b),
            Source::WalkFromSidewalk(pos) | Source::Drive(pos) => ID::Lane(pos.lane()),
        };
        let mut opts = DrawOptions::new();
        opts.override_colors.insert(src, ui.cs.get("selected"));
        ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());

        if let Some((_, Some(ref trace))) = self.maybe_goal {
            g.draw_polygon(ui.cs.get("route"), &trace.make_polygons(LANE_THICKNESS));
        }

        self.menu.draw(g);
        CommonState::draw_osd(g, ui, &ui.primary.current_selection);
    }
}

fn spawn_agents_around(i: IntersectionID, ui: &mut UI, ctx: &EventCtx) {
    let map = &ui.primary.map;
    let sim = &mut ui.primary.sim;
    let mut rng = ui.primary.current_flags.sim_flags.make_rng();

    for l in &map.get_i(i).incoming_lanes {
        let lane = map.get_l(*l);
        if lane.is_driving() {
            for _ in 0..10 {
                let vehicle_spec = if rng.gen_bool(0.7) {
                    Scenario::rand_car(&mut rng)
                } else {
                    Scenario::rand_bike(&mut rng)
                };
                if vehicle_spec.length > lane.length() {
                    continue;
                }
                sim.schedule_trip(
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
                );
            }
        } else if lane.is_sidewalk() {
            for _ in 0..5 {
                sim.schedule_trip(
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
                );
            }
        }
    }

    sim.spawn_all_trips(map, &mut Timer::throwaway(), false);
    sim.step(map, SMALL_DT);
    ui.recalculate_current_selection(ctx);
}

// Returns optional error message
fn schedule_trip(
    src: &Source,
    raw_goal: Goal,
    map: &Map,
    sim: &mut Sim,
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
                sim.schedule_trip(
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
                );
            } else {
                sim.schedule_trip(
                    sim.time(),
                    TripSpec::JustWalking {
                        start,
                        goal,
                        ped_speed,
                    },
                    map,
                );
            }
        }
        _ => {
            // Driving
            let goal = match raw_goal {
                Goal::Building(to) => DrivingGoal::ParkNear(to),
                Goal::Border(to) => {
                    if let Some(g) = DrivingGoal::end_at_border(to, vec![LaneType::Driving], map) {
                        g
                    } else {
                        return Some(format!("Can't end a car trip at {}; no driving lanes", to));
                    }
                }
            };
            match src {
                Source::Drive(from) => {
                    if let Some(start_pos) = TripSpec::spawn_car_at(*from, map) {
                        sim.schedule_trip(
                            sim.time(),
                            TripSpec::CarAppearing {
                                start_pos,
                                vehicle_spec: Scenario::rand_car(rng),
                                goal,
                                ped_speed: Scenario::rand_ped_speed(rng),
                            },
                            map,
                        );
                    } else {
                        return Some(format!("Can't make a car appear at {:?}", from));
                    }
                }
                Source::WalkFromBldgThenMaybeUseCar(b) => {
                    sim.schedule_trip(
                        sim.time(),
                        TripSpec::MaybeUsingParkedCar {
                            start_bldg: *b,
                            goal,
                            ped_speed: Scenario::rand_ped_speed(rng),
                        },
                        map,
                    );
                }
                _ => unreachable!(),
            }
        }
    }
    None
}
