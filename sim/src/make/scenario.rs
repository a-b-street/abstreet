use std::collections::{BTreeMap, HashSet, VecDeque};

use anyhow::Result;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use abstutil::{prettyprint_usize, Counter, Timer};
use geom::{Distance, Pt2D, Speed};
use map_model::{
    BuildingID, Map, OffstreetParking, PathConstraints, PathRequest, Position, RoadID,
};
use synthpop::{Scenario, TripEndpoint, TripMode};

use crate::make::fork_rng;
use crate::{
    DrivingGoal, ParkingSpot, SidewalkSpot, Sim, StartTripArgs, TripInfo, Vehicle, VehicleSpec,
    VehicleType, BIKE_LENGTH, MAX_CAR_LENGTH, MIN_CAR_LENGTH,
};

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
        sim.set_run_name(self.scenario_name.clone());

        timer.start(format!("Instantiating {}", self.scenario_name));

        if let Some(ref routes) = self.only_seed_buses {
            for route in map.all_transit_routes() {
                if routes.contains(&route.long_name) {
                    sim.seed_bus_route(route);
                }
            }
        } else {
            // All of them
            for route in map.all_transit_routes() {
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
            for (trip, maybe_idx) in p.trips.iter().zip(vehicle_foreach_trip) {
                schedule_trips.push((
                    person.id,
                    TripInfo {
                        departure: trip.depart,
                        mode: trip.mode,
                        start: trip.origin,
                        end: trip.destination,
                        purpose: trip.purpose,
                        modified: trip.modified,
                        cancellation_reason: if trip.cancelled {
                            Some("cancelled by ScenarioModifier".to_string())
                        } else {
                            None
                        },
                    },
                    StartTripArgs {
                        retry_if_no_room,
                        use_vehicle: maybe_idx.map(|idx| person.vehicles[idx].id),
                    },
                ));
            }
        }

        // parked_cars is stable over map edits, so don't fork.
        parked_cars.shuffle(rng);
        seed_parked_cars(parked_cars, sim, map, rng, timer);

        sim.spawn_trips(schedule_trips, map, timer);
        timer.stop(format!("Instantiating {}", self.scenario_name));
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
                    let need_parked_at = match trip.origin {
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
            map_model::MAX_BIKE_SPEED,
        ));
        VehicleSpec {
            vehicle_type: VehicleType::Bike,
            length: BIKE_LENGTH,
            max_speed,
        }
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
        Scenario::rand_speed(
            rng,
            Speed::miles_per_hour(2.0),
            map_model::MAX_WALKING_SPEED,
        )
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
        timer.start_iter("seed parked cars in infinite mode", parked_cars.len());
        for (vehicle, b) in parked_cars {
            timer.next();
            if let Some(spot) = sim.get_free_offstreet_spots(b).pop() {
                sim.seed_parked_car(vehicle, spot);
            } else {
                blackholed += 1;
            }
        }
        if blackholed > 0 {
            warn!(
                "{} parked cars weren't seeded, due to blackholed buildings",
                prettyprint_usize(blackholed)
            );
        }
        return;
    }

    let mut open_spots_per_road: BTreeMap<RoadID, Vec<(ParkingSpot, Option<BuildingID>)>> =
        BTreeMap::new();
    for spot in sim.get_all_parking_spots().1 {
        let (r, restriction) = match spot {
            ParkingSpot::Onstreet(l, _) => (l.road, None),
            ParkingSpot::Offstreet(b, _) => (
                map.get_b(b).sidewalk().road,
                match map.get_b(b).parking {
                    OffstreetParking::PublicGarage(_, _) => None,
                    OffstreetParking::Private(_, _) => Some(b),
                },
            ),
            ParkingSpot::Lot(pl, _) => (map.get_pl(pl).driving_pos.lane().road, None),
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
            warn!(
                "Not enough room to seed parked cars. Only found spots for {} of {}",
                prettyprint_usize(seeded),
                prettyprint_usize(total_cars)
            );
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

impl TripEndpoint {
    /// Figure out a single PathRequest that goes between two TripEndpoints. Assume a single mode
    /// the entire time -- no walking to a car before driving, for instance. The result probably
    /// won't be exactly what would happen on a real trip between the endpoints because of this
    /// assumption.
    pub fn path_req(
        from: TripEndpoint,
        to: TripEndpoint,
        mode: TripMode,
        map: &Map,
    ) -> Option<PathRequest> {
        let start = from.pos(mode, true, map)?;
        let end = to.pos(mode, false, map)?;
        Some(match mode {
            TripMode::Walk | TripMode::Transit => PathRequest::walking(start, end),
            TripMode::Bike => PathRequest::vehicle(start, end, PathConstraints::Bike),
            // Only cars leaving from a building might turn out from the driveway in a special way
            TripMode::Drive => {
                if matches!(from, TripEndpoint::Bldg(_)) {
                    PathRequest::leave_from_driveway(start, end, PathConstraints::Car, map)
                } else {
                    PathRequest::vehicle(start, end, PathConstraints::Car)
                }
            }
        })
    }

    fn start_sidewalk_spot(&self, map: &Map) -> Result<SidewalkSpot> {
        match self {
            TripEndpoint::Bldg(b) => Ok(SidewalkSpot::building(*b, map)),
            TripEndpoint::Border(i) => SidewalkSpot::start_at_border(*i, map)
                .ok_or_else(|| anyhow!("can't start walking from {}", i)),
            TripEndpoint::SuddenlyAppear(pos) => Ok(SidewalkSpot::suddenly_appear(*pos, map)),
        }
    }

    fn end_sidewalk_spot(&self, map: &Map) -> Result<SidewalkSpot> {
        match self {
            TripEndpoint::Bldg(b) => Ok(SidewalkSpot::building(*b, map)),
            TripEndpoint::Border(i) => SidewalkSpot::end_at_border(*i, map)
                .ok_or_else(|| anyhow!("can't end walking at {}", i)),
            TripEndpoint::SuddenlyAppear(_) => unreachable!(),
        }
    }

    fn driving_goal(&self, constraints: PathConstraints, map: &Map) -> Result<DrivingGoal> {
        match self {
            TripEndpoint::Bldg(b) => Ok(DrivingGoal::ParkNear(*b)),
            TripEndpoint::Border(i) => map
                .get_i(*i)
                .some_incoming_road(map)
                .and_then(|dr| {
                    let lanes = dr.lanes(constraints, map);
                    if lanes.is_empty() {
                        None
                    } else {
                        // TODO ideally could use any
                        Some(DrivingGoal::Border(dr.dst_i(map), lanes[0]))
                    }
                })
                .ok_or_else(|| anyhow!("can't end at {} for {:?}", i, constraints)),
            TripEndpoint::SuddenlyAppear(_) => unreachable!(),
        }
    }

    fn pos(self, mode: TripMode, from: bool, map: &Map) -> Option<Position> {
        match mode {
            TripMode::Walk | TripMode::Transit => (if from {
                self.start_sidewalk_spot(map)
            } else {
                self.end_sidewalk_spot(map)
            })
            .ok()
            .map(|spot| spot.sidewalk_pos),
            TripMode::Drive | TripMode::Bike => {
                if from {
                    match self {
                        // Fall through and use DrivingGoal also to start.
                        TripEndpoint::Bldg(_) => {}
                        TripEndpoint::Border(i) => {
                            return map.get_i(i).some_outgoing_road(map).and_then(|dr| {
                                dr.lanes(mode.to_constraints(), map)
                                    .get(0)
                                    .map(|l| Position::start(*l))
                            });
                        }
                        TripEndpoint::SuddenlyAppear(pos) => {
                            return Some(pos);
                        }
                    }
                }
                self.driving_goal(mode.to_constraints(), map)
                    .ok()
                    .and_then(|goal| goal.goal_pos(mode.to_constraints(), map))
            }
        }
    }

    /// Returns a point representing where this endpoint is.
    pub fn pt(&self, map: &Map) -> Pt2D {
        match self {
            TripEndpoint::Bldg(b) => map.get_b(*b).polygon.center(),
            TripEndpoint::Border(i) => map.get_i(*i).polygon.center(),
            TripEndpoint::SuddenlyAppear(pos) => pos.pt(map),
        }
    }
}
