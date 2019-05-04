use crate::common::CommonState;
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{EventCtx, GfxCtx, Key, ModalMenu};
use geom::PolyLine;
use map_model::{
    BuildingID, IntersectionID, IntersectionType, LaneType, PathRequest, Position, LANE_THICKNESS,
};
use rand::seq::SliceRandom;
use sim::{DrivingGoal, Scenario, SidewalkSpot, TripSpec};

pub struct AgentSpawner {
    menu: ModalMenu,
    from: Source,
    maybe_goal: Option<(Goal, Option<PolyLine>)>,
}

#[derive(Clone)]
enum Source {
    Walking(BuildingID),
    Driving(Position),
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
    ) -> Option<AgentSpawner> {
        let menu = ModalMenu::new("Agent Spawner", vec![(Some(Key::Escape), "quit")], ctx);
        let map = &ui.primary.map;
        match ui.primary.current_selection {
            Some(ID::Building(id)) => {
                if ctx
                    .input
                    .contextual_action(Key::F3, "spawn a pedestrian starting here")
                {
                    return Some(AgentSpawner {
                        menu,
                        from: Source::Walking(id),
                        maybe_goal: None,
                    });
                }
                let b = map.get_b(id);
                if let Ok(driving_lane) =
                    map.find_closest_lane(b.sidewalk(), vec![LaneType::Driving])
                {
                    if ctx
                        .input
                        .contextual_action(Key::F4, "spawn a car starting here")
                    {
                        return Some(AgentSpawner {
                            menu,
                            from: Source::Driving(
                                b.front_path.sidewalk.equiv_pos(driving_lane, map),
                            ),
                            maybe_goal: None,
                        });
                    }
                }
            }
            Some(ID::Lane(id)) => {
                if map.get_l(id).is_driving()
                    && ctx
                        .input
                        .contextual_action(Key::F3, "spawn an agent starting here")
                {
                    return Some(AgentSpawner {
                        menu,
                        from: Source::Driving(Position::new(id, map.get_l(id).length() / 2.0)),
                        maybe_goal: None,
                    });
                }
            }
            Some(ID::Intersection(i)) => {
                if ctx
                    .input
                    .contextual_action(Key::Z, "spawn agents around this intersection")
                {
                    spawn_agents_around(i, ui);
                }
            }
            None => {
                if ui.primary.sim.is_empty() {
                    if sandbox_menu.action("seed the sim with agents") {
                        // TODO This covers up the map. :\
                        ctx.loading_screen(|_, timer| {
                            let map = &ui.primary.map;
                            Scenario::scaled_run(map, ui.primary.current_flags.num_agents)
                                .instantiate(
                                    &mut ui.primary.sim,
                                    map,
                                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                                    timer,
                                );
                            ui.primary.sim.step(map);
                        });
                    }
                }
            }
            _ => {}
        }
        None
    }

    // Returns true if the spawner editor is done and we should go back to main sandbox mode.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        // TODO Instructions to select target building/lane
        self.menu.handle_event(ctx, None);
        if self.menu.action("quit") {
            return true;
        }

        ctx.canvas.handle_event(ctx.input);
        ui.primary.current_selection =
            ui.handle_mouseover(ctx, None, &ui.primary.sim, &ShowEverything::new(), false);

        let map = &ui.primary.map;

        let new_goal = match ui.primary.current_selection {
            Some(ID::Building(b)) => Goal::Building(b),
            Some(ID::Intersection(i))
                if map.get_i(i).intersection_type == IntersectionType::Border =>
            {
                Goal::Border(i)
            }
            _ => {
                self.maybe_goal = None;
                return false;
            }
        };

        let recalculate = match self.maybe_goal {
            Some((ref g, _)) => *g == new_goal,
            None => true,
        };

        if recalculate {
            let start = match self.from {
                Source::Walking(from) => map.get_b(from).front_path.sidewalk,
                Source::Driving(from) => from,
            };
            let end = match new_goal {
                Goal::Building(to) => match self.from {
                    Source::Walking(_) => map.get_b(to).front_path.sidewalk,
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
                        return true;
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
            match (self.from.clone(), self.maybe_goal.take().unwrap().0) {
                (Source::Walking(from), Goal::Building(to)) => {
                    sim.schedule_trip(
                        sim.time(),
                        TripSpec::JustWalking(
                            SidewalkSpot::building(from, map),
                            SidewalkSpot::building(to, map),
                        ),
                        map,
                    );
                }
                (Source::Walking(from), Goal::Border(to)) => {
                    if let Some(goal) = SidewalkSpot::end_at_border(to, map) {
                        sim.schedule_trip(
                            sim.time(),
                            TripSpec::JustWalking(SidewalkSpot::building(from, map), goal),
                            map,
                        );
                    } else {
                        println!("Can't end a walking trip at {}; no sidewalks", to);
                    }
                }
                (Source::Driving(from), Goal::Building(to)) => {
                    sim.schedule_trip(
                        sim.time(),
                        TripSpec::CarAppearing(
                            from,
                            Scenario::rand_car(&mut rng),
                            DrivingGoal::ParkNear(to),
                        ),
                        map,
                    );
                }
                (Source::Driving(from), Goal::Border(to)) => {
                    if let Some(goal) = DrivingGoal::end_at_border(to, vec![LaneType::Driving], map)
                    {
                        sim.schedule_trip(
                            sim.time(),
                            TripSpec::CarAppearing(from, Scenario::rand_car(&mut rng), goal),
                            map,
                        );
                    } else {
                        println!("Can't end a car trip at {}; no driving lanes", to);
                    }
                }
            };
            sim.spawn_all_trips(map, &mut Timer::new("spawn trip"));
            sim.step(map);
            //*ctx.recalculate_current_selection = true;
            return true;
        }

        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let src = match self.from {
            Source::Walking(b1) => ID::Building(b1),
            Source::Driving(pos1) => ID::Lane(pos1.lane()),
        };
        let mut opts = DrawOptions::new();
        opts.override_colors.insert(src, ui.cs.get("selected"));
        ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());

        self.menu.draw(g);

        if let Some((_, Some(ref trace))) = self.maybe_goal {
            g.draw_polygon(ui.cs.get("route"), &trace.make_polygons(LANE_THICKNESS));
        }

        CommonState::draw_osd(g, ui);
    }
}

