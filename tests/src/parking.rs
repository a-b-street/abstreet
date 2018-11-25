use runner::TestRunner;

pub fn run(_t: &mut TestRunner) {}

/*
#[test]
fn park_on_goal_st() {
    let (map, control_map, mut sim) = setup("park_on_goal_st", make_test_map());
    let (south_parking, north_parking) = (LaneID(1), LaneID(4));
    let (north_bldg, south_bldg) = (BuildingID(0), BuildingID(1));

    assert_eq!(map.get_l(south_parking).number_parking_spots(), 8);
    assert_eq!(map.get_l(north_parking).number_parking_spots(), 8);
    let car = sim.seed_specific_parked_cars(south_parking, south_bldg, (0..8).collect())[2];
    sim.seed_specific_parked_cars(north_parking, north_bldg, (0..4).collect());
    sim.seed_specific_parked_cars(north_parking, north_bldg, (5..8).collect());
    sim.make_ped_using_car(&map, car, north_bldg);

    sim.run_until_expectations_met(
        &map,
        &control_map,
        vec![sim::Event::CarReachedParkingSpot(
            car,
            sim::ParkingSpot::new(north_parking, 4),
        )],
        sim::Tick::from_minutes(1),
    );
    sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
}

#[test]
fn wander_around_for_parking() {
    let (map, control_map, mut sim) = setup("wander_around_for_parking", make_test_map());
    let (south_parking, north_parking) = (LaneID(1), LaneID(4));
    let (north_bldg, south_bldg) = (BuildingID(0), BuildingID(1));

    assert_eq!(map.get_l(south_parking).number_parking_spots(), 8);
    assert_eq!(map.get_l(north_parking).number_parking_spots(), 8);
    // There's a free spot behind the car, so they have to loop around to their original lane to
    // find it.
    let car = sim.seed_specific_parked_cars(south_parking, south_bldg, (1..8).collect())[2];
    sim.seed_specific_parked_cars(north_parking, north_bldg, (0..8).collect());
    sim.make_ped_using_car(&map, car, north_bldg);

    sim.run_until_expectations_met(
        &map,
        &control_map,
        vec![sim::Event::CarReachedParkingSpot(
            car,
            sim::ParkingSpot::new(south_parking, 0),
        )],
        sim::Tick::from_minutes(2),
    );
    sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
}

fn setup(run_name: &str, map: map_model::Map) -> (map_model::Map, control::ControlMap, sim::Sim) {
    let rng_seed = 123;
    let control_map = control::ControlMap::new(&map, BTreeMap::new(), BTreeMap::new());
    let sim = sim::Sim::new(&map, run_name.to_string(), Some(rng_seed), None);
    (map, control_map, sim)
}

// Creates a test map with a single two-way road
fn make_test_map() -> map_model::Map {
    use dimensioned::si;
    use map_model::{raw_data, LaneType};

    let left = geom::LonLat::new(100.0, 50.0);
    let right = geom::LonLat::new(200.0, 50.0);

    let north_pts = triangle_around(150.0, 10.0);
    let south_pts = triangle_around(150.0, 90.0);

    let map = map_model::Map::create_from_raw(
        "test_map".to_string(),
        raw_data::Map {
            roads: vec![raw_data::Road {
                points: vec![left, right],
                osm_tags: BTreeMap::new(),
                osm_way_id: 123,
            }],
            intersections: vec![
                raw_data::Intersection {
                    point: left,
                    elevation: 0.0 * si::M,
                    has_traffic_signal: false,
                },
                raw_data::Intersection {
                    point: right,
                    elevation: 0.0 * si::M,
                    has_traffic_signal: false,
                },
            ],
            buildings: vec![
                raw_data::Building {
                    points: north_pts,
                    osm_tags: BTreeMap::new(),
                    osm_way_id: 456,
                },
                raw_data::Building {
                    points: south_pts,
                    osm_tags: BTreeMap::new(),
                    osm_way_id: 789,
                },
            ],
            parcels: Vec::new(),
            bus_routes: Vec::new(),
            areas: Vec::new(),
            coordinates_in_world_space: true,
        },
        map_model::RoadEdits::new(),
        &mut abstutil::Timer::new("setup test"),
    );

    assert_eq!(map.all_roads().len(), 1);
    // The south side, unless I'm backwards ><
    assert_eq!(
        map.get_r(map_model::RoadID(0)).children_forwards,
        vec![
            (LaneID(0), LaneType::Driving),
            (LaneID(1), LaneType::Parking),
            (LaneID(2), LaneType::Sidewalk),
        ]
    );
    // The north side
    assert_eq!(
        map.get_r(map_model::RoadID(0)).children_backwards,
        vec![
            (LaneID(3), LaneType::Driving),
            (LaneID(4), LaneType::Parking),
            (LaneID(5), LaneType::Sidewalk),
        ]
    );
    map
}

fn triangle_around(x: f64, y: f64) -> Vec<geom::LonLat> {
    vec![
        geom::LonLat::new(x - 5.0, y - 5.0),
        geom::LonLat::new(x + 5.0, y - 5.0),
        geom::LonLat::new(x, y + 5.0),
    ]
}
*/
