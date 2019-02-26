use crate::objects::{DrawCtx, ID};
use crate::plugins::sim::new_des_model;
use crate::plugins::{BlockingPlugin, PluginCtx};
use crate::render::MIN_ZOOM_FOR_DETAIL;
use abstutil::Timer;
use ezgui::{EventLoopMode, GfxCtx, Key};
use geom::{Distance, Duration, Speed};
use map_model::{BuildingID, LaneID, Map, Position, Traversable};
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use sim::{CarID, DrawCarInput, DrawPedestrianInput, GetDrawAgents, PedestrianID, VehicleType};

pub struct EvenSimplerModelController {
    sim: new_des_model::Sim,
    auto_mode: bool,
}

impl EvenSimplerModelController {
    pub fn new(ctx: &mut PluginCtx) -> Option<EvenSimplerModelController> {
        if let Some(ID::Lane(id)) = ctx.primary.current_selection {
            if ctx.primary.map.get_l(id).is_driving()
                && ctx
                    .input
                    .contextual_action(Key::Z, "start even simpler model")
            {
                return Some(EvenSimplerModelController {
                    sim: populate_sim(id, &ctx.primary.map),
                    auto_mode: false,
                });
            }
        }
        None
    }
}

impl BlockingPlugin for EvenSimplerModelController {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode_with_prompt(
            "Even Simpler Model",
            format!("Even Simpler Model at {}", self.sim.time()),
            &ctx.canvas,
        );
        if self.auto_mode {
            ctx.hints.mode = EventLoopMode::Animation;
            if ctx.input.modal_action("toggle forwards play") {
                self.auto_mode = false;
            } else if ctx.input.is_update_event() {
                self.sim.step_if_needed(&ctx.primary.map);
            }
        } else {
            if ctx.input.modal_action("forwards") {
                self.sim.step_if_needed(&ctx.primary.map);
            } else if ctx.input.modal_action("toggle forwards play") {
                self.auto_mode = true;
                ctx.hints.mode = EventLoopMode::Animation;
            } else if ctx.input.modal_action("spawn tons of cars everywhere") {
                self.sim = densely_populate_sim(&ctx.primary.map);
            }
        }
        if ctx.input.modal_action("quit") {
            return false;
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            self.sim.draw_unzoomed(g, &ctx.map);
        }
    }
}

impl GetDrawAgents for EvenSimplerModelController {
    fn time(&self) -> Duration {
        self.sim.time()
    }

    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        self.sim
            .get_all_draw_cars(map)
            .into_iter()
            .find(|x| x.id == id)
    }

    fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrianInput> {
        self.sim
            .get_all_draw_peds(map)
            .into_iter()
            .find(|x| x.id == id)
    }

    fn get_draw_cars(&self, on: Traversable, map: &Map) -> Vec<DrawCarInput> {
        self.sim.get_draw_cars_on(on, map)
    }

    fn get_draw_peds(&self, on: Traversable, map: &Map) -> Vec<DrawPedestrianInput> {
        self.sim.get_draw_peds_on(on, map)
    }

    fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        self.sim.get_all_draw_cars(map)
    }

    fn get_all_draw_peds(&self, map: &Map) -> Vec<DrawPedestrianInput> {
        self.sim.get_all_draw_peds(map)
    }
}

fn populate_sim(start: LaneID, map: &Map) -> new_des_model::Sim {
    let mut timer = Timer::new("populate_sim");
    let mut sim = new_des_model::Sim::new(map);

    let mut sources = vec![start];
    // Try to find a lane likely to have conflicts
    {
        for t in map.get_turns_from_lane(start) {
            if t.turn_type == map_model::TurnType::Straight {
                if let Some(l) = map
                    .get_parent(t.id.dst)
                    .any_on_other_side(t.id.dst, map_model::LaneType::Driving)
                {
                    sources.push(l);
                }
            }
        }
    }

    let mut rng = XorShiftRng::from_seed([42; 16]);
    for source in sources {
        let len = map.get_l(source).length();
        if len < new_des_model::MAX_CAR_LENGTH {
            println!("Can't spawn cars on {}, it's only {} long", source, len);
            continue;
        }

        for _ in 0..10 {
            spawn_car(&mut sim, &mut rng, map, source);
        }

        seed_parked_cars_near(source, &mut rng, &mut sim, map);

        random_ped_near(source, &mut sim, map, &mut rng);
    }

    sim.spawn_all_trips(map, &mut timer);
    timer.done();
    sim
}

