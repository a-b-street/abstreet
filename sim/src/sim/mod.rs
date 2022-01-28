// This file has a jumbled mess of queries, setup, and mutating methods.

use std::collections::{BTreeSet, HashSet};

use anyhow::Result;
use instant::Instant;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use abstio::{CityName, MapName};
use abstutil::{prettyprint_usize, serialized_size_bytes, Timer};
use geom::{Distance, Duration, Speed, Time};
use map_model::{
    BuildingID, IntersectionID, LaneID, Map, ParkingLotID, Path, PathConstraints, PathRequest,
    Position, TransitRoute, Traversable,
};
use synthpop::OrigPersonID;

pub use self::queries::{AgentProperties, DelayCause};
use crate::{
    AgentID, AlertLocation, Analytics, CarID, Command, CreateCar, DrivingSimState, Event,
    IntersectionSimState, PandemicModel, ParkedCar, ParkingSim, ParkingSimState, ParkingSpot,
    Person, PersonID, Router, Scheduler, SidewalkPOI, SidewalkSpot, StartTripArgs, TrafficRecorder,
    TransitSimState, TripID, TripInfo, TripManager, TripPhaseType, Vehicle, VehicleSpec,
    VehicleType, WalkingSimState, BUS_LENGTH, LIGHT_RAIL_LENGTH, MIN_CAR_LENGTH,
};

mod queries;
mod scenario;

// TODO Do something else.
const BLIND_RETRY_TO_SPAWN: Duration = Duration::const_seconds(5.0);

/// The Sim ties together all the pieces of the simulation. Its main property is the current time.
#[derive(Serialize, Deserialize, Clone)]
pub struct Sim {
    driving: DrivingSimState,
    parking: ParkingSimState,
    walking: WalkingSimState,
    intersections: IntersectionSimState,
    transit: TransitSimState,
    trips: TripManager,
    #[serde(skip_serializing, skip_deserializing)]
    pandemic: Option<PandemicModel>,
    scheduler: Scheduler,
    time: Time,

    // These're needed to load from a savestate.
    pub(crate) map_name: MapName,
    pub(crate) edits_name: String,
    // Some tests deliberately set different scenario names for comparisons.
    // TODO Maybe get rid of this, now that savestates aren't used
    run_name: String,
    step_count: usize,
    highlighted_people: Option<BTreeSet<PersonID>>,

    analytics: Analytics,
    // This is created interactively, and there's no reason to preserve one for savestates.
    #[serde(skip_serializing, skip_deserializing)]
    recorder: Option<TrafficRecorder>,

    #[serde(skip_serializing, skip_deserializing)]
    alerts: AlertHandler,
}

pub(crate) struct Ctx<'a> {
    pub parking: &'a mut ParkingSimState,
    pub intersections: &'a mut IntersectionSimState,
    pub scheduler: &'a mut Scheduler,
    pub map: &'a Map,
    /// If present, live map edits are being processed, and the agents specified are in the process
    /// of being deleted. Some regular work should maybe be skipped.
    pub handling_live_edits: Option<BTreeSet<AgentID>>,
}

