use crate::{
    AgentID, Command, CreatePedestrian, DistanceInterval, DrawPedestrianInput,
    IntersectionSimState, ParkingSimState, PedestrianID, Scheduler, SidewalkPOI, SidewalkSpot,
    TimeInterval, TransitSimState, TripManager,
};
use abstutil::{deserialize_multimap, serialize_multimap, MultiMap};
use geom::{Distance, Duration, Line, PolyLine, Speed};
use map_model::{BuildingID, Map, Path, PathStep, Traversable};
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
        now: Duration,
        params: CreatePedestrian,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        let start_lane = params.start.sidewalk_pos.lane();
        assert_eq!(params.path.current_step().as_lane(), start_lane);
        assert_eq!(
            params.path.last_step().as_lane(),
            params.goal.sidewalk_pos.lane()
        );

        let mut ped = Pedestrian {
            id: params.id,
            // Temporary bogus thing
            state: PedState::Crossing(
                DistanceInterval::new_walking(Distance::ZERO, Distance::meters(1.0)),
                TimeInterval::new(Duration::ZERO, Duration::seconds(1.0)),
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

        scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
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

    pub fn update_ped(
        &mut self,
        id: PedestrianID,
        now: Duration,
        map: &Map,
        intersections: &mut IntersectionSimState,
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
        trips: &mut TripManager,
        transit: &mut TransitSimState,
    ) {
        let mut ped = self.peds.get_mut(&id).unwrap();
        match ped.state {
            PedState::Crossing(ref dist_int, _) => {
                if ped.path.is_last_step() {
                    match ped.goal.connection {
                        SidewalkPOI::ParkingSpot(spot) => {
                            self.peds_per_traversable
                                .remove(ped.path.current_step().as_traversable(), ped.id);
                            trips.ped_reached_parking_spot(
                                now, ped.id, spot, map, parking, scheduler,
                            );
                            self.peds.remove(&id);
                        }
                        SidewalkPOI::Building(b) => {
                            ped.state = PedState::EnteringBuilding(
                                b,
                                TimeInterval::new(
                                    now,
                                    now + map.get_b(b).front_path.line.length() / SPEED,
                                ),
                            );
                            scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                        }
                        SidewalkPOI::BusStop(stop) => {
                            if trips.ped_reached_bus_stop(ped.id, stop, map, transit) {
                                self.peds_per_traversable
                                    .remove(ped.path.current_step().as_traversable(), ped.id);
                                self.peds.remove(&id);
                            } else {
                                ped.state = PedState::WaitingForBus;
                            }
                        }
                        SidewalkPOI::Border(i) => {
                            self.peds_per_traversable
                                .remove(ped.path.current_step().as_traversable(), ped.id);
                            trips.ped_reached_border(now, ped.id, i, map);
                            self.peds.remove(&id);
                        }
                        SidewalkPOI::BikeRack(driving_pos) => {
                            let pt1 = ped.goal.sidewalk_pos.pt(map);
                            let pt2 = driving_pos.pt(map);
                            ped.state = PedState::StartingToBike(
                                ped.goal.clone(),
                                Line::new(pt1, pt2),
                                TimeInterval::new(now, now + TIME_TO_START_BIKING),
                            );
                            scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                        }
                        SidewalkPOI::SuddenlyAppear => unreachable!(),
                    }
                } else {
                    if let PathStep::Turn(t) = ped.path.current_step() {
                        intersections.turn_finished(now, AgentID::Pedestrian(ped.id), t, scheduler);
                    }

                    let dist = dist_int.end;
                    if ped.maybe_transition(now, map, intersections, &mut self.peds_per_traversable)
                    {
                        scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                    } else {
                        // Must've failed because we can't turn yet. Don't schedule a retry here.
                        ped.state = PedState::WaitingToTurn(dist);
                    }
                }
            }
            PedState::WaitingToTurn(_) => {
                if ped.maybe_transition(now, map, intersections, &mut self.peds_per_traversable) {
                    scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                }
            }
            PedState::LeavingBuilding(b, _) => {
                ped.state =
                    ped.crossing_state(map.get_b(b).front_path.sidewalk.dist_along(), now, map);
                scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
            }
            PedState::EnteringBuilding(bldg, _) => {
                self.peds_per_traversable
                    .remove(ped.path.current_step().as_traversable(), ped.id);
                trips.ped_reached_building(now, ped.id, bldg, map);
                self.peds.remove(&id);
            }
            PedState::StartingToBike(ref spot, _, _) => {
                self.peds_per_traversable
                    .remove(ped.path.current_step().as_traversable(), ped.id);
                trips.ped_ready_to_bike(now, ped.id, spot.clone(), map, scheduler);
                self.peds.remove(&id);
            }
            PedState::FinishingBiking(ref spot, _, _) => {
                ped.state = ped.crossing_state(spot.sidewalk_pos.dist_along(), now, map);
                scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
            }
            PedState::WaitingForBus => unreachable!(),
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
            format!("{} on {:?}", p.id, p.path.current_step()),
            format!("{} lanes left in path", p.path.num_lanes()),
            format!("{:?}", p.state),
        ]
    }

    pub fn trace_route(
        &self,
        time: Duration,
        id: PedestrianID,
        map: &Map,
        dist_ahead: Option<Distance>,
    ) -> Option<PolyLine> {
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
        PedState::Crossing(dist_int, time_int)
    }

    fn get_dist_along(&self, time: Duration, map: &Map) -> Distance {
        match self.state {
            PedState::Crossing(ref dist_int, ref time_int) => dist_int.lerp(time_int.percent(time)),
            PedState::WaitingToTurn(dist) => dist,
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
            PedState::Crossing(ref dist_int, ref time_int) => {
                let percent = if time > time_int.end {
                    1.0
                } else {
                    time_int.percent(time)
                };
                on.dist_along(dist_int.lerp(percent), map).0
            }
            PedState::WaitingToTurn(dist) => on.dist_along(dist, map).0,
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

    // True if we successfully continued to the next step of our path
    fn maybe_transition(
        &mut self,
        now: Duration,
        map: &Map,
        intersections: &mut IntersectionSimState,
        peds_per_traversable: &mut MultiMap<Traversable, PedestrianID>,
    ) -> bool {
        if let PathStep::Turn(t) = self.path.next_step() {
            if !intersections.maybe_start_turn(AgentID::Pedestrian(self.id), t, now, map) {
                return false;
            }
        }

        peds_per_traversable.remove(self.path.current_step().as_traversable(), self.id);
        self.path.shift();
        let start_dist = match self.path.current_step() {
            PathStep::Lane(_) => Distance::ZERO,
            PathStep::ContraflowLane(l) => map.get_l(l).length(),
            PathStep::Turn(_) => Distance::ZERO,
        };
        self.state = self.crossing_state(start_dist, now, map);
        peds_per_traversable.insert(self.path.current_step().as_traversable(), self.id);
        true
    }
}

// crossing front path, bike parking, waiting at bus stop, etc
#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum PedState {
    Crossing(DistanceInterval, TimeInterval),
    // The Distance is either 0 or the current traversable's length
    WaitingToTurn(Distance),
    LeavingBuilding(BuildingID, TimeInterval),
    EnteringBuilding(BuildingID, TimeInterval),
    StartingToBike(SidewalkSpot, Line, TimeInterval),
    FinishingBiking(SidewalkSpot, Line, TimeInterval),
    WaitingForBus,
}

impl PedState {
    fn get_end_time(&self) -> Duration {
        match self {
            PedState::Crossing(_, ref time_int) => time_int.end,
            PedState::WaitingToTurn(_) => unreachable!(),
            PedState::LeavingBuilding(_, ref time_int) => time_int.end,
            PedState::EnteringBuilding(_, ref time_int) => time_int.end,
            PedState::StartingToBike(_, _, ref time_int) => time_int.end,
            PedState::FinishingBiking(_, _, ref time_int) => time_int.end,
            PedState::WaitingForBus => unreachable!(),
        }
    }
}
