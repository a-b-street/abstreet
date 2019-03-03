use crate::{
    AgentID, CreatePedestrian, DistanceInterval, DrawPedestrianInput, IntersectionSimState,
    ParkingSimState, PedestrianID, PriorityQueue, Scheduler, SidewalkPOI, SidewalkSpot,
    TimeInterval, TransitSimState, TripManager,
};
use abstutil::{deserialize_multimap, serialize_multimap, MultiMap};
use geom::{Distance, Duration, Line, Speed};
use map_model::{BuildingID, Map, Path, PathStep, Trace, Traversable};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

// TODO These are comically fast.
const SPEED: Speed = Speed::const_meters_per_second(3.9);
const TIME_TO_START_BIKING: Duration = Duration::const_seconds(30.0);
const TIME_TO_FINISH_BIKING: Duration = Duration::const_seconds(45.0);

#[derive(Serialize, Deserialize, PartialEq)]
pub struct WalkingSimState {
    // BTreeMap not for deterministic simulation, but to make serialized things easier to compare.
    peds: BTreeMap<PedestrianID, Pedestrian>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,

    events: PriorityQueue<PedestrianID>,
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds: BTreeMap::new(),
            peds_per_traversable: MultiMap::new(),
            events: PriorityQueue::new(),
        }
    }

    pub fn spawn_ped(&mut self, now: Duration, params: CreatePedestrian, map: &Map) {
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
                TimeInterval::new(now, now + map.get_b(b).front_path.line.length() / SPEED),
            ),
            SidewalkPOI::BikeRack(driving_pos) => PedState::FinishingBiking(
                params.start.clone(),
                Line::new(driving_pos.pt(map), params.start.sidewalk_pos.pt(map)),
                TimeInterval::new(now, now + TIME_TO_FINISH_BIKING),
            ),
            _ => ped.crossing_state(params.start.sidewalk_pos.dist_along(), now, map),
        };

        self.events.push(ped.state.get_end_time().unwrap(), ped.id);
        self.peds.insert(ped.id, ped);
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
            .get(on)
            .iter()
            .map(|id| self.peds[id].get_draw_ped(time, map))
            .collect()
    }

    pub fn step_if_needed(
        &mut self,
        now: Duration,
        map: &Map,
        intersections: &mut IntersectionSimState,
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
        trips: &mut TripManager,
        transit: &mut TransitSimState,
    ) {
        let mut delete = Vec::new();
        for id in self.events.get_for_time(now).into_iter() {
            let mut ped = self.peds.get_mut(&id).unwrap();
            match ped.state {
                PedState::Crossing(_, _, ref mut turn_finished) => {
                    if ped.path.is_last_step() {
                        match ped.goal.connection {
                            SidewalkPOI::ParkingSpot(spot) => {
                                delete.push(ped.id);
                                self.peds_per_traversable
                                    .remove(ped.path.current_step().as_traversable(), ped.id);
                                trips.ped_reached_parking_spot(
                                    now, ped.id, spot, map, parking, scheduler,
                                );
                            }
                            SidewalkPOI::Building(b) => {
                                ped.state = PedState::EnteringBuilding(
                                    b,
                                    TimeInterval::new(
                                        now,
                                        now + map.get_b(b).front_path.line.length() / SPEED,
                                    ),
                                );
                                self.events.push(ped.state.get_end_time().unwrap(), ped.id);
                            }
                            SidewalkPOI::BusStop(stop) => {
                                if trips.ped_reached_bus_stop(ped.id, stop, map, transit) {
                                    delete.push(ped.id);
                                    self.peds_per_traversable
                                        .remove(ped.path.current_step().as_traversable(), ped.id);
                                } else {
                                    ped.state = PedState::WaitingForBus;
                                }
                            }
                            SidewalkPOI::Border(i) => {
                                delete.push(ped.id);
                                self.peds_per_traversable
                                    .remove(ped.path.current_step().as_traversable(), ped.id);
                                trips.ped_reached_border(now, ped.id, i, map);
                            }
                            SidewalkPOI::BikeRack(driving_pos) => {
                                let pt1 = ped.goal.sidewalk_pos.pt(map);
                                let pt2 = driving_pos.pt(map);
                                ped.state = PedState::StartingToBike(
                                    ped.goal.clone(),
                                    Line::new(pt1, pt2),
                                    TimeInterval::new(now, now + TIME_TO_START_BIKING),
                                );
                                self.events.push(ped.state.get_end_time().unwrap(), ped.id);
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
                                now,
                                map,
                            ) {
                                // TODO separate state to block on the turn
                                self.events.push(now + Duration::EPSILON, ped.id);
                                continue;
                            }
                        }

                        self.peds_per_traversable
                            .remove(ped.path.current_step().as_traversable(), ped.id);
                        ped.path.shift();
                        let start_dist = match ped.path.current_step() {
                            PathStep::Lane(_) => Distance::ZERO,
                            PathStep::ContraflowLane(l) => map.get_l(l).length(),
                            PathStep::Turn(_) => Distance::ZERO,
                        };
                        ped.state = ped.crossing_state(start_dist, now, map);
                        self.peds_per_traversable
                            .insert(ped.path.current_step().as_traversable(), ped.id);
                        self.events.push(ped.state.get_end_time().unwrap(), ped.id);
                    }
                }
                PedState::LeavingBuilding(b, _) => {
                    ped.state =
                        ped.crossing_state(map.get_b(b).front_path.sidewalk.dist_along(), now, map);
                    self.events.push(ped.state.get_end_time().unwrap(), ped.id);
                }
                PedState::EnteringBuilding(bldg, _) => {
                    delete.push(ped.id);
                    self.peds_per_traversable
                        .remove(ped.path.current_step().as_traversable(), ped.id);
                    trips.ped_reached_building(now, ped.id, bldg, map);
                }
                PedState::StartingToBike(ref spot, _, _) => {
                    delete.push(ped.id);
                    self.peds_per_traversable
                        .remove(ped.path.current_step().as_traversable(), ped.id);
                    trips.ped_ready_to_bike(now, ped.id, spot.clone(), map, scheduler);
                }
                PedState::FinishingBiking(ref spot, _, _) => {
                    ped.state = ped.crossing_state(spot.sidewalk_pos.dist_along(), now, map);
                    self.events.push(ped.state.get_end_time().unwrap(), ped.id);
                }
                PedState::WaitingForBus => unreachable!(),
            };
        }
        for id in delete {
            self.peds.remove(&id);
        }
    }

    pub fn ped_boarded_bus(&mut self, id: PedestrianID) {
        let ped = self.peds.remove(&id).unwrap();
        match ped.state {
            PedState::WaitingForBus => {
                self.peds_per_traversable
                    .remove(ped.path.current_step().as_traversable(), id);
            }
            _ => unreachable!(),
        };
    }

    pub fn debug_ped(&self, id: PedestrianID) {
        if let Some(ped) = self.peds.get(&id) {
            println!("{}", abstutil::to_json(ped));
        } else {
            println!("{} doesn't exist", id);
        }
    }

    pub fn ped_tooltip(&self, id: PedestrianID) -> Vec<String> {
        let p = &self.peds[&id];
        vec![
            format!("{}", p.id),
            format!("{} lanes left in path", p.path.num_lanes()),
        ]
    }

    pub fn trace_route(
        &self,
        time: Duration,
        id: PedestrianID,
        map: &Map,
        dist_ahead: Option<Distance>,
    ) -> Option<Trace> {
        let p = self.peds.get(&id)?;
        p.path.trace(map, p.get_dist_along(time, map), dist_ahead)
    }

    pub fn get_path(&self, id: PedestrianID) -> Option<&Path> {
        let p = self.peds.get(&id)?;
        Some(&p.path)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
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

    fn get_dist_along(&self, time: Duration, map: &Map) -> Distance {
        match self.state {
            PedState::Crossing(ref dist_int, ref time_int, _) => {
                let percent = if time > time_int.end {
                    1.0
                } else {
                    time_int.percent(time)
                };
                dist_int.lerp(percent)
            }
            PedState::LeavingBuilding(b, _) => map.get_b(b).front_path.sidewalk.dist_along(),
            PedState::EnteringBuilding(b, _) => map.get_b(b).front_path.sidewalk.dist_along(),
            PedState::StartingToBike(ref spot, _, _) => spot.sidewalk_pos.dist_along(),
            PedState::FinishingBiking(ref spot, _, _) => spot.sidewalk_pos.dist_along(),
            PedState::WaitingForBus => self.goal.sidewalk_pos.dist_along(),
        }
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
                front_path
                    .line
                    .dist_along(time_int.percent(time) * front_path.line.length())
            }
            PedState::EnteringBuilding(b, ref time_int) => {
                let front_path = &map.get_b(b).front_path;
                front_path
                    .line
                    .reverse()
                    .dist_along(time_int.percent(time) * front_path.line.length())
            }
            PedState::StartingToBike(_, ref line, ref time_int) => {
                line.percent_along(time_int.percent(time))
            }
            PedState::FinishingBiking(_, ref line, ref time_int) => {
                line.percent_along(time_int.percent(time))
            }
            PedState::WaitingForBus => self.goal.sidewalk_pos.pt(map),
        };

        DrawPedestrianInput {
            id: self.id,
            pos,
            waiting_for_turn: None,
            preparing_bike: match self.state {
                PedState::StartingToBike(_, _, _) | PedState::FinishingBiking(_, _, _) => true,
                _ => false,
            },
            on,
        }
    }
}

// crossing front path, bike parking, waiting at bus stop, etc
#[derive(Serialize, Deserialize, PartialEq)]
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
    WaitingForBus,
}

impl PedState {
    fn get_end_time(&self) -> Option<Duration> {
        match self {
            // TODO Need a state for waiting on intersection
            PedState::Crossing(_, ref time_int, _) => Some(time_int.end),
            PedState::LeavingBuilding(_, ref time_int) => Some(time_int.end),
            PedState::EnteringBuilding(_, ref time_int) => Some(time_int.end),
            PedState::StartingToBike(_, _, ref time_int) => Some(time_int.end),
            PedState::FinishingBiking(_, _, ref time_int) => Some(time_int.end),
            PedState::WaitingForBus => None,
        }
    }
}
