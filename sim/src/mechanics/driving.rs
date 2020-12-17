use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_hashmap, serialize_hashmap, FixedMap, IndexableKey};
use geom::{Distance, Duration, PolyLine, Speed, Time};
use map_model::{IntersectionID, LaneID, Map, Path, Position, Traversable};

use crate::mechanics::car::{Car, CarState};
use crate::mechanics::Queue;
use crate::sim::Ctx;
use crate::{
    ActionAtEnd, AgentID, AgentProperties, CarID, Command, CreateCar, DelayCause, DistanceInterval,
    DrawCarInput, Event, IntersectionSimState, ParkedCar, ParkingSim, ParkingSpot, PersonID,
    SimOptions, TimeInterval, TransitSimState, TripID, TripManager, UnzoomedAgent, Vehicle,
    WalkingSimState, FOLLOWING_DISTANCE,
};

const TIME_TO_WAIT_AT_BUS_STOP: Duration = Duration::const_seconds(10.0);

// TODO Do something else.
pub const BLIND_RETRY_TO_CREEP_FORWARDS: Duration = Duration::const_seconds(0.1);
pub const BLIND_RETRY_TO_REACH_END_DIST: Duration = Duration::const_seconds(5.0);

/// Simulates vehicles!
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct DrivingSimState {
    // This spends some space to save time. If a simulation contains 1 million cars over the course
    // of a day, but only 100,000 are ever active simultaneously, we store 900,000 `None`s. But we
    // gain much faster lookup, which has shown dramatic speedups in the scenarios being run so
    // far.
    cars: FixedMap<CarID, Car>,
    // Note this uses a HashMap for faster lookup. Although the order of iterating over the HashMap
    // is random, determinism in the simulation is preserved, because nothing iterates over
    // everything.
    #[serde(
        serialize_with = "serialize_hashmap",
        deserialize_with = "deserialize_hashmap"
    )]
    queues: HashMap<Traversable, Queue>,
    events: Vec<Event>,

    waiting_to_spawn: BTreeMap<CarID, (Position, Option<PersonID>)>,

    recalc_lanechanging: bool,
    handle_uber_turns: bool,

    time_to_unpark_onstreet: Duration,
    time_to_park_onstreet: Duration,
    time_to_unpark_offstreet: Duration,
    time_to_park_offstreet: Duration,
}

// Mutations
impl DrivingSimState {
    pub fn new(map: &Map, opts: &SimOptions) -> DrivingSimState {
        let mut sim = DrivingSimState {
            cars: FixedMap::new(),
            queues: HashMap::new(),
            events: Vec::new(),
            recalc_lanechanging: opts.recalc_lanechanging,
            handle_uber_turns: opts.handle_uber_turns,
            waiting_to_spawn: BTreeMap::new(),

            time_to_unpark_onstreet: Duration::seconds(10.0),
            time_to_park_onstreet: Duration::seconds(15.0),
            time_to_unpark_offstreet: Duration::seconds(5.0),
            time_to_park_offstreet: Duration::seconds(5.0),
        };
        if opts.infinite_parking {
            sim.time_to_unpark_offstreet = Duration::seconds(0.1);
            sim.time_to_park_offstreet = Duration::seconds(0.1);
        }

        for l in map.all_lanes() {
            if l.lane_type.is_for_moving_vehicles() {
                let q = Queue::new(Traversable::Lane(l.id), map);
                sim.queues.insert(q.id, q);
            }
        }
        for t in map.all_turns().values() {
            if !t.between_sidewalks() {
                let q = Queue::new(Traversable::Turn(t.id), map);
                sim.queues.insert(q.id, q);
            }
        }

        sim
    }

