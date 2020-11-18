use std::collections::BTreeSet;

use geom::Time;
use map_model::{IntersectionID, Map, PathStep, Position, Traversable};

use crate::{
    AgentID, DrivingSimState, Event, IndividTrip, PersonID, PersonSpec, Scenario, TripEndpoint,
    TripID, TripManager, TripMode, TripPurpose, VehicleType,
};

/// Records trips beginning and ending at a specified set of intersections. This can be used to
/// capture and reproduce behavior in a gridlock-prone chunk of the map, without simulating
/// everything.
#[derive(Clone)]
pub struct TrafficRecorder {
    capture_points: BTreeSet<IntersectionID>,
    // TODO The RNG will determine vehicle length, so this won't be a perfect capture. Hopefully
    // good enough.
    trips: Vec<(TripEndpoint, IndividTrip)>,
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

    pub fn handle_event(
        &mut self,
        time: Time,
        ev: &Event,
        map: &Map,
        driving: &DrivingSimState,
        trips: &TripManager,
    ) {
        if let Event::AgentEntersTraversable(a, on, _) = ev {
            if let AgentID::Car(car) = a {
                if let Some(trip) = trips.agent_to_trip(AgentID::Car(*car)) {
                    if self.seen_trips.contains(&trip) {
                        return;
                    }
                    if let Traversable::Lane(l) = on {
                        if self.capture_points.contains(&map.get_l(*l).src_i) {
                            // Where do they exit?
                            for step in driving.get_path(*car).unwrap().get_steps() {
                                if let PathStep::Turn(t) = step {
                                    if self.capture_points.contains(&t.parent) {
                                        self.trips.push((
                                            TripEndpoint::SuddenlyAppear(Position::start(*l)),
                                            IndividTrip::new(
                                                time,
                                                TripPurpose::Shopping,
                                                TripEndpoint::Border(t.parent),
                                                if car.1 == VehicleType::Bike {
                                                    TripMode::Bike
                                                } else {
                                                    TripMode::Drive
                                                },
                                            ),
                                        ));
                                        self.seen_trips.insert(trip);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn num_recorded_trips(&self) -> usize {
        self.trips.len()
    }

    pub fn save(mut self, map: &Map) {
        let mut people = Vec::new();
        for (origin, trip) in self.trips.drain(..) {
            people.push(PersonSpec {
                id: PersonID(people.len()),
                orig_id: None,
                origin,
                trips: vec![trip],
            });
        }
        Scenario {
            scenario_name: "recorded".to_string(),
            map_name: map.get_name().clone(),
            people,
            only_seed_buses: None,
        }
        .save();
    }
}
