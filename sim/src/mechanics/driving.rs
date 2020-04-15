use crate::mechanics::car::{Car, CarState};
use crate::mechanics::Queue;
use crate::{
    ActionAtEnd, AgentID, AgentProperties, CarID, Command, CreateCar, DistanceInterval,
    DrawCarInput, Event, IntersectionSimState, ParkedCar, ParkingSimState, PersonID, Scheduler,
    TimeInterval, TransitSimState, TripManager, TripPositions, UnzoomedAgent, WalkingSimState,
    FOLLOWING_DISTANCE,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Distance, Duration, PolyLine, Time};
use map_model::{LaneID, Map, Path, PathStep, Traversable};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};

const TIME_TO_UNPARK: Duration = Duration::const_seconds(10.0);
const TIME_TO_PARK: Duration = Duration::const_seconds(15.0);
const TIME_TO_WAIT_AT_STOP: Duration = Duration::const_seconds(10.0);

// TODO Do something else.
pub(crate) const BLIND_RETRY_TO_CREEP_FORWARDS: Duration = Duration::const_seconds(0.1);
pub(crate) const BLIND_RETRY_TO_REACH_END_DIST: Duration = Duration::const_seconds(5.0);

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct DrivingSimState {
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    cars: BTreeMap<CarID, Car>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    queues: BTreeMap<Traversable, Queue>,
    events: Vec<Event>,

    recalc_lanechanging: bool,
    clear_laggy_head_early: bool,
}

