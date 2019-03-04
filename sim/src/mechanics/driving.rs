use crate::mechanics::car::{Car, CarState};
use crate::mechanics::queue::Queue;
use crate::{
    ActionAtEnd, AgentID, CarID, CreateCar, DrawCarInput, IntersectionSimState, ParkedCar,
    ParkingSimState, PriorityQueue, Scheduler, TimeInterval, TransitSimState, TripManager,
    WalkingSimState, BUS_LENGTH, FOLLOWING_DISTANCE,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use ezgui::{Color, GfxCtx};
use geom::{Distance, Duration};
use map_model::{BuildingID, Map, Path, Trace, Traversable, LANE_THICKNESS};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

const FREEFLOW: Color = Color::CYAN;
const WAITING: Color = Color::RED;

const TIME_TO_UNPARK: Duration = Duration::const_seconds(10.0);
const TIME_TO_PARK: Duration = Duration::const_seconds(15.0);
const TIME_TO_WAIT_AT_STOP: Duration = Duration::const_seconds(10.0);

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

    events: PriorityQueue<CarID>,
}

impl DrivingSimState {
    pub fn new(map: &Map) -> DrivingSimState {
        let mut sim = DrivingSimState {
            cars: BTreeMap::new(),
            queues: BTreeMap::new(),
            events: PriorityQueue::new(),
        };

        for l in map.all_lanes() {
            if l.is_for_moving_vehicles() {
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
        ) {
            let mut car = Car {
                vehicle: params.vehicle,
                router: params.router,
                state: CarState::Queued,
                last_steps: VecDeque::new(),
            };
            if params.maybe_parked_car.is_some() {
                car.state = CarState::Unparking(
                    params.start_dist,
                    TimeInterval::new(time, time + TIME_TO_UNPARK),
                );
            } else {
                car.state = car.crossing_state(params.start_dist, time, map);
            }
            self.events.push(car.state.get_end_time(), car.vehicle.id);
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

    pub fn step_if_needed(
        &mut self,
        time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        intersections: &mut IntersectionSimState,
        trips: &mut TripManager,
        scheduler: &mut Scheduler,
        transit: &mut TransitSimState,
        walking: &mut WalkingSimState,
    ) {
        // State transitions:
        //
        // Crossing -> Queued
        // Unparking -> Crossing
        // Idling -> Crossing
        // Queued -> try to advance to the next step of the path
        // Queued -> last step handling (Parking or done)
        // Parking -> done
        //
        // Why is it safe to process cars in any order, rather than making sure to follow the order
        // of queues? Because of the invariant that distances should never suddenly jump when a car
        // has entered/exiting a queue.

        while let Some(id) = self.events.get_next(time) {
            // This car might have reached the router's end distance, but maybe not -- might
            // actually be stuck behind other cars. We have to calculate the distances right now to
            // be sure.
            let need_distances = {
                let car = &self.cars[&id];
                match car.state {
                    CarState::Queued => car.router.last_step(),
                    CarState::Parking(_, _, _) => true,
                    _ => false,
                }
            };
            if need_distances {
                // Do this before removing the car!
                let dists =
                    self.queues[&self.cars[&id].router.head()].get_car_positions(time, &self.cars);

                // We need to mutate two different cars in some cases. To avoid fighting the borrow
                // checker, temporarily move one of them out of the BTreeMap.
                let mut car = self.cars.remove(&id).unwrap();
                // Responsibility of update_car_with_distances to manage events!
                if self.update_car_with_distances(
                    &mut car, dists, time, map, parking, trips, scheduler, transit, walking,
                ) {
                    self.cars.insert(id, car);
                }
            } else {
                // We need to mutate two different cars in one case. To avoid fighting the borrow
                // checker, temporarily move one of them out of the BTreeMap.
                let mut car = self.cars.remove(&id).unwrap();
                // Responsibility of update_car to manage events!
                self.update_car(&mut car, time, map, parking, intersections, transit);
                self.cars.insert(id, car);
            }
        }
    }

    fn update_car(
        &mut self,
        car: &mut Car,
        time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        intersections: &mut IntersectionSimState,
        transit: &mut TransitSimState,
    ) {
        match car.state {
            CarState::Crossing(_, _) => {
                car.state = CarState::Queued;
                if car.router.last_step()
                    || self.queues[&car.router.head()].cars[0] == car.vehicle.id
                {
                    self.events.push(time + Duration::EPSILON, car.vehicle.id);
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
                self.events.push(car.state.get_end_time(), car.vehicle.id);
            }
            CarState::Idling(dist, _) => {
                car.router = transit.bus_departed_from_stop(car.vehicle.id, map);
                car.state = car.crossing_state(dist, time, map);
                self.events.push(car.state.get_end_time(), car.vehicle.id);
            }
            CarState::Queued => {
                // 'car' is the leader.
                let from = car.router.head();
                let goto = car.router.next();
                assert!(from != goto);

                // Always need to do this check.
                // Note that 'car' is currently missing from self.cars, but they can't be on
                // 'goto' right now -- they're on 'from'.
                if !self.queues[&goto].room_at_end(time, &self.cars) {
                    self.events.push(time + Duration::EPSILON, car.vehicle.id);
                    return;
                }

                if let Traversable::Turn(t) = goto {
                    if !intersections.maybe_start_turn(AgentID::Car(car.vehicle.id), t, time, map) {
                        self.events.push(time + Duration::EPSILON, car.vehicle.id);
                        return;
                    }
                }

                assert_eq!(
                    self.queues
                        .get_mut(&from)
                        .unwrap()
                        .cars
                        .pop_front()
                        .unwrap(),
                    car.vehicle.id
                );

                // Update the follower so that they don't suddenly jump forwards.
                if let Some(follower_id) = self.queues[&from].cars.front() {
                    let mut follower = self.cars.get_mut(&follower_id).unwrap();
                    // TODO This still jumps a bit -- the lead car's back is still sticking out. Need
                    // to still be bound by them.
                    match follower.state {
                        CarState::Queued => {
                            follower.state = follower.crossing_state(
                                // Since the follower was Queued, this must be where they are. This
                                // update case is when the lead car was NOT on their last step, so
                                // they were indeed all the way at the end of 'from'.
                                from.length(map) - car.vehicle.length - FOLLOWING_DISTANCE,
                                time,
                                map,
                            );
                            self.events
                                .update(*follower_id, follower.state.get_end_time());
                        }
                        // They weren't blocked. Note that there's no way the Crossing state could jump
                        // forwards here; the leader vanished from the end of the traversable.
                        CarState::Crossing(_, _)
                        | CarState::Unparking(_, _)
                        | CarState::Parking(_, _, _)
                        | CarState::Idling(_, _) => {}
                    }
                }

                let last_step = car.router.advance(&car.vehicle, parking, map);
                car.last_steps.push_front(last_step);
                car.trim_last_steps(map);
                car.state = car.crossing_state(Distance::ZERO, time, map);
                self.events.push(car.state.get_end_time(), car.vehicle.id);

                if goto.maybe_lane().is_some() {
                    // TODO Actually, don't call turn_finished until the car is at least
                    // vehicle.length + FOLLOWING_DISTANCE into the next lane. This'll be hard
                    // to predict when we're event-based, so hold off on this bit of realism.
                    intersections.turn_finished(AgentID::Car(car.vehicle.id), last_step.as_turn());
                }

                self.queues
                    .get_mut(&goto)
                    .unwrap()
                    .cars
                    .push_back(car.vehicle.id);
            }
            CarState::Parking(_, _, _) => unreachable!(),
        }
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
    ) -> bool {
        let idx = dists
            .iter()
            .position(|(id, _)| *id == car.vehicle.id)
            .unwrap();
        let our_dist = dists[idx].1;

        // Just two cases here.
        match car.state {
            CarState::Crossing(_, _) | CarState::Unparking(_, _) | CarState::Idling(_, _) => {
                unreachable!()
            }
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
                        self.events.push(car.state.get_end_time(), car.vehicle.id);
                        return true;
                    }
                    Some(ActionAtEnd::GotoLaneEnd) => {
                        car.state = car.crossing_state(our_dist, time, map);
                        self.events.push(car.state.get_end_time(), car.vehicle.id);
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
                        self.events.push(car.state.get_end_time(), car.vehicle.id);
                        return true;
                    }
                    None => {
                        self.events.push(time + Duration::EPSILON, car.vehicle.id);
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
                    self.events
                        .update(follower_id, follower.state.get_end_time());
                }
                // They weren't blocked
                CarState::Unparking(_, _) | CarState::Parking(_, _, _) | CarState::Idling(_, _) => {
                }
            }
        }

        false
    }

    pub fn draw_unzoomed(&self, _time: Duration, g: &mut GfxCtx, map: &Map) {
        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            // TODO blocked and not blocked? Eh
            let mut num_waiting = 0;
            let mut num_freeflow = 0;
            for id in &queue.cars {
                match self.cars[id].state {
                    CarState::Crossing(_, _)
                    | CarState::Unparking(_, _)
                    | CarState::Parking(_, _, _)
                    | CarState::Idling(_, _) => {
                        num_freeflow += 1;
                    }
                    CarState::Queued => {
                        num_waiting += 1;
                    }
                };
            }

            if num_waiting > 0 {
                // Short lanes/turns exist
                let start = (queue.geom_len
                    - f64::from(num_waiting) * (BUS_LENGTH + FOLLOWING_DISTANCE))
                    .max(Distance::ZERO);
                g.draw_polygon(
                    WAITING,
                    &queue
                        .id
                        .slice(start, queue.geom_len, map)
                        .unwrap()
                        .0
                        .make_polygons(LANE_THICKNESS),
                );
            }
            if num_freeflow > 0 {
                g.draw_polygon(
                    FREEFLOW,
                    &queue
                        .id
                        .slice(
                            Distance::ZERO,
                            f64::from(num_freeflow) * (BUS_LENGTH + FOLLOWING_DISTANCE),
                            map,
                        )
                        .unwrap()
                        .0
                        .make_polygons(LANE_THICKNESS),
                );
            }
        }
    }

    pub fn get_all_draw_cars(&self, time: Duration, map: &Map) -> Vec<DrawCarInput> {
        let mut result = Vec::new();
        for queue in self.queues.values() {
            result.extend(
                queue
                    .get_car_positions(time, &self.cars)
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
                .get_car_positions(time, &self.cars)
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
        Some(vec![format!(
            "Car {:?}, owned by {:?}, {} lanes left",
            id,
            car.vehicle.owner,
            car.router.get_path().num_lanes()
        )])
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
    ) -> Option<Trace> {
        let car = self.cars.get(&id)?;
        let front = self.queues[&car.router.head()]
            .get_car_positions(time, &self.cars)
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
}
