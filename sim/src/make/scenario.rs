use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::fmt;

use anyhow::Result;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{prettyprint_usize, Counter, Parallelism, Timer};
use geom::{Distance, Speed, Time};
use map_model::{BuildingID, Map, OffstreetParking, RoadID};

use crate::make::fork_rng;
use crate::{
    OrigPersonID, ParkingSpot, Sim, TripEndpoint, TripInfo, TripMode, TripSpec, Vehicle,
    VehicleSpec, VehicleType, BIKE_LENGTH, MAX_CAR_LENGTH, MIN_CAR_LENGTH,
};

/// A Scenario describes all the input to a simulation. Usually a scenario covers one day.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Scenario {
    pub scenario_name: String,
    pub map_name: MapName,

    pub people: Vec<PersonSpec>,
    /// None means seed all buses. Otherwise the route name must be present here.
    pub only_seed_buses: Option<BTreeSet<String>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PersonSpec {
    /// Just used for debugging
    pub orig_id: Option<OrigPersonID>,
    /// The first trip starts here
    pub origin: TripEndpoint,
    /// Each trip starts at the destination of the previous trip
    pub trips: Vec<IndividTrip>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IndividTrip {
    pub depart: Time,
    pub destination: TripEndpoint,
    pub mode: TripMode,
    pub purpose: TripPurpose,
    pub cancelled: bool,
    /// Did a ScenarioModifier affect this?
    pub modified: bool,
}

impl IndividTrip {
    pub fn new(
        depart: Time,
        purpose: TripPurpose,
        destination: TripEndpoint,
        mode: TripMode,
    ) -> IndividTrip {
        IndividTrip {
            depart,
            destination,
            mode,
            purpose,
            cancelled: false,
            modified: false,
        }
    }
}

/// Lifted from Seattle's Soundcast model, but seems general enough to use anyhere.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum TripPurpose {
    Home,
    Work,
    School,
    Escort,
    PersonalBusiness,
    Shopping,
    Meal,
    Social,
    Recreation,
    Medical,
    ParkAndRideTransfer,
}

impl fmt::Display for TripPurpose {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TripPurpose::Home => "home",
                TripPurpose::Work => "work",
                TripPurpose::School => "school",
                // Is this like a parent escorting a child to school?
                TripPurpose::Escort => "escort",
                TripPurpose::PersonalBusiness => "personal business",
                TripPurpose::Shopping => "shopping",
                TripPurpose::Meal => "eating",
                TripPurpose::Social => "social",
                TripPurpose::Recreation => "recreation",
                TripPurpose::Medical => "medical",
                TripPurpose::ParkAndRideTransfer => "park-and-ride transfer",
            }
        )
    }
}

impl Scenario {
    pub fn instantiate(&self, sim: &mut Sim, map: &Map, rng: &mut XorShiftRng, timer: &mut Timer) {
        self.instantiate_without_retries(sim, map, rng, true, timer);
    }

