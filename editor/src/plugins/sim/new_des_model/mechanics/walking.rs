use crate::plugins::sim::new_des_model::{
    DistanceInterval, IntersectionSimState, SidewalkPOI, SidewalkSpot, TimeInterval,
};
use abstutil::{deserialize_multimap, serialize_multimap};
use geom::{Distance, Duration, Speed};
use map_model::{Map, Path, PathStep, Traversable};
use multimap::MultiMap;
use serde_derive::{Deserialize, Serialize};
use sim::{AgentID, DrawPedestrianInput, PedestrianID};
use std::collections::BTreeMap;

// TODO This is comically fast.
const SPEED: Speed = Speed::const_meters_per_second(3.9);

#[derive(Serialize, Deserialize)]
pub struct WalkingSimState {
    // BTreeMap not for deterministic simulation, but to make serialized things easier to compare.
    peds: BTreeMap<PedestrianID, Pedestrian>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds: BTreeMap::new(),
            peds_per_traversable: MultiMap::new(),
        }
    }

    pub fn spawn_ped(
        &mut self,
        id: PedestrianID,
        start_time: Duration,
        start: SidewalkSpot,
        goal: SidewalkSpot,
        path: Path,
        map: &Map,
    ) {
        let start_lane = start.sidewalk_pos.lane();
        assert_eq!(
            path.current_step().as_traversable(),
            Traversable::Lane(start_lane)
        );
        assert_eq!(
            path.last_step().as_traversable(),
            Traversable::Lane(goal.sidewalk_pos.lane())
        );

        let mut ped = Pedestrian {
            id,
            // Temporary bogus thing
            state: PedState::Crossing(
                DistanceInterval::new_walking(Distance::ZERO, Distance::meters(1.0)),
                TimeInterval::new(Duration::ZERO, Duration::seconds(1.0)),
                true,
            ),
            path,
            goal,
        };
        ped.state = match start.connection {
            SidewalkPOI::BikeRack => {
                ped.crossing_state(start.sidewalk_pos.dist_along(), start_time, map)
            }
            _ => panic!("Don't support {:?} yet", start.connection),
        };

        self.peds.insert(id, ped);
        self.peds_per_traversable
            .insert(Traversable::Lane(start.sidewalk_pos.lane()), id);
    }

    pub fn get_all_draw_peds(&self, time: Duration, map: &Map) -> Vec<DrawPedestrianInput> {
        self.peds
            .values()
            .map(|p| p.get_draw_ped(time, map))
            .collect()
    }

    pub fn get_draw_peds(
        &self,
        time: Duration,
        on: Traversable,
        map: &Map,
    ) -> Vec<DrawPedestrianInput> {
        self.peds_per_traversable
            .get_vec(&on)
            .unwrap_or(&Vec::new())
            .iter()
            .map(|id| self.peds[id].get_draw_ped(time, map))
            .collect()
    }

    pub fn step_if_needed(
        &mut self,
        time: Duration,
        map: &Map,
        intersections: &mut IntersectionSimState,
    ) {
        let mut delete = Vec::new();
        for ped in self.peds.values_mut() {
            match ped.state {
                PedState::Crossing(_, ref time_int, ref mut turn_finished) => {
                    if time > time_int.end {
                        if ped.path.is_last_step() {
                            // TODO Use goal
                            delete.push(ped.id);

                            // TODO Ew O_O
                            self.peds_per_traversable
                                .get_vec_mut(&ped.path.current_step().as_traversable())
                                .unwrap()
                                .retain(|&p| p != ped.id);
                        } else {
                            if !*turn_finished {
                                if let PathStep::Turn(t) = ped.path.current_step() {
                                    intersections.turn_finished(AgentID::Pedestrian(ped.id), t);
                                    *turn_finished = true;
                                }
                            }

                            if let PathStep::Turn(t) = ped.path.next_step() {
                                if !intersections.maybe_start_turn(
                                    AgentID::Pedestrian(ped.id),
                                    t,
                                    time,
                                    map,
                                ) {
                                    continue;
                                }
                            }

                            // TODO Ew O_O
                            self.peds_per_traversable
                                .get_vec_mut(&ped.path.current_step().as_traversable())
                                .unwrap()
                                .retain(|&p| p != ped.id);

                            ped.path.shift();
                            let start_dist = match ped.path.current_step() {
                                PathStep::Lane(_) => Distance::ZERO,
                                PathStep::ContraflowLane(l) => map.get_l(l).length(),
                                PathStep::Turn(_) => Distance::ZERO,
                            };
                            ped.state = ped.crossing_state(start_dist, time, map);
                            self.peds_per_traversable
                                .insert(ped.path.current_step().as_traversable(), ped.id);
                        }
                    }
                }
            };
        }
        for id in delete {
            self.peds.remove(&id);
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Pedestrian {
    id: PedestrianID,
    state: PedState,

    path: Path,
    goal: SidewalkSpot,
}

impl Pedestrian {
    fn crossing_state(&self, start_dist: Distance, start_time: Duration, map: &Map) -> PedState {
        let end_dist = if self.path.is_last_step() {
            self.goal.sidewalk_pos.dist_along()
        } else {
            // TODO PathStep should have a end_dist... or end_pos
            match self.path.current_step() {
                PathStep::Lane(l) => map.get_l(l).length(),
                PathStep::ContraflowLane(_) => Distance::ZERO,
                PathStep::Turn(t) => map.get_t(t).geom.length(),
            }
        };
        let dist_int = DistanceInterval::new_walking(start_dist, end_dist);
        let time_int = TimeInterval::new(start_time, start_time + dist_int.length() / SPEED);
        PedState::Crossing(dist_int, time_int, false)
    }

    fn get_draw_ped(&self, time: Duration, map: &Map) -> DrawPedestrianInput {
        let (on, dist) = match self.state {
            PedState::Crossing(ref dist_int, ref time_int, _) => {
                let percent = if time > time_int.end {
                    1.0
                } else {
                    time_int.percent(time)
                };
                (
                    self.path.current_step().as_traversable(),
                    dist_int.lerp(percent),
                )
            }
        };

        DrawPedestrianInput {
            id: self.id,
            pos: on.dist_along(dist, map).0,
            waiting_for_turn: None,
            preparing_bike: false,
            on,
        }
    }
}

// crossing front path, bike parking, waiting at bus stop, etc
#[derive(Serialize, Deserialize)]
enum PedState {
    // If we're past the TimeInterval, then blocked on a turn.
    // The bool is true when we've marked the turn finished. We might experience two turn sequences
    // and the second turn isn't accepted. Don't block the intersection by waiting. Turns always
    // end at little sidewalk corners, so the ped is actually out of the way.
    Crossing(DistanceInterval, TimeInterval, bool),
}
