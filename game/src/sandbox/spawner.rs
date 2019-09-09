use crate::common::CommonState;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{hotkey, EventCtx, GfxCtx, Key, ModalMenu, Wizard};
use geom::{Duration, PolyLine};
use map_model::{BuildingID, IntersectionID, LaneType, PathRequest, Position, LANE_THICKNESS};
use rand::seq::SliceRandom;
use rand::Rng;
use sim::{DrivingGoal, Scenario, SidewalkSpot, TripSpec};

const SMALL_DT: Duration = Duration::const_seconds(0.1);

pub struct AgentSpawner {
    menu: ModalMenu,
    from: Source,
    maybe_goal: Option<(Goal, Option<PolyLine>)>,
}

enum Source {
    Walking(WalkingSource),
    Driving(Position),
}

enum WalkingSource {
    Bldg(BuildingID),
    Sidewalk(Position),
}

#[derive(PartialEq)]
enum Goal {
    Building(BuildingID),
    Border(IntersectionID),
}

impl AgentSpawner {
    pub fn new(
        ctx: &mut EventCtx,
        ui: &mut UI,
        sandbox_menu: &mut ModalMenu,
    ) -> Option<Box<dyn State>> {
        let menu = ModalMenu::new(
            "Agent Spawner",
            vec![vec![(hotkey(Key::Escape), "quit")]],
            ctx,
        );
        let map = &ui.primary.map;
        match ui.primary.current_selection {
            Some(ID::Building(id)) => {
                if ctx
                    .input
                    .contextual_action(Key::F3, "spawn a pedestrian starting here")
                {
                    return Some(Box::new(AgentSpawner {
                        menu,
                        from: Source::Walking(WalkingSource::Bldg(id)),
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
                            from: Source::Driving(pos),
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
                        from: Source::Driving(Position::new(id, map.get_l(id).length() / 2.0)),
                        maybe_goal: None,
                    }));
                } else if map.get_l(id).is_sidewalk()
                    && ctx
                        .input
                        .contextual_action(Key::F3, "spawn a pedestrian starting here")
                {
                    return Some(Box::new(AgentSpawner {
                        menu,
                        from: Source::Walking(WalkingSource::Sidewalk(Position::new(
                            id,
                            map.get_l(id).length() / 2.0,
                        ))),
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
            None => {
                if ui.primary.sim.is_empty() && sandbox_menu.action("start a scenario") {
                    return Some(WizardState::new(Box::new(instantiate_scenario)));
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
        self.menu.handle_event(ctx, None);
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
            let start = match self.from {
                Source::Walking(WalkingSource::Bldg(b)) => Position::bldg_via_walking(b, map),
                Source::Walking(WalkingSource::Sidewalk(pos)) | Source::Driving(pos) => pos,
            };
            let end = match new_goal {
                Goal::Building(to) => match self.from {
                    Source::Walking(_) => Position::bldg_via_walking(to, map),
                    Source::Driving(_) => {
                        let end = map.find_driving_lane_near_building(to);
                        Position::new(end, map.get_l(end).length())
                    }
                },
                Goal::Border(to) => {
                    let lanes = map.get_i(to).get_incoming_lanes(
                        map,
                        match self.from {
                            Source::Walking(_) => LaneType::Sidewalk,
                            Source::Driving(_) => LaneType::Driving,
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
            match (&self.from, self.maybe_goal.take().unwrap().0) {
                (Source::Walking(src), Goal::Building(to)) => {
                    let start = match src {
                        WalkingSource::Bldg(b) => SidewalkSpot::building(*b, map),
                        WalkingSource::Sidewalk(pos) => {
                            SidewalkSpot::suddenly_appear(pos.lane(), pos.dist_along(), map)
                        }
                    };
                    let goal = SidewalkSpot::building(to, map);
                    let ped_speed = Scenario::rand_ped_speed(&mut rng);

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
                (Source::Walking(src), Goal::Border(to)) => {
                    if let Some(goal) = SidewalkSpot::end_at_border(to, map) {
                        let start = match src {
                            WalkingSource::Bldg(b) => SidewalkSpot::building(*b, map),
                            WalkingSource::Sidewalk(pos) => {
                                SidewalkSpot::suddenly_appear(pos.lane(), pos.dist_along(), map)
                            }
                        };
                        sim.schedule_trip(
                            sim.time(),
                            TripSpec::JustWalking {
                                start,
                                goal,
                                ped_speed: Scenario::rand_ped_speed(&mut rng),
                            },
                            map,
                        );
                    } else {
                        println!("Can't end a walking trip at {}; no sidewalks", to);
                    }
                }
                (Source::Driving(from), Goal::Building(to)) => {
                    if let Some(start_pos) = TripSpec::spawn_car_at(*from, map) {
                        sim.schedule_trip(
                            sim.time(),
                            TripSpec::CarAppearing {
                                start_pos,
                                vehicle_spec: Scenario::rand_car(&mut rng),
                                goal: DrivingGoal::ParkNear(to),
                                ped_speed: Scenario::rand_ped_speed(&mut rng),
                            },
                            map,
                        );
                    } else {
                        println!("Can't make a car appear at {:?}", from);
                    }
                }
                (Source::Driving(from), Goal::Border(to)) => {
                    if let Some(goal) = DrivingGoal::end_at_border(to, vec![LaneType::Driving], map)
                    {
                        sim.schedule_trip(
                            sim.time(),
                            TripSpec::CarAppearing {
                                start_pos: *from,
                                vehicle_spec: Scenario::rand_car(&mut rng),
                                goal,
                                ped_speed: Scenario::rand_ped_speed(&mut rng),
                            },
                            map,
                        );
                    } else {
                        println!("Can't end a car trip at {}; no driving lanes", to);
                    }
                }
            };
            sim.spawn_all_trips(map, &mut Timer::new("spawn trip"), false);
            sim.step(map, SMALL_DT);
            ui.recalculate_current_selection(ctx);
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let src = match self.from {
            Source::Walking(WalkingSource::Bldg(b)) => ID::Building(b),
            Source::Driving(pos) | Source::Walking(WalkingSource::Sidewalk(pos)) => {
                ID::Lane(pos.lane())
            }
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

fn instantiate_scenario(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let num_agents = ui.primary.current_flags.num_agents;
    let builtin = if let Some(n) = num_agents {
        format!("random scenario with {} agents", n)
    } else {
        "random scenario with some agents".to_string()
    };
    let map = &ui.primary.map;

    let scenario_name = wiz
        .wrap(ctx)
        .choose_string("Instantiate which scenario?", || {
            let mut list = vec![builtin.clone(), "just buses".to_string()];
            list.extend(abstutil::list_all_objects(
                abstutil::SCENARIOS,
                map.get_name(),
            ));
            list
        })?;

    let scenario = if scenario_name == builtin {
        if let Some(n) = num_agents {
            Scenario::scaled_run(map, n)
        } else {
            Scenario::small_run(map)
        }
    } else if scenario_name == "just buses" {
        Scenario::empty(map)
    } else {
        abstutil::read_binary(
            &abstutil::path1_bin(map.get_name(), abstutil::SCENARIOS, &scenario_name),
            &mut Timer::throwaway(),
        )
        .unwrap()
    };
    ctx.loading_screen("instantiate scenario", |_, timer| {
        scenario.instantiate(
            &mut ui.primary.sim,
            &ui.primary.map,
            &mut ui.primary.current_flags.sim_flags.make_rng(),
            timer,
        );
        ui.primary.sim.step(&ui.primary.map, SMALL_DT);
    });
    Some(Transition::Pop)
}
