use crate::mechanics::car::{Car, CarState};
use crate::mechanics::queue::Queue;
use crate::{
    ActionAtEnd, AgentID, CarID, Command, CreateCar, DistanceInterval, DrawCarInput,
    IntersectionSimState, ParkedCar, ParkingSimState, Scheduler, TimeInterval, TransitSimState,
    TripManager, WalkingSimState, FOLLOWING_DISTANCE,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Distance, Duration, PolyLine, Polygon};
use map_model::{BuildingID, DirectedRoadID, IntersectionID, LaneID, Map, Path, Traversable};
use petgraph::graph::{Graph, NodeIndex};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};

const TIME_TO_UNPARK: Duration = Duration::const_seconds(10.0);
const TIME_TO_PARK: Duration = Duration::const_seconds(15.0);
const TIME_TO_WAIT_AT_STOP: Duration = Duration::const_seconds(10.0);

// TODO Do something else.
pub(crate) const BLIND_RETRY_TO_CREEP_FORWARDS: Duration = Duration::const_seconds(0.1);
pub(crate) const BLIND_RETRY_TO_REACH_END_DIST: Duration = Duration::const_seconds(5.0);

#[derive(Serialize, Deserialize, PartialEq)]
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
}

impl DrivingSimState {
    pub fn new(map: &Map) -> DrivingSimState {
        let mut sim = DrivingSimState {
            cars: BTreeMap::new(),
            queues: BTreeMap::new(),
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
        time: Duration,
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
            time,
            &self.cars,
            &self.queues,
        ) {
            let mut car = Car {
                vehicle: params.vehicle,
                router: params.router,
                // Temporary
                state: CarState::Queued,
                last_steps: VecDeque::new(),
            };
            if params.maybe_parked_car.is_some() {
                car.state = CarState::Unparking(
                    params.start_dist,
                    TimeInterval::new(time, time + TIME_TO_UNPARK),
                );
            } else {
                // Have to do this early
                if car.router.last_step() {
                    match car
                        .router
                        .maybe_handle_end(params.start_dist, &car.vehicle, parking, map)
                    {
                        None | Some(ActionAtEnd::GotoLaneEnd) => {}
                        x => {
                            panic!("Car with one-step route {:?} had unexpected result from maybe_handle_end: {:?}", car.router, x);
                        }
                    }
                }

                car.state = car.crossing_state(params.start_dist, time, map);
            }
            scheduler.push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
            self.queues
                .get_mut(&Traversable::Lane(first_lane))
                .unwrap()
                .cars
                .insert(idx, car.vehicle.id);
            self.cars.insert(car.vehicle.id, car);
            return true;
        }
        false
    }

    pub fn update_car(
        &mut self,
        id: CarID,
        time: Duration,
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
                CarState::Queued => car.router.last_step(),
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
                time,
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
                time,
                &self.cars,
                &self.queues,
            );