/// Options controlling the traffic simulation.
#[derive(Clone, StructOpt)]
pub struct SimOptions {
    /// Used to distinguish savestates for running the same scenario.
    #[structopt(long, default_value = "unnamed")]
    pub run_name: String,
    /// Ignore all stop signs and traffic signals, instead using a "freeform" policy to control
    /// access to intersections. If a requested turn doesn't conflict with an already accepted one,
    /// immediately accept it. FIFO ordering, no balancing between different movements.
    #[structopt(long)]
    pub use_freeform_policy_everywhere: bool,
    /// Allow a vehicle to start a turn, even if their target lane is already full. This may mean
    /// they'll get stuck blocking the intersection.
    #[structopt(long)]
    pub allow_block_the_box: bool,
    /// Normally as a vehicle follows a route, it opportunistically make small changes to use a different lane,
    /// based on some score of "least-loaded" lane. Disable this default behavior.
    #[structopt(long)]
    pub dont_recalc_lanechanging: bool,
    /// Normally if a cycle of vehicles depending on each other to turn is detected, temporarily allow
    /// "blocking the box" to try to break gridlock. Disable this default behavior.
    #[structopt(long)]
    pub dont_break_turn_conflict_cycles: bool,
    /// Disable experimental handling for "uber-turns", sequences of turns through complex
    /// intersections with short roads. "Locks" the entire movement before starting, and ignores
    /// red lights after starting.
    #[structopt(long)]
    pub dont_handle_uber_turns: bool,
    /// Enable an experimental SEIR pandemic model. This requires an RNG seed, which can be the
    /// same or different from the one used for the rest of the simulation.
    #[structopt(long, parse(try_from_str = parse_rng))]
    pub enable_pandemic_model: Option<XorShiftRng>,
    /// When a warning is encountered during simulation, specifies how to respond.
    #[structopt(long, parse(try_from_str = parse_alert_handler), default_value = "print")]
    pub alerts: AlertHandler,
    /// Ignore parking data in the map and instead treat every building as if it has unlimited
    /// capacity for vehicles.
    ///
    /// Some maps always have this hardcoded on -- see the code for the list.
    #[structopt(long)]
    pub infinite_parking: bool,
    /// Allow all agents to immediately proceed into an intersection, even if they'd hit another
    /// agent. Obviously this destroys realism of the simulation, but can be used to debug
    /// gridlock. Also implies freeform_policy, so vehicles ignore traffic signals.
    #[structopt(long)]
    pub disable_turn_conflicts: bool,
    /// Don't collect any analytics. Only useful for benchmarking and debugging gridlock more
    /// quickly.
    #[structopt(long)]
    pub skip_analytics: bool,
}

impl SimOptions {
    pub fn new(run_name: &str) -> SimOptions {
        SimOptions {
            run_name: run_name.to_string(),
            use_freeform_policy_everywhere: false,
            allow_block_the_box: false,
            dont_recalc_lanechanging: false,
            dont_break_turn_conflict_cycles: false,
            dont_handle_uber_turns: false,
            enable_pandemic_model: None,
            alerts: AlertHandler::Print,
            infinite_parking: false,
            disable_turn_conflicts: false,
            skip_analytics: false,
        }
    }
}

impl Default for SimOptions {
    fn default() -> SimOptions {
        SimOptions::new("tmp")
    }
}

fn parse_rng(x: &str) -> Result<XorShiftRng> {
    let seed: u64 = x.parse()?;
    Ok(XorShiftRng::seed_from_u64(seed))
}

#[derive(Clone)]
pub enum AlertHandler {
    /// Just print the alert to STDOUT
    Print,
    /// Print the alert to STDOUT and don't proceed until the UI calls clear_alerts()
    Block,
    /// Don't do anything
    Silence,
}

impl Default for AlertHandler {
    fn default() -> AlertHandler {
        AlertHandler::Print
    }
}

fn parse_alert_handler(x: &str) -> Result<AlertHandler> {
    match x {
        "print" => Ok(AlertHandler::Print),
        "block" => Ok(AlertHandler::Block),
        "silence" => Ok(AlertHandler::Silence),
        _ => bail!("Bad --alerts={}. Must be print|block|silence", x),
    }
}

// Setup
impl Sim {
    pub fn new(map: &Map, mut opts: SimOptions) -> Sim {
        let mut timer = Timer::new("create blank sim");
        let mut scheduler = Scheduler::new();

        // Always disable parking for two maps. See
        // https://github.com/a-b-street/abstreet/issues/688 for discussion of how to set this
        // properly.
        if map.get_name() == &MapName::seattle("arboretum")
            || map.get_name().city == CityName::new("ir", "tehran")
            || map.get_name() == &MapName::new("gb", "poundbury", "center")
            || map.get_name() == &MapName::new("us", "phoenix", "tempe")
        {
            opts.infinite_parking = true;
        }

        // Hack around simulation bugs to get a Tehran map running.
        if map.get_name() == &MapName::new("ir", "tehran", "parliament") {
            opts.allow_block_the_box = true;
        }

        Sim {
            driving: DrivingSimState::new(map, &opts),
            parking: ParkingSimState::new(map, opts.infinite_parking, &mut timer),
            walking: WalkingSimState::new(),
            intersections: IntersectionSimState::new(map, &mut scheduler, &opts),
            transit: TransitSimState::new(map),
            trips: TripManager::new(),
            pandemic: opts.enable_pandemic_model.map(PandemicModel::new),
            scheduler,
            time: Time::START_OF_DAY,

            map_name: map.get_name().clone(),
            edits_name: map.get_edits().edits_name.clone(),
            run_name: opts.run_name,
            step_count: 0,
            highlighted_people: None,
            alerts: opts.alerts,

            analytics: Analytics::new(!opts.skip_analytics),
            recorder: None,
        }
    }

