use crate::{
    DrivingGoal, ParkingSpot, PersonID, SidewalkPOI, SidewalkSpot, Sim, TripSpec, VehicleSpec,
    VehicleType, BIKE_LENGTH, MAX_CAR_LENGTH, MIN_CAR_LENGTH,
};
use abstutil::{MultiMap, Timer};
use geom::{Distance, Duration, Speed, Time};
use map_model::{BuildingID, BusRouteID, BusStopID, IntersectionID, Map, Position, RoadID};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde_derive::{Deserialize, Serialize};
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
    pub trips: Vec<IndividTrip>,
    // 3 possibilities: no car, car appears from outside the map, or car starts at a building
    pub has_car: bool,
    pub car_initially_parked_at: Option<BuildingID>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IndividTrip {
    pub depart: Time,
    pub trip: SpawnTrip,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SpawnTrip {
    CarAppearing {
        // TODO Replace start with building|border
        start: Position,
        goal: DrivingGoal,
        // For bikes starting at a border, use CarAppearing. UsingBike implies a walk->bike trip.
        is_bike: bool,
    },
    MaybeUsingParkedCar(BuildingID, DrivingGoal),
    UsingBike(SidewalkSpot, DrivingGoal),
    JustWalking(SidewalkSpot, SidewalkSpot),
    UsingTransit(SidewalkSpot, SidewalkSpot, BusRouteID, BusStopID, BusStopID),
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

        let mut spawner = sim.make_spawner();

        let mut parked_cars: Vec<(BuildingID, PersonID)> = Vec::new();
        for (b, owners) in self.parked_cars_per_bldg().consume() {
            for p in owners {
                parked_cars.push((b, p));
            }
        }
        // parked_cars is stable over map edits, so don't fork.
        parked_cars.shuffle(rng);
        seed_parked_cars(parked_cars, sim, map, rng, timer);

        timer.start_iter("trips for People", self.people.len());
        for p in &self.people {
            timer.next();
            // TODO Or spawner?
            sim.new_person(p.id, p.has_car);
            for t in &p.trips {
                // The RNG call is stable over edits.
                let spec = t.trip.clone().to_trip_spec(rng);
                spawner.schedule_trip(p.id, t.depart, spec, map, sim);
            }
        }

        sim.flush_spawner(spawner, map, timer, true);
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
        // 2-3mph
        Scenario::rand_speed(
            rng,
            Speed::meters_per_second(0.894),
            Speed::meters_per_second(1.34),
        )
    }

    // TODO Utter hack. Blindly repeats all trips taken by each person every day. If
    // avoid_inbound_trips is true, then don't repeat driving trips that start outside the map and
    // come in, because those often lead to parking spots leaking. This isn't realistic, but none
    // of this is; even the original 1-day scenario doesn't yet guarantee continuity of people. A
    // person might be in the middle of one trip, and they start the next one!
    pub fn repeat_days(mut self, days: usize, avoid_inbound_trips: bool) -> Scenario {
        self.scenario_name = format!("{} repeated for {} days", self.scenario_name, days);
        for person in &mut self.people {
            let mut trips = Vec::new();
            let mut offset = Duration::ZERO;
            for day in 0..days {
                for trip in &person.trips {
                    let inbound = match trip.trip {
                        SpawnTrip::CarAppearing { is_bike, .. } => !is_bike,
                        _ => false,
                    };
                    if day > 0 && inbound && avoid_inbound_trips {
                        continue;
                    }

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

    pub fn parked_cars_per_bldg(&self) -> MultiMap<BuildingID, PersonID> {
        let mut per_bldg = MultiMap::new();
        for p in &self.people {
            if let Some(b) = p.car_initially_parked_at {
                per_bldg.insert(b, p.id);
            }
        }
        per_bldg
    }
}

fn seed_parked_cars(
    parked_cars: Vec<(BuildingID, PersonID)>,
    sim: &mut Sim,
    map: &Map,
    base_rng: &mut XorShiftRng,
    timer: &mut Timer,
) {
    // We always need the same number of cars
    let mut rand_cars: Vec<VehicleSpec> = std::iter::repeat_with(|| Scenario::rand_car(base_rng))
        .take(parked_cars.len())
        .collect();

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
    for (b, owner) in parked_cars {
        timer.next();
        if !ok {
            continue;
        }
        if let Some(spot) = find_spot_near_building(b, &mut open_spots_per_road, map, timer) {
            sim.seed_parked_car(rand_cars.pop().unwrap(), spot, Some(owner));
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
    fn to_trip_spec(self, rng: &mut XorShiftRng) -> TripSpec {
        match self {
            SpawnTrip::CarAppearing {
                start,
                goal,
                is_bike,
                ..
            } => TripSpec::CarAppearing {
                start_pos: start,
                goal,
                vehicle_spec: if is_bike {
                    Scenario::rand_bike(rng)
                } else {
                    Scenario::rand_car(rng)
                },
                ped_speed: Scenario::rand_ped_speed(rng),
            },
            SpawnTrip::MaybeUsingParkedCar(start_bldg, goal) => TripSpec::MaybeUsingParkedCar {
                start_bldg,
                goal,
                ped_speed: Scenario::rand_ped_speed(rng),
            },
            SpawnTrip::UsingBike(start, goal) => TripSpec::UsingBike {
                start,
                goal,
                vehicle: Scenario::rand_bike(rng),
                ped_speed: Scenario::rand_ped_speed(rng),
            },
            SpawnTrip::JustWalking(start, goal) => TripSpec::JustWalking {
                start,
                goal,
                ped_speed: Scenario::rand_ped_speed(rng),
            },
            SpawnTrip::UsingTransit(start, goal, route, stop1, stop2) => TripSpec::UsingTransit {
                start,
                goal,
                route,
                stop1,
                stop2,
                ped_speed: Scenario::rand_ped_speed(rng),
            },
        }
    }

    pub fn start_from_bldg(&self) -> Option<BuildingID> {
        match self {
            SpawnTrip::CarAppearing { .. } => None,
            SpawnTrip::MaybeUsingParkedCar(b, _) => Some(*b),
            SpawnTrip::UsingBike(ref spot, _)
            | SpawnTrip::JustWalking(ref spot, _)
            | SpawnTrip::UsingTransit(ref spot, _, _, _, _) => match spot.connection {
                SidewalkPOI::Building(b) => Some(b),
                _ => None,
            },
        }
    }

    pub fn start_from_border(&self) -> Option<IntersectionID> {
        match self {
            // TODO CarAppearing might be from a border
            SpawnTrip::CarAppearing { .. } => None,
            SpawnTrip::MaybeUsingParkedCar(_, _) => None,
            SpawnTrip::UsingBike(ref spot, _)
            | SpawnTrip::JustWalking(ref spot, _)
            | SpawnTrip::UsingTransit(ref spot, _, _, _, _) => match spot.connection {
                SidewalkPOI::Border(i) => Some(i),
                _ => None,
            },
        }
    }

    pub fn end_at_bldg(&self) -> Option<BuildingID> {
        match self {
            SpawnTrip::CarAppearing { ref goal, .. }
            | SpawnTrip::MaybeUsingParkedCar(_, ref goal)
            | SpawnTrip::UsingBike(_, ref goal) => match goal {
                DrivingGoal::ParkNear(b) => Some(*b),
                DrivingGoal::Border(_, _) => None,
            },
            SpawnTrip::JustWalking(_, ref spot) | SpawnTrip::UsingTransit(_, ref spot, _, _, _) => {
                match spot.connection {
                    SidewalkPOI::Building(b) => Some(b),
                    _ => None,
                }
            }
        }
    }

    pub fn end_at_border(&self) -> Option<IntersectionID> {
        match self {
            SpawnTrip::CarAppearing { ref goal, .. }
            | SpawnTrip::MaybeUsingParkedCar(_, ref goal)
            | SpawnTrip::UsingBike(_, ref goal) => match goal {
                DrivingGoal::ParkNear(_) => None,
                DrivingGoal::Border(i, _) => Some(*i),
            },
            SpawnTrip::JustWalking(_, ref spot) | SpawnTrip::UsingTransit(_, ref spot, _, _, _) => {
                match spot.connection {
                    SidewalkPOI::Border(i) => Some(i),
                    _ => None,
                }
            }
        }
    }
}