            // We need to mutate two different cars in some cases. To avoid fighting the borrow
            // checker, temporarily move one of them out of the BTreeMap.
            let mut car = self.cars.remove(&id).unwrap();
            // Responsibility of update_car_with_distances to manage scheduling stuff!
            if self.update_car_with_distances(
                &mut car,
                dists,
                time,
                map,
                parking,
                trips,
                scheduler,
                transit,
                walking,
                intersections,
            ) {
                self.cars.insert(id, car);
            }
        }
    }

    // If this returns true, we need to immediately run update_car_with_distances. If we don't,
    // then the car will briefly be Queued and might immediately become something else, which
    // affects how leaders update followers.
    fn update_car_without_distances(
        &mut self,
        car: &mut Car,
        time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        intersections: &mut IntersectionSimState,
        transit: &mut TransitSimState,
        scheduler: &mut Scheduler,
    ) -> bool {
        match car.state {
            CarState::Crossing(_, _) => {
                car.state = CarState::Queued;
                if car.router.last_step() {
                    // Immediately run update_car_with_distances.
                    return true;
                }
                let queue = &self.queues[&car.router.head()];
                if queue.cars[0] == car.vehicle.id && queue.laggy_head.is_none() {
                    // Want to re-run, but no urgency about it happening immediately.
                    car.state = CarState::WaitingToAdvance;
                    scheduler.push(time, Command::UpdateCar(car.vehicle.id));
                }
            }
            CarState::Unparking(front, _) => {
                if car.router.last_step() {
                    // Actually, we need to do this first. Ignore the answer -- if we're
                    // doing something weird like vanishing or re-parking immediately
                    // (quite unlikely), the next loop will pick that up. Just trigger the
                    // side effect of choosing an end_dist.
                    car.router
                        .maybe_handle_end(front, &car.vehicle, parking, map);
                }
                car.state = car.crossing_state(front, time, map);
                scheduler.push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
            }
            CarState::Idling(dist, _) => {
                car.router = transit.bus_departed_from_stop(car.vehicle.id);
                car.state = car.crossing_state(dist, time, map);
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
                        CarState::Queued => {
                            // If they're on their last step, they might be ending early and not
                            // right behind us.
                            if !follower.router.last_step() {
                                follower.state = follower.crossing_state(
                                    // Since the follower was Queued, this must be where they are.
                                    dist - car.vehicle.length - FOLLOWING_DISTANCE,
                                    time,
                                    map,
                                );
                                scheduler.update(
                                    Command::UpdateCar(follower.vehicle.id),
                                    follower.state.get_end_time(),
                                );
                            }
                        }
                        CarState::WaitingToAdvance => unreachable!(),
                        // They weren't blocked. Note that there's no way the Crossing state could jump
                        // forwards here; the leader is still in front of them.
                        CarState::Crossing(_, _)
                        | CarState::Unparking(_, _)
                        | CarState::Parking(_, _, _)
                        | CarState::Idling(_, _) => {}
                    }
                }
            }
            CarState::Queued => unreachable!(),
            CarState::WaitingToAdvance => {
                // 'car' is the leader.
                let from = car.router.head();
                let goto = car.router.next();
                assert!(from != goto);

                if let Traversable::Turn(t) = goto {
                    if !intersections.maybe_start_turn(
                        AgentID::Car(car.vehicle.id),
                        t,
                        time,
                        map,
                        scheduler,
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

                let last_step = car.router.advance(&car.vehicle, parking, map);
                car.state = car.crossing_state(Distance::ZERO, time, map);
                scheduler.push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));

                car.last_steps.push_front(last_step);
                if goto.length(map) >= car.vehicle.length + FOLLOWING_DISTANCE {
                    // Optimistically assume we'll be out of the way ASAP.
                    scheduler.push(
                        car.crossing_state_with_end_dist(
                            DistanceInterval::new_driving(
                                Distance::ZERO,
                                car.vehicle.length + FOLLOWING_DISTANCE,
                            ),
                            time,
                            map,
                        )
                        .get_end_time(),
                        Command::UpdateLaggyHead(car.vehicle.id),
                    );
                }
                // Bit unrealistic, but don't unblock shorter intermediate steps until we're all
                // the way into a lane later.

                // Don't mark turn_finished until our back is out of the turn.

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
        dists: Vec<(CarID, Distance)>,
        time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        transit: &mut TransitSimState,
        walking: &mut WalkingSimState,
        intersections: &mut IntersectionSimState,
    ) -> bool {
        let idx = dists
            .iter()
            .position(|(id, _)| *id == car.vehicle.id)
            .unwrap();
        let our_dist = dists[idx].1;

        // Just two cases here.
        match car.state {
            CarState::Crossing(_, _)
            | CarState::Unparking(_, _)
            | CarState::Idling(_, _)
            | CarState::WaitingToAdvance => unreachable!(),
            CarState::Queued => {
                match car
                    .router
                    .maybe_handle_end(our_dist, &car.vehicle, parking, map)
                {
                    Some(ActionAtEnd::VanishAtBorder(i)) => {
                        trips.car_or_bike_reached_border(time, car.vehicle.id, i);
                    }
                    Some(ActionAtEnd::StartParking(spot)) => {
                        car.state = CarState::Parking(
                            our_dist,
                            spot,
                            TimeInterval::new(time, time + TIME_TO_PARK),
                        );
                        // If we don't do this, then we might have another car creep up
                        // behind, see the spot free, and start parking too. This can
                        // happen with multiple lanes and certain vehicle lengths.
                        parking.reserve_spot(spot);
                        scheduler
                            .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                        return true;
                    }
                    Some(ActionAtEnd::GotoLaneEnd) => {
                        car.state = car.crossing_state(our_dist, time, map);
                        scheduler
                            .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                        return true;
                    }
                    Some(ActionAtEnd::StopBiking(bike_rack)) => {
                        trips.bike_reached_end(time, car.vehicle.id, bike_rack, map, scheduler);
                    }
                    Some(ActionAtEnd::BusAtStop) => {
                        transit.bus_arrived_at_stop(
                            time,
                            car.vehicle.id,
                            trips,
                            walking,
                            scheduler,
                            map,
                        );
                        car.state = CarState::Idling(
                            our_dist,
                            TimeInterval::new(time, time + TIME_TO_WAIT_AT_STOP),
                        );
                        scheduler
                            .push(car.state.get_end_time(), Command::UpdateCar(car.vehicle.id));
                        return true;
                    }
                    None => {
                        scheduler.push(
                            time + BLIND_RETRY_TO_REACH_END_DIST,
                            Command::UpdateCar(car.vehicle.id),
                        );

                        // TODO For now, always use BLIND_RETRY_TO_REACH_END_DIST. Measured things
                        // to be slower otherwise. :(
                        /*
                        // If this car wasn't blocked at all, when would it reach its goal?
                        let ideal_end_time = match car.crossing_state(our_dist, time, map) {
                            CarState::Crossing(time_int, _) => time_int.end,
                            _ => unreachable!(),
                        };
                        if ideal_end_time == time {
                            // Haha, no such luck. We're super super close to the goal, but not
                            // quite there yet.
                            scheduler.push(time + BLIND_RETRY_TO_REACH_END_DIST, Command::UpdateCar(car.vehicle.id));
                        } else {
                            scheduler.push(ideal_end_time, Command::UpdateCar(car.vehicle.id));
                        }
                        // TODO For cars stuck on their last step, this will spam a fair bit. But
                        // that should be pretty rare.
                        */

                        return true;
                    }
                }
            }
            CarState::Parking(_, spot, _) => {
                parking.add_parked_car(ParkedCar {
                    vehicle: car.vehicle.clone(),
                    spot,
                });
                trips.car_reached_parking_spot(time, car.vehicle.id, spot, map, parking, scheduler);
            }
        }

        assert_eq!(
            self.queues
                .get_mut(&car.router.head())
                .unwrap()
                .cars
                .remove(idx)
                .unwrap(),
            car.vehicle.id
        );

        // We might be vanishing while partly clipping into other stuff.
        self.clear_last_steps(time, car, intersections, scheduler);

        // Update the follower so that they don't suddenly jump forwards.
        if idx != dists.len() - 1 {
            let (follower_id, follower_dist) = dists[idx + 1];
            let mut follower = self.cars.get_mut(&follower_id).unwrap();
            // TODO If the leader vanished at a border node, this still jumps a bit -- the
            // lead car's back is still sticking out. Need to still be bound by them, even
            // though they don't exist! If the leader just parked, then we're fine.
            match follower.state {
                CarState::Queued | CarState::Crossing(_, _) => {
                    // If the follower was still Crossing, they might not've been blocked
                    // by leader yet. In that case, recalculating their Crossing state is a
                    // no-op. But if they were blocked, then this will prevent them from
                    // jumping forwards.
                    follower.state = follower.crossing_state(follower_dist, time, map);
                    scheduler.update(
                        Command::UpdateCar(follower_id),
                        follower.state.get_end_time(),
                    );
                }
                // They weren't blocked
                CarState::Unparking(_, _) | CarState::Parking(_, _, _) | CarState::Idling(_, _) => {
                }
                CarState::WaitingToAdvance => unreachable!(),
            }
        }

        false
    }

    pub fn update_laggy_head(
        &mut self,
        id: CarID,
        time: Duration,
        map: &Map,
        intersections: &mut IntersectionSimState,
        scheduler: &mut Scheduler,
    ) {
        // TODO The impl here is pretty gross; play the same trick and remove car temporarily?
        let dists = self.queues[&self.cars[&id].router.head()].get_car_positions(
            time,
            &self.cars,
            &self.queues,
        );
        // This car must be the tail.
        assert_eq!(id, dists.last().unwrap().0);
        let our_len = self.cars[&id].vehicle.length + FOLLOWING_DISTANCE;

        // Have we made it far enough yet? Unfortunately, we have some math imprecision issues...
        {
            let our_dist = dists.last().unwrap().1;
            let car = &self.cars[&id];
            if our_dist < our_len {
                let retry_at = car
                    .crossing_state_with_end_dist(
                        DistanceInterval::new_driving(our_dist, our_len),
                        time,
                        map,
                    )
                    .get_end_time();
                // Sometimes due to rounding, retry_at will be exactly time, but we really need to
                // wait a bit longer.
                // TODO Smarter retry based on states and stuckness?
                if retry_at > time {
                    scheduler.push(retry_at, Command::UpdateLaggyHead(car.vehicle.id));
                } else {
                    // If we look up car positions before this retry happens, weird things can
                    // happen -- the laggy head could be well clear of the old queue by then. Make
                    // sure to handle that there. Consequences of this retry being long? A follower
                    // will wait a bit before advancing.
                    scheduler.push(
                        time + BLIND_RETRY_TO_CREEP_FORWARDS,
                        Command::UpdateLaggyHead(car.vehicle.id),
                    );
                }
                return;
            }
        }

        // Argh, fight the borrow checker.
        let mut car = self.cars.remove(&id).unwrap();
        self.clear_last_steps(time, &mut car, intersections, scheduler);
        self.cars.insert(id, car);
    }

    fn clear_last_steps(
        &mut self,
        time: Duration,
        car: &mut Car,
        intersections: &mut IntersectionSimState,
        scheduler: &mut Scheduler,
    ) {
        // If we were blocking a few short lanes, should be better now. Very last one might have
        // somebody to wake up.
        let last_steps: Vec<Traversable> = car.last_steps.drain(..).collect();

        for (idx, on) in last_steps.iter().enumerate() {
            let old_queue = self.queues.get_mut(&on).unwrap();
            assert_eq!(old_queue.laggy_head, Some(car.vehicle.id));
            old_queue.laggy_head = None;

            if let Traversable::Turn(t) = on {
                intersections.turn_finished(time, AgentID::Car(car.vehicle.id), *t, scheduler);
            }
            if idx == last_steps.len() - 1 {
                // Wake up the follower
                if let Some(follower_id) = old_queue.cars.front() {
                    let mut follower = self.cars.get_mut(&follower_id).unwrap();

                    match follower.state {
                        CarState::Queued => {
                            // If they're on their last step, they might be ending early and not right
                            // behind us.
                            if !follower.router.last_step() {
                                // The follower has been smoothly following while the laggy head gets out
                                // of the way. So immediately promote them to WaitingToAdvance.
                                follower.state = CarState::WaitingToAdvance;
                                scheduler.push(time, Command::UpdateCar(*follower_id));
                            }
                        }
                        CarState::WaitingToAdvance => unreachable!(),
                        // They weren't blocked. Note that there's no way the Crossing state could jump
                        // forwards here; the leader vanished from the end of the traversable.
                        CarState::Crossing(_, _)
                        | CarState::Unparking(_, _)
                        | CarState::Parking(_, _, _)
                        | CarState::Idling(_, _) => {}
                    }
                }
            } else {
                assert!(self.queues[&on].cars.is_empty());
            }
        }
    }

    pub fn get_unzoomed_polygons(&self, map: &Map) -> (Vec<Polygon>, Vec<Polygon>) {
        // These are the max over all lanes
        let mut max_moving: HashMap<DirectedRoadID, Distance> = HashMap::new();
        let mut max_waiting: HashMap<DirectedRoadID, Distance> = HashMap::new();

        let mut moving = Vec::new();
        let mut waiting = Vec::new();

        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            // Really coarse, strange behavior for turns. Overwrite blindly if there are concurrent turns
            // happening. :(
            if let Traversable::Turn(t) = queue.id {
                let polygon = map.get_i(t.parent).polygon.clone();
                match self.cars[&queue.cars[0]].state {
                    CarState::Crossing(_, _)
                    | CarState::Unparking(_, _)
                    | CarState::Parking(_, _, _)
                    | CarState::Idling(_, _) => {
                        moving.push(polygon);
                    }
                    CarState::Queued | CarState::WaitingToAdvance => {
                        waiting.push(polygon);
                    }
                }
                continue;
            }

            let mut moving_len = Distance::ZERO;
            let mut waiting_len = Distance::ZERO;
            let mut found_moving = false;
            for id in &queue.cars {
                let car = &self.cars[id];
                if found_moving {
                    if moving_len == Distance::ZERO {
                        moving_len += FOLLOWING_DISTANCE;
                    }
                    moving_len += car.vehicle.length;
                } else {
                    match car.state {
                        CarState::Crossing(_, _)
                        | CarState::Unparking(_, _)
                        | CarState::Parking(_, _, _)
                        | CarState::Idling(_, _) => {
                            found_moving = true;
                            if moving_len == Distance::ZERO {
                                moving_len += FOLLOWING_DISTANCE;
                            }
                            moving_len += car.vehicle.length;
                        }
                        CarState::Queued | CarState::WaitingToAdvance => {
                            if waiting_len == Distance::ZERO {
                                waiting_len += FOLLOWING_DISTANCE;
                            }
                            waiting_len += car.vehicle.length;
                        }
                    }
                }
            }
            let dr = map.get_l(queue.id.as_lane()).get_directed_parent(map);

            if moving_len > Distance::ZERO {
                let dist = max_moving.entry(dr).or_insert(Distance::ZERO);
                *dist = moving_len.max(*dist);
            }
            if waiting_len > Distance::ZERO {
                let dist = max_waiting.entry(dr).or_insert(Distance::ZERO);
                *dist = waiting_len.max(*dist);
            }
        }

        for (dr, len) in max_moving {
            let (pl, width) = map
                .get_r(dr.id)
                .get_center_for_side(dr.forwards)
                .unwrap()
                .unwrap();
            // Some cars might be only partially on this road, so the length sum might be too big.
            let clamped_len = len.min(pl.length());
            moving.push(
                pl.exact_slice(Distance::ZERO, clamped_len)
                    .make_polygons(width),
            );
        }

        for (dr, len) in max_waiting {
            let (pl, width) = map
                .get_r(dr.id)
                .get_center_for_side(dr.forwards)
                .unwrap()
                .unwrap();
            // Some cars might be only partially on this road, so the length sum might be too big.
            let clamped_len = len.min(pl.length());
            waiting.push(
                pl.exact_slice(pl.length() - clamped_len, pl.length())
                    .make_polygons(width),
            );
        }

        (moving, waiting)
    }

    pub fn get_all_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        let mut result = Vec::new();
        for queue in self.queues.values() {
            result.extend(
                queue
                    .get_car_positions(time, &self.cars, &self.queues)
                    .into_iter()
                    .map(|(id, dist)| self.cars[&id].get_draw_car(dist, time, map)),
            );
        }
        result
    }

    pub fn get_draw_cars_on(
        &self,
        time: Duration,
        on: Traversable,
        map: &Map,
    ) -> Vec<DrawCarInput> {
        match self.queues.get(&on) {
            Some(q) => q
                .get_car_positions(time, &self.cars, &self.queues)
                .into_iter()
                .map(|(id, dist)| self.cars[&id].get_draw_car(dist, time, map))
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn debug_car(&self, id: CarID) {
        if let Some(ref car) = self.cars.get(&id) {
            println!("{}", abstutil::to_json(car));
        } else {
            println!("{} is parked somewhere", id);
        }
    }

    pub fn tooltip_lines(&self, id: CarID) -> Option<Vec<String>> {
        let car = self.cars.get(&id)?;
        Some(vec![
            format!("{} on {}", id, car.router.head()),
            format!("Owned by {:?}", car.vehicle.owner),
            format!("{} lanes left", car.router.get_path().num_lanes()),
            format!("{:?}", car.state),
        ])
    }

    pub fn get_path(&self, id: CarID) -> Option<&Path> {
        let car = self.cars.get(&id)?;
        Some(car.router.get_path())
    }

    pub fn trace_route(
        &self,
        time: Duration,
        id: CarID,
        map: &Map,
        dist_ahead: Option<Distance>,
    ) -> Option<PolyLine> {
        let car = self.cars.get(&id)?;
        let front = self.queues[&car.router.head()]
            .get_car_positions(time, &self.cars, &self.queues)
            .into_iter()
            .find(|(c, _)| *c == id)
            .unwrap()
            .1;
        car.router.get_path().trace(map, front, dist_ahead)
    }

    pub fn get_owner_of_car(&self, id: CarID) -> Option<BuildingID> {
        let car = self.cars.get(&id)?;
        car.vehicle.owner
    }

    // This ignores capacity, pedestrians, and traffic signal overtime. So it should yield false
    // positives (thinks there's gridlock, when there isn't) but never false negatives.
    pub fn detect_gridlock(&self, map: &Map) -> bool {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        enum Node {
            Lane(LaneID),
            Intersection(IntersectionID),
        }

        // TODO petgraph wrapper to map nodes -> node index and handle duplicate nodes
        let mut deps: Graph<Node, ()> = Graph::new();
        let mut nodes: HashMap<Node, NodeIndex<u32>> = HashMap::new();

        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            match queue.id {
                Traversable::Lane(l) => {
                    let lane_id = Node::Lane(l);
                    // Assume lead car will proceed to the intersection
                    nodes.insert(lane_id, deps.add_node(lane_id));

                    let int_id = Node::Intersection(map.get_l(l).dst_i);
                    if !nodes.contains_key(&int_id) {
                        nodes.insert(int_id, deps.add_node(int_id));
                    }

                    deps.add_edge(nodes[&lane_id], nodes[&int_id], ());
                }
                Traversable::Turn(t) => {
                    let int_id = Node::Intersection(t.parent);
                    if !nodes.contains_key(&int_id) {
                        nodes.insert(int_id, deps.add_node(int_id));
                    }

                    let target_lane_id = Node::Lane(t.dst);
                    if !nodes.contains_key(&target_lane_id) {
                        nodes.insert(target_lane_id, deps.add_node(target_lane_id));
                    }

                    deps.add_edge(nodes[&int_id], nodes[&target_lane_id], ());
                }
            }
        }

        if let Err(cycle) = petgraph::algo::toposort(&deps, None) {
            // Super lame, we only get one node in the cycle, and A* won't attempt to look for
            // loops.
            for start in deps.neighbors(cycle.node_id()) {
                if let Some((_, raw_nodes)) =
                    petgraph::algo::astar(&deps, start, |n| n == cycle.node_id(), |_| 0, |_| 0)
                {
                    println!("Gridlock involving:");
                    for n in raw_nodes {
                        println!("- {:?}", deps[n]);
                    }
                    return true;
                }
            }
            println!(
                "Gridlock involving {:?}, but couldn't find the cycle!",
                cycle.node_id()
            );
            return true;
        }
        false
    }
}
