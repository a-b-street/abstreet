use crate::{
    AgentID, Command, CreatePedestrian, DistanceInterval, DrawPedCrowdInput, DrawPedestrianInput,
    IntersectionSimState, ParkingSimState, PedestrianID, Scheduler, SidewalkPOI, SidewalkSpot,
    TimeInterval, TransitSimState, TripID, TripManager, TripPositions, UnzoomedAgent,
};
use abstutil::{deserialize_multimap, serialize_multimap, MultiMap};
use geom::{Distance, Duration, Line, PolyLine, Speed};
use map_model::{BuildingID, Map, Path, PathStep, Traversable, LANE_THICKNESS};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
            speed: params.speed,
            blocked_since: None,
            started_at: now,
            path: params.path,
            goal: params.goal,
            trip: params.trip,
        };
        ped.state = match params.start.connection {
            SidewalkPOI::Building(b) => PedState::LeavingBuilding(
                b,
                TimeInterval::new(now, now + map.get_b(b).front_path.line.length() / ped.speed),
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

    pub fn get_all_draw_peds(&self, now: Duration, map: &Map) -> Vec<DrawPedestrianInput> {
        self.peds
            .values()
            .map(|p| p.get_draw_ped(now, map))
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
                                    now + map.get_b(b).front_path.line.length() / ped.speed,
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
                                ped.blocked_since = Some(now);
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
                    if ped.maybe_transition(
                        now,
                        map,
                        intersections,
                        &mut self.peds_per_traversable,
                        scheduler,
                    ) {
                        scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                    } else {
                        // Must've failed because we can't turn yet. Don't schedule a retry here.
                        ped.state = PedState::WaitingToTurn(dist);
                        ped.blocked_since = Some(now);
                    }
                }
            }
            PedState::WaitingToTurn(_) => {
                if ped.maybe_transition(
                    now,
                    map,
                    intersections,
                    &mut self.peds_per_traversable,
                    scheduler,
                ) {
                    scheduler.push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                    ped.blocked_since = None;
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

    pub fn ped_tooltip(&self, id: PedestrianID, now: Duration) -> Vec<String> {
        let p = &self.peds[&id];
        vec![
            format!("{} on {:?}", p.id, p.path.current_step()),
            format!("{} lanes left in path", p.path.num_lanes()),
            format!(
                "Crossed {} / {} of path",
                p.path.crossed_so_far(),
                p.path.total_length()
            ),
            format!(
                "Blocked for {}",
                p.blocked_since.map(|t| now - t).unwrap_or(Duration::ZERO)
            ),
            format!("Trip time so far: {}", now - p.started_at),
            format!("{:?}", p.state),
        ]
    }

    pub fn trace_route(
        &self,
        now: Duration,
        id: PedestrianID,
        map: &Map,
        dist_ahead: Option<Distance>,
    ) -> Option<PolyLine> {
        let p = self.peds.get(&id)?;
        let body_radius = LANE_THICKNESS / 4.0;
        let dist = (p.get_dist_along(now, map) + body_radius)
            .min(p.path.current_step().as_traversable().length(map));
        p.path.trace(map, dist, dist_ahead)
    }

    pub fn get_path(&self, id: PedestrianID) -> Option<&Path> {
        let p = self.peds.get(&id)?;
        Some(&p.path)
    }

    pub fn get_unzoomed_agents(&self, now: Duration, map: &Map) -> Vec<UnzoomedAgent> {
        let mut peds = Vec::new();

        for ped in self.peds.values() {
            peds.push(UnzoomedAgent {
                vehicle_type: None,
                pos: ped.get_draw_ped(now, map).pos,
                time_spent_blocked: ped.blocked_since.map(|t| now - t).unwrap_or(Duration::ZERO),
                percent_dist_crossed: ped.path.percent_dist_crossed(),
                trip_time_so_far: now - ped.started_at,
            });
        }

        peds
    }

    pub fn populate_trip_positions(&self, trip_positions: &mut TripPositions, map: &Map) {
        for ped in self.peds.values() {
            trip_positions
                .canonical_pt_per_trip
                .insert(ped.trip, ped.get_draw_ped(trip_positions.time, map).pos);
        }
    }

    pub fn get_draw_peds_on(
        &self,
        now: Duration,
        on: Traversable,
        map: &Map,
    ) -> (Vec<DrawPedestrianInput>, Vec<DrawPedCrowdInput>) {
        // Classify into direction-based groups or by building front path.
        let mut forwards: Vec<(PedestrianID, Distance)> = Vec::new();
        let mut backwards: Vec<(PedestrianID, Distance)> = Vec::new();
        let mut front_path: MultiMap<BuildingID, (PedestrianID, Distance)> = MultiMap::new();

        let mut loners: Vec<DrawPedestrianInput> = Vec::new();

        for id in self.peds_per_traversable.get(on) {
            let ped = &self.peds[id];
            let dist = ped.get_dist_along(now, map);

            match ped.state {
                PedState::Crossing(ref dist_int, _) => {
                    if dist_int.start < dist_int.end {
                        forwards.push((*id, dist));
                    } else {
                        backwards.push((*id, dist));
                    }
                }
                PedState::WaitingToTurn(dist) => {
                    if dist == Distance::ZERO {
                        backwards.push((*id, dist));
                    } else {
                        forwards.push((*id, dist));
                    }
                }
                PedState::LeavingBuilding(b, _) | PedState::EnteringBuilding(b, _) => {
                    // TODO Group on front paths too.
                    if false {
                        // TODO Distance along the front path
                        front_path.insert(b, (*id, dist));
                    } else {
                        loners.push(ped.get_draw_ped(now, map));
                    }
                }
                PedState::StartingToBike(_, _, _)
                | PedState::FinishingBiking(_, _, _)
                | PedState::WaitingForBus => {
                    // The backwards half of the sidewalk is closer to the road.
                    backwards.push((*id, dist));
                }
            }
        }

        let mut crowds: Vec<DrawPedCrowdInput> = Vec::new();
        let on_len = on.length(map);

        // For each group, sort by distance along. Attempt to bundle into intervals.
        for (idx, mut group) in vec![forwards, backwards]
            .into_iter()
            .chain(
                front_path
                    .consume()
                    .values()
                    .map(|set| set.into_iter().cloned().collect::<Vec<_>>()),
            )
            .enumerate()
        {
            if group.is_empty() {
                continue;
            }
            group.sort_by_key(|(_, dist)| *dist);
            let (individs, these_crowds) = find_crowds(group, on);
            for id in individs {
                loners.push(self.peds[&id].get_draw_ped(now, map));
            }
            for mut crowd in these_crowds {
                crowd.contraflow = idx == 1;
                // Clamp the distance intervals.
                if crowd.low < Distance::ZERO {
                    crowd.low = Distance::ZERO;
                }
                if crowd.high > on_len {
                    crowd.high = on_len;
                }
                crowds.push(crowd);
            }
        }

        (loners, crowds)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
struct Pedestrian {
    id: PedestrianID,
    state: PedState,
    speed: Speed,
    blocked_since: Option<Duration>,
    // TODO organize analytics better.
    started_at: Duration,

    path: Path,
    goal: SidewalkSpot,
    trip: TripID,
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
        let time_int = TimeInterval::new(start_time, start_time + dist_int.length() / self.speed);
        PedState::Crossing(dist_int, time_int)
    }

    fn get_dist_along(&self, now: Duration, map: &Map) -> Distance {
        match self.state {
            PedState::Crossing(ref dist_int, ref time_int) => dist_int.lerp(time_int.percent(now)),
            PedState::WaitingToTurn(dist) => dist,
            PedState::LeavingBuilding(b, _) => map.get_b(b).front_path.sidewalk.dist_along(),
            PedState::EnteringBuilding(b, _) => map.get_b(b).front_path.sidewalk.dist_along(),
            PedState::StartingToBike(ref spot, _, _) => spot.sidewalk_pos.dist_along(),
            PedState::FinishingBiking(ref spot, _, _) => spot.sidewalk_pos.dist_along(),
            PedState::WaitingForBus => self.goal.sidewalk_pos.dist_along(),
        }
    }

    fn get_draw_ped(&self, now: Duration, map: &Map) -> DrawPedestrianInput {
        let on = self.path.current_step().as_traversable();
        let (pos, facing) = match self.state {
            PedState::Crossing(ref dist_int, ref time_int) => {
                let percent = if now > time_int.end {
                    1.0
                } else {
                    time_int.percent(now)
                };
                let (pos, orig_angle) = on.dist_along(dist_int.lerp(percent), map);
                let facing = if dist_int.start < dist_int.end {
                    orig_angle
                } else {
                    orig_angle.opposite()
                };
                (
                    pos.project_away(LANE_THICKNESS / 4.0, facing.rotate_degs(90.0)),
                    facing,
                )
            }
            PedState::WaitingToTurn(dist) => {
                let (pos, orig_angle) = on.dist_along(dist, map);
                let facing = if dist == Distance::ZERO {
                    orig_angle.opposite()
                } else {
                    orig_angle
                };
                (
                    pos.project_away(LANE_THICKNESS / 4.0, facing.rotate_degs(90.0)),
                    facing,
                )
            }
            PedState::LeavingBuilding(b, ref time_int) => {
                let front_path = &map.get_b(b).front_path;
                (
                    front_path
                        .line
                        .dist_along(time_int.percent(now) * front_path.line.length())
                        .project_away(
                            LANE_THICKNESS / 4.0,
                            front_path.line.angle().rotate_degs(90.0),
                        ),
                    front_path.line.angle(),
                )
            }
            PedState::EnteringBuilding(b, ref time_int) => {
                let front_path = &map.get_b(b).front_path;
                (
                    front_path
                        .line
                        .reverse()
                        .dist_along(time_int.percent(now) * front_path.line.length())
                        .project_away(
                            LANE_THICKNESS / 4.0,
                            front_path.line.angle().rotate_degs(-90.0),
                        ),
                    front_path.line.angle().opposite(),
                )
            }
            PedState::StartingToBike(_, ref line, ref time_int) => {
                (line.percent_along(time_int.percent(now)), line.angle())
            }
            PedState::FinishingBiking(_, ref line, ref time_int) => {
                (line.percent_along(time_int.percent(now)), line.angle())
            }
            PedState::WaitingForBus => {
                let (pt, angle) = self.goal.sidewalk_pos.pt_and_angle(map);
                // Face the road
                (pt, angle.rotate_degs(90.0))
            }
        };

        DrawPedestrianInput {
            id: self.id,
            pos,
            facing,
            waiting_for_turn: match self.state {
                PedState::WaitingToTurn(_) => Some(self.path.next_step().as_turn()),
                _ => None,
            },
            preparing_bike: match self.state {
                PedState::StartingToBike(_, _, _) | PedState::FinishingBiking(_, _, _) => true,
                _ => false,
            },
            on,
            time_spent_blocked: self
                .blocked_since
                .map(|t| now - t)
                .unwrap_or(Duration::ZERO),
            percent_dist_crossed: self.path.percent_dist_crossed(),
            trip_time_so_far: now - self.started_at,
        }
    }

    // True if we successfully continued to the next step of our path
    fn maybe_transition(
        &mut self,
        now: Duration,
        map: &Map,
        intersections: &mut IntersectionSimState,
        peds_per_traversable: &mut MultiMap<Traversable, PedestrianID>,
        scheduler: &mut Scheduler,
    ) -> bool {
        if let PathStep::Turn(t) = self.path.next_step() {
            if !intersections.maybe_start_turn(
                AgentID::Pedestrian(self.id),
                t,
                self.speed,
                now,
                map,
                scheduler,
                None,
            ) {
                return false;
            }
        }

        peds_per_traversable.remove(self.path.current_step().as_traversable(), self.id);
        self.path.shift(map);
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

// The crowds returned here may have low/high values extending up to radius past the real geometry.
fn find_crowds(
    input: Vec<(PedestrianID, Distance)>,
    on: Traversable,
) -> (Vec<PedestrianID>, Vec<DrawPedCrowdInput>) {
    let mut loners = Vec::new();
    let mut crowds = Vec::new();
    let radius = LANE_THICKNESS / 4.0;

    let mut current_crowd = DrawPedCrowdInput {
        low: input[0].1 - radius,
        high: input[0].1 + radius,
        contraflow: false,
        members: vec![input[0].0],
        on,
    };
    for (id, dist) in input.into_iter().skip(1) {
        // If the pedestrian circles would overlap at all,
        if dist - radius <= current_crowd.high {
            current_crowd.members.push(id);
            current_crowd.high = dist + radius;
        } else {
            if current_crowd.members.len() == 1 {
                loners.push(current_crowd.members[0]);
            } else {
                crowds.push(current_crowd);
            }
            // Reset current_crowd
            current_crowd = DrawPedCrowdInput {
                low: dist - radius,
                high: dist + radius,
                contraflow: false,
                members: vec![id],
                on,
            };
        }
    }
    // Handle the last bit
    if current_crowd.members.len() == 1 {
        loners.push(current_crowd.members[0]);
    } else {
        crowds.push(current_crowd);
    }

    (loners, crowds)
}