    /// None if it worked, otherwise returns the CreateCar unmodified for possible retry.
    pub fn start_car_on_lane(
        &mut self,
        now: Time,
        mut params: CreateCar,
        ctx: &mut Ctx,
    ) -> Option<CreateCar> {
        let first_lane = params.router.head().as_lane();
        let start_dist = params.router.get_path().get_req().start.dist_along();

        if !ctx
            .intersections
            .nobody_headed_towards(first_lane, ctx.map.get_l(first_lane).src_i)
        {
            return Some(params);
        }
        if let Some(idx) = self.queues[&Traversable::Lane(first_lane)].get_idx_to_insert_car(
            start_dist,
            params.vehicle.length,
            now,
            &self.cars,
            &self.queues,
        ) {
            let mut car = Car {
                vehicle: params.vehicle,
                router: params.router,
                // Temporary
                state: CarState::Queued { blocked_since: now },
                last_steps: VecDeque::new(),
                started_at: now,
                total_blocked_time: Duration::ZERO,
                trip_and_person: params.trip_and_person,
            };
            if let Some(p) = params.maybe_parked_car {
                let delay = match p.spot {
                    ParkingSpot::Onstreet(_, _) => self.time_to_unpark_onstreet,
                    ParkingSpot::Offstreet(_, _) | ParkingSpot::Lot(_, _) => {
                        self.time_to_unpark_offstreet
                    }
                };
                car.state =
                    CarState::Unparking(start_dist, p.spot, TimeInterval::new(now, now + delay));
            } else {
                // Have to do this early
                if car.router.last_step() {
                    match car.router.maybe_handle_end(
                        start_dist,
                        &car.vehicle,
                        ctx.parking,
                        ctx.map,
                        car.trip_and_person,
                        &mut self.events,
                    ) {
                        None | Some(ActionAtEnd::GotoLaneEnd) => {}
                        x => {
                            panic!(
                                "Car with one-step route {:?} had unexpected result from \
                                 maybe_handle_end: {:?}",
                                car.router, x
                            );
                        }
                    }
                    // We might've decided to go park somewhere farther, so get_end_dist no longer
                    // makes sense.
                    if car.router.last_step() && start_dist > car.router.get_end_dist() {
                        println!(
                            "WARNING: {} wants to spawn at {}, which is past their end of {} on a \
                             one-step path {}",
                            car.vehicle.id,
                            start_dist,
                            car.router.get_end_dist(),
                            first_lane
                        );
                        params.router = car.router;
                        params.vehicle = car.vehicle;
                        return Some(params);
                    }
                }

                car.state = car.crossing_state(start_dist, now, ctx.map);
            }
            ctx.scheduler
                .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
            {
                let queue = self.queues.get_mut(&Traversable::Lane(first_lane)).unwrap();
                queue.cars.insert(idx, car.vehicle.id);
                // Don't use try_to_reserve_entry -- it's overly conservative.
                // get_idx_to_insert_car does a more detailed check of the current space usage.
                queue.reserved_length += car.vehicle.length + FOLLOWING_DISTANCE;
            }
            self.waiting_to_spawn.remove(&car.vehicle.id);
            self.cars.insert(car.vehicle.id, car);
            return None;
        }
        Some(params)
    }

    /// If start_car_on_lane fails and a retry is scheduled, this is an idempotent way to mark the
    /// vehicle as active, but waiting to spawn.
    pub fn vehicle_waiting_to_spawn(&mut self, id: CarID, pos: Position, person: Option<PersonID>) {
        self.waiting_to_spawn.insert(id, (pos, person));
    }

    /// State transitions for this car:
    ///
    /// Crossing -> Queued or WaitingToAdvance
    /// Unparking -> Crossing
    /// IdlingAtStop -> Crossing
    /// Queued -> last step handling (Parking or done)
    /// WaitingToAdvance -> try to advance to the next step of the path
    /// Parking -> done
    ///
    /// State transitions for other cars:
    ///
    /// Crossing -> Crossing (recalculate dist/time)
    /// Queued -> Crossing
    ///
    /// Why is it safe to process cars in any order, rather than making sure to follow the order
    /// of queues? Because of the invariant that distances should never suddenly jump when a car
    /// has entered/exiting a queue.
    /// This car might have reached the router's end distance, but maybe not -- might
    /// actually be stuck behind other cars. We have to calculate the distances right now to
    /// be sure.
    pub fn update_car(
        &mut self,
        id: CarID,
        now: Time,
        ctx: &mut Ctx,
        trips: &mut TripManager,
        transit: &mut TransitSimState,
        walking: &mut WalkingSimState,
    ) {
        let mut need_distances = {
            let car = &self.cars[&id];
            match car.state {
                CarState::Queued { .. } => car.router.last_step(),
                CarState::Parking(_, _, _) => true,
                _ => false,
            }
        };

        if !need_distances {
            // We need to mutate two different cars in one case. To avoid fighting the borrow
            // checker, temporarily move one of them out of the map.
            let mut car = self.cars.remove(&id).unwrap();
            // Responsibility of update_car to manage scheduling stuff!
            need_distances = self.update_car_without_distances(&mut car, now, ctx, transit);
            self.cars.insert(id, car);
        }

        if need_distances {
            // Do this before removing the car!
            let dists = self.queues[&self.cars[&id].router.head()].get_car_positions(
                now,
                &self.cars,
                &self.queues,
            );
            let idx = dists.iter().position(|(c, _)| *c == id).unwrap();

            // We need to mutate two different cars in some cases. To avoid fighting the borrow
            // checker, temporarily move one of them out of the map.
            let mut car = self.cars.remove(&id).unwrap();
            // Responsibility of update_car_with_distances to manage scheduling stuff!
            if self
                .update_car_with_distances(&mut car, &dists, idx, now, ctx, trips, transit, walking)
            {
                self.cars.insert(id, car);
            } else {
                self.delete_car_internal(&mut car, dists, idx, now, ctx);
            }
        }
    }