    pub(crate) fn spawn_trips(
        &mut self,
        input: Vec<(PersonID, TripInfo, StartTripArgs)>,
        map: &Map,
        timer: &mut Timer,
    ) {
        timer.start_iter("spawn trips", input.len());
        for (p, info, args) in input {
            timer.next();

            let trip = self.trips.new_trip(p, info.clone());
            // This might be immediately true due to ScenarioModifiers
            if let Some(msg) = info.cancellation_reason {
                self.trips.cancel_unstarted_trip(trip, msg);
            } else {
                self.scheduler
                    .push(info.departure, Command::StartTrip(trip, args));
            }
        }

        if let Some(ref mut m) = self.pandemic {
            m.initialize(self.trips.get_all_people(), &mut self.scheduler);
        }

        self.dispatch_events(Vec::new(), map);
    }

    pub fn get_free_onstreet_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        self.parking.get_free_onstreet_spots(l)
    }

    pub fn get_free_offstreet_spots(&self, b: BuildingID) -> Vec<ParkingSpot> {
        self.parking.get_free_offstreet_spots(b)
    }

    pub fn get_free_lot_spots(&self, pl: ParkingLotID) -> Vec<ParkingSpot> {
        self.parking.get_free_lot_spots(pl)
    }

    /// (Filled, available)
    pub fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>) {
        self.parking.get_all_parking_spots()
    }

    pub fn bldg_to_parked_cars(&self, b: BuildingID) -> Vec<CarID> {
        self.parking.bldg_to_parked_cars(b)
    }

    pub fn walking_path_to_nearest_parking_spot(&self, map: &Map, b: BuildingID) -> Option<Path> {
        let vehicle = Vehicle {
            id: CarID {
                id: 0,
                vehicle_type: VehicleType::Car,
            },
            owner: None,
            vehicle_type: VehicleType::Car,
            length: MIN_CAR_LENGTH,
            max_speed: None,
        };
        let driving_lane = map.find_driving_lane_near_building(b);

        // Anything on the current lane? TODO Should find the closest one to the sidewalk, but
        // need a new method in ParkingSimState to make that easy.
        // TODO Refactor the logic in router
        let spot = if let Some((spot, _)) = self
            .parking
            .get_all_free_spots(Position::start(driving_lane), &vehicle, b, map)
            .get(0)
        {
            *spot
        } else {
            let (_, spot, _) =
                self.parking
                    .path_to_free_parking_spot(driving_lane, &vehicle, b, map)?;
            spot
        };

        let start = SidewalkSpot::building(b, map).sidewalk_pos;
        let end = SidewalkSpot::parking_spot(spot, map, &self.parking).sidewalk_pos;
        map.pathfind(PathRequest::walking(start, end)).ok()
    }

    pub(crate) fn new_person(
        &mut self,
        orig_id: Option<OrigPersonID>,
        ped_speed: Speed,
        vehicle_specs: Vec<VehicleSpec>,
    ) -> &Person {
        self.trips.new_person(orig_id, ped_speed, vehicle_specs)
    }
    pub(crate) fn seed_parked_car(&mut self, vehicle: Vehicle, spot: ParkingSpot) {
        self.parking.reserve_spot(spot, vehicle.id);
        self.parking.add_parked_car(ParkedCar {
            vehicle,
            spot,
            parked_since: self.time,
        });
    }

    pub(crate) fn seed_bus_route(&mut self, route: &TransitRoute) {
        for t in &route.spawn_times {
            self.scheduler.push(*t, Command::StartBus(route.id, *t));
        }
    }

    fn start_bus(&mut self, route: &TransitRoute, map: &Map) {
        // Spawn one bus for the first leg.
        let path = self.transit.create_empty_route(route, map);

        // For now, no desire for randomness. Caller can pass in list of specs if that ever
        // changes.
        let (vehicle_type, length) = match route.route_type {
            PathConstraints::Bus => (VehicleType::Bus, BUS_LENGTH),
            PathConstraints::Train => (VehicleType::Train, LIGHT_RAIL_LENGTH),
            _ => unreachable!(),
        };
        let vehicle = VehicleSpec {
            vehicle_type,
            length,
            max_speed: None,
        }
        .make(
            CarID {
                id: self.trips.new_car_id(),
                vehicle_type,
            },
            None,
        );

        self.scheduler.push(
            self.time,
            Command::SpawnCar(
                CreateCar {
                    router: Router::follow_bus_route(vehicle.id, path),
                    vehicle,
                    maybe_parked_car: None,
                    trip_and_person: None,
                    maybe_route: Some(route.id),
                },
                true,
            ),
        );
    }

    pub fn set_run_name(&mut self, name: String) {
        self.run_name = name;
    }

    pub fn get_run_name(&self) -> &String {
        &self.run_name
    }
}

