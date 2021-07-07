use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use geom::{Distance, Duration, PolyLine, Time, EPSILON_DIST};
use map_model::{Direction, LaneID, Map, Traversable};

use crate::{
    CarID, CarStatus, DistanceInterval, DrawCarInput, Intent, ParkingSpot, PersonID, Router,
    TimeInterval, TransitSimState, TripID, Vehicle, VehicleType,
};

/// Represents a single vehicle. Note "car" is a misnomer; it could also be a bus or bike.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct Car {
    pub vehicle: Vehicle,
    pub state: CarState,
    pub router: Router,
    /// None for buses
    // TODO Can we scrap person here and use vehicle owner?
    pub trip_and_person: Option<(TripID, PersonID)>,
    pub started_at: Time,
    pub total_blocked_time: Duration,

    /// In reverse order -- most recently left is first. The sum length of these must be >=
    /// vehicle.length.
    pub last_steps: VecDeque<Traversable>,

    /// Since lane over-taking isn't implemented yet, a vehicle tends to be stuck behind a slow
    /// leader for a while. Avoid duplicate events.
    pub wants_to_overtake: BTreeSet<CarID>,
}

impl Car {
    /// Assumes the current head of the path is the thing to cross.
    pub fn crossing_state(&self, start_dist: Distance, start_time: Time, map: &Map) -> CarState {
        let dist_int = DistanceInterval::new_driving(
            start_dist,
            if self.router.last_step() {
                self.router.get_end_dist()
            } else {
                self.router.head().get_polyline(map).length()
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
        let (speed, percent_incline) = self
            .router
            .get_path()
            .current_step()
            .max_speed_and_incline_along(
                self.vehicle.max_speed,
                self.vehicle.vehicle_type.to_constraints(),
                map,
            );
        let dt = (dist_int.end - dist_int.start) / speed;
        CarState::Crossing {
            time_int: TimeInterval::new(start_time, start_time + dt),
            dist_int,
            steep_uphill: percent_incline >= 0.08,
        }
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
                .get_polyline(map)
                .exact_slice(front - self.vehicle.length, front)
        } else {
            // TODO This is redoing some of the Path::trace work...
            let mut result = self
                .router
                .head()
                .get_polyline(map)
                .slice(Distance::ZERO, front)
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
                let len = self.last_steps[i].get_polyline(map).length();
                let start = (len - leftover).max(Distance::ZERO);
                let piece = self.last_steps[i]
                    .get_polyline(map)
                    .slice(start, len)
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
                // Vehicles spawning at a border start with their front at literally 0 distance.
                // Usually by the time we first try to render, they've advanced at least a little.
                // But sometimes there's a race when we try to immediately draw them.
                if let Ok((pl, _)) = self
                    .router
                    .head()
                    .get_polyline(map)
                    .slice(Distance::ZERO, 2.0 * EPSILON_DIST)
                {
                    result = pl.into_points();
                }
            }
            match PolyLine::new(result) {
                Ok(pl) => pl,
                Err(err) => panic!("Weird body for {} at {}: {}", self.vehicle.id, now, err),
            }
        };

        let body = match self.state {
            CarState::ChangingLanes {
                from,
                to,
                ref lc_time,
                ..
            } => {
                let percent_time = 1.0 - lc_time.percent(now);
                // TODO Can probably simplify this! Lifted from the parking case
                // The car's body is already at 'to', so shift back
                let mut diff = (to.offset as isize) - (from.offset as isize);
                let from = map.get_l(from);
                if from.dir == Direction::Fwd {
                    diff *= -1;
                }
                // TODO Careful with this width math
                let width = from.width * (diff as f64) * percent_time;
                match raw_body.shift_right(width) {
                    Ok(pl) => pl,
                    Err(err) => {
                        println!(
                            "Body for lane-changing {} at {} broken: {}",
                            self.vehicle.id, now, err
                        );
                        raw_body
                    }
                }
            }
            CarState::Unparking {
                ref spot,
                ref time_int,
                ..
            }
            | CarState::Parking(_, ref spot, ref time_int) => {
                let (percent_time, is_parking) = match self.state {
                    CarState::Unparking { .. } => (1.0 - time_int.percent(now), false),
                    CarState::Parking(_, _, _) => (time_int.percent(now), true),
                    _ => unreachable!(),
                };
                match spot {
                    ParkingSpot::Onstreet(parking_l, _) => {
                        let driving_offset = self.router.head().as_lane().offset;
                        let parking_offset = parking_l.offset;
                        let mut diff = (parking_offset as isize) - (driving_offset as isize);
                        if map.get_l(self.router.head().as_lane()).dir == Direction::Back {
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
                                raw_body
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
                            // It's possible to exit a driveway onto something other than the lane
                            // closest to the building. So use force_extend to handle possibly
                            // mismatching points.
                            driveway
                                .clone()
                                .force_extend(raw_body.clone())
                                .map(|pl| pl.reversed())
                        };
                        let sliced = match maybe_full_piece {
                            Ok(full_piece) => {
                                // Then make the car creep along the added length of the driveway (which
                                // could be really short)
                                let creep_along = driveway.length() * percent_time;
                                // TODO Ideally the car would slowly (dis)appear into the building, but
                                // some stuff downstream needs to understand that the windows and such will
                                // get cut off. :)
                                full_piece
                                    .exact_slice(creep_along, creep_along + self.vehicle.length)
                            }
                            Err(err) => {
                                // Just avoid crashing; we'll display something nonsensical (just
                                // part of the car body on the lane) in the meantime
                                error!(
                                    "Body and driveway for {} at {} broken: {}",
                                    self.vehicle.id, now, err
                                );
                                raw_body
                            }
                        };
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
                CarState::Crossing { .. } => CarStatus::Moving,
                CarState::ChangingLanes { .. } => CarStatus::Moving,
                CarState::Unparking { .. } => CarStatus::Moving,
                CarState::Parking(_, _, _) => CarStatus::Moving,
                // Changing color for idling buses is helpful
                CarState::IdlingAtStop(_, _) => CarStatus::Parked,
            },
            intent: if self.is_parking() || matches!(self.state, CarState::Unparking { .. }) {
                Some(Intent::Parking)
            } else {
                match self.state {
                    CarState::Crossing { steep_uphill, .. } if steep_uphill => {
                        Some(Intent::SteepUphill)
                    }
                    _ => None,
                }
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
            person: self.trip_and_person.map(|(_, p)| p),
        }
    }

    pub fn is_parking(&self) -> bool {
        if let CarState::Parking(_, _, _) = self.state {
            return true;
        }
        self.router.is_parking()
    }
}

/// See <https://a-b-street.github.io/docs/tech/trafficsim/discrete_event.html> for details about the
/// state machine encoded here.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) enum CarState {
    Crossing {
        time_int: TimeInterval,
        dist_int: DistanceInterval,
        steep_uphill: bool,
    },
    ChangingLanes {
        from: LaneID,
        to: LaneID,
        // For the most part, act just like a Crossing state with these intervals
        new_time: TimeInterval,
        new_dist: DistanceInterval,
        // How long does the lane-changing itself last? This must end before new_time_int does.
        lc_time: TimeInterval,
        must_return: bool,
    },
    Queued {
        blocked_since: Time,
        // (target lane, do we have to return to the original lane to preserve the path?)
        want_to_change_lanes: Option<(LaneID, bool)>,
    },
    WaitingToAdvance {
        blocked_since: Time,
    },
    /// Where's the front of the car while this is happening?
    Unparking {
        front: Distance,
        spot: ParkingSpot,
        time_int: TimeInterval,
        blocked_starts: Vec<LaneID>,
    },
    Parking(Distance, ParkingSpot, TimeInterval),
    IdlingAtStop(Distance, TimeInterval),
}

impl CarState {
    pub fn get_end_time(&self) -> Time {
        match self {
            CarState::Crossing { ref time_int, .. } => time_int.end,
            CarState::Queued { .. } => unreachable!(),
            CarState::WaitingToAdvance { .. } => unreachable!(),
            // Note this state lasts for lc_time, NOT for new_time.
            CarState::ChangingLanes { ref lc_time, .. } => lc_time.end,
            CarState::Unparking { ref time_int, .. } => time_int.end,
            CarState::Parking(_, _, ref time_int) => time_int.end,
            CarState::IdlingAtStop(_, ref time_int) => time_int.end,
        }
    }

    pub fn time_spent_waiting(&self, now: Time) -> Duration {
        match self {
            CarState::Queued { blocked_since, .. }
            | CarState::WaitingToAdvance { blocked_since } => now - *blocked_since,
            _ => Duration::ZERO,
        }
    }
}
