use crate::objects::{DrawCtx, ID};
use crate::plugins::{BlockingPlugin, PluginCtx};
use abstutil::Timer;
use ezgui::{Color, GfxCtx, Key};
use geom::PolyLine;
use map_model::{
    BuildingID, IntersectionID, IntersectionType, LaneType, PathRequest, Position, LANE_THICKNESS,
};
use rand::seq::SliceRandom;
use sim::{DrivingGoal, Scenario, SidewalkSpot, TripSpec};

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

pub struct SpawnAgent {
    from: Source,
    maybe_goal: Option<(Goal, Option<PolyLine>)>,
}

impl SpawnAgent {
    pub fn new(ctx: &mut PluginCtx) -> Option<SpawnAgent> {
        let map = &ctx.primary.map;
        match ctx.primary.current_selection {
            Some(ID::Building(id)) => {
                if ctx
                    .input
                    .contextual_action(Key::F3, "spawn a pedestrian starting here")
                {
                    return Some(SpawnAgent {
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
                        return Some(SpawnAgent {
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
                    return Some(SpawnAgent {
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
                    spawn_agents_around(i, ctx);
                }
            }
            None => {
                if ctx.primary.sim.is_empty() {
                    if ctx.input.action_chosen("seed the sim with agents") {
                        Scenario::scaled_run(map, ctx.primary.current_flags.num_agents)
                            .instantiate(
                                &mut ctx.primary.sim,
                                map,
                                &mut ctx.primary.current_flags.sim_flags.make_rng(),
                                &mut Timer::new("seed sim"),
                            );
                        ctx.primary.sim.step(map);
                        *ctx.recalculate_current_selection = true;
                        // TODO Also maybe step the secondary sim. Oops.
                    }
                }
            }
            _ => {}
        }
        None
    }
}

impl BlockingPlugin for SpawnAgent {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Agent Spawner", &ctx.canvas);
        if ctx.input.modal_action("quit") {
            return false;
        }
        let map = &ctx.primary.map;

        let new_goal = match ctx.primary.current_selection {
            Some(ID::Building(b)) => Goal::Building(b),
            Some(ID::Intersection(i))
                if map.get_i(i).intersection_type == IntersectionType::Border =>
            {
                Goal::Border(i)
            }
            _ => {
                self.maybe_goal = None;
                return true;
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
            let mut rng = ctx.primary.current_flags.sim_flags.make_rng();
            let sim = &mut ctx.primary.sim;
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
            *ctx.recalculate_current_selection = true;
            // TODO Also maybe step the secondary sim. Oops.
            return false;
        }

        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        if let Some((_, Some(ref trace))) = self.maybe_goal {
            g.draw_polygon(ctx.cs.get("route"), &trace.make_polygons(LANE_THICKNESS));
        }
    }

    fn color_for(&self, obj: ID, ctx: &DrawCtx) -> Option<Color> {
        match (&self.from, obj) {
            (Source::Walking(ref b1), ID::Building(b2)) if *b1 == b2 => {
                Some(ctx.cs.get("selected"))
            }
            (Source::Driving(ref pos1), ID::Lane(l2)) if pos1.lane() == l2 => {
                Some(ctx.cs.get("selected"))
            }
            _ => None,
        }
    }
}

fn spawn_agents_around(i: IntersectionID, ctx: &mut PluginCtx) {
    let map = &ctx.primary.map;
    let sim = &mut ctx.primary.sim;
    let mut rng = ctx.primary.current_flags.sim_flags.make_rng();

    for l in &map.get_i(i).incoming_lanes {
        let lane = map.get_l(*l);
        if lane.is_driving() {
            for _ in 0..10 {
                let vehicle = Scenario::rand_car(&mut rng);
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
    *ctx.recalculate_current_selection = true;
    // TODO Also maybe step the secondary sim. Oops.
}