// Running
impl Sim {
    // Advances time as minimally as possible, also limited by max_dt. Returns true if the callback
    // said to halt the sim.
    fn minimal_step(
        &mut self,
        map: &Map,
        max_dt: Duration,
        maybe_cb: &mut Option<Box<dyn SimCallback>>,
    ) -> bool {
        self.step_count += 1;

        let max_time = if let Some(t) = self.scheduler.peek_next_time() {
            if t > self.time + max_dt {
                // Next event is after when we want to stop.
                self.time += max_dt;
                return false;
            }
            t
        } else {
            // No events left at all
            self.time += max_dt;
            return false;
        };

        let mut halt = false;
        while let Some(time) = self.scheduler.peek_next_time() {
            if time > max_time {
                return false;
            }
            if let Some(cmd) = self.scheduler.get_next() {
                if self.do_step(map, time, cmd, maybe_cb) {
                    halt = true;
                    break;
                }
            }
        }

        halt
    }

    // If true, halt simulation because the callback said so.
    fn do_step(
        &mut self,
        map: &Map,
        time: Time,
        cmd: Command,
        maybe_cb: &mut Option<Box<dyn SimCallback>>,
    ) -> bool {
        self.time = time;
        let mut events = Vec::new();
        let mut halt = false;

        let mut ctx = Ctx {
            parking: &mut self.parking,
            intersections: &mut self.intersections,
            scheduler: &mut self.scheduler,
            map,
            handling_live_edits: None,
        };

        match cmd {
            Command::StartTrip(id, args) => {
                self.trips.start_trip(self.time, id, args, &mut ctx);
            }
            Command::SpawnCar(create_car, retry_if_no_room) => {
                // If this SpawnCar is being retried and the map was live-edited since the first
                // attempt, the path might've become invalid. TODO Skip this check
                // most of the time.
                let constraints = create_car.vehicle.vehicle_type.to_constraints();
                let mut ok = true;
                for step in create_car.router.get_path().get_steps() {
                    match step.as_traversable() {
                        Traversable::Lane(l) => {
                            if !constraints.can_use(ctx.map.get_l(l), ctx.map) {
                                ok = false;
                                break;
                            }
                        }
                        Traversable::Turn(t) => {
                            if ctx.map.maybe_get_t(t).is_none() {
                                ok = false;
                                break;
                            }
                        }
                    }
                }
                if !ok {
                    self.trips.cancel_trip(
                        self.time,
                        create_car.trip_and_person.unwrap().0,
                        "path is no longer valid after map edits".to_string(),
                        Some(create_car.vehicle),
                        &mut ctx,
                    );
                } else {
                    // create_car contains a Path, which is expensive to clone. We need different
                    // parts of create_car after attempting start_car_on_lane.
                    // Make copies just of those here. In no case do we ever
                    // clone the path.
                    let id = create_car.vehicle.id;
                    let maybe_route = create_car.maybe_route;
                    let trip_and_person = create_car.trip_and_person;
                    let maybe_parked_car = create_car.maybe_parked_car.clone();
                    let req = create_car.router.get_path().get_req().clone();

                    if let Some(create_car) = self
                        .driving
                        .start_car_on_lane(self.time, create_car, &mut ctx)
                    {
                        // Starting the car failed for some reason.
                        if retry_if_no_room {
                            // Although the agent isn't on the map yet, they're trying.
                            if let Some((trip, _)) = trip_and_person {
                                self.trips.agent_starting_trip_leg(AgentID::Car(id), trip);
                            }
                            self.driving.vehicle_waiting_to_spawn(
                                id,
                                req.start,
                                trip_and_person.map(|(_, p)| p),
                            );

                            // TODO Record this in the trip log
                            self.scheduler.push(
                                self.time + BLIND_RETRY_TO_SPAWN,
                                Command::SpawnCar(create_car, retry_if_no_room),
                            );
                        } else if let Some((trip, person)) = create_car.trip_and_person {
                            self.trips.cancel_trip(
                                self.time,
                                trip,
                                format!(
                                    "no room to spawn car for {} by {}, not retrying",
                                    trip, person
                                ),
                                Some(create_car.vehicle),
                                &mut ctx,
                            );
                        }
                    } else {
                        // Creating the car succeeded.
                        if let Some((trip, person)) = trip_and_person {
                            self.trips.agent_starting_trip_leg(AgentID::Car(id), trip);
                            events.push(Event::TripPhaseStarting(
                                trip,
                                person,
                                Some(req),
                                if id.vehicle_type == VehicleType::Car {
                                    TripPhaseType::Driving
                                } else {
                                    TripPhaseType::Biking
                                },
                            ));
                        }
                        if let Some(parked_car) = maybe_parked_car {
                            if let ParkingSpot::Offstreet(b, _) = parked_car.spot {
                                // Buses don't start in parking garages, so trip must exist
                                events.push(Event::PersonLeavesBuilding(
                                    trip_and_person.unwrap().1,
                                    b,
                                ));
                            }
                            self.parking.remove_parked_car(parked_car);
                        }
                        if let Some(route) = maybe_route {
                            self.transit.bus_created(id, route);
                        }
                        self.analytics
                            .record_demand(self.driving.get_path(id).unwrap(), map);
                    }
                }
            }
            Command::SpawnPed(create_ped) => {
                // Do the order a bit backwards so we don't have to clone the
                // CreatePedestrian. spawn_ped can't fail.
                self.trips
                    .agent_starting_trip_leg(AgentID::Pedestrian(create_ped.id), create_ped.trip);
                events.push(Event::TripPhaseStarting(
                    create_ped.trip,
                    create_ped.person,
                    Some(create_ped.path.get_req().clone()),
                    TripPhaseType::Walking,
                ));
                self.analytics.record_demand(&create_ped.path, map);

                // Maybe there's actually no work to do!
                match (&create_ped.start.connection, &create_ped.goal.connection) {
                    (
                        SidewalkPOI::Building(b1),
                        SidewalkPOI::ParkingSpot(ParkingSpot::Offstreet(b2, idx)),
                    ) if b1 == b2 => {
                        self.trips.ped_reached_parking_spot(
                            self.time,
                            create_ped.id,
                            ParkingSpot::Offstreet(*b2, *idx),
                            Duration::ZERO,
                            Distance::ZERO,
                            &mut ctx,
                        );
                    }
                    _ => {
                        if let SidewalkPOI::Building(b) = &create_ped.start.connection {
                            events.push(Event::PersonLeavesBuilding(create_ped.person, *b));
                        }

                        self.walking
                            .spawn_ped(self.time, create_ped, map, &mut self.scheduler);
                    }
                }
            }
            Command::UpdateCar(car) => {
                self.driving.update_car(
                    car,
                    self.time,
                    &mut ctx,
                    &mut self.trips,
                    &mut self.transit,
                    &mut self.walking,
                );
            }
            Command::UpdateLaggyHead(car) => {
                self.driving.update_laggy_head(car, self.time, &mut ctx);
            }
            Command::UpdatePed(ped) => {
                self.walking.update_ped(
                    ped,
                    self.time,
                    &mut ctx,
                    &mut self.trips,
                    &mut self.transit,
                );
            }
            Command::UpdateIntersection(i) => {
                self.intersections
                    .update_intersection(self.time, i, map, &mut self.scheduler);
            }
            Command::Callback(frequency) => {
                self.scheduler
                    .push(self.time + frequency, Command::Callback(frequency));
                if maybe_cb.as_mut().unwrap().run(self, map) {
                    halt = true;
                }
            }
            Command::Pandemic(cmd) => {
                self.pandemic
                    .as_mut()
                    .unwrap()
                    .handle_cmd(self.time, cmd, &mut self.scheduler);
            }
            Command::StartBus(r, _) => {
                self.start_bus(map.get_tr(r), map);
            }
        }

        // Record events at precisely the time they occur.
        self.dispatch_events(events, map);

        halt
    }