impl DrivingSimState {
    pub fn new(
        map: &Map,
        recalc_lanechanging: bool,
        clear_laggy_head_early: bool,
    ) -> DrivingSimState {
        let mut sim = DrivingSimState {
            cars: BTreeMap::new(),
            queues: BTreeMap::new(),
            events: Vec::new(),
            recalc_lanechanging,
            clear_laggy_head_early,
        };

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

    // True if it worked
    pub fn start_car_on_lane(
        &mut self,
        now: Time,
        params: CreateCar,
        map: &Map,
        intersections: &IntersectionSimState,
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
    ) -> bool {
        let first_lane = params.router.head().as_lane();

        if !intersections.nobody_headed_towards(first_lane, map.get_l(first_lane).src_i) {
            return false;
        }
        if let Some(idx) = self.queues[&Traversable::Lane(first_lane)].get_idx_to_insert_car(
            params.start_dist,
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
                car.state = CarState::Unparking(
                    params.start_dist,
                    p.spot,
                    TimeInterval::new(now, now + TIME_TO_UNPARK),
                );
            } else {
                // Have to do this early
                if car.router.last_step() {
                    match car.router.maybe_handle_end(
                        params.start_dist,
                        &car.vehicle,
                        parking,
                        map,
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
                    if car.router.last_step() && params.start_dist > car.router.get_end_dist() {
                        println!(
                            "WARNING: {} wants to spawn past their end on a one-step path",
                            car.vehicle.id
                        );
                        return false;
                    }
                }

                car.state = car.crossing_state(params.start_dist, now, map);
            }
            scheduler.push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
            {
                let queue = self.queues.get_mut(&Traversable::Lane(first_lane)).unwrap();
                queue.cars.insert(idx, car.vehicle.id);
                // Don't use try_to_reserve_entry -- it's overly conservative.
                // get_idx_to_insert_car does a more detailed check of the current space usage.
                queue.reserved_length += car.vehicle.length + FOLLOWING_DISTANCE;
            }
            self.cars.insert(car.vehicle.id, car);
            return true;
        }
        false
    }

    pub fn update_car(
        &mut self,
        id: CarID,
        now: Time,
        map: &Map,
        parking: &mut ParkingSimState,
        intersections: &mut IntersectionSimState,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        transit: &mut TransitSimState,
        walking: &mut WalkingSimState,
    ) {
        // State transitions for this car:
        //
        // Crossing -> Queued or WaitingToAdvance
        // Unparking -> Crossing
        // Idling -> Crossing
        // Queued -> last step handling (Parking or done)
        // WaitingToAdvance -> try to advance to the next step of the path
        // Parking -> done
        //
        // State transitions for other cars:
        //
        // Crossing -> Crossing (recalculate dist/time)
        // Queued -> Crossing
        //
        // Why is it safe to process cars in any order, rather than making sure to follow the order
        // of queues? Because of the invariant that distances should never suddenly jump when a car
        // has entered/exiting a queue.
        // This car might have reached the router's end distance, but maybe not -- might
        // actually be stuck behind other cars. We have to calculate the distances right now to
        // be sure.
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
            // checker, temporarily move one of them out of the BTreeMap.
            let mut car = self.cars.remove(&id).unwrap();
            // Responsibility of update_car to manage scheduling stuff!
            need_distances = self.update_car_without_distances(
                &mut car,
                now,
                map,
                parking,
                intersections,
                transit,
                scheduler,
            );
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
            // checker, temporarily move one of them out of the BTreeMap.
            let mut car = self.cars.remove(&id).unwrap();
            // Responsibility of update_car_with_distances to manage scheduling stuff!
            if self.update_car_with_distances(
                &mut car, &dists, idx, now, map, parking, trips, scheduler, transit, walking,
            ) {
                self.cars.insert(id, car);
            } else {
                self.delete_car(&mut car, dists, idx, now, map, scheduler, intersections);
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
        map: &Map,
        parking: &mut ParkingSimState,
        intersections: &mut IntersectionSimState,
        transit: &mut TransitSimState,
        scheduler: &mut Scheduler,
    ) -> bool {
        match car.state {
            CarState::Crossing(_, _) => {
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
                        car.router.opportunistically_lanechange(&self.queues, map);
                    }
                    scheduler.push(now, Command::UpdateCar(car.vehicle.id));
                }
            }
            CarState::Unparking(front, _, _) => {
                if car.router.last_step() {
                    // Actually, we need to do this first. Ignore the answer -- if we're
                    // doing something weird like vanishing or re-parking immediately
                    // (quite unlikely), the next loop will pick that up. Just trigger the
                    // side effect of choosing an end_dist.
                    car.router.maybe_handle_end(
                        front,
                        &car.vehicle,
                        parking,
                        map,
                        car.trip_and_person,
                        &mut self.events,
                    );
                }
                car.state = car.crossing_state(front, now, map);
                scheduler.push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
            }
            CarState::Idling(dist, _) => {
                car.router = transit.bus_departed_from_stop(car.vehicle.id);
                self.events
                    .push(Event::PathAmended(car.router.get_path().clone()));
                car.state = car.crossing_state(dist, now, map);
                scheduler.push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));

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
                                    map,
                                );
                                scheduler.update(
                                    follower.state.get_end_time(),
                                    Command::UpdateCar(follower.vehicle.id),
                                );
                            }
                        }
                        CarState::WaitingToAdvance { .. } => unreachable!(),
                        // They weren't blocked. Note that there's no way the Crossing state could
                        // jump forwards here; the leader is still in front
                        // of them.
                        CarState::Crossing(_, _)
                        | CarState::Unparking(_, _, _)
                        | CarState::Parking(_, _, _)
                        | CarState::Idling(_, _) => {}
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
                    let mut speed = goto.speed_limit(map);
                    if let Some(s) = car.vehicle.max_speed {
                        speed = speed.min(s);
                    }
                    if !intersections.maybe_start_turn(
                        AgentID::Car(car.vehicle.id),
                        t,
                        speed,
                        now,
                        map,
                        scheduler,
                        Some((
                            self.queues.get_mut(&Traversable::Lane(t.dst)).unwrap(),
                            &car,
                        )),
                    ) {
                        // Don't schedule a retry here.
                        return false;
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
                    parking,
                    map,
                    car.trip_and_person,
                    &mut self.events,
                );
                car.total_blocked_time += now - blocked_since;
                car.state = car.crossing_state(Distance::ZERO, now, map);
                scheduler.push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                self.events.push(Event::AgentEntersTraversable(
                    AgentID::Car(car.vehicle.id),
                    goto,
                ));

                car.last_steps.push_front(last_step);
                // Bit unrealistic, but don't unblock shorter intermediate steps until we're all
                // the way into a lane later.
                // Don't mark turn_finished until our back is out of the turn.
                // TODO Don't even bother updating laggy head (which will unblock intermediate
                // steps and call turn_finished and such) if we're bound for a tiny lane. Unless
                // we're trying the experimental clear_laggy_head_early strategy.
                if goto.length(map) >= car.vehicle.length + FOLLOWING_DISTANCE
                    || self.clear_laggy_head_early
                {
                    // Optimistically assume we'll be out of the way ASAP.
                    // This is update, not push, because we might've scheduled a blind retry too
                    // late, and the car actually crosses an entire new traversable in the
                    // meantime.
                    scheduler.update(
                        car.crossing_state_with_end_dist(
                            DistanceInterval::new_driving(
                                Distance::ZERO,
                                car.vehicle.length + FOLLOWING_DISTANCE,
                            ),
                            now,
                            map,
                        )
                        .get_end_time(),
                        Command::UpdateLaggyHead(car.vehicle.id),
                    );
                }

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
        map: &Map,
        parking: &mut ParkingSimState,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        transit: &mut TransitSimState,
        walking: &mut WalkingSimState,
    ) -> bool {
        let our_dist = dists[idx].1;

        match car.state {
            CarState::Crossing(_, _)
            | CarState::Unparking(_, _, _)
            | CarState::Idling(_, _)
            | CarState::WaitingToAdvance { .. } => unreachable!(),
            CarState::Queued { blocked_since } => {
                match car.router.maybe_handle_end(
                    our_dist,
                    &car.vehicle,
                    parking,
                    map,
                    car.trip_and_person,
                    &mut self.events,
                ) {
                    Some(ActionAtEnd::VanishAtBorder(i)) => {
                        car.total_blocked_time += now - blocked_since;
                        trips.car_or_bike_reached_border(
                            now,
                            car.vehicle.id,
                            i,
                            car.total_blocked_time,
                        );
                        false
                    }
                    Some(ActionAtEnd::AbortTrip) => {
                        car.total_blocked_time += now - blocked_since;
                        trips.abort_trip_impossible_parking(car.vehicle.id);
                        false
                    }
                    Some(ActionAtEnd::StartParking(spot)) => {
                        car.total_blocked_time += now - blocked_since;
                        car.state = CarState::Parking(
                            our_dist,
                            spot,
                            TimeInterval::new(now, now + TIME_TO_PARK),
                        );
                        // If we don't do this, then we might have another car creep up
                        // behind, see the spot free, and start parking too. This can
                        // happen with multiple lanes and certain vehicle lengths.
                        parking.reserve_spot(spot);
                        scheduler
                            .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                        true
                    }
                    Some(ActionAtEnd::GotoLaneEnd) => {
                        car.total_blocked_time += now - blocked_since;
                        car.state = car.crossing_state(our_dist, now, map);
                        scheduler
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
                            map,
                            scheduler,
                        );
                        false
                    }
                    Some(ActionAtEnd::BusAtStop) => {
                        transit.bus_arrived_at_stop(
                            now,
                            car.vehicle.id,
                            trips,
                            walking,
                            scheduler,
                            map,
                        );
                        car.total_blocked_time += now - blocked_since;
                        car.state = CarState::Idling(
                            our_dist,
                            TimeInterval::new(now, now + TIME_TO_WAIT_AT_STOP),
                        );
                        scheduler
                            .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                        true
                    }
                    None => {
                        scheduler.push(
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
                parking.add_parked_car(ParkedCar {
                    vehicle: car.vehicle.clone(),
                    spot,
                });
                trips.car_reached_parking_spot(
                    now,
                    car.vehicle.id,
                    spot,
                    car.total_blocked_time,
                    map,
                    parking,
                    scheduler,
                );
                false
            }
        }
    }

    pub fn kill_stuck_car(
        &mut self,
        c: CarID,
        now: Time,
        map: &Map,
        scheduler: &mut Scheduler,
        intersections: &mut IntersectionSimState,
    ) {
        let dists = self.queues[&self.cars[&c].router.head()].get_car_positions(
            now,
            &self.cars,
            &self.queues,
        );
        let idx = dists.iter().position(|(id, _)| *id == c).unwrap();
        let mut car = self.cars.remove(&c).unwrap();

        // Hacks to delete cars that're mid-turn
        if let Traversable::Turn(_) = car.router.head() {
            let queue = self.queues.get_mut(&car.router.head()).unwrap();
            queue.reserved_length += car.vehicle.length + FOLLOWING_DISTANCE;
        }
        if let Some(Traversable::Turn(t)) = car.router.maybe_next() {
            intersections.cancel_request(AgentID::Car(c), t);
        }

        self.delete_car(&mut car, dists, idx, now, map, scheduler, intersections);
        // delete_car cancels UpdateLaggyHead
        scheduler.cancel(Command::UpdateCar(c));
    }

    fn delete_car(
        &mut self,
        car: &mut Car,
        dists: Vec<(CarID, Distance)>,
        idx: usize,
        now: Time,
        map: &Map,
        scheduler: &mut Scheduler,
        intersections: &mut IntersectionSimState,
    ) {
        {
            let queue = self.queues.get_mut(&car.router.head()).unwrap();
            assert_eq!(queue.cars.remove(idx).unwrap(), car.vehicle.id);
            // clear_last_steps doesn't actually include the current queue!
            queue.free_reserved_space(car);
            let i = match queue.id {
                Traversable::Lane(l) => map.get_l(l).src_i,
                Traversable::Turn(t) => t.parent,
            };
            intersections.space_freed(now, i, scheduler, map);
        }

        // We might be vanishing while partly clipping into other stuff.
        self.clear_last_steps(now, car, intersections, scheduler, map);

        // We might've scheduled one of those using BLIND_RETRY_TO_CREEP_FORWARDS.
        scheduler.cancel(Command::UpdateLaggyHead(car.vehicle.id));

        // Update the follower so that they don't suddenly jump forwards.
        if idx != dists.len() - 1 {
            let (follower_id, follower_dist) = dists[idx + 1];
            let mut follower = self.cars.get_mut(&follower_id).unwrap();
            // TODO If the leader vanished at a border node, this still jumps a bit -- the
            // lead car's back is still sticking out. Need to still be bound by them, even
            // though they don't exist! If the leader just parked, then we're fine.
            match follower.state {
                CarState::Queued { blocked_since } => {
                    // Prevent them from jumping forwards.
                    follower.total_blocked_time += now - blocked_since;
                    follower.state = follower.crossing_state(follower_dist, now, map);
                    scheduler.update(
                        follower.state.get_end_time(),
                        Command::UpdateCar(follower_id),
                    );
                }
                CarState::Crossing(_, _) => {
                    // If the follower was still Crossing, they might not've been blocked
                    // by leader yet. In that case, recalculating their Crossing state is a
                    // no-op.
                    follower.state = follower.crossing_state(follower_dist, now, map);
                    scheduler.update(
                        follower.state.get_end_time(),
                        Command::UpdateCar(follower_id),
                    );
                }
                // They weren't blocked
                CarState::Unparking(_, _, _)
                | CarState::Parking(_, _, _)
                | CarState::Idling(_, _) => {}
                CarState::WaitingToAdvance { .. } => unreachable!(),
            }
        }
    }

    pub fn update_laggy_head(
        &mut self,
        id: CarID,
        now: Time,
        map: &Map,
        intersections: &mut IntersectionSimState,
        scheduler: &mut Scheduler,
    ) {
        // TODO The impl here is pretty gross; play the same trick and remove car temporarily?
        let on = self.cars[&id].router.head();
        let dists = self.queues[&on].get_car_positions(now, &self.cars, &self.queues);
        // This car must be the tail.
        {
            let last = dists.last().unwrap().0;
            if id != last {
                panic!(
                    "At {} on {:?}, laggy head {} isn't the last on the lane; it's {}",
                    now, on, id, last
                );
            }
        }
        let our_len = self.cars[&id].vehicle.length + FOLLOWING_DISTANCE;

        // Have we made it far enough yet? Unfortunately, we have some math imprecision issues...
        {
            let our_dist = dists.last().unwrap().1;
            let car = &self.cars[&id];

            // TODO Not working yet -- causes something to spawn too close to short stuff.
            let avoid_gridlock = self.clear_laggy_head_early
                && our_dist == on.length(map)
                && on.maybe_lane().is_some();

            if our_dist < our_len && !avoid_gridlock {
                let retry_at = car
                    .crossing_state_with_end_dist(
                        DistanceInterval::new_driving(our_dist, our_len),
                        now,
                        map,
                    )
                    .get_end_time();
                // Sometimes due to rounding, retry_at will be exactly time, but we really need to
                // wait a bit longer.
                // TODO Smarter retry based on states and stuckness?
                if retry_at > now {
                    scheduler.push(retry_at, Command::UpdateLaggyHead(car.vehicle.id));
                } else {
                    // If we look up car positions before this retry happens, weird things can
                    // happen -- the laggy head could be well clear of the old queue by then. Make
                    // sure to handle that there. Consequences of this retry being long? A follower
                    // will wait a bit before advancing.
                    scheduler.push(
                        now + BLIND_RETRY_TO_CREEP_FORWARDS,
                        Command::UpdateLaggyHead(car.vehicle.id),
                    );
                }
                return;
            }
        }

        // Argh, fight the borrow checker.
        let mut car = self.cars.remove(&id).unwrap();
        self.clear_last_steps(now, &mut car, intersections, scheduler, map);
        self.cars.insert(id, car);
    }

    fn clear_last_steps(
        &mut self,
        now: Time,
        car: &mut Car,
        intersections: &mut IntersectionSimState,
        scheduler: &mut Scheduler,
        map: &Map,
    ) {
        // If we were blocking a few short lanes, should be better now. Very last one might have
        // somebody to wake up.
        let last_steps: Vec<Traversable> = car.last_steps.drain(..).collect();

        for (idx, on) in last_steps.iter().enumerate() {
            let old_queue = self.queues.get_mut(&on).unwrap();
            assert_eq!(old_queue.laggy_head, Some(car.vehicle.id));
            old_queue.laggy_head = None;
            match on {
                Traversable::Turn(t) => {
                    intersections.turn_finished(
                        now,
                        AgentID::Car(car.vehicle.id),
                        *t,
                        scheduler,
                        map,
                    );
                }
                Traversable::Lane(l) => {
                    old_queue.free_reserved_space(car);
                    intersections.space_freed(now, map.get_l(*l).src_i, scheduler, map);
                }
            }

            if idx == last_steps.len() - 1 {
                // Wake up the follower
                if let Some(follower_id) = old_queue.cars.front() {
                    let mut follower = self.cars.get_mut(&follower_id).unwrap();

                    match follower.state {
                        CarState::Queued { blocked_since } => {
                            // If they're on their last step, they might be ending early and not
                            // right behind us.
                            if !follower.router.last_step() {
                                // The follower has been smoothly following while the laggy head
                                // gets out of the way. So
                                // immediately promote them to WaitingToAdvance.
                                follower.state = CarState::WaitingToAdvance { blocked_since };
                                if self.recalc_lanechanging {
                                    follower
                                        .router
                                        .opportunistically_lanechange(&self.queues, map);
                                }
                                scheduler.push(now, Command::UpdateCar(follower.vehicle.id));
                            }
                        }
                        CarState::WaitingToAdvance { .. } => unreachable!(),
                        // They weren't blocked. Note that there's no way the Crossing state could
                        // jump forwards here; the leader vanished from the
                        // end of the traversable.
                        CarState::Crossing(_, _)
                        | CarState::Unparking(_, _, _)
                        | CarState::Parking(_, _, _)
                        | CarState::Idling(_, _) => {}
                    }
                }
            } else {
                assert!(self.queues[&on].cars.is_empty());
            }
        }
    }

    pub fn get_unzoomed_agents(&self, now: Time, map: &Map) -> Vec<UnzoomedAgent> {
        let mut result = Vec::new();

        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }

            for (c, dist) in queue.get_car_positions(now, &self.cars, &self.queues) {
                let car = &self.cars[&c];
                result.push(UnzoomedAgent {
                    vehicle_type: Some(car.vehicle.vehicle_type),
                    pos: queue.id.dist_along(dist, map).0,
                    person: car.trip_and_person.map(|(_, p)| p),
                });
            }
        }

        result
    }

    pub fn does_car_exist(&self, id: CarID) -> bool {
        self.cars.contains_key(&id)
    }

    pub fn populate_trip_positions(&self, trip_positions: &mut TripPositions, map: &Map) {
        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }

            for (car, dist) in
                queue.get_car_positions(trip_positions.time, &self.cars, &self.queues)
            {
                if let Some((trip, _)) = self.cars[&car].trip_and_person {
                    trip_positions
                        .canonical_pt_per_trip
                        .insert(trip, queue.id.dist_along(dist, map).0);
                }
            }
        }
    }

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

    // This is about as expensive as get_draw_cars_on.
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

        let time_spent_waiting = match car.state {
            CarState::Queued { blocked_since } | CarState::WaitingToAdvance { blocked_since } => {
                now - blocked_since
            }
            _ => Duration::ZERO,
        };

        AgentProperties {
            total_time: now - car.started_at,
            waiting_here: time_spent_waiting,
            total_waiting: car.total_blocked_time + time_spent_waiting,
            dist_crossed: path.crossed_so_far(),
            total_dist: path.total_length(),
            lanes_crossed: path.lanes_crossed_so_far(),
            total_lanes: path.total_lanes(),
        }
    }

    pub fn get_path(&self, id: CarID) -> Option<&Path> {
        let car = self.cars.get(&id)?;
        Some(car.router.get_path())
    }

    pub fn trace_route(
        &self,
        now: Time,
        id: CarID,
        map: &Map,
        dist_ahead: Option<Distance>,
    ) -> Option<PolyLine> {
        let car = self.cars.get(&id)?;
        let front = self.queues[&car.router.head()]
            .get_car_positions(now, &self.cars, &self.queues)
            .into_iter()
            .find(|(c, _)| *c == id)
            .unwrap()
            .1;
        car.router.get_path().trace(map, front, dist_ahead)
    }

    pub fn percent_along_route(&self, id: CarID) -> f64 {
        self.cars[&id].router.get_path().percent_dist_crossed()
    }

    pub fn get_owner_of_car(&self, id: CarID) -> Option<PersonID> {
        let car = self.cars.get(&id)?;
        car.vehicle.owner
    }

    // TODO Clean this up
    pub fn find_blockage_front(
        &self,
        start: CarID,
        map: &Map,
        intersections: &IntersectionSimState,
    ) -> String {
        let mut seen_intersections = HashSet::new();

        let mut current_head = start;
        let mut current_lane = match self.cars[&start].router.head() {
            Traversable::Lane(l) => l,
            Traversable::Turn(_) => {
                return "TODO Doesn't support starting from a turn yet".to_string();
            }
        };
        loop {
            current_head =
                if let Some(c) = self.queues[&Traversable::Lane(current_lane)].cars.get(0) {
                    *c
                } else {
                    return format!("no gridlock, {}", current_head);
                };

            let i = map.get_l(current_lane).dst_i;
            if seen_intersections.contains(&i) {
                return format!("Gridlock near {}! {:?}", i, seen_intersections);
            }
            seen_intersections.insert(i);

            // Why isn't current_head proceeding? Pedestrians can never get stuck in an
            // intersection.
            if intersections
                .get_accepted_agents(i)
                .iter()
                .any(|a| matches!(a, AgentID::Car(_)))
            {
                return format!("someone's turning in {} still", i);
            }

            current_lane = if let Some(PathStep::Lane(l)) = self.cars[&current_head]
                .router
                .get_path()
                .get_steps()
                .get(2)
            {
                *l
            } else {
                return format!(
                    "{} is near end of path, probably tmp blockage",
                    current_head
                );
            };

            // Lack of capacity?
            if self.queues[&Traversable::Lane(current_lane)].room_for_car(&self.cars[&current_head])
            {
                return format!("{} is about to proceed, tmp blockage", current_head);
            }
        }
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }
}
