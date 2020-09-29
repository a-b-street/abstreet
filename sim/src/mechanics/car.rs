use crate::{
    CarStatus, DistanceInterval, DrawCarInput, ParkingSpot, PersonID, Router, TimeInterval,
    TransitSimState, TripID, Vehicle, VehicleType,
};
use geom::{Distance, Duration, PolyLine, Time};
use map_model::{Direction, Map, Traversable};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Serialize, Deserialize, Clone)]
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
        let mut partly_on = Vec::new();
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
                .ok()
                .unwrap_or_else(Vec::new);
            let mut leftover = self.vehicle.length - front;
            let mut i = 0;
            while leftover > Distance::ZERO {
                if i == self.last_steps.len() {
                    // The vehicle is gradually appearing from somewhere. That's fine, just return
                    // a truncated body.
                    break;
                }
                partly_on.push(self.last_steps[i]);
                let len = self.last_steps[i].length(map);
                let start = (len - leftover).max(Distance::ZERO);
                let piece = self.last_steps[i]
                    .slice(start, len, map)
                    .map(|(pl, _)| pl.into_points())
                    .ok()
                    .unwrap_or_else(Vec::new);
                result = match PolyLine::append(piece, result) {
                    Ok(pl) => pl,
                    Err(err) => panic!(
                        "{} at {} has weird geom along {:?}: {}",
                        self.vehicle.id, now, self.last_steps, err
                    ),
                };
                leftover -= len;
                i += 1;
            }

            if result.len() < 2 {
                panic!(
                    "{} at {} has front at {} of {:?}. Didn't even wind up with two points",
                    self.vehicle.id,
                    now,
                    front,
                    self.router.head()
                );
            }
            match PolyLine::new(result) {
                Ok(pl) => pl,
                Err(err) => panic!("Weird body for {} at {}: {}", self.vehicle.id, now, err),
            }
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
                        let r = map.get_parent(*parking_l);
                        let driving_offset = r.offset(self.router.head().as_lane());
                        let parking_offset = r.offset(*parking_l);
                        let mut diff = (parking_offset as isize) - (driving_offset as isize);
                        if r.dir(self.router.head().as_lane()) == Direction::Back {
                            diff *= -1;
                        }
                        // TODO Sum widths in between, don't assume they're all the same as the
                        // parking lane width!
                        let width = map.get_l(*parking_l).width * (diff as f64) * percent_time;
                        match raw_body.shift_right(width) {
                            Ok(pl) => pl,
                            Err(err) => {
                                println!(
                                    "Body for onstreet {} at {} broken: {}",
                                    self.vehicle.id, now, err
                                );
                                raw_body.clone()
                            }
                        }
                    }
                    _ => {
                        let driveway = match spot {
                            ParkingSpot::Offstreet(b, _) => {
                                map.get_b(*b).driving_connection(map).unwrap().1
                            }
                            ParkingSpot::Lot(pl, _) => map.get_pl(*pl).driveway_line.clone(),
                            _ => unreachable!(),
                        };

                        // Append the car's polyline on the street with the driveway
                        let maybe_full_piece = if is_parking {
                            raw_body.clone().extend(driveway.reversed())
                        } else {
                            driveway
                                .clone()
                                .extend(raw_body.clone())
                                .map(|pl| pl.reversed())
                        };
                        let full_piece = match maybe_full_piece {
                            Ok(pl) => pl,
                            Err(err) => {
                                println!(
                                    "Body and driveway for {} at {} broken: {}",
                                    self.vehicle.id, now, err
                                );
                                raw_body.clone()
                            }
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
                CarState::IdlingAtStop(_, _) => CarStatus::Parked,
            },
            on: self.router.head(),
            partly_on,
            label: if self.vehicle.vehicle_type == VehicleType::Bus
                || self.vehicle.vehicle_type == VehicleType::Train
            {
                Some(
                    map.get_br(transit.bus_route(self.vehicle.id))
                        .short_name
                        .clone(),
                )
            } else {
                None
            },
            body,
        }
    }

    pub fn is_parking(&self) -> bool {
        if let CarState::Parking(_, _, _) = self.state {
            return true;
        }
        self.router.is_parking()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CarState {
    Crossing(TimeInterval, DistanceInterval),
    Queued { blocked_since: Time },
    WaitingToAdvance { blocked_since: Time },
    // Where's the front of the car while this is happening?
    Unparking(Distance, ParkingSpot, TimeInterval),
    Parking(Distance, ParkingSpot, TimeInterval),
    IdlingAtStop(Distance, TimeInterval),
}

impl CarState {
    pub fn get_end_time(&self) -> Time {
        match self {
            CarState::Crossing(ref time_int, _) => time_int.end,
            CarState::Queued { .. } => unreachable!(),
            CarState::WaitingToAdvance { .. } => unreachable!(),
            CarState::Unparking(_, _, ref time_int) => time_int.end,
            CarState::Parking(_, _, ref time_int) => time_int.end,
            CarState::IdlingAtStop(_, ref time_int) => time_int.end,
        }
    }

    pub fn time_spent_waiting(&self, now: Time) -> Duration {
        match self {
            CarState::Queued { blocked_since } | CarState::WaitingToAdvance { blocked_since } => {
                now - *blocked_since
            }
            _ => Duration::ZERO,
        }
    }
}