    fn dispatch_events(&mut self, mut events: Vec<Event>, map: &Map) {
        events.extend(self.trips.collect_events());
        events.extend(self.transit.collect_events());
        events.extend(self.driving.collect_events());
        events.extend(self.walking.collect_events());
        events.extend(self.intersections.collect_events());
        events.extend(self.parking.collect_events());
        for ev in events {
            if let Some(ref mut m) = self.pandemic {
                m.handle_event(self.time, &ev, &mut self.scheduler);
            }
            if let Some(ref mut r) = self.recorder {
                r.handle_event(self.time, &ev, map, &self.driving);
            }

            self.analytics.event(ev, self.time, map);
        }
    }

    pub fn timed_step(
        &mut self,
        map: &Map,
        dt: Duration,
        maybe_cb: &mut Option<Box<dyn SimCallback>>,
        timer: &mut Timer,
    ) {
        let end_time = self.time + dt;
        let start = Instant::now();
        let mut last_update = Instant::now();

        timer.start(format!("Advance sim to {}", end_time));
        while self.time < end_time {
            if self.minimal_step(map, end_time - self.time, maybe_cb) {
                break;
            }
            if !self.analytics.alerts.is_empty() {
                match self.alerts {
                    AlertHandler::Print => {
                        for (t, loc, msg) in self.analytics.alerts.drain(..) {
                            println!("Alert at {} ({:?}): {}", t, loc, msg);
                        }
                    }
                    AlertHandler::Block => {
                        for (t, loc, msg) in &self.analytics.alerts {
                            println!("Alert at {} ({:?}): {}", t, loc, msg);
                        }
                        break;
                    }
                    AlertHandler::Silence => {
                        self.analytics.alerts.clear();
                    }
                }
            }
            if Duration::realtime_elapsed(last_update) >= Duration::seconds(1.0) {
                // TODO Not timer?
                println!(
                    "- After {}, the sim is at {}. {} live agents",
                    Duration::realtime_elapsed(start),
                    self.time,
                    prettyprint_usize(self.num_active_agents()),
                );
                last_update = Instant::now();
            }
        }
        timer.stop(format!("Advance sim to {}", end_time));
    }
    pub fn tiny_step(&mut self, map: &Map, maybe_cb: &mut Option<Box<dyn SimCallback>>) {
        self.timed_step(
            map,
            Duration::seconds(0.1),
            maybe_cb,
            &mut Timer::throwaway(),
        );
    }

