use crate::app::App;
use crate::game::Transition;
use crate::helpers::ID;
use abstutil::Timer;
use ezgui::{EventCtx, Key};
use geom::Duration;
use map_model::{IntersectionID, Position};
use rand::seq::SliceRandom;
use rand::Rng;
use sim::{DrivingGoal, Scenario, SidewalkSpot, TripEndpoint, TripSpec};

const SMALL_DT: Duration = Duration::const_seconds(0.1);

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
                    map,
                );
            }
        } else if lane.is_sidewalk() {
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
                    map,
                );
            }
        }
    }

    sim.flush_spawner(spawner, map, &mut timer);
    sim.normal_step(map, SMALL_DT);
}

pub fn actions(_: &App, id: ID) -> Vec<(Key, String)> {
    match id {
        ID::Intersection(_) => vec![(Key::Z, "spawn agents here".to_string())],
        _ => Vec::new(),
    }
}

pub fn execute(_: &mut EventCtx, app: &mut App, id: ID, action: String) -> Transition {
    match (id, action.as_ref()) {
        (ID::Intersection(id), "spawn agents here") => {
            spawn_agents_around(id, app);
            Transition::Keep
        }
        _ => unreachable!(),
    }
}