fn densely_populate_sim(map: &Map) -> new_des_model::Sim {
    let mut timer = Timer::new("densely_populate_sim");
    let mut sim = new_des_model::Sim::new(map);
    let mut rng = XorShiftRng::from_seed([42; 16]);
    new_des_model::Scenario::small_run(map).instantiate(&mut sim, map, &mut rng, &mut timer);
    timer.done();
    sim
}

fn spawn_car(sim: &mut new_des_model::Sim, rng: &mut XorShiftRng, map: &Map, start_lane: LaneID) {
    let path = random_path(start_lane, rng, map);
    let last_lane = path.last().unwrap().as_lane();
    let vehicle = rand_vehicle(rng);
    let start_dist = rand_dist(rng, vehicle.length, map.get_l(start_lane).length());
    let spawn_time = Duration::seconds(0.2) * rng.gen_range(0, 5) as f64;

    sim.schedule_trip(
        spawn_time,
        new_des_model::TripSpec::CarAppearing(
            Position::new(start_lane, start_dist),
            vehicle,
            new_des_model::DrivingGoal::Border(map.get_l(last_lane).dst_i, last_lane),
        ),
        map,
    );
}

// And start some of them after a bit
fn seed_parked_cars_near(
    driving_lane: LaneID,
    rng: &mut XorShiftRng,
    sim: &mut new_des_model::Sim,
    map: &Map,
) {
    for l in map.get_parent(driving_lane).all_lanes() {
        if map.get_l(l).is_parking() {
            for spot in sim.get_free_spots(l) {
                if rng.gen_bool(0.2) {
                    sim.seed_parked_car(rand_vehicle(rng), spot, None);

                    if rng.gen_bool(0.3) {
                        if let Some(start_bldg) = random_bldg_near(l, map, rng) {
                            sim.schedule_trip(
                                Duration::seconds(5.0),
                                new_des_model::TripSpec::UsingParkedCar(
                                    new_des_model::SidewalkSpot::building(start_bldg, map),
                                    spot,
                                    new_des_model::DrivingGoal::ParkNear(
                                        map.all_buildings().choose(rng).unwrap().id,
                                    ),
                                ),
                                map,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn random_path(start: LaneID, rng: &mut XorShiftRng, map: &Map) -> Vec<Traversable> {
    let mut path = vec![Traversable::Lane(start)];
    let mut last_lane = start;
    for _ in 0..1 {
        if let Some(t) = map.get_turns_from_lane(last_lane).choose(rng) {
            path.push(Traversable::Turn(t.id));
            path.push(Traversable::Lane(t.id.dst));
            last_lane = t.id.dst;
        } else {
            break;
        }
    }
    path
}

fn rand_dist(rng: &mut XorShiftRng, low: Distance, high: Distance) -> Distance {
    Distance::meters(rng.gen_range(low.inner_meters(), high.inner_meters()))
}

fn rand_vehicle(rng: &mut XorShiftRng) -> new_des_model::VehicleSpec {
    let vehicle_len = rand_dist(
        rng,
        new_des_model::MIN_CAR_LENGTH,
        new_des_model::MAX_CAR_LENGTH,
    );
    let max_speed = if rng.gen_bool(0.1) {
        Some(Speed::miles_per_hour(10.0))
    } else {
        None
    };
    new_des_model::VehicleSpec {
        vehicle_type: VehicleType::Car,
        length: vehicle_len,
        max_speed,
    }
}

fn random_ped_near(
    start_near: LaneID,
    sim: &mut new_des_model::Sim,
    map: &Map,
    rng: &mut XorShiftRng,
) {
    let spawn_time = Duration::seconds(0.2) * rng.gen_range(0, 5) as f64;
    let end_near = random_path(start_near, rng, map).last().unwrap().as_lane();
    let (spot1, spot2) = match (
        random_bldg_near(start_near, map, rng),
        random_bldg_near(end_near, map, rng),
    ) {
        (Some(b1), Some(b2)) => (
            new_des_model::SidewalkSpot::building(b1, map),
            new_des_model::SidewalkSpot::building(b2, map),
        ),
        _ => {
            return;
        }
    };

    sim.schedule_trip(
        spawn_time,
        new_des_model::TripSpec::JustWalking(spot1, spot2),
        map,
    );
}

fn random_bldg_near(lane: LaneID, map: &Map, rng: &mut XorShiftRng) -> Option<BuildingID> {
    let mut candidates = Vec::new();
    for id in map.get_parent(lane).all_lanes() {
        candidates.extend(map.get_l(id).building_paths.clone());
    }
    candidates.choose(rng).cloned()
}