    pub fn time_limited_step(
        &mut self,
        map: &Map,
        dt: Duration,
        real_time_limit: Duration,
        maybe_cb: &mut Option<Box<dyn SimCallback>>,
    ) {
        let started_at = Instant::now();
        let end_time = self.time + dt;

        while self.time < end_time && Duration::realtime_elapsed(started_at) < real_time_limit {
            if self.minimal_step(map, end_time - self.time, maybe_cb) {
                break;
            }
            if !self.analytics.alerts.is_empty() {
                match self.alerts {
                    AlertHandler::Print => {
                        for (t, loc, msg) in self.analytics.alerts.drain(..) {
                            println!("Alert at {} ({:?}): {}", t, loc, msg);
                        }
                    }
                    AlertHandler::Block => {
                        for (t, loc, msg) in &self.analytics.alerts {
                            println!("Alert at {} ({:?}): {}", t, loc, msg);
                        }
                        break;
                    }
                    AlertHandler::Silence => {
                        self.analytics.alerts.clear();
                    }
                }
            }
        }
    }

    pub fn dump_before_abort(&self) {
        println!("At {}", self.time);
        if let Some(path) = self.find_previous_savestate(self.time) {
            println!("Debug from {}", path);
        }
    }
}

// Savestating
impl Sim {
    pub fn save_dir(&self) -> String {
        abstio::path_all_saves(&self.map_name, &self.edits_name, &self.run_name)
    }

