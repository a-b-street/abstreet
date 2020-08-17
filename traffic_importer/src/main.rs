use abstutil::{CmdArgs, Timer};
use geom::{Distance, FindClosest, LonLat, Pt2D, Time};
use map_model::Map;
use serde::Deserialize;
use sim::{IndividTrip, PersonID, PersonSpec, Scenario, SpawnTrip, TripEndpoint, TripMode};

fn main() {
    let mut args = CmdArgs::new();
    let map = args.required("--map");
    let input = args.required("--input");
    args.done();

    let mut timer = Timer::new("import traffic demand data");
    let map = Map::new(map, &mut timer);
    let input: Input = abstutil::read_json(input, &mut timer);

    let mut closest: FindClosest<TripEndpoint> = FindClosest::new(map.get_bounds());
    for b in map.all_buildings() {
        closest.add(TripEndpoint::Bldg(b.id), b.polygon.points());
    }
    for i in map.all_intersections() {
        closest.add(TripEndpoint::Border(i.id, None), i.polygon.points());
    }
    let lookup_pt = |gps| match closest.closest_pt(
        Pt2D::from_gps(gps, map.get_gps_bounds()),
        Distance::meters(100.0),
    ) {
        Some((x, _)) => x,
        None => panic!("No building or border intersection within 100m of {}", gps),
    };

    let mut s = Scenario::empty(&map, &input.scenario_name);
    // Include all buses/trains
    s.only_seed_buses = None;
    for person in input.people {
        let mut spec = PersonSpec {
            id: PersonID(s.people.len()),
            orig_id: None,
            trips: Vec::new(),
        };
        let mut from = lookup_pt(person.origin);
        for trip in person.trips {
            let to = lookup_pt(trip.position);
            if let Some(t) = SpawnTrip::new(from.clone(), to.clone(), trip.mode, &map) {
                spec.trips.push(IndividTrip::new(trip.departure, t));
                from = to;
            } else {
                panic!(
                    "Can't make a {} trip from {:?} to {:?}",
                    trip.mode.ongoing_verb(),
                    from,
                    to
                );
            }
        }
        s.people.push(spec);
    }
    s.save();
}

#[derive(Deserialize)]
struct Input {
    scenario_name: String,
    people: Vec<Person>,
}

#[derive(Deserialize)]
struct Person {
    origin: LonLat,
    trips: Vec<Trip>,
}

#[derive(Deserialize)]
struct Trip {
    departure: Time,
    position: LonLat,
    mode: TripMode,
}
