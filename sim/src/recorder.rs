use std::collections::BTreeSet;

use geom::Time;
use map_model::{IntersectionID, LaneID, Map, PathStep, Position, Traversable};
use synthpop::{IndividTrip, PersonSpec, Scenario, TripEndpoint, TripMode, TripPurpose};

use crate::{AgentID, CarID, DrivingSimState, Event, TripID, VehicleType};

/// Records trips beginning and ending at a specified set of intersections. This can be used to
/// capture and reproduce behavior in a gridlock-prone chunk of the map, without simulating
/// everything.
#[derive(Clone)]
pub(crate) struct TrafficRecorder {
    capture_points: BTreeSet<IntersectionID>,
    // TODO The RNG will determine vehicle length, so this won't be a perfect capture. Hopefully
    // good enough.
    trips: Vec<IndividTrip>,
    seen_trips: BTreeSet<TripID>,
}

impl TrafficRecorder {
    pub fn new(capture_points: BTreeSet<IntersectionID>) -> TrafficRecorder {
        TrafficRecorder {
            capture_points,
            trips: Vec::new(),
            seen_trips: BTreeSet::new(),
        }
    }

    pub fn handle_event(&mut self, time: Time, ev: &Event, map: &Map, driving: &DrivingSimState) {
        if let Event::AgentEntersTraversable(AgentID::Car(car), Some(trip), on, _) = ev {
            self.on_car_enters_traversable(time, *car, *trip, *on, map, driving);
        }
    }

    fn on_car_enters_traversable(
        &mut self,
        time: Time,
        car: CarID,
        trip: TripID,
        on: Traversable,
        map: &Map,
        driving: &DrivingSimState,
    ) {
        if self.seen_trips.contains(&trip) {
            return;
        }
        if let Traversable::Lane(lane) = on {
            self.on_car_enters_lane(time, car, trip, lane, map, driving);
        }
    }

    fn on_car_enters_lane(
        &mut self,
        time: Time,
        car: CarID,
        trip: TripID,
        lane: LaneID,
        map: &Map,
        driving: &DrivingSimState,
    ) {
        if !self.capture_points.contains(&map.get_l(lane).src_i) {
            return;
        }
        // Where do they exit?
        let exit_intersection =
            driving
                .get_path(car)
                .unwrap()
                .get_steps()
                .iter()
                .find_map(|step| {
                    if let PathStep::Turn(t) = step {
                        if self.capture_points.contains(&t.parent) {
                            return Some(t.parent);
                        }
                    }
                    None
                });
        if let Some(exit_intersection) = exit_intersection {
            self.trips.push(IndividTrip::new(
                time,
                TripPurpose::Shopping,
                TripEndpoint::SuddenlyAppear(Position::start(lane)),
                TripEndpoint::Border(exit_intersection),
                if car.vehicle_type == VehicleType::Bike {
                    TripMode::Bike
                } else {
                    TripMode::Drive
                },
            ));
            self.seen_trips.insert(trip);
        };
    }

    pub fn num_recorded_trips(&self) -> usize {
        self.trips.len()
    }

    pub fn save(mut self, map: &Map) {
        Scenario {
            scenario_name: "recorded".to_string(),
            map_name: map.get_name().clone(),
            people: self
                .trips
                .drain(..)
                .map(|trip| PersonSpec {
                    orig_id: None,
                    trips: vec![trip],
                })
                .collect::<Vec<_>>(),
            only_seed_buses: None,
        }
        .save();
    }
}