fn spawn_agents_around(i: IntersectionID, ui: &mut UI) {
    let map = &ui.primary.map;
    let sim = &mut ui.primary.sim;
    let mut rng = ui.primary.current_flags.sim_flags.make_rng();

    for l in &map.get_i(i).incoming_lanes {
        let lane = map.get_l(*l);
        if lane.is_driving() {
            for _ in 0..10 {
                let vehicle = Scenario::rand_car(&mut rng);
                if vehicle.length > lane.length() {
                    continue;
                }
                sim.schedule_trip(
                    // TODO +1?
                    sim.time(),
                    TripSpec::CarAppearing(
                        Position::new(
                            lane.id,
                            Scenario::rand_dist(&mut rng, vehicle.length, lane.length()),
                        ),
                        vehicle,
                        DrivingGoal::ParkNear(map.all_buildings().choose(&mut rng).unwrap().id),
                    ),
                    map,
                );
            }
        } else if lane.is_sidewalk() {
            for _ in 0..5 {
                sim.schedule_trip(
                    sim.time(),
                    TripSpec::JustWalking(
                        SidewalkSpot::suddenly_appear(
                            lane.id,
                            Scenario::rand_dist(&mut rng, 0.1 * lane.length(), 0.9 * lane.length()),
                            map,
                        ),
                        SidewalkSpot::building(
                            map.all_buildings().choose(&mut rng).unwrap().id,
                            map,
                        ),
                    ),
                    map,
                );
            }
        }
    }

    sim.spawn_all_trips(map, &mut Timer::throwaway());
    sim.step(map);
    //*ctx.recalculate_current_selection = true;
}