    /// If retry_if_no_room is false, any vehicles that fail to spawn because of something else in
    /// the way will just wind up as cancelled trips.
    pub fn instantiate_without_retries(
        &self,
        sim: &mut Sim,
        map: &Map,
        rng: &mut XorShiftRng,
        retry_if_no_room: bool,
        timer: &mut Timer,
    ) {
        // Any case where map edits could change the calls to the RNG, we have to fork.
        sim.set_name(self.scenario_name.clone());

        timer.start(format!("Instantiating {}", self.scenario_name));

        if let Some(ref routes) = self.only_seed_buses {
            for route in map.all_bus_routes() {
                if routes.contains(&route.full_name) {
                    sim.seed_bus_route(route);
                }
            }
        } else {
            // All of them
            for route in map.all_bus_routes() {
                sim.seed_bus_route(route);
            }
        }

        timer.start_iter("trips for People", self.people.len());
        let mut parked_cars: Vec<(Vehicle, BuildingID)> = Vec::new();
        let mut schedule_trips = Vec::new();
        for p in &self.people {
            timer.next();

            if let Err(err) = p.check_schedule() {
                panic!("{}", err);
            }

            let (vehicle_specs, cars_initially_parked_at, vehicle_foreach_trip) =
                p.get_vehicles(rng);
            let person = sim.new_person(p.orig_id, Scenario::rand_ped_speed(rng), vehicle_specs);
            for (idx, b) in cars_initially_parked_at {
                parked_cars.push((person.vehicles[idx].clone(), b));
            }
            let mut from = p.origin.clone();
            for (t, maybe_idx) in p.trips.iter().zip(vehicle_foreach_trip) {
                // The RNG call might change over edits for picking the spawning lane from a border
                // with multiple choices for a vehicle type.
                let mut tmp_rng = fork_rng(rng);
                let spec = match TripSpec::maybe_new(
                    from.clone(),
                    t.destination.clone(),
                    t.mode,
                    maybe_idx.map(|idx| person.vehicles[idx].id),
                    retry_if_no_room,
                    &mut tmp_rng,
                    map,
                ) {
                    Ok(spec) => spec,
                    Err(error) => TripSpec::SpawningFailure {
                        use_vehicle: maybe_idx.map(|idx| person.vehicles[idx].id),
                        error: error.to_string(),
                    },
                };
                schedule_trips.push((
                    person.id,
                    spec,
                    TripInfo {
                        departure: t.depart,
                        mode: t.mode,
                        start: from,
                        end: t.destination.clone(),
                        purpose: t.purpose,
                        modified: t.modified,
                        capped: false,
                        cancellation_reason: if t.cancelled {
                            Some(format!("cancelled by ScenarioModifier"))
                        } else {
                            None
                        },
                    },
                ));
                from = t.destination.clone();
            }
        }

        let results = timer.parallelize(
            "schedule trips",
            Parallelism::Fastest,
            schedule_trips,
            |(p, spec, info)| spec.to_plan(p, info, map),
        );

        // parked_cars is stable over map edits, so don't fork.
        parked_cars.shuffle(rng);
        seed_parked_cars(parked_cars, sim, map, rng, timer);

        sim.spawn_trips(results, map, timer);
        timer.stop(format!("Instantiating {}", self.scenario_name));
    }

    pub fn save(&self) {
        abstio::write_binary(
            abstio::path_scenario(&self.map_name, &self.scenario_name),
            self,
        );
    }

    pub fn empty(map: &Map, name: &str) -> Scenario {
        Scenario {
            scenario_name: name.to_string(),
            map_name: map.get_name().clone(),
            people: Vec::new(),
            only_seed_buses: Some(BTreeSet::new()),
        }
    }

    fn rand_car(rng: &mut XorShiftRng) -> VehicleSpec {
        let length = Scenario::rand_dist(rng, MIN_CAR_LENGTH, MAX_CAR_LENGTH);
        VehicleSpec {
            vehicle_type: VehicleType::Car,
            length,
            max_speed: None,
        }
    }

    fn rand_bike(rng: &mut XorShiftRng) -> VehicleSpec {
        let max_speed = Some(Scenario::rand_speed(
            rng,
            Speed::miles_per_hour(8.0),
            Scenario::max_bike_speed(),
        ));
        VehicleSpec {
            vehicle_type: VehicleType::Bike,
            length: BIKE_LENGTH,
            max_speed,
        }
    }
    pub fn max_bike_speed() -> Speed {
        Speed::miles_per_hour(10.0)
    }

    pub fn rand_dist(rng: &mut XorShiftRng, low: Distance, high: Distance) -> Distance {
        assert!(high > low);
        Distance::meters(rng.gen_range(low.inner_meters()..high.inner_meters()))
    }