    // If this returns true, we need to immediately run update_car_with_distances. If we don't,
    // then the car will briefly be Queued and might immediately become something else, which
    // affects how leaders update followers.
    fn update_car_without_distances(
        &mut self,
        car: &mut Car,
        now: Time,
        ctx: &mut Ctx,
        transit: &mut TransitSimState,
    ) -> bool {
        match car.state {
            CarState::Crossing(time_int, dist_int) => {
                let time_cross = now - time_int.start;
                if time_cross > Duration::ZERO {
                    let avg_speed = Speed::from_dist_time(dist_int.length(), time_cross);

                    let route = car.router.head();
                    let max_speed = route.speed_limit(ctx.map).min(
                        car.vehicle
                            .max_speed
                            .unwrap_or(Speed::meters_per_second(100.0)),
                    );

                    if let Some((trip, _)) = car.trip_and_person {
                        if let Traversable::Lane(lane) = route {
                            self.events
                                .push(Event::LaneSpeedPercentage(trip, lane, avg_speed, max_speed));
                        }
                    }
                }

                car.state = CarState::Queued { blocked_since: now };
                if car.router.last_step() {
                    // Immediately run update_car_with_distances.
                    return true;
                }
                let queue = &self.queues[&car.router.head()];
                if queue.cars[0] == car.vehicle.id && queue.laggy_head.is_none() {
                    // Want to re-run, but no urgency about it happening immediately.
                    car.state = CarState::WaitingToAdvance { blocked_since: now };
                    if self.recalc_lanechanging {
                        car.router.opportunistically_lanechange(
                            &self.queues,
                            ctx.map,
                            self.handle_uber_turns,
                        );
                    }
                    ctx.scheduler.push(now, Command::UpdateCar(car.vehicle.id));
                }
            }
            CarState::Unparking(front, _, _) => {
                if car.router.last_step() {
                    // Actually, we need to do this first. Ignore the answer -- if we're doing
                    // something weird like vanishing or re-parking immediately (quite unlikely),
                    // the next loop will pick that up. Just trigger the side effect of choosing an
                    // end_dist.
                    car.router.maybe_handle_end(
                        front,
                        &car.vehicle,
                        ctx.parking,
                        ctx.map,
                        car.trip_and_person,
                        &mut self.events,
                    );
                }
                car.state = car.crossing_state(front, now, ctx.map);
                ctx.scheduler
                    .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
            }
            CarState::IdlingAtStop(dist, _) => {
                car.router = transit.bus_departed_from_stop(car.vehicle.id, ctx.map);
                self.events
                    .push(Event::PathAmended(car.router.get_path().clone()));
                car.state = car.crossing_state(dist, now, ctx.map);
                ctx.scheduler
                    .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));

                // Update our follower, so they know we stopped idling.
                let queue = &self.queues[&car.router.head()];
                let idx = queue
                    .cars
                    .iter()
                    .position(|c| *c == car.vehicle.id)
                    .unwrap();
                if idx != queue.cars.len() - 1 {
                    let mut follower = self.cars.get_mut(&queue.cars[idx + 1]).unwrap();
                    match follower.state {
                        CarState::Queued { blocked_since } => {
                            // If they're on their last step, they might be ending early and not
                            // right behind us.
                            if !follower.router.last_step() {
                                follower.total_blocked_time += now - blocked_since;
                                follower.state = follower.crossing_state(
                                    // Since the follower was Queued, this must be where they are.
                                    dist - car.vehicle.length - FOLLOWING_DISTANCE,
                                    now,
                                    ctx.map,
                                );
                                ctx.scheduler.update(
                                    follower.state.get_end_time(),
                                    Command::UpdateCar(follower.vehicle.id),
                                );
                            }
                        }
                        CarState::WaitingToAdvance { .. } => unreachable!(),
                        // They weren't blocked. Note that there's no way the Crossing state could
                        // jump forwards here; the leader is still in front of them.
                        CarState::Crossing(_, _)
                        | CarState::Unparking(_, _, _)
                        | CarState::Parking(_, _, _)
                        | CarState::IdlingAtStop(_, _) => {}
                    }
                }
            }
            CarState::Queued { .. } => unreachable!(),
            CarState::WaitingToAdvance { blocked_since } => {
                // 'car' is the leader.
                let from = car.router.head();
                let goto = car.router.next();
                assert!(from != goto);

                if let Traversable::Turn(t) = goto {
                    let mut speed = goto.speed_limit(ctx.map);
                    if let Some(s) = car.vehicle.max_speed {
                        speed = speed.min(s);
                    }
                    if !ctx.intersections.maybe_start_turn(
                        AgentID::Car(car.vehicle.id),
                        t,
                        speed,
                        now,
                        ctx.map,
                        ctx.scheduler,
                        Some((&car, &self.cars, &mut self.queues)),
                    ) {
                        // Don't schedule a retry here.
                        return false;
                    }
                    if let Some((trip, _)) = car.trip_and_person {
                        self.events.push(Event::TripIntersectionDelay(
                            trip,
                            t,
                            AgentID::Car(car.vehicle.id),
                            now - blocked_since,
                        ));
                    }
                }

                {
                    let mut queue = self.queues.get_mut(&from).unwrap();
                    assert_eq!(queue.cars.pop_front().unwrap(), car.vehicle.id);
                    queue.laggy_head = Some(car.vehicle.id);
                }

                // We do NOT need to update the follower. If they were Queued, they'll remain that
                // way, until laggy_head is None.

                let last_step = car.router.advance(
                    &car.vehicle,
                    ctx.parking,
                    ctx.map,
                    car.trip_and_person,
                    &mut self.events,
                );
                car.total_blocked_time += now - blocked_since;
                car.state = car.crossing_state(Distance::ZERO, now, ctx.map);
                ctx.scheduler
                    .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                self.events.push(Event::AgentEntersTraversable(
                    AgentID::Car(car.vehicle.id),
                    goto,
                    if car.vehicle.vehicle_type.is_transit() {
                        Some(transit.get_passengers(car.vehicle.id).len())
                    } else {
                        None
                    },
                ));

                // Don't mark turn_finished until our back is out of the turn.
                car.last_steps.push_front(last_step);

                // Optimistically assume we'll be out of the way ASAP.
                // This is update, not push, because we might've scheduled a blind retry too late,
                // and the car actually crosses an entire new traversable in the meantime.
                ctx.scheduler.update(
                    car.crossing_state_with_end_dist(
                        DistanceInterval::new_driving(
                            Distance::ZERO,
                            car.vehicle.length + FOLLOWING_DISTANCE,
                        ),
                        now,
                        ctx.map,
                    )
                    .get_end_time(),
                    Command::UpdateLaggyHead(car.vehicle.id),
                );

                self.queues
                    .get_mut(&goto)
                    .unwrap()
                    .cars
                    .push_back(car.vehicle.id);
            }
            CarState::Parking(_, _, _) => unreachable!(),
        }
        false
    }

    // Returns true if the car survives.
    fn update_car_with_distances(
        &mut self,
        car: &mut Car,
        dists: &Vec<(CarID, Distance)>,
        idx: usize,
        now: Time,
        ctx: &mut Ctx,
        trips: &mut TripManager,
        transit: &mut TransitSimState,
        walking: &mut WalkingSimState,
    ) -> bool {
        let our_dist = dists[idx].1;

        match car.state {
            CarState::Crossing(_, _)
            | CarState::Unparking(_, _, _)
            | CarState::IdlingAtStop(_, _)
            | CarState::WaitingToAdvance { .. } => unreachable!(),
            CarState::Queued { blocked_since } => {
                match car.router.maybe_handle_end(
                    our_dist,
                    &car.vehicle,
                    ctx.parking,
                    ctx.map,
                    car.trip_and_person,
                    &mut self.events,
                ) {
                    Some(ActionAtEnd::VanishAtBorder(i)) => {
                        car.total_blocked_time += now - blocked_since;
                        // Don't do this for buses
                        if car.trip_and_person.is_some() {
                            trips.car_or_bike_reached_border(
                                now,
                                car.vehicle.id,
                                i,
                                car.total_blocked_time,
                                car.router.get_path().total_length(),
                                ctx,
                            );
                        }
                        false
                    }
                    Some(ActionAtEnd::GiveUpOnParking) => {
                        car.total_blocked_time += now - blocked_since;
                        trips.cancel_trip(
                            now,
                            car.trip_and_person.unwrap().0,
                            format!("no available parking anywhere"),
                            // If we couldn't find parking normally, doesn't make sense to warp the
                            // car to the destination. There's no parking!
                            None,
                            ctx,
                        );
                        false
                    }
                    Some(ActionAtEnd::StartParking(spot)) => {
                        car.total_blocked_time += now - blocked_since;
                        let delay = match spot {
                            ParkingSpot::Onstreet(_, _) => self.time_to_park_onstreet,
                            ParkingSpot::Offstreet(_, _) | ParkingSpot::Lot(_, _) => {
                                self.time_to_park_offstreet
                            }
                        };
                        car.state =
                            CarState::Parking(our_dist, spot, TimeInterval::new(now, now + delay));
                        // If we don't do this, then we might have another car creep up behind, see
                        // the spot free, and start parking too. This can happen with multiple
                        // lanes and certain vehicle lengths.
                        ctx.parking.reserve_spot(spot, car.vehicle.id);
                        ctx.scheduler
                            .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                        true
                    }
                    Some(ActionAtEnd::GotoLaneEnd) => {
                        car.total_blocked_time += now - blocked_since;
                        car.state = car.crossing_state(our_dist, now, ctx.map);
                        ctx.scheduler
                            .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                        true
                    }
                    Some(ActionAtEnd::StopBiking(bike_rack)) => {
                        car.total_blocked_time += now - blocked_since;
                        trips.bike_reached_end(
                            now,
                            car.vehicle.id,
                            bike_rack,
                            car.total_blocked_time,
                            car.router.get_path().total_length(),
                            ctx,
                        );
                        false
                    }
                    Some(ActionAtEnd::BusAtStop) => {
                        car.total_blocked_time += now - blocked_since;
                        if transit.bus_arrived_at_stop(now, car.vehicle.id, trips, walking, ctx) {
                            car.state = CarState::IdlingAtStop(
                                our_dist,
                                TimeInterval::new(now, now + TIME_TO_WAIT_AT_BUS_STOP),
                            );
                            ctx.scheduler
                                .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                            true
                        } else {
                            // Vanishing at a border
                            false
                        }
                    }
                    None => {
                        ctx.scheduler.push(
                            now + BLIND_RETRY_TO_REACH_END_DIST,
                            Command::UpdateCar(car.vehicle.id),
                        );

                        // TODO For now, always use BLIND_RETRY_TO_REACH_END_DIST. Measured things
                        // to be slower otherwise. :(
                        /*
                        // If this car wasn't blocked at all, when would it reach its goal?
                        let ideal_end_time = match car.crossing_state(our_dist, now, map) {
                            CarState::Crossing(time_int, _) => time_int.end,
                            _ => unreachable!(),
                        };
                        if ideal_end_time == now {
                            // Haha, no such luck. We're super super close to the goal, but not
                            // quite there yet.
                            scheduler.push(now + BLIND_RETRY_TO_REACH_END_DIST, Command::UpdateCar(car.vehicle.id));
                        } else {
                            scheduler.push(ideal_end_time, Command::UpdateCar(car.vehicle.id));
                        }
                        // TODO For cars stuck on their last step, this will spam a fair bit. But
                        // that should be pretty rare.
                        */

                        true
                    }
                }
            }
            CarState::Parking(_, spot, _) => {
                ctx.parking.add_parked_car(ParkedCar {
                    vehicle: car.vehicle.clone(),
                    spot,
                    parked_since: now,
                });
                trips.car_reached_parking_spot(
                    now,
                    car.vehicle.id,
                    spot,
                    car.total_blocked_time,
                    car.router.get_path().total_length(),
                    ctx,
                );
                false
            }
        }
    }

    /// Abruptly remove a vehicle from the simulation. They may be in any arbitrary state, like in
    /// the middle of a turn or parking.
    pub fn delete_car(&mut self, c: CarID, now: Time, ctx: &mut Ctx) -> Vehicle {
        self.waiting_to_spawn.remove(&c);

        let dists = self.queues[&self.cars[&c].router.head()].get_car_positions(
            now,
            &self.cars,
            &self.queues,
        );
        let idx = dists.iter().position(|(id, _)| *id == c).unwrap();
        let mut car = self.cars.remove(&c).unwrap();

        // Hacks to delete cars that're mid-turn
        if let Traversable::Turn(t) = car.router.head() {
            let queue = self.queues.get_mut(&car.router.head()).unwrap();
            queue.reserved_length += car.vehicle.length + FOLLOWING_DISTANCE;
            ctx.intersections.agent_deleted_mid_turn(AgentID::Car(c), t);
        }
        if let Some(Traversable::Turn(t)) = car.router.maybe_next() {
            ctx.intersections.cancel_request(AgentID::Car(c), t);
        }

        self.delete_car_internal(&mut car, dists, idx, now, ctx);
        // delete_car_internal cancels UpdateLaggyHead
        ctx.scheduler.cancel(Command::UpdateCar(c));
        car.vehicle
    }

    fn delete_car_internal(
        &mut self,
        car: &mut Car,
        dists: Vec<(CarID, Distance)>,
        idx: usize,
        now: Time,
        ctx: &mut Ctx,
    ) {
        {
            let queue = self.queues.get_mut(&car.router.head()).unwrap();
            assert_eq!(queue.cars.remove(idx).unwrap(), car.vehicle.id);
            // trim_last_steps doesn't actually include the current queue!
            queue.free_reserved_space(car);
            let i = match queue.id {
                Traversable::Lane(l) => ctx.map.get_l(l).src_i,
                Traversable::Turn(t) => t.parent,
            };
            if !ctx.handling_live_edits {
                ctx.intersections
                    .space_freed(now, i, ctx.scheduler, ctx.map);
            }
        }

        ctx.intersections.vehicle_gone(car.vehicle.id);

        // We might be vanishing while partly clipping into other stuff.
        self.trim_last_steps(car, now, car.last_steps.len(), ctx);

        // We might've scheduled one of those using BLIND_RETRY_TO_CREEP_FORWARDS.
        ctx.scheduler
            .cancel(Command::UpdateLaggyHead(car.vehicle.id));

        // Update the follower so that they don't suddenly jump forwards.
        if idx != dists.len() - 1 {
            let (follower_id, follower_dist) = dists[idx + 1];
            let mut follower = self.cars.get_mut(&follower_id).unwrap();
            // TODO If the leader vanished at a border node, this still jumps a bit -- the lead
            // car's back is still sticking out. Need to still be bound by them, even though they
            // don't exist! If the leader just parked, then we're fine.
            match follower.state {
                CarState::Queued { blocked_since } => {
                    // Prevent them from jumping forwards.
                    follower.total_blocked_time += now - blocked_since;
                    follower.state = follower.crossing_state(follower_dist, now, ctx.map);
                    ctx.scheduler.update(
                        follower.state.get_end_time(),
                        Command::UpdateCar(follower_id),
                    );
                }
                CarState::Crossing(_, _) => {
                    // If the follower was still Crossing, they might not've been blocked by leader
                    // yet. In that case, recalculating their Crossing state is a no-op.
                    follower.state = follower.crossing_state(follower_dist, now, ctx.map);
                    ctx.scheduler.update(
                        follower.state.get_end_time(),
                        Command::UpdateCar(follower_id),
                    );
                }
                // They weren't blocked
                CarState::Unparking(_, _, _)
                | CarState::Parking(_, _, _)
                | CarState::IdlingAtStop(_, _) => {}
                CarState::WaitingToAdvance { .. } => unreachable!(),
            }
        }
    }

    pub fn update_laggy_head(&mut self, id: CarID, now: Time, ctx: &mut Ctx) {
        let currently_on = self.cars[&id].router.head();
        // This car must be the tail.
        let dist_along_last = {
            let (last_id, dist) = self.queues[&currently_on]
                .get_last_car_position(now, &self.cars, &self.queues)
                .unwrap();
            if id != last_id {
                panic!(
                    "At {} on {:?}, laggy head {} isn't the last on the lane; it's {}",
                    now, currently_on, id, last_id
                );
            }
            dist
        };

        // Trim off as many of the oldest last_steps as we've made distance.
        let mut dist_left_to_cleanup = self.cars[&id].vehicle.length + FOLLOWING_DISTANCE;
        dist_left_to_cleanup -= dist_along_last;
        let mut num_to_trim = None;
        for (idx, step) in self.cars[&id].last_steps.iter().enumerate() {
            if dist_left_to_cleanup <= Distance::ZERO {
                num_to_trim = Some(self.cars[&id].last_steps.len() - idx);
                break;
            }
            dist_left_to_cleanup -= step.length(ctx.map);
        }

        if let Some(n) = num_to_trim {
            let mut car = self.cars.remove(&id).unwrap();
            self.trim_last_steps(&mut car, now, n, ctx);
            self.cars.insert(id, car);
        }

        if !self.cars[&id].last_steps.is_empty() {
            // Might have to retry again later.
            let retry_at = self.cars[&id]
                .crossing_state_with_end_dist(
                    // Update again when we've completely cleared all last_steps. We could be more
                    // precise and do it sooner when we clear the last step, but a little delay is
                    // fine for correctness.
                    DistanceInterval::new_driving(
                        dist_along_last,
                        self.cars[&id].vehicle.length + FOLLOWING_DISTANCE,
                    ),
                    now,
                    ctx.map,
                )
                .get_end_time();
            // Sometimes due to rounding, retry_at will be exactly time, but we really need to
            // wait a bit longer.
            // TODO Smarter retry based on states and stuckness?
            if retry_at > now {
                ctx.scheduler.push(retry_at, Command::UpdateLaggyHead(id));
            } else {
                // If we look up car positions before this retry happens, weird things can
                // happen -- the laggy head could be well clear of the old queue by then. Make
                // sure to handle that there. Consequences of this retry being long? A follower
                // will wait a bit before advancing.
                ctx.scheduler.push(
                    now + BLIND_RETRY_TO_CREEP_FORWARDS,
                    Command::UpdateLaggyHead(id),
                );
            }
        }
    }

    // Caller has to figure out how many steps to trim!
    fn trim_last_steps(&mut self, car: &mut Car, now: Time, n: usize, ctx: &mut Ctx) {
        for i in 0..n {
            let on = car.last_steps.pop_back().unwrap();
            let old_queue = self.queues.get_mut(&on).unwrap();
            assert_eq!(old_queue.laggy_head, Some(car.vehicle.id));
            old_queue.laggy_head = None;
            match on {
                Traversable::Turn(t) => {
                    ctx.intersections.turn_finished(
                        now,
                        AgentID::Car(car.vehicle.id),
                        t,
                        ctx.scheduler,
                        ctx.map,
                        ctx.handling_live_edits,
                    );
                }
                Traversable::Lane(l) => {
                    old_queue.free_reserved_space(car);
                    if !ctx.handling_live_edits {
                        ctx.intersections.space_freed(
                            now,
                            ctx.map.get_l(l).src_i,
                            ctx.scheduler,
                            ctx.map,
                        );
                    }
                }
            }

            if i == 0 {
                // Wake up the follower
                if let Some(follower_id) = old_queue.cars.front() {
                    let mut follower = self.cars.get_mut(&follower_id).unwrap();

                    match follower.state {
                        CarState::Queued { blocked_since } => {
                            // If they're on their last step, they might be ending early and not
                            // right behind us.
                            if !follower.router.last_step() {
                                // The follower has been smoothly following while the laggy head
                                // gets out of the way. So immediately promote them to
                                // WaitingToAdvance.
                                follower.state = CarState::WaitingToAdvance { blocked_since };
                                if self.recalc_lanechanging && !ctx.handling_live_edits {
                                    follower.router.opportunistically_lanechange(
                                        &self.queues,
                                        ctx.map,
                                        self.handle_uber_turns,
                                    );
                                }
                                ctx.scheduler
                                    .push(now, Command::UpdateCar(follower.vehicle.id));
                            }
                        }
                        CarState::WaitingToAdvance { .. } => unreachable!(),
                        // They weren't blocked. Note that there's no way the Crossing state could
                        // jump forwards here; the leader vanished from the end of the traversable.
                        CarState::Crossing(_, _)
                        | CarState::Unparking(_, _, _)
                        | CarState::Parking(_, _, _)
                        | CarState::IdlingAtStop(_, _) => {}
                    }
                }
            } else {
                // Only the last step we cleared could possibly have cars. Any intermediates, this
                // car was previously completely blocking them.
                assert!(old_queue.cars.is_empty());
            }
        }
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }

    pub fn handle_live_edits(&mut self, map: &Map) {
        // Calculate all queues that should exist now.
        let mut new_queues = HashSet::new();
        for l in map.all_lanes() {
            if l.lane_type.is_for_moving_vehicles() {
                new_queues.insert(Traversable::Lane(l.id));
            }
        }
        for t in map.all_turns().values() {
            if !t.between_sidewalks() {
                new_queues.insert(Traversable::Turn(t.id));
            }
        }

        // Delete any old queues.
        self.queues.retain(|k, v| {
            if new_queues.remove(k) {
                // No changes
                true
            } else {
                // Make sure it's empty!
                if v.laggy_head.is_some() || !v.cars.is_empty() {
                    panic!(
                        "After live map edits, deleted queue {} still has vehicles! {:?}, {:?}",
                        k, v.laggy_head, v.cars
                    );
                }
                false
            }
        });

        // Create any new queues
        for key in new_queues {
            self.queues.insert(key, Queue::new(key, map));
        }
    }
}

