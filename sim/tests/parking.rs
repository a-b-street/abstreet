extern crate control;
extern crate dimensioned;
extern crate geom;
extern crate map_model;
extern crate sim;

use map_model::LaneID;

// TODO refactor a few more things to make these more succinct?

#[test]
fn park_on_goal_st() {
    let (map, control_map, mut sim) = setup(make_test_map());
    let (parking1, parking2, driving2) = (LaneID(1), LaneID(4), LaneID(3));

    assert_eq!(map.get_l(parking1).number_parking_spots(), 8);
    assert_eq!(map.get_l(parking2).number_parking_spots(), 8);
    let car = sim.seed_specific_parked_cars(parking1, (0..8).collect())[2];
    sim.seed_specific_parked_cars(parking2, (0..4).collect());
    sim.seed_specific_parked_cars(parking2, (5..8).collect());
    sim.start_parked_car_with_goal(&map, car, driving2);

    loop {
        if let Some(p) = sim.step(&map, &control_map).first() {
            assert_eq!(p.car, car);
            assert_eq!(p.spot.parking_lane, parking2);
            assert_eq!(p.spot.spot_idx, 4);
            break;
        }
        if sim.time.is_multiple_of_minute() {
            println!("{}", sim.summary());
        }
        // TODO time limit
    }
    println!("Expected conditions met at {}", sim.time);
}

#[test]
fn wander_around_for_parking() {
    let (map, control_map, mut sim) = setup(make_test_map());
    let (parking1, parking2, driving2) = (LaneID(1), LaneID(4), LaneID(3));

    assert_eq!(map.get_l(parking1).number_parking_spots(), 8);
    assert_eq!(map.get_l(parking2).number_parking_spots(), 8);
    // There's a free spot behind the car, so they have to loop around to their original lane to
    // find it.
    let car = sim.seed_specific_parked_cars(parking1, (1..8).collect())[2];
    sim.seed_specific_parked_cars(parking2, (0..8).collect());
    sim.start_parked_car_with_goal(&map, car, driving2);

    loop {
        if let Some(p) = sim.step(&map, &control_map).first() {
            assert_eq!(p.car, car);
            assert_eq!(p.spot.parking_lane, parking1);
            assert_eq!(p.spot.spot_idx, 0);
            break;
        }
        if sim.time.is_multiple_of_minute() {
            println!("{}", sim.summary());
        }
        // TODO time limit
    }
    println!("Expected conditions met at {}", sim.time);
}

fn setup(map: map_model::Map) -> (map_model::Map, control::ControlMap, sim::Sim) {
    let rng_seed = 123;
    let control_map = control::ControlMap::new(&map);
    let sim = sim::Sim::new(&map, Some(rng_seed));
    (map, control_map, sim)
}

// Creates a test map with a single two-way road
fn make_test_map() -> map_model::Map {
    use dimensioned::si;
    use map_model::{raw_data, LaneType};
    use std::collections::BTreeMap;

    let left = geom::LonLat::new(100.0, 50.0);
    let right = geom::LonLat::new(200.0, 50.0);

    let north_pts = triangle_around(150.0, 10.0);
    let south_pts = triangle_around(150.0, 90.0);

    let map = map_model::Map::create_from_raw(
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
            coordinates_in_world_space: true,
        },
        &map_model::Edits::new(),
    );

    assert_eq!(map.all_roads().len(), 1);
    assert_eq!(
        map.get_r(map_model::RoadID(0)).children_forwards,
        vec![
            (LaneID(0), LaneType::Driving),
            (LaneID(1), LaneType::Parking),
            (LaneID(2), LaneType::Sidewalk),
        ]
    );
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
