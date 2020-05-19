use crate::{
    CarID, DrivingGoal, OrigPersonID, ParkingSpot, PersonID, SidewalkPOI, SidewalkSpot, Sim,
    TripEndpoint, TripMode, TripSpec, Vehicle, VehicleSpec, VehicleType, BIKE_LENGTH,
    MAX_CAR_LENGTH, MIN_CAR_LENGTH,
};
use abstutil::{prettyprint_usize, Counter, Timer};
use geom::{Distance, Duration, LonLat, Speed, Time};
use map_model::{
    BuildingID, BusRouteID, BusStopID, DirectedRoadID, Map, PathConstraints, Position, RoadID,
};
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

// How to start a simulation.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Scenario {
    pub scenario_name: String,
    pub map_name: String,

    pub people: Vec<PersonSpec>,
    // None means seed all buses. Otherwise the route name must be present here.
    pub only_seed_buses: Option<BTreeSet<String>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PersonSpec {
    pub id: PersonID,
    // Just used for debugging
    pub orig_id: Option<OrigPersonID>,
    pub trips: Vec<IndividTrip>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IndividTrip {
    pub depart: Time,
    pub trip: SpawnTrip,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SpawnTrip {
    // Only for interactive / debug trips
    VehicleAppearing {
        start: Position,
        goal: DrivingGoal,
        is_bike: bool,
    },
    FromBorder {
        dr: DirectedRoadID,
        goal: DrivingGoal,
        // For bikes starting at a border, use FromBorder. UsingBike implies a walk->bike trip.
        is_bike: bool,
        origin: Option<OffMapLocation>,
    },
    UsingParkedCar(BuildingID, DrivingGoal),
    UsingBike(SidewalkSpot, DrivingGoal),
    JustWalking(SidewalkSpot, SidewalkSpot),
    UsingTransit(SidewalkSpot, SidewalkSpot, BusRouteID, BusStopID, BusStopID),
    // Completely off-map trip. Don't really simulate much of it.
    Remote {
        from: OffMapLocation,
        to: OffMapLocation,
        trip_time: Duration,
        mode: TripMode,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OffMapLocation {
    pub parcel_id: usize,
    pub gps: LonLat,
}

impl Scenario {
    // Any case where map edits could change the calls to the RNG, we have to fork.
    pub fn instantiate(&self, sim: &mut Sim, map: &Map, rng: &mut XorShiftRng, timer: &mut Timer) {
        sim.set_name(self.scenario_name.clone());

        timer.start(format!("Instantiating {}", self.scenario_name));

        if let Some(ref routes) = self.only_seed_buses {
            for route in map.get_all_bus_routes() {
                if routes.contains(&route.name) {
                    sim.seed_bus_route(route, map, timer);
                }
            }
        } else {
            // All of them
            for route in map.get_all_bus_routes() {
                sim.seed_bus_route(route, map, timer);
            }
        }

        timer.start_iter("trips for People", self.people.len());
        let mut spawner = sim.make_spawner();
        let mut parked_cars: Vec<(Vehicle, BuildingID)> = Vec::new();
        for p in &self.people {
            timer.next();

            if let Err(err) = p.check_schedule(map) {
                panic!("{}", err);
            }

            let (vehicle_specs, cars_initially_parked_at, vehicle_foreach_trip) =
                p.get_vehicles(rng);
            sim.new_person(
                p.id,
                p.orig_id,
                Scenario::rand_ped_speed(rng),
                vehicle_specs,
            );
            let person = sim.get_person(p.id);
            for (idx, b) in cars_initially_parked_at {
                parked_cars.push((person.vehicles[idx].clone(), b));
            }
            for (t, maybe_idx) in p.trips.iter().zip(vehicle_foreach_trip) {
                // The RNG call might change over edits for picking the spawning lane from a border
                // with multiple choices for a vehicle type.
                let mut tmp_rng = abstutil::fork_rng(rng);
                let spec = t.trip.clone().to_trip_spec(
                    maybe_idx.map(|idx| person.vehicles[idx].id),
                    &mut tmp_rng,
                    map,
                );
                spawner.schedule_trip(person, t.depart, spec, t.trip.start(map), map);
            }
        }

        // parked_cars is stable over map edits, so don't fork.
        parked_cars.shuffle(rng);
        seed_parked_cars(parked_cars, sim, map, rng, timer);

        sim.flush_spawner(spawner, map, timer);
        timer.stop(format!("Instantiating {}", self.scenario_name));
    }

    pub fn save(&self) {
        abstutil::write_binary(
            abstutil::path_scenario(&self.map_name, &self.scenario_name),
            self,
        );
    }

    pub fn empty(map: &Map, name: &str) -> Scenario {
        Scenario {
            scenario_name: name.to_string(),
            map_name: map.get_name().to_string(),
            people: Vec::new(),
            only_seed_buses: Some(BTreeSet::new()),
        }
    }

    pub fn rand_car(rng: &mut XorShiftRng) -> VehicleSpec {
        let length = Scenario::rand_dist(rng, MIN_CAR_LENGTH, MAX_CAR_LENGTH);
        VehicleSpec {
            vehicle_type: VehicleType::Car,
            length,
            max_speed: None,
        }
    }

    pub fn rand_bike(rng: &mut XorShiftRng) -> VehicleSpec {
        let max_speed = Some(Scenario::rand_speed(
            rng,
            Speed::miles_per_hour(8.0),
            Speed::miles_per_hour(10.0),
        ));
        VehicleSpec {
            vehicle_type: VehicleType::Bike,
            length: BIKE_LENGTH,
            max_speed,
        }
    }

    pub fn rand_dist(rng: &mut XorShiftRng, low: Distance, high: Distance) -> Distance {
        assert!(high > low);
        Distance::meters(rng.gen_range(low.inner_meters(), high.inner_meters()))
    }

    fn rand_speed(rng: &mut XorShiftRng, low: Speed, high: Speed) -> Speed {
        assert!(high > low);
        Speed::meters_per_second(rng.gen_range(
            low.inner_meters_per_second(),
            high.inner_meters_per_second(),
        ))
    }

    pub fn rand_ped_speed(rng: &mut XorShiftRng) -> Speed {
        Scenario::rand_speed(rng, Speed::miles_per_hour(2.0), Speed::miles_per_hour(3.0))
    }

    // Utter hack. Blindly repeats all trips taken by each person every day.
    //
    // What happens if the last place a person winds up in a day isn't the same as where their
    // first trip the next starts? Will crash as soon as the scenario is instantiated, through
    // check_schedule().
    //
    // The bigger problem is that any people that seem to require multiple cars... will wind up
    // needing LOTS of cars.
    pub fn repeat_days(mut self, days: usize) -> Scenario {
        self.scenario_name = format!("{} repeated for {} days", self.scenario_name, days);
        for person in &mut self.people {
            let mut trips = Vec::new();
            let mut offset = Duration::ZERO;
            for _ in 0..days {
                for trip in &person.trips {
                    trips.push(IndividTrip {
                        depart: trip.depart + offset,
                        trip: trip.trip.clone(),
                    });
                }
                offset += Duration::hours(24);
            }
            person.trips = trips;
        }
        self
    }

    pub fn count_parked_cars_per_bldg(&self) -> Counter<BuildingID> {
        let mut per_bldg = Counter::new();
        // Pass in a dummy RNG
        let mut rng = XorShiftRng::from_seed([0; 16]);
        for p in &self.people {
            let (_, cars_initially_parked_at, _) = p.get_vehicles(&mut rng);
            for (_, b) in cars_initially_parked_at {
                per_bldg.inc(b);
            }
        }
        per_bldg
    }

    pub fn remove_weird_schedules(mut self, map: &Map) -> Scenario {
        let orig = self.people.len();
        self.people
            .retain(|person| match person.check_schedule(map) {
                Ok(()) => true,
                Err(err) => {
                    println!("{}", err);
                    false
                }
            });
        println!(
            "{} of {} people have nonsense schedules",
            prettyprint_usize(orig - self.people.len()),
            prettyprint_usize(orig)
        );
        // Fix up IDs
        for (idx, person) in self.people.iter_mut().enumerate() {
            person.id = PersonID(idx);
        }
        self
    }
}

fn seed_parked_cars(
    parked_cars: Vec<(Vehicle, BuildingID)>,
    sim: &mut Sim,
    map: &Map,
    base_rng: &mut XorShiftRng,
    timer: &mut Timer,
) {
    let mut open_spots_per_road: BTreeMap<RoadID, Vec<ParkingSpot>> = BTreeMap::new();
    for spot in sim.get_all_parking_spots().1 {
        let r = match spot {
            ParkingSpot::Onstreet(l, _) => map.get_l(l).parent,
            ParkingSpot::Offstreet(b, _) => map.get_l(map.get_b(b).sidewalk()).parent,
        };
        open_spots_per_road
            .entry(r)
            .or_insert_with(Vec::new)
            .push(spot);
    }
    // Changing parking on one road shouldn't affect far-off roads. Fork carefully.
    for r in map.all_roads() {
        let mut tmp_rng = abstutil::fork_rng(base_rng);
        if let Some(ref mut spots) = open_spots_per_road.get_mut(&r.id) {
            spots.shuffle(&mut tmp_rng);
        }
    }

    timer.start_iter("seed parked cars", parked_cars.len());
    let mut ok = true;
    for (vehicle, b) in parked_cars {
        timer.next();
        if !ok {
            continue;
        }
        if let Some(spot) = find_spot_near_building(b, &mut open_spots_per_road, map, timer) {
            sim.seed_parked_car(vehicle, spot);
        } else {
            timer.warn("Not enough room to seed parked cars.".to_string());
            ok = false;
        }
    }
}

// Pick a parking spot for this building. If the building's road has a free spot, use it. If not,
// start BFSing out from the road in a deterministic way until finding a nearby road with an open
// spot.
fn find_spot_near_building(
    b: BuildingID,
    open_spots_per_road: &mut BTreeMap<RoadID, Vec<ParkingSpot>>,
    map: &Map,
    timer: &mut Timer,
) -> Option<ParkingSpot> {
    let mut roads_queue: VecDeque<RoadID> = VecDeque::new();
    let mut visited: HashSet<RoadID> = HashSet::new();
    {
        let start = map.building_to_road(b).id;
        roads_queue.push_back(start);
        visited.insert(start);
    }

    loop {
        if roads_queue.is_empty() {
            timer.warn(format!(
                "Giving up looking for a free parking spot, searched {} roads of {}: {:?}",
                visited.len(),
                open_spots_per_road.len(),
                visited
            ));
        }
        let r = roads_queue.pop_front()?;
        if let Some(spots) = open_spots_per_road.get_mut(&r) {
            // TODO With some probability, skip this available spot and park farther away
            if !spots.is_empty() {
                return spots.pop();
            }
        }

        for next_r in map.get_next_roads(r).into_iter() {
            if !visited.contains(&next_r) {
                roads_queue.push_back(next_r);
                visited.insert(next_r);
            }
        }
    }
}

impl SpawnTrip {
    fn to_trip_spec(
        self,
        use_vehicle: Option<CarID>,
        rng: &mut XorShiftRng,
        map: &Map,
    ) -> TripSpec {
        match self {
            SpawnTrip::VehicleAppearing { start, goal, .. } => TripSpec::VehicleAppearing {
                start_pos: start,
                goal,
                use_vehicle: use_vehicle.unwrap(),
                retry_if_no_room: true,
                origin: None,
            },
            SpawnTrip::FromBorder {
                dr,
                goal,
                is_bike,
                origin,
            } => {
                if let Some(start_pos) = dr
                    .lanes(
                        if is_bike {
                            PathConstraints::Bike
                        } else {
                            PathConstraints::Car
                        },
                        map,
                    )
                    .choose(rng)
                    // TODO We could be more precise and say exactly what vehicle will be used here
                    .and_then(|l| {
                        TripSpec::spawn_vehicle_at(Position::new(*l, Distance::ZERO), is_bike, map)
                    })
                {
                    TripSpec::VehicleAppearing {
                        start_pos,
                        goal,
                        use_vehicle: use_vehicle.unwrap(),
                        retry_if_no_room: true,
                        origin,
                    }
                } else {
                    TripSpec::NoRoomToSpawn {
                        i: dr.src_i(map),
                        goal,
                        use_vehicle: use_vehicle.unwrap(),
                        origin,
                    }
                }
            }
            SpawnTrip::UsingParkedCar(start_bldg, goal) => TripSpec::UsingParkedCar {
                start_bldg,
                goal,
                car: use_vehicle.unwrap(),
            },
            SpawnTrip::UsingBike(start, goal) => TripSpec::UsingBike {
                bike: use_vehicle.unwrap(),
                start,
                goal,
            },
            SpawnTrip::JustWalking(start, goal) => TripSpec::JustWalking { start, goal },
            SpawnTrip::UsingTransit(start, goal, route, stop1, stop2) => TripSpec::UsingTransit {
                start,
                goal,
                route,
                stop1,
                stop2,
            },
            SpawnTrip::Remote {
                from,
                to,
                trip_time,
                mode,
            } => TripSpec::Remote {
                from,
                to,
                trip_time,
                mode,
            },
        }
    }

    pub fn start(&self, map: &Map) -> TripEndpoint {
        match self {
            SpawnTrip::VehicleAppearing { ref start, .. } => {
                TripEndpoint::Border(map.get_l(start.lane()).src_i, None)
            }
            SpawnTrip::FromBorder { dr, ref origin, .. } => {
                TripEndpoint::Border(dr.src_i(map), origin.clone())
            }
            SpawnTrip::UsingParkedCar(b, _) => TripEndpoint::Bldg(*b),
            SpawnTrip::UsingBike(ref spot, _)
            | SpawnTrip::JustWalking(ref spot, _)
            | SpawnTrip::UsingTransit(ref spot, _, _, _, _) => match spot.connection {
                SidewalkPOI::Building(b) => TripEndpoint::Bldg(b),
                SidewalkPOI::Border(i, ref loc) => TripEndpoint::Border(i, loc.clone()),
                SidewalkPOI::SuddenlyAppear => {
                    TripEndpoint::Border(map.get_l(spot.sidewalk_pos.lane()).src_i, None)
                }
                _ => unreachable!(),
            },
            // Pick an arbitrary border
            SpawnTrip::Remote { ref from, .. } => {
                TripEndpoint::Border(map.all_outgoing_borders()[0].id, Some(from.clone()))
            }
        }
    }

    pub fn end(&self, map: &Map) -> TripEndpoint {
        match self {
            SpawnTrip::VehicleAppearing { ref goal, .. }
            | SpawnTrip::FromBorder { ref goal, .. }
            | SpawnTrip::UsingParkedCar(_, ref goal)
            | SpawnTrip::UsingBike(_, ref goal) => match goal {
                DrivingGoal::ParkNear(b) => TripEndpoint::Bldg(*b),
                DrivingGoal::Border(i, _, ref loc) => TripEndpoint::Border(*i, loc.clone()),
            },
            SpawnTrip::JustWalking(_, ref spot) | SpawnTrip::UsingTransit(_, ref spot, _, _, _) => {
                match spot.connection {
                    SidewalkPOI::Building(b) => TripEndpoint::Bldg(b),
                    SidewalkPOI::Border(i, ref loc) => TripEndpoint::Border(i, loc.clone()),
                    _ => unreachable!(),
                }
            }
            // Pick an arbitrary border
            SpawnTrip::Remote { ref to, .. } => {
                TripEndpoint::Border(map.all_incoming_borders()[0].id, Some(to.clone()))
            }
        }
    }
}

impl PersonSpec {
    // Verify that the trip start/endpoints of the person match up
    fn check_schedule(&self, map: &Map) -> Result<(), String> {
        for pair in self.trips.iter().zip(self.trips.iter().skip(1)) {
            if pair.0.depart >= pair.1.depart {
                return Err(format!(
                    "{} {:?} starts two trips in the wrong order: {} then {}",
                    self.id, self.orig_id, pair.0.depart, pair.1.depart
                ));
            }

            // Once off-map, re-enter via any border node.
            let end_bldg = match pair.0.trip.end(map) {
                TripEndpoint::Bldg(b) => Some(b),
                TripEndpoint::Border(_, _) => None,
            };
            let start_bldg = match pair.1.trip.start(map) {
                TripEndpoint::Bldg(b) => Some(b),
                TripEndpoint::Border(_, _) => None,
            };

            if end_bldg != start_bldg {
                return Err(format!(
                    "At {}, {} {:?} warps between some trips, from {:?} to {:?}",
                    pair.1.depart, self.id, self.orig_id, end_bldg, start_bldg
                ));
            }

            // But actually, make sure pairs of remote trips match up.
            if let (SpawnTrip::Remote { ref to, .. }, SpawnTrip::Remote { ref from, .. }) =
                (&pair.0.trip, &pair.1.trip)
            {
                if to != from {
                    return Err(format!(
                        "At {}, {} {:?} warps between some trips, from {:?} to {:?}",
                        pair.1.depart, self.id, self.orig_id, to, from
                    ));
                }
            }
        }
        Ok(())
    }

    fn get_vehicles(
        &self,
        rng: &mut XorShiftRng,
    ) -> (
        Vec<VehicleSpec>,
        Vec<(usize, BuildingID)>,
        Vec<Option<usize>>,
    ) {
        let mut vehicle_specs = Vec::new();
        let mut cars_initially_parked_at = Vec::new();
        let mut vehicle_foreach_trip = Vec::new();

        let mut bike_idx = None;
        // For each indexed car, is it parked somewhere, or off-map?
        let mut car_locations: Vec<(usize, Option<BuildingID>)> = Vec::new();

        for trip in &self.trips {
            let use_for_trip = match trip.trip {
                SpawnTrip::VehicleAppearing {
                    is_bike, ref goal, ..
                }
                | SpawnTrip::FromBorder {
                    is_bike, ref goal, ..
                } => {
                    if is_bike {
                        if bike_idx.is_none() {
                            bike_idx = Some(vehicle_specs.len());
                            vehicle_specs.push(Scenario::rand_bike(rng));
                        }
                        bike_idx
                    } else {
                        // Any available cars off-map?
                        let idx = if let Some(idx) = car_locations
                            .iter()
                            .find(|(_, parked_at)| parked_at.is_none())
                            .map(|(idx, _)| *idx)
                        {
                            idx
                        } else {
                            // Need a new car, starting off-map
                            let idx = vehicle_specs.len();
                            vehicle_specs.push(Scenario::rand_car(rng));
                            idx
                        };

                        // Where does this car wind up?
                        car_locations.retain(|(i, _)| idx != *i);
                        match goal {
                            DrivingGoal::ParkNear(b) => {
                                car_locations.push((idx, Some(*b)));
                            }
                            DrivingGoal::Border(_, _, _) => {
                                car_locations.push((idx, None));
                            }
                        }

                        Some(idx)
                    }
                }
                SpawnTrip::UsingParkedCar(b, ref goal) => {
                    // Is there already a car parked here?
                    let idx = if let Some(idx) = car_locations
                        .iter()
                        .find(|(_, parked_at)| *parked_at == Some(b))
                        .map(|(idx, _)| *idx)
                    {
                        idx
                    } else {
                        // Need a new car, starting at this building
                        let idx = vehicle_specs.len();
                        vehicle_specs.push(Scenario::rand_car(rng));
                        cars_initially_parked_at.push((idx, b));
                        idx
                    };

                    // Where does this car wind up?
                    car_locations.retain(|(i, _)| idx != *i);
                    match goal {
                        DrivingGoal::ParkNear(b) => {
                            car_locations.push((idx, Some(*b)));
                        }
                        DrivingGoal::Border(_, _, _) => {
                            car_locations.push((idx, None));
                        }
                    }

                    Some(idx)
                }
                SpawnTrip::UsingBike(_, _) => {
                    if bike_idx.is_none() {
                        bike_idx = Some(vehicle_specs.len());
                        vehicle_specs.push(Scenario::rand_bike(rng));
                    }
                    bike_idx
                }
                SpawnTrip::JustWalking(_, _) | SpawnTrip::UsingTransit(_, _, _, _, _) => None,
                SpawnTrip::Remote { .. } => None,
            };
            vehicle_foreach_trip.push(use_for_trip);
        }

        // For debugging
        if false {
            let mut n = vehicle_specs.len();
            if bike_idx.is_some() {
                n -= 1;
            }
            if n > 1 {
                println!("{} needs {} cars", self.id, n);
            }
        }

        (
            vehicle_specs,
            cars_initially_parked_at,
            vehicle_foreach_trip,
        )
    }
}
