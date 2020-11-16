use std::collections::{BTreeMap, BTreeSet};

use geom::Time;
use map_model::{IntersectionID, LaneID, Map, Position, Traversable};

use crate::{
    AgentID, CarID, DrivingGoal, Event, IndividTrip, PersonID, PersonSpec, Scenario, SpawnTrip,
    TripPurpose, VehicleType,
};

/// Records trips beginning and ending at a specified set of intersections. This can be used to
/// capture and reproduce behavior in a gridlock-prone chunk of the map, without simulating
/// everything.
#[derive(Clone)]
pub struct TrafficRecorder {
    capture_points: BTreeSet<IntersectionID>,
    // TODO The RNG will determine vehicle length, so this won't be a perfect capture. Hopefully
    // good enough.
    trips: Vec<IndividTrip>,
    // Where and when did a car encounter one of the capture_points?
    entered: BTreeMap<CarID, (Time, LaneID)>,
}

impl TrafficRecorder {
    pub fn new(capture_points: BTreeSet<IntersectionID>) -> TrafficRecorder {
        TrafficRecorder {
            capture_points,
            trips: Vec::new(),
            entered: BTreeMap::new(),
        }
    }

    pub fn handle_event(&mut self, time: Time, ev: &Event, map: &Map) {
        if let Event::AgentEntersTraversable(a, on, _) = ev {
            if let AgentID::Car(car) = a {
                if let Traversable::Lane(l) = on {
                    let lane = map.get_l(*l);
                    if self.capture_points.contains(&lane.src_i) {
                        self.entered.insert(*car, (time, *l));
                    } else if self.capture_points.contains(&lane.dst_i) {
                        if let Some((depart, from)) = self.entered.remove(car) {
                            self.trips.push(IndividTrip::new(
                                depart,
                                TripPurpose::Shopping,
                                SpawnTrip::VehicleAppearing {
                                    start: Position::start(from),
                                    goal: DrivingGoal::Border(lane.dst_i, lane.id, None),
                                    is_bike: car.1 == VehicleType::Bike,
                                },
                            ));
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
        for trip in self.trips.drain(..) {
            people.push(PersonSpec {
                id: PersonID(people.len()),
                orig_id: None,
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
