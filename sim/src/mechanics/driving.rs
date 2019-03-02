use crate::mechanics::car::{Car, CarState};
use crate::mechanics::queue::Queue;
use crate::{
    ActionAtEnd, AgentID, CarID, CreateCar, DrawCarInput, IntersectionSimState, ParkedCar,
    ParkingSimState, Scheduler, TimeInterval, TransitSimState, TripManager, WalkingSimState,
    BUS_LENGTH, FOLLOWING_DISTANCE,
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
    queues: BTreeMap<Traversable, Queue>,
}

impl DrivingSimState {
    pub fn new(map: &Map) -> DrivingSimState {
        let mut sim = DrivingSimState {
            queues: BTreeMap::new(),
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

    pub fn draw_unzoomed(&self, _time: Duration, g: &mut GfxCtx, map: &Map) {
        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            // TODO blocked and not blocked? Eh
            let mut num_waiting = 0;
            let mut num_freeflow = 0;
            for car in &queue.cars {
                match car.state {
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
                    .get_car_positions(time)
                    .into_iter()
                    .map(|(car, dist)| car.get_draw_car(dist, time, map)),
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
                .get_car_positions(time)
                .into_iter()
                .map(|(car, dist)| car.get_draw_car(dist, time, map))
                .collect(),
            None => Vec::new(),
        }
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
            self.queues
                .get_mut(&Traversable::Lane(first_lane))
                .unwrap()
                .cars
                .insert(idx, car);
            //println!("{} spawned at {}", vehicle.id, time);
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
        // Promote Crossing to Queued and Unparking to Crossing.
        for queue in self.queues.values_mut() {
            for car in queue.cars.iter_mut() {
                if let CarState::Crossing(ref time_int, _) = car.state {
                    if time > time_int.end {
                        car.state = CarState::Queued;
                    }
                } else if let CarState::Unparking(front, ref time_int) = car.state {
                    if time > time_int.end {
                        if car.router.last_step() {
                            // Actually, we need to do this first. Ignore the answer -- if we're
                            // doing something weird like vanishing or re-parking immediately
                            // (quite unlikely), the next loop will pick that up. Just trigger the
                            // side effect of choosing an end_dist.
                            car.router
                                .maybe_handle_end(front, &car.vehicle, parking, map);
                        }
                        car.state = car.crossing_state(front, time, map);
                    }
                }
            }
        }

        // Handle cars on their last step. Some of them will vanish or finish parking; others will
        // start.
        for queue in self.queues.values_mut() {
            if queue.cars.iter().any(|car| car.router.last_step()) {
                // This car might have reached the router's end distance, but maybe not -- might
                // actually be stuck behind other cars. We have to calculate the distances right
                // now to be sure.
                // TODO This calculates distances a little unnecessarily -- might just be a car
                // parking.
                let mut delete_indices = Vec::new();
                // Intermediate collect() to end the borrow of &Car from get_car_positions.
                for (idx, dist) in queue
                    .get_car_positions(time)
                    .into_iter()
                    .map(|(_, dist)| dist)
                    .collect::<Vec<Distance>>()
                    .into_iter()
                    .enumerate()
                {
                    let car = &mut queue.cars[idx];
                    if !car.router.last_step() {
                        continue;
                    }
                    match car.state {
                        CarState::Queued => {
                            match car
                                .router
                                .maybe_handle_end(dist, &car.vehicle, parking, map)
                            {
                                Some(ActionAtEnd::VanishAtBorder(i)) => {
                                    trips.car_or_bike_reached_border(time, car.vehicle.id, i);
                                    delete_indices.push((idx, dist));
                                }
                                Some(ActionAtEnd::StartParking(spot)) => {
                                    car.state = CarState::Parking(
                                        dist,
                                        spot,
                                        TimeInterval::new(time, time + TIME_TO_PARK),
                                    );
                                    // If we don't do this, then we might have another car creep up
                                    // behind, see the spot free, and start parking too. This can
                                    // happen with multiple lanes and certain vehicle lengths.
                                    parking.reserve_spot(spot);
                                }
                                Some(ActionAtEnd::GotoLaneEnd) => {
                                    car.state = car.crossing_state(dist, time, map);
                                }
                                Some(ActionAtEnd::StopBiking(bike_rack)) => {
                                    delete_indices.push((idx, dist));
                                    trips.bike_reached_end(
                                        time,
                                        car.vehicle.id,
                                        bike_rack,
                                        map,
                                        scheduler,
                                    );
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
                                        dist,
                                        TimeInterval::new(time, time + TIME_TO_WAIT_AT_STOP),
                                    );
                                }
                                None => {}
                            }
                        }
                        CarState::Parking(_, spot, ref time_int) => {
                            if time > time_int.end {
                                delete_indices.push((idx, dist));
                                parking.add_parked_car(ParkedCar {
                                    vehicle: car.vehicle.clone(),
                                    spot,
                                });
                                trips.car_reached_parking_spot(
                                    time,
                                    car.vehicle.id,
                                    spot,
                                    map,
                                    parking,
                                    scheduler,
                                );
                            }
                        }
                        CarState::Idling(dist, ref time_int) => {
                            if time > time_int.end {
                                car.router = transit.bus_departed_from_stop(car.vehicle.id, map);
                                car.state = car.crossing_state(dist, time, map);
                            }
                        }
                        _ => {}
                    }
                }

                // Remove the finished cars starting from the end of the queue, so indices aren't
                // messed up.
                delete_indices.reverse();
                for (idx, leader_dist) in delete_indices {
                    let leader = queue.cars.remove(idx).unwrap();

                    // Update the follower so that they don't suddenly jump forwards.
                    if idx != queue.cars.len() {
                        let mut follower = &mut queue.cars[idx];
                        // TODO If the leader vanished at a border node, this still jumps a bit --
                        // the lead car's back is still sticking out. Need to still be bound by
                        // them, even though they don't exist! If the leader just parked, then
                        // we're fine.
                        match follower.state {
                            CarState::Queued => {
                                follower.state = follower.crossing_state(
                                    // Since the follower was Queued, this must be where they are
                                    leader_dist - leader.vehicle.length - FOLLOWING_DISTANCE,
                                    time,
                                    map,
                                );
                            }
                            // They weren't blocked
                            CarState::Crossing(_, _)
                            | CarState::Unparking(_, _)
                            | CarState::Parking(_, _, _)
                            | CarState::Idling(_, _) => {}
                        }
                    }
                }
            }
        }

        // Figure out where everybody wants to go next.
        let mut head_cars_ready_to_advance: Vec<Traversable> = Vec::new();
        for queue in self.queues.values() {
            if queue.cars.is_empty() {
                continue;
            }
            let car = &queue.cars[0];
            if car.is_queued() && !car.router.last_step() {
                head_cars_ready_to_advance.push(queue.id);
            }
        }

        // Carry out the transitions.
        for from in head_cars_ready_to_advance {
            let car_id = self.queues[&from].cars[0].vehicle.id;
            let goto = self.queues[&from].cars[0].router.next();

            // Always need to do this check.
            if !self.queues[&goto].room_at_end(time) {
                continue;
            }

            if let Traversable::Turn(t) = goto {
                if !intersections.maybe_start_turn(AgentID::Car(car_id), t, time, map) {
                    continue;
                }
            }

            let mut car = self
                .queues
                .get_mut(&from)
                .unwrap()
                .cars
                .pop_front()
                .unwrap();

            // Update the follower so that they don't suddenly jump forwards.
            if let Some(ref mut follower) = self.queues.get_mut(&from).unwrap().cars.front_mut() {
                // TODO This still jumps a bit -- the lead car's back is still sticking out. Need
                // to still be bound by them.
                match follower.state {
                    CarState::Queued => {
                        follower.state = follower.crossing_state(
                            // Since the follower was Queued, this must be where they are
                            from.length(map) - car.vehicle.length - FOLLOWING_DISTANCE,
                            time,
                            map,
                        );
                    }
                    // They weren't blocked
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

            if goto.maybe_lane().is_some() {
                // TODO Actually, don't call turn_finished until the car is at least vehicle.length
                // + FOLLOWING_DISTANCE into the next lane. This'll be hard to predict when we're
                // event-based, so hold off on this bit of realism.
                intersections.turn_finished(AgentID::Car(car.vehicle.id), last_step.as_turn());
            }

            self.queues.get_mut(&goto).unwrap().cars.push_back(car);
        }
    }

    fn get_car(&self, id: CarID) -> Option<&Car> {
        // TODO Faster
        for q in self.queues.values() {
            for car in &q.cars {
                if car.vehicle.id == id {
                    return Some(&car);
                }
            }
        }
        None
    }

    pub fn debug_car(&self, id: CarID) {
        if let Some(car) = self.get_car(id) {
            println!("{}", abstutil::to_json(car));
        } else {
            println!("{} is parked somewhere", id);
        }
    }

    pub fn tooltip_lines(&self, id: CarID) -> Option<Vec<String>> {
        let car = self.get_car(id)?;
        Some(vec![format!(
            "Car {:?}, owned by {:?}, {} lanes left",
            id,
            car.vehicle.owner,
            car.router.get_path().num_lanes()
        )])
    }

    pub fn get_path(&self, id: CarID) -> Option<&Path> {
        let car = self.get_car(id)?;
        Some(car.router.get_path())
    }

    pub fn trace_route(
        &self,
        time: Duration,
        id: CarID,
        map: &Map,
        dist_ahead: Option<Distance>,
    ) -> Option<Trace> {
        let car = self.get_car(id)?;
        let front = self.queues[&car.router.head()]
            .get_car_positions(time)
            .into_iter()
            .find(|(c, _)| c.vehicle.id == id)
            .unwrap()
            .1;
        car.router.get_path().trace(map, front, dist_ahead)
    }

    pub fn get_owner_of_car(&self, id: CarID) -> Option<BuildingID> {
        let car = self.get_car(id)?;
        car.vehicle.owner
    }
}