// Queries
impl DrivingSimState {
    /// Note the ordering of results is non-deterministic!
    pub fn get_unzoomed_agents(&self, now: Time, map: &Map) -> Vec<UnzoomedAgent> {
        let mut result = Vec::new();

        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }

            for (c, dist) in queue.get_car_positions(now, &self.cars, &self.queues) {
                let car = &self.cars[&c];
                result.push(UnzoomedAgent {
                    id: AgentID::Car(car.vehicle.id),
                    pos: match queue.id.dist_along(dist, map) {
                        Ok((pt, _)) => pt,
                        Err(err) => panic!(
                            "At {}, invalid dist_along of {} for queue {}: {}",
                            now, dist, queue.id, err
                        ),
                    },
                    person: car.trip_and_person.map(|(_, p)| p),
                    parking: car.is_parking(),
                });
            }
        }

        for (id, (pos, person)) in &self.waiting_to_spawn {
            result.push(UnzoomedAgent {
                id: AgentID::Car(*id),
                pos: pos.pt(map),
                person: *person,
                parking: false,
            });
        }

        result
    }

    pub fn does_car_exist(&self, id: CarID) -> bool {
        self.cars.contains_key(&id)
    }

    /// Note the ordering of results is non-deterministic!
    pub fn get_all_draw_cars(
        &self,
        now: Time,
        map: &Map,
        transit: &TransitSimState,
    ) -> Vec<DrawCarInput> {
        let mut result = Vec::new();
        for queue in self.queues.values() {
            result.extend(
                queue
                    .get_car_positions(now, &self.cars, &self.queues)
                    .into_iter()
                    .map(|(id, dist)| self.cars[&id].get_draw_car(dist, now, map, transit)),
            );
        }
        result
    }

    /// This is about as expensive as get_draw_cars_on.
    pub fn get_single_draw_car(
        &self,
        id: CarID,
        now: Time,
        map: &Map,
        transit: &TransitSimState,
    ) -> Option<DrawCarInput> {
        let car = self.cars.get(&id)?;
        self.get_draw_cars_on(now, car.router.head(), map, transit)
            .into_iter()
            .find(|d| d.id == id)
    }

    pub fn get_draw_cars_on(
        &self,
        now: Time,
        on: Traversable,
        map: &Map,
        transit: &TransitSimState,
    ) -> Vec<DrawCarInput> {
        match self.queues.get(&on) {
            Some(q) => q
                .get_car_positions(now, &self.cars, &self.queues)
                .into_iter()
                .map(|(id, dist)| self.cars[&id].get_draw_car(dist, now, map, transit))
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn debug_car(&self, id: CarID) {
        if let Some(ref car) = self.cars.get(&id) {
            println!("{}", abstutil::to_json(car));
            println!("State: {:?}", car.state);
        } else {
            println!("{} is parked somewhere", id);
        }
    }

    pub fn debug_lane(&self, id: LaneID) {
        if let Some(ref queue) = self.queues.get(&Traversable::Lane(id)) {
            println!("{}", abstutil::to_json(queue));
        }
    }

    pub fn agent_properties(&self, id: CarID, now: Time) -> AgentProperties {
        let car = self.cars.get(&id).unwrap();
        let path = car.router.get_path();
        let time_spent_waiting = car.state.time_spent_waiting(now);

        // In all cases, we can figure out exactly where we are along the current queue, then
        // assume we've travelled from the start of that, unless it's the very first step.
        let front = self.get_car_front(now, car);
        let current_state_dist =
            if car.router.head() == Traversable::Lane(path.get_req().start.lane()) {
                front - path.get_req().start.dist_along()
            } else {
                front
            };

        AgentProperties {
            total_time: now - car.started_at,
            waiting_here: time_spent_waiting,
            total_waiting: car.total_blocked_time + time_spent_waiting,
            dist_crossed: path.crossed_so_far() + current_state_dist,
            total_dist: path.total_length(),
        }
    }

    pub fn get_path(&self, id: CarID) -> Option<&Path> {
        let car = self.cars.get(&id)?;
        Some(car.router.get_path())
    }
    pub fn get_all_driving_paths(&self) -> Vec<&Path> {
        self.cars
            .values()
            .map(|car| car.router.get_path())
            .collect()
    }

    pub fn trace_route(&self, now: Time, id: CarID, map: &Map) -> Option<PolyLine> {
        let car = self.cars.get(&id)?;
        let front = self.get_car_front(now, car);
        car.router.get_path().trace_from_start(map, front)
    }

    pub fn percent_along_route(&self, id: CarID) -> f64 {
        self.cars[&id].router.get_path().percent_dist_crossed()
    }

    pub fn get_owner_of_car(&self, id: CarID) -> Option<PersonID> {
        let car = self.cars.get(&id)?;
        car.vehicle.owner
    }

    pub fn target_lane_penalty(&self, l: LaneID) -> (usize, usize) {
        self.queues[&Traversable::Lane(l)].target_lane_penalty()
    }

    pub fn find_trips_to_edited_parking(
        &self,
        spots: BTreeSet<ParkingSpot>,
    ) -> Vec<(AgentID, TripID)> {
        let mut affected = Vec::new();
        for car in self.cars.values() {
            if let Some(spot) = car.router.get_parking_spot_goal() {
                if !spots.contains(spot) {
                    // Buses don't park
                    affected.push((AgentID::Car(car.vehicle.id), car.trip_and_person.unwrap().0));
                }
            }
        }
        affected
    }

    /// Finds vehicles that're laggy heads on affected parts of the map.
    pub fn find_vehicles_affected_by_live_edits(
        &self,
        closed_intersections: &HashSet<IntersectionID>,
        edited_lanes: &BTreeSet<LaneID>,
    ) -> Vec<(AgentID, TripID)> {
        let mut affected = Vec::new();
        for car in self.cars.values() {
            if car.last_steps.iter().any(|step| match step {
                Traversable::Lane(l) => edited_lanes.contains(&l),
                Traversable::Turn(t) => {
                    closed_intersections.contains(&t.parent)
                        || edited_lanes.contains(&t.src)
                        || edited_lanes.contains(&t.dst)
                }
            }) {
                // TODO Buses aren't handled yet! Mostly not a big deal, because they're pretty
                // much never created anyway.
                if let Some((trip, _)) = car.trip_and_person {
                    affected.push((AgentID::Car(car.vehicle.id), trip));
                }
            }
        }
        affected
    }

    pub fn all_waiting_people(&self, now: Time, delays: &mut BTreeMap<PersonID, Duration>) {
        for c in self.cars.values() {
            if let Some((_, person)) = c.trip_and_person {
                let delay = c.state.time_spent_waiting(now);
                if delay > Duration::ZERO {
                    delays.insert(person, delay);
                }
            }
        }
    }

    pub fn debug_queue_lengths(&self, l: LaneID) -> Option<(Distance, Distance)> {
        let queue = self.queues.get(&Traversable::Lane(l))?;
        Some((queue.reserved_length, queue.geom_len))
    }

    pub fn get_blocked_by_graph(
        &self,
        now: Time,
        map: &Map,
        intersections: &IntersectionSimState,
    ) -> BTreeMap<AgentID, (Duration, DelayCause)> {
        let mut graph = BTreeMap::new();

        // Just look for every case where somebody is behind someone else, whether or not they're
        // blocked by them and waiting.
        for queue in self.queues.values() {
            if let Some(head) = queue.laggy_head {
                if let Some(next) = queue.cars.front() {
                    graph.insert(
                        AgentID::Car(*next),
                        (
                            self.cars[&head].state.time_spent_waiting(now),
                            DelayCause::Agent(AgentID::Car(head)),
                        ),
                    );
                }
            }
            for (head, tail) in queue.cars.iter().zip(queue.cars.iter().skip(1)) {
                graph.insert(
                    AgentID::Car(*tail),
                    (
                        self.cars[tail].state.time_spent_waiting(now),
                        DelayCause::Agent(AgentID::Car(*head)),
                    ),
                );
            }
        }

        intersections.populate_blocked_by(now, &mut graph, map, &self.cars, &self.queues);
        graph
    }

    fn get_car_front(&self, now: Time, car: &Car) -> Distance {
        self.queues[&car.router.head()]
            .get_car_positions(now, &self.cars, &self.queues)
            .into_iter()
            .find(|(c, _)| *c == car.vehicle.id)
            .unwrap()
            .1
    }
}

impl IndexableKey for CarID {
    fn index(&self) -> usize {
        self.0
    }
}
