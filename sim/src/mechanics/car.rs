use crate::{
    CarStatus, DistanceInterval, DrawCarInput, ParkingSpot, PersonID, Router, TimeInterval,
    TransitSimState, TripID, Vehicle, VehicleType,
};
use geom::{Distance, Duration, PolyLine, Time};
use map_model::{Map, Traversable};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Car {
    pub vehicle: Vehicle,
    pub state: CarState,
    pub router: Router,
    // None for buses
    // TODO Can we scrap person here and use vehicle owner?
    pub trip_and_person: Option<(TripID, PersonID)>,
    pub started_at: Time,
    pub total_blocked_time: Duration,

    // In reverse order -- most recently left is first. The sum length of these must be >=
    // vehicle.length.
    pub last_steps: VecDeque<Traversable>,
}

impl Car {
    // Assumes the current head of the path is the thing to cross.
    pub fn crossing_state(&self, start_dist: Distance, start_time: Time, map: &Map) -> CarState {
        let dist_int = DistanceInterval::new_driving(
            start_dist,
            if self.router.last_step() {
                self.router.get_end_dist()
            } else {
                self.router.head().length(map)
            },
        );
        self.crossing_state_with_end_dist(dist_int, start_time, map)
    }

    pub fn crossing_state_with_end_dist(
        &self,
        dist_int: DistanceInterval,
        start_time: Time,
        map: &Map,
    ) -> CarState {
        let on = self.router.head();
        let mut speed = on.speed_limit(map);
        if let Some(s) = self.vehicle.max_speed {
            speed = speed.min(s);
        }
        let dt = (dist_int.end - dist_int.start) / speed;
        CarState::Crossing(TimeInterval::new(start_time, start_time + dt), dist_int)
    }

    pub fn get_draw_car(
        &self,
        front: Distance,
        now: Time,
        map: &Map,
        transit: &TransitSimState,
    ) -> DrawCarInput {
        assert!(front >= Distance::ZERO);
        // This goes from back to front
        let raw_body = if front >= self.vehicle.length {
            self.router
                .head()
                .exact_slice(front - self.vehicle.length, front, map)
        } else {
            // TODO This is redoing some of the Path::trace work...
            let mut result = self
                .router
                .head()
                .slice(Distance::ZERO, front, map)
                .map(|(pl, _)| pl.into_points())
                .unwrap_or_else(Vec::new);
            let mut leftover = self.vehicle.length - front;
            let mut i = 0;
            while leftover > Distance::ZERO {
                if i == self.last_steps.len() {
                    panic!(
                        "{} spawned too close to short stuff; still need to account for {}",
                        self.vehicle.id, leftover
                    );
                }
                let len = self.last_steps[i].length(map);
                let start = (len - leftover).max(Distance::ZERO);
                let piece = self.last_steps[i]
                    .slice(start, len, map)
                    .map(|(pl, _)| pl.into_points())
                    .unwrap_or_else(Vec::new);
                result = PolyLine::append(piece, result);
                leftover -= len;
                i += 1;
            }

            PolyLine::new(result)
        };

        let body = match self.state {
            CarState::Unparking(_, ref spot, ref time_int)
            | CarState::Parking(_, ref spot, ref time_int) => {
                let (percent_time, is_parking) = match self.state {
                    CarState::Unparking(_, _, _) => (1.0 - time_int.percent(now), false),
                    CarState::Parking(_, _, _) => (time_int.percent(now), true),
                    _ => unreachable!(),
                };
                match spot {
                    ParkingSpot::Onstreet(parking_l, _) => {
                        let width = map.get_l(*parking_l).width * percent_time;
                        let driving_l = self.router.head().as_lane();
                        let parent = map.get_parent(driving_l);
                        // Is the parking lane to the left or right of the driving lane?
                        let shift = if parent.dir_and_offset(driving_l).0
                            == parent.dir_and_offset(*parking_l).0
                        {
                            width
                        } else {
                            -width
                        };
                        raw_body.shift_right(shift).unwrap()
                    }
                    ParkingSpot::Offstreet(b, _) => {
                        // Append the car's polyline on the street with the driveway
                        let driveway = &map.get_b(*b).parking.as_ref().unwrap().driveway_line;
                        let full_piece = if is_parking {
                            raw_body.extend(driveway.reversed())
                        } else {
                            driveway.clone().extend(raw_body).reversed()
                        };
                        // Then make the car creep along the added length of the driveway (which
                        // could be really short)
                        let creep_along = driveway.length() * percent_time;
                        // TODO Ideally the car would slowly (dis)appear into the building, but
                        // some stuff downstream needs to understand that the windows and such will
                        // get cut off. :)
                        let sliced =
                            full_piece.exact_slice(creep_along, creep_along + self.vehicle.length);
                        if is_parking {
                            sliced
                        } else {
                            sliced.reversed()
                        }
                    }
                }
            }
            _ => raw_body,
        };

        DrawCarInput {
            id: self.vehicle.id,
            waiting_for_turn: match self.state {
                // TODO Maybe also when Crossing?
                CarState::WaitingToAdvance { .. } | CarState::Queued { .. } => {
                    match self.router.maybe_next() {
                        Some(Traversable::Turn(t)) => Some(t),
                        _ => None,
                    }
                }
                _ => None,
            },
            status: match self.state {
                CarState::Queued { .. } => CarStatus::Moving,
                CarState::WaitingToAdvance { .. } => CarStatus::Moving,
                CarState::Crossing(_, _) => CarStatus::Moving,
                // Eh they're technically moving, but this is a bit easier to spot
                CarState::Unparking(_, _, _) => CarStatus::Parked,
                CarState::Parking(_, _, _) => CarStatus::Parked,
                // Changing color for idling buses is helpful
                CarState::Idling(_, _) => CarStatus::Parked,
            },
            on: self.router.head(),
            label: if self.vehicle.vehicle_type == VehicleType::Bus {
                Some(
                    map.get_br(transit.bus_route(self.vehicle.id))
                        .name
                        .to_string(),
                )
            } else {
                None
            },
            body,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum CarState {
    Crossing(TimeInterval, DistanceInterval),
    Queued { blocked_since: Time },
    WaitingToAdvance { blocked_since: Time },
    // Where's the front of the car while this is happening?
    Unparking(Distance, ParkingSpot, TimeInterval),
    Parking(Distance, ParkingSpot, TimeInterval),
    Idling(Distance, TimeInterval),
}

impl CarState {
    pub fn get_end_time(&self) -> Time {
        match self {
            CarState::Crossing(ref time_int, _) => time_int.end,
            CarState::Queued { .. } => unreachable!(),
            CarState::WaitingToAdvance { .. } => unreachable!(),
            CarState::Unparking(_, _, ref time_int) => time_int.end,
            CarState::Parking(_, _, ref time_int) => time_int.end,
            CarState::Idling(_, ref time_int) => time_int.end,
        }
    }
}