    fn save_path(&self, base_time: Time) -> String {
        // If we wanted to be even more reproducible, we'd encode RNG seed, version of code, etc,
        // but that's overkill right now.
        abstio::path_save(
            &self.map_name,
            &self.edits_name,
            &self.run_name,
            base_time.as_filename(),
        )
    }

    pub fn save(&mut self) -> String {
        if false {
            println!("sim savestate breakdown:");
            println!(
                "- driving: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.driving))
            );
            println!(
                "- parking: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.parking))
            );
            println!(
                "- walking: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.walking))
            );
            println!(
                "- intersections: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.intersections))
            );
            println!(
                "- transit: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.transit))
            );
            println!(
                "- trips: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.trips))
            );
            println!(
                "- scheduler: {} bytes",
                prettyprint_usize(serialized_size_bytes(&self.scheduler))
            );
        }

        let path = self.save_path(self.time);
        abstio::write_binary(path.clone(), self);

        path
    }

    pub fn find_previous_savestate(&self, base_time: Time) -> Option<String> {
        abstio::find_prev_file(self.save_path(base_time))
    }

    pub fn find_next_savestate(&self, base_time: Time) -> Option<String> {
        abstio::find_next_file(self.save_path(base_time))
    }

    pub fn load_savestate(path: String, timer: &mut Timer) -> Result<Sim> {
        abstio::maybe_read_binary(path, timer)
    }
}

// Live edits
impl Sim {
    pub fn handle_live_edited_traffic_signals(&mut self, map: &Map) {
        self.intersections
            .handle_live_edited_traffic_signals(self.time, map, &mut self.scheduler)
    }

    /// Respond to arbitrary map edits without resetting the simulation. Returns the number of
    /// (trips cancelled, parked cars displaced).
    pub fn handle_live_edits(&mut self, map: &Map, timer: &mut Timer) -> (usize, usize) {
        self.edits_name = map.get_edits().edits_name.clone();

        let (affected, num_parked_cars) = self.find_trips_affected_by_live_edits(map, timer);
        let num_trips_cancelled = affected.len();
        let affected_agents: BTreeSet<AgentID> = affected.iter().map(|(a, _)| *a).collect();

        // V1: Just cancel every trip crossing an affected area.
        // (V2 is probably rerouting everyone, only cancelling when that fails)
        // TODO If we delete a bus, deal with all its passengers
        let mut ctx = Ctx {
            parking: &mut self.parking,
            intersections: &mut self.intersections,
            scheduler: &mut self.scheduler,
            map,
            handling_live_edits: Some(affected_agents),
        };
        for (agent, trip) in affected {
            match agent {
                AgentID::Car(car) => {
                    let vehicle = self.driving.delete_car(car, self.time, &mut ctx);
                    // TODO Plumb more info about the reason
                    self.trips.cancel_trip(
                        self.time,
                        trip,
                        "map edited without reset".to_string(),
                        Some(vehicle),
                        &mut ctx,
                    );
                    self.trips.trip_abruptly_cancelled(trip, AgentID::Car(car));
                }
                AgentID::Pedestrian(ped) => {
                    self.walking.delete_ped(ped, &mut ctx);
                    self.trips.cancel_trip(
                        self.time,
                        trip,
                        "map edited without reset".to_string(),
                        None,
                        &mut ctx,
                    );
                    self.trips
                        .trip_abruptly_cancelled(trip, AgentID::Pedestrian(ped));
                }
                AgentID::BusPassenger(_, _) => unreachable!(),
            }
        }

        self.driving.handle_live_edits(map);
        self.intersections.handle_live_edits(map);

        (num_trips_cancelled, num_parked_cars)
    }