    fn rand_speed(rng: &mut XorShiftRng, low: Speed, high: Speed) -> Speed {
        assert!(high > low);
        Speed::meters_per_second(
            rng.gen_range(low.inner_meters_per_second()..high.inner_meters_per_second()),
        )
    }

    pub fn rand_ped_speed(rng: &mut XorShiftRng) -> Speed {
        Scenario::rand_speed(rng, Speed::miles_per_hour(2.0), Speed::miles_per_hour(3.0))
    }
    pub fn max_ped_speed() -> Speed {
        Speed::miles_per_hour(3.0)
    }

    pub fn count_parked_cars_per_bldg(&self) -> Counter<BuildingID> {
        let mut per_bldg = Counter::new();
        // Pass in a dummy RNG
        let mut rng = XorShiftRng::seed_from_u64(0);
        for p in &self.people {
            let (_, cars_initially_parked_at, _) = p.get_vehicles(&mut rng);
            for (_, b) in cars_initially_parked_at {
                per_bldg.inc(b);
            }
        }
        per_bldg
    }

    pub fn remove_weird_schedules(mut self) -> Scenario {
        let orig = self.people.len();
        self.people.retain(|person| match person.check_schedule() {
            Ok(()) => true,
            Err(err) => {
                println!("{}", err);
                false
            }
        });
        warn!(
            "{} of {} people have nonsense schedules",
            prettyprint_usize(orig - self.people.len()),
            prettyprint_usize(orig)
        );
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
    if sim.infinite_parking() {
        let mut blackholed = 0;
        for (vehicle, b) in parked_cars {
            if let Some(spot) = sim.get_free_offstreet_spots(b).pop() {
                sim.seed_parked_car(vehicle, spot);
            } else {
                blackholed += 1;
            }
        }
        if blackholed > 0 {
            timer.warn(format!(
                "{} parked cars weren't seeded, due to blackholed buildings",
                prettyprint_usize(blackholed)
            ));
        }
        return;
    }

    let mut open_spots_per_road: BTreeMap<RoadID, Vec<(ParkingSpot, Option<BuildingID>)>> =
        BTreeMap::new();
    for spot in sim.get_all_parking_spots().1 {
        let (r, restriction) = match spot {
            ParkingSpot::Onstreet(l, _) => (map.get_l(l).parent, None),
            ParkingSpot::Offstreet(b, _) => (
                map.get_l(map.get_b(b).sidewalk()).parent,
                match map.get_b(b).parking {
                    OffstreetParking::PublicGarage(_, _) => None,
                    OffstreetParking::Private(_, _) => Some(b),
                },
            ),
            ParkingSpot::Lot(pl, _) => (map.get_l(map.get_pl(pl).driving_pos.lane()).parent, None),
        };
        open_spots_per_road
            .entry(r)
            .or_insert_with(Vec::new)
            .push((spot, restriction));
    }
    // Changing parking on one road shouldn't affect far-off roads. Fork carefully.
    for r in map.all_roads() {
        let mut tmp_rng = fork_rng(base_rng);
        if let Some(ref mut spots) = open_spots_per_road.get_mut(&r.id) {
            spots.shuffle(&mut tmp_rng);
        }
    }

    timer.start_iter("seed parked cars", parked_cars.len());
    let mut ok = true;
    let total_cars = parked_cars.len();
    let mut seeded = 0;
    for (vehicle, b) in parked_cars {
        timer.next();
        if !ok {
            continue;
        }
        if let Some(spot) = find_spot_near_building(b, &mut open_spots_per_road, map) {
            seeded += 1;
            sim.seed_parked_car(vehicle, spot);
        } else {
            timer.warn(format!(
                "Not enough room to seed parked cars. Only found spots for {} of {}",
                prettyprint_usize(seeded),
                prettyprint_usize(total_cars)
            ));
            ok = false;
        }
    }
}

// Pick a parking spot for this building. If the building's road has a free spot, use it. If not,
// start BFSing out from the road in a deterministic way until finding a nearby road with an open
// spot.
fn find_spot_near_building(
    b: BuildingID,
    open_spots_per_road: &mut BTreeMap<RoadID, Vec<(ParkingSpot, Option<BuildingID>)>>,
    map: &Map,
) -> Option<ParkingSpot> {
    let mut roads_queue: VecDeque<RoadID> = VecDeque::new();
    let mut visited: HashSet<RoadID> = HashSet::new();
    {
        let start = map.building_to_road(b).id;
        roads_queue.push_back(start);
        visited.insert(start);
    }

    loop {
        let r = roads_queue.pop_front()?;
        if let Some(spots) = open_spots_per_road.get_mut(&r) {
            // Fill in all private parking first before
            // TODO With some probability, skip this available spot and park farther away
            if let Some(idx) = spots
                .iter()
                .position(|(_, restriction)| restriction == &Some(b))
            {
                return Some(spots.remove(idx).0);
            }
            if let Some(idx) = spots
                .iter()
                .position(|(_, restriction)| restriction.is_none())
            {
                return Some(spots.remove(idx).0);
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

impl PersonSpec {
    /// Verify that a person's trips make sense
    fn check_schedule(&self) -> Result<()> {
        for pair in self.trips.windows(2) {
            if pair[0].depart >= pair[1].depart {
                bail!(
                    "Person ({:?}) starts two trips in the wrong order: {} then {}",
                    self.orig_id,
                    pair[0].depart,
                    pair[1].depart
                );
            }
        }

        let mut endpts = vec![self.origin.clone()];
        for t in &self.trips {
            endpts.push(t.destination.clone());
        }
        for pair in endpts.windows(2) {
            if pair[0] == pair[1] {
                bail!(
                    "Person ({:?}) has two adjacent trips between the same place: {:?}",
                    self.orig_id,
                    pair[0]
                );
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

        // TODO If the trip is cancelled, this should be affected...
        let mut from = self.origin.clone();
        for trip in &self.trips {
            let use_for_trip = match trip.mode {
                TripMode::Walk | TripMode::Transit => None,
                TripMode::Bike => {
                    if bike_idx.is_none() {
                        bike_idx = Some(vehicle_specs.len());
                        vehicle_specs.push(Scenario::rand_bike(rng));
                    }
                    bike_idx
                }
                TripMode::Drive => {
                    let need_parked_at = match from {
                        TripEndpoint::Bldg(b) => Some(b),
                        _ => None,
                    };

                    // Any available cars in the right spot?
                    let idx = if let Some(idx) = car_locations
                        .iter()
                        .find(|(_, parked_at)| *parked_at == need_parked_at)
                        .map(|(idx, _)| *idx)
                    {
                        idx
                    } else {
                        // Need a new car, starting in the right spot
                        let idx = vehicle_specs.len();
                        vehicle_specs.push(Scenario::rand_car(rng));
                        if let Some(b) = need_parked_at {
                            cars_initially_parked_at.push((idx, b));
                        }
                        idx
                    };

                    // Where does this car wind up?
                    car_locations.retain(|(i, _)| idx != *i);
                    match trip.destination {
                        TripEndpoint::Bldg(b) => {
                            car_locations.push((idx, Some(b)));
                        }
                        TripEndpoint::Border(_) | TripEndpoint::SuddenlyAppear(_) => {
                            car_locations.push((idx, None));
                        }
                    }

                    Some(idx)
                }
            };
            from = trip.destination.clone();
            vehicle_foreach_trip.push(use_for_trip);
        }

        // For debugging
        if false {
            let mut n = vehicle_specs.len();
            if bike_idx.is_some() {
                n -= 1;
            }
            if n > 1 {
                println!("Someone needs {} cars", n);
            }
        }

        (
            vehicle_specs,
            cars_initially_parked_at,
            vehicle_foreach_trip,
        )
    }
}
