use crate::plugins::sim::new_des_model::{
    CreatePedestrian, DistanceInterval, IntersectionSimState, ParkingSimState, Scheduler,
    SidewalkPOI, SidewalkSpot, TimeInterval, TripManager,
};
use abstutil::{deserialize_multimap, serialize_multimap};
use geom::{Distance, Duration, Line, Speed};
use map_model::{BuildingID, Map, Path, PathStep, Traversable};
use multimap::MultiMap;
use serde_derive::{Deserialize, Serialize};
use sim::{AgentID, DrawPedestrianInput, PedestrianID};
use std::collections::BTreeMap;

// TODO These are comically fast.
const SPEED: Speed = Speed::const_meters_per_second(3.9);
const TIME_TO_START_BIKING: Duration = Duration::const_seconds(30.0);
const TIME_TO_FINISH_BIKING: Duration = Duration::const_seconds(45.0);

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

    pub fn spawn_ped(&mut self, time: Duration, params: CreatePedestrian, map: &Map) {
        let start_lane = params.start.sidewalk_pos.lane();
        assert_eq!(
            params.path.current_step().as_traversable(),
            Traversable::Lane(start_lane)
        );
        assert_eq!(
            params.path.last_step().as_traversable(),
            Traversable::Lane(params.goal.sidewalk_pos.lane())
        );

        let mut ped = Pedestrian {
            id: params.id,
            // Temporary bogus thing
            state: PedState::Crossing(
                DistanceInterval::new_walking(Distance::ZERO, Distance::meters(1.0)),
                TimeInterval::new(Duration::ZERO, Duration::seconds(1.0)),
                true,
            ),
            path: params.path,
            goal: params.goal,
        };
        ped.state = match params.start.connection {
            SidewalkPOI::Building(b) => PedState::LeavingBuilding(
                b,
                TimeInterval::new(time, time + map.get_b(b).front_path.line.length() / SPEED),
            ),
            SidewalkPOI::BikeRack(driving_pos) => PedState::FinishingBiking(
                params.start.clone(),
                Line::new(driving_pos.pt(map), params.start.sidewalk_pos.pt(map)),
                TimeInterval::new(time, time + TIME_TO_FINISH_BIKING),
            ),
            _ => ped.crossing_state(params.start.sidewalk_pos.dist_along(), time, map),
        };

        self.peds.insert(params.id, ped);
        self.peds_per_traversable.insert(
            Traversable::Lane(params.start.sidewalk_pos.lane()),
            params.id,
        );
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
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
        trips: &mut TripManager,
    ) {
        let mut delete = Vec::new();
        for ped in self.peds.values_mut() {
            match ped.state {
                PedState::Crossing(_, ref time_int, ref mut turn_finished) => {
                    if time > time_int.end {
                        if ped.path.is_last_step() {
                            match ped.goal.connection {
                                SidewalkPOI::ParkingSpot(spot) => {
                                    delete.push(ped.id);
                                    delete_ped_from_current_step(
                                        &mut self.peds_per_traversable,
                                        ped,
                                    );
                                    trips.ped_reached_parking_spot(
                                        time, ped.id, spot, map, parking, scheduler,
                                    );
                                }
                                SidewalkPOI::Building(b) => {
                                    ped.state = PedState::EnteringBuilding(
                                        b,
                                        TimeInterval::new(
                                            time,
                                            time + map.get_b(b).front_path.line.length() / SPEED,
                                        ),
                                    );
                                }
                                SidewalkPOI::BusStop(stop) => {
                                    panic!("implement");
                                }
                                SidewalkPOI::Border(_) => {
                                    delete.push(ped.id);
                                    delete_ped_from_current_step(
                                        &mut self.peds_per_traversable,
                                        ped,
                                    );
                                }
                                SidewalkPOI::BikeRack(driving_pos) => {
                                    let pt1 = ped.goal.sidewalk_pos.pt(map);
                                    let pt2 = driving_pos.pt(map);
                                    ped.state = PedState::StartingToBike(
                                        ped.goal.clone(),
                                        Line::new(pt1, pt2),
                                        TimeInterval::new(time, time + TIME_TO_START_BIKING),
                                    );
                                }
                            }
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

                            delete_ped_from_current_step(&mut self.peds_per_traversable, ped);
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
                PedState::LeavingBuilding(b, ref time_int) => {
                    if time > time_int.end {
                        ped.state = ped.crossing_state(
                            map.get_b(b).front_path.sidewalk.dist_along(),
                            time,
                            map,
                        );
                    }
                }
                PedState::EnteringBuilding(_, ref time_int) => {
                    if time > time_int.end {
                        delete.push(ped.id);
                        delete_ped_from_current_step(&mut self.peds_per_traversable, ped);
                    }
                }
                PedState::StartingToBike(ref spot, _, ref time_int) => {
                    if time > time_int.end {
                        delete.push(ped.id);
                        delete_ped_from_current_step(&mut self.peds_per_traversable, ped);
                        trips.ped_ready_to_bike(time, ped.id, spot.clone(), map, scheduler);
                    }
                }
                PedState::FinishingBiking(ref spot, _, ref time_int) => {
                    if time > time_int.end {
                        ped.state = ped.crossing_state(spot.sidewalk_pos.dist_along(), time, map);
                    }
                }
            };
        }
        for id in delete {
            self.peds.remove(&id);
        }
    }
}

fn delete_ped_from_current_step(map: &mut MultiMap<Traversable, PedestrianID>, ped: &Pedestrian) {
    // API is so bad that we have this helper!
    map.get_vec_mut(&ped.path.current_step().as_traversable())
        .unwrap()
        .retain(|&p| p != ped.id);
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
        let on = self.path.current_step().as_traversable();
        let pos = match self.state {
            PedState::Crossing(ref dist_int, ref time_int, _) => {
                let percent = if time > time_int.end {
                    1.0
                } else {
                    time_int.percent(time)
                };
                on.dist_along(dist_int.lerp(percent), map).0
            }
            PedState::LeavingBuilding(b, ref time_int) => {
                let front_path = &map.get_b(b).front_path;
                let pt = front_path
                    .line
                    .dist_along(time_int.percent(time) * front_path.line.length());
                pt
            }
            PedState::EnteringBuilding(b, ref time_int) => {
                let front_path = &map.get_b(b).front_path;
                let pt = front_path
                    .line
                    .reverse()
                    .dist_along(time_int.percent(time) * front_path.line.length());
                pt
            }
            PedState::StartingToBike(_, ref line, ref time_int) => {
                line.percent_along(time_int.percent(time))
            }
            PedState::FinishingBiking(_, ref line, ref time_int) => {
                line.percent_along(time_int.percent(time))
            }
        };

        DrawPedestrianInput {
            id: self.id,
            pos,
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
    LeavingBuilding(BuildingID, TimeInterval),
    EnteringBuilding(BuildingID, TimeInterval),
    StartingToBike(SidewalkSpot, Line, TimeInterval),
    FinishingBiking(SidewalkSpot, Line, TimeInterval),
}