    /// Returns (trips affected, number of parked cars displaced)
    fn find_trips_affected_by_live_edits(
        &mut self,
        map: &Map,
        timer: &mut Timer,
    ) -> (BTreeSet<(AgentID, TripID)>, usize) {
        let mut affected: BTreeSet<(AgentID, TripID)> = BTreeSet::new();

        // TODO Handle changes to access restrictions

        {
            // Find every active trip whose path crosses a modified lane or intersection
            let (edited_lanes, _) = map.get_edits().changed_lanes(map);
            let mut closed_intersections = HashSet::new();
            for i in map.get_edits().original_intersections.keys() {
                if map.get_i(*i).is_closed() {
                    closed_intersections.insert(*i);
                }
            }
            for (a, trip) in self.trips.active_agents_and_trips() {
                if let Some(path) = self.get_path(*a) {
                    if path
                        .get_steps()
                        .iter()
                        .any(|step| match step.as_traversable() {
                            Traversable::Lane(l) => edited_lanes.contains(&l),
                            Traversable::Turn(t) => {
                                closed_intersections.contains(&t.parent)
                                    || edited_lanes.contains(&t.src)
                                    || edited_lanes.contains(&t.dst)
                            }
                        })
                    {
                        affected.insert((*a, *trip));
                    }
                }
            }

            affected.extend(
                self.driving
                    .find_vehicles_affected_by_live_edits(&closed_intersections, &edited_lanes),
            );
        }

        let num_evicted = {
            let (evicted_cars, cars_parking_in_the_void) =
                self.parking.handle_live_edits(map, timer);
            let num_evicted = evicted_cars.len();
            affected.extend(self.walking.find_trips_to_parking(evicted_cars));
            for car in cars_parking_in_the_void {
                let a = AgentID::Car(car);
                affected.insert((a, self.agent_to_trip(a).unwrap()));
            }

            if !self.parking.is_infinite() {
                let (filled, avail) = self.parking.get_all_parking_spots();
                let mut all_spots: BTreeSet<ParkingSpot> = BTreeSet::new();
                all_spots.extend(filled);
                all_spots.extend(avail);
                affected.extend(self.driving.find_trips_to_edited_parking(all_spots));
            }
            num_evicted
        };

        (affected, num_evicted)
    }
}

// Invasive debugging
impl Sim {
    pub fn delete_car(&mut self, id: CarID, map: &Map) {
        if let Some(trip) = self.agent_to_trip(AgentID::Car(id)) {
            let mut ctx = Ctx {
                parking: &mut self.parking,
                intersections: &mut self.intersections,
                scheduler: &mut self.scheduler,
                map,
                handling_live_edits: None,
            };
            let vehicle = self.driving.delete_car(id, self.time, &mut ctx);
            self.trips.cancel_trip(
                self.time,
                trip,
                format!("{} deleted manually through the UI", id),
                Some(vehicle),
                &mut ctx,
            );
        } else {
            println!("{} has no trip?!", id);
        }
    }

    pub fn clear_alerts(&mut self) -> Vec<(Time, AlertLocation, String)> {
        std::mem::take(&mut self.analytics.alerts)
    }
}

// Callbacks
pub trait SimCallback: downcast_rs::Downcast {
    // Run at some scheduled time. If this returns true, halt simulation.
    fn run(&mut self, sim: &Sim, map: &Map) -> bool;
}
downcast_rs::impl_downcast!(SimCallback);

impl Sim {
    /// Only one at a time supported.
    pub fn set_periodic_callback(&mut self, frequency: Duration) {
        // TODO Round up time nicely?
        self.scheduler
            .push(self.time + frequency, Command::Callback(frequency));
    }
    pub fn unset_periodic_callback(&mut self) {
        // Frequency doesn't matter
        self.scheduler
            .cancel(Command::Callback(Duration::seconds(1.0)));
    }
}

// Recording traffic
impl Sim {
    pub fn record_traffic_for(&mut self, intersections: BTreeSet<IntersectionID>) {
        assert!(self.recorder.is_none());
        self.recorder = Some(TrafficRecorder::new(intersections));
    }

    pub fn num_recorded_trips(&self) -> Option<usize> {
        Some(self.recorder.as_ref()?.num_recorded_trips())
    }

    pub fn save_recorded_traffic(&mut self, map: &Map) {
        self.recorder.take().unwrap().save(map);
    }
}

// Managing highlighted people
impl Sim {
    pub fn set_highlighted_people(&mut self, people: BTreeSet<PersonID>) {
        self.highlighted_people = Some(people);
    }
}
