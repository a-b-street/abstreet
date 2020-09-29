use crate::sim::Ctx;
use crate::{
    AgentID, AgentProperties, Command, CreatePedestrian, DistanceInterval, DrawPedCrowdInput,
    DrawPedestrianInput, Event, IntersectionSimState, ParkedCar, ParkingSpot, PedCrowdLocation,
    PedestrianID, PersonID, Scheduler, SidewalkPOI, SidewalkSpot, TimeInterval, TransitSimState,
    TripID, TripManager, UnzoomedAgent,
};
use abstutil::{deserialize_multimap, serialize_multimap, MultiMap};
use geom::{Distance, Duration, Line, PolyLine, Speed, Time};
use map_model::{
    BuildingID, BusRouteID, DrivingSide, Map, ParkingLotID, Path, PathStep, Traversable,
    SIDEWALK_THICKNESS,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const TIME_TO_START_BIKING: Duration = Duration::const_seconds(30.0);
const TIME_TO_FINISH_BIKING: Duration = Duration::const_seconds(45.0);

#[derive(Serialize, Deserialize, Clone)]
pub struct WalkingSimState {
    // BTreeMap not for deterministic simulation, but to make serialized things easier to compare.
    peds: BTreeMap<PedestrianID, Pedestrian>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
    events: Vec<Event>,
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds: BTreeMap::new(),
            peds_per_traversable: MultiMap::new(),
            events: Vec::new(),
        }
    }

    pub fn spawn_ped(
        &mut self,
        now: Time,
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
                TimeInterval::new(
                    Time::START_OF_DAY,
                    Time::START_OF_DAY + Duration::seconds(1.0),
                ),
            ),
            speed: params.speed,
            total_blocked_time: Duration::ZERO,
            started_at: now,
            path: params.path,
            goal: params.goal,
            trip: params.trip,
            person: params.person,
        };
        ped.state = match params.start.connection {
            SidewalkPOI::Building(b) | SidewalkPOI::ParkingSpot(ParkingSpot::Offstreet(b, _)) => {
                PedState::LeavingBuilding(
                    b,
                    TimeInterval::new(now, now + map.get_b(b).driveway_geom.length() / ped.speed),
                )
            }
            SidewalkPOI::ParkingSpot(ParkingSpot::Lot(pl, _)) => PedState::LeavingParkingLot(
                pl,
                TimeInterval::new(now, now + map.get_pl(pl).sidewalk_line.length() / ped.speed),
            ),
            SidewalkPOI::BikeRack(driving_pos) => PedState::FinishingBiking(
                params.start.clone(),
                Line::must_new(driving_pos.pt(map), params.start.sidewalk_pos.pt(map)),
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

    pub fn get_draw_ped(
        &self,
        id: PedestrianID,
        now: Time,
        map: &Map,
    ) -> Option<DrawPedestrianInput> {
        self.peds.get(&id).map(|p| p.get_draw_ped(now, map))
    }

    pub fn get_all_draw_peds(&self, now: Time, map: &Map) -> Vec<DrawPedestrianInput> {
        self.peds
            .values()
            .map(|p| p.get_draw_ped(now, map))
            .collect()
    }

    pub fn update_ped(
        &mut self,
        id: PedestrianID,
        now: Time,
        ctx: &mut Ctx,
        trips: &mut TripManager,
        transit: &mut TransitSimState,
    ) {
        let mut ped = self.peds.get_mut(&id).unwrap();
        match ped.state {
            PedState::Crossing(ref dist_int, _) => {
                if ped.path.is_last_step() {
                    match ped.goal.connection {
                        SidewalkPOI::ParkingSpot(spot) => {
                            if let ParkingSpot::Lot(pl, _) = spot {
                                ped.state = PedState::EnteringParkingLot(
                                    pl,
                                    TimeInterval::new(
                                        now,
                                        now + ctx.map.get_pl(pl).sidewalk_line.length() / ped.speed,
                                    ),
                                );
                                ctx.scheduler
                                    .push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                            } else {
                                self.peds_per_traversable
                                    .remove(ped.path.current_step().as_traversable(), ped.id);
                                trips.ped_reached_parking_spot(
                                    now,
                                    ped.id,
                                    spot,
                                    ped.total_blocked_time,
                                    ctx,
                                );
                                self.peds.remove(&id);
                            }
                        }
                        SidewalkPOI::Building(b) => {
                            ped.state = PedState::EnteringBuilding(
                                b,
                                TimeInterval::new(
                                    now,
                                    now + ctx.map.get_b(b).driveway_geom.length() / ped.speed,
                                ),
                            );
                            ctx.scheduler
                                .push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                        }
                        SidewalkPOI::BusStop(stop) => {
                            if let Some(route) = trips.ped_reached_bus_stop(
                                now,
                                ped.id,
                                stop,
                                ped.total_blocked_time,
                                ctx,
                                transit,
                            ) {
                                ped.state = PedState::WaitingForBus(route, now);
                            } else {
                                self.peds_per_traversable
                                    .remove(ped.path.current_step().as_traversable(), ped.id);
                                self.peds.remove(&id);
                            }
                        }
                        SidewalkPOI::Border(i, _) => {
                            self.peds_per_traversable
                                .remove(ped.path.current_step().as_traversable(), ped.id);
                            trips.ped_reached_border(now, ped.id, i, ped.total_blocked_time, ctx);
                            self.peds.remove(&id);
                        }
                        SidewalkPOI::BikeRack(driving_pos) => {
                            let pt1 = ped.goal.sidewalk_pos.pt(ctx.map);
                            let pt2 = driving_pos.pt(ctx.map);
                            ped.state = PedState::StartingToBike(
                                ped.goal.clone(),
                                Line::must_new(pt1, pt2),
                                TimeInterval::new(now, now + TIME_TO_START_BIKING),
                            );
                            ctx.scheduler
                                .push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                        }
                        SidewalkPOI::SuddenlyAppear => unreachable!(),
                        SidewalkPOI::DeferredParkingSpot => unreachable!(),
                    }
                } else {
                    if let PathStep::Turn(t) = ped.path.current_step() {
                        ctx.intersections.turn_finished(
                            now,
                            AgentID::Pedestrian(ped.id),
                            t,
                            ctx.scheduler,
                            ctx.map,
                        );
                    }

                    let dist = dist_int.end;
                    if ped.maybe_transition(
                        now,
                        ctx.map,
                        ctx.intersections,
                        &mut self.peds_per_traversable,
                        &mut self.events,
                        ctx.scheduler,
                    ) {
                        ctx.scheduler
                            .push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                    } else {
                        // Must've failed because we can't turn yet. Don't schedule a retry here.
                        ped.state = PedState::WaitingToTurn(dist, now);
                    }
                }
            }
            PedState::WaitingToTurn(_, blocked_since) => {
                if ped.maybe_transition(
                    now,
                    ctx.map,
                    ctx.intersections,
                    &mut self.peds_per_traversable,
                    &mut self.events,
                    ctx.scheduler,
                ) {
                    ctx.scheduler
                        .push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
                    ped.total_blocked_time += now - blocked_since;
                }
            }
            PedState::LeavingBuilding(b, _) => {
                ped.state =
                    ped.crossing_state(ctx.map.get_b(b).sidewalk_pos.dist_along(), now, ctx.map);
                ctx.scheduler
                    .push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
            }
            PedState::EnteringBuilding(bldg, _) => {
                self.peds_per_traversable
                    .remove(ped.path.current_step().as_traversable(), ped.id);
                trips.ped_reached_building(now, ped.id, bldg, ped.total_blocked_time, ctx);
                self.peds.remove(&id);
            }
            PedState::LeavingParkingLot(pl, _) => {
                ped.state =
                    ped.crossing_state(ctx.map.get_pl(pl).sidewalk_pos.dist_along(), now, ctx.map);
                ctx.scheduler
                    .push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
            }
            PedState::EnteringParkingLot(_, _) => {
                self.peds_per_traversable
                    .remove(ped.path.current_step().as_traversable(), ped.id);
                trips.ped_reached_parking_spot(
                    now,
                    ped.id,
                    match ped.goal.connection {
                        SidewalkPOI::ParkingSpot(spot) => spot,
                        _ => unreachable!(),
                    },
                    ped.total_blocked_time,
                    ctx,
                );
                self.peds.remove(&id);
            }
            PedState::StartingToBike(ref spot, _, _) => {
                self.peds_per_traversable
                    .remove(ped.path.current_step().as_traversable(), ped.id);
                trips.ped_ready_to_bike(now, ped.id, spot.clone(), ped.total_blocked_time, ctx);
                self.peds.remove(&id);
            }
            PedState::FinishingBiking(ref spot, _, _) => {
                ped.state = ped.crossing_state(spot.sidewalk_pos.dist_along(), now, ctx.map);
                ctx.scheduler
                    .push(ped.state.get_end_time(), Command::UpdatePed(ped.id));
            }
            PedState::WaitingForBus(_, _) => unreachable!(),
        }
    }

    pub fn ped_boarded_bus(&mut self, now: Time, id: PedestrianID) {
        let mut ped = self.peds.remove(&id).unwrap();
        match ped.state {
            PedState::WaitingForBus(_, blocked_since) => {
                self.peds_per_traversable
                    .remove(ped.path.current_step().as_traversable(), id);
                ped.total_blocked_time += now - blocked_since;
            }
            _ => unreachable!(),
        };
    }

    pub fn delete_ped(&mut self, id: PedestrianID, scheduler: &mut Scheduler) {
        let ped = self.peds.remove(&id).unwrap();
        self.peds_per_traversable
            .remove(ped.path.current_step().as_traversable(), id);
        scheduler.cancel(Command::UpdatePed(id));
    }

    pub fn debug_ped(&self, id: PedestrianID) {
        if let Some(ped) = self.peds.get(&id) {
            println!("{}", abstutil::to_json(ped));
        } else {
            println!("{} doesn't exist", id);
        }
    }

    pub fn agent_properties(&self, id: PedestrianID, now: Time) -> AgentProperties {
        let p = &self.peds[&id];

        let time_spent_waiting = p.state.time_spent_waiting(now);
        // TODO Incorporate this somewhere
        /*if let PedState::WaitingForBus(r, _) = p.state {
            extra.push(format!("Waiting for bus {}", map.get_br(r).name));
        }*/

        AgentProperties {
            total_time: now - p.started_at,
            waiting_here: time_spent_waiting,
            total_waiting: p.total_blocked_time + time_spent_waiting,
            dist_crossed: p.path.crossed_so_far(),
            total_dist: p.path.total_length(),
            lanes_crossed: p.path.lanes_crossed_so_far(),
            total_lanes: p.path.total_lanes(),
        }
    }

    pub fn trace_route(
        &self,
        now: Time,
        id: PedestrianID,
        map: &Map,
        dist_ahead: Option<Distance>,
    ) -> Option<PolyLine> {
        let p = self.peds.get(&id)?;
        let body_radius = SIDEWALK_THICKNESS / 4.0;
        let dist = (p.get_dist_along(now, map) + body_radius)
            .min(p.path.current_step().as_traversable().length(map));
        p.path.trace(map, dist, dist_ahead)
    }

    pub fn get_path(&self, id: PedestrianID) -> Option<&Path> {
        let p = self.peds.get(&id)?;
        Some(&p.path)
    }

    pub fn get_unzoomed_agents(&self, now: Time, map: &Map) -> Vec<UnzoomedAgent> {
        let mut peds = Vec::new();

        for ped in self.peds.values() {
            peds.push(UnzoomedAgent {
                vehicle_type: None,
                pos: ped.get_draw_ped(now, map).pos,
                person: Some(ped.person),
                parking: false,
            });
        }

        peds
    }

    pub fn does_ped_exist(&self, id: PedestrianID) -> bool {
        self.peds.contains_key(&id)
    }

    pub fn get_draw_peds_on(
        &self,
        now: Time,
        on: Traversable,
        map: &Map,
    ) -> (Vec<DrawPedestrianInput>, Vec<DrawPedCrowdInput>) {
        // Classify into direction-based groups or by building/parking lot driveway.
        let mut forwards: Vec<(PedestrianID, Distance)> = Vec::new();
        let mut backwards: Vec<(PedestrianID, Distance)> = Vec::new();
        let mut bldg_driveway: MultiMap<BuildingID, (PedestrianID, Distance)> = MultiMap::new();
        let mut lot_driveway: MultiMap<ParkingLotID, (PedestrianID, Distance)> = MultiMap::new();

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
                PedState::WaitingToTurn(dist, _) => {
                    if dist == Distance::ZERO {
                        backwards.push((*id, dist));
                    } else {
                        forwards.push((*id, dist));
                    }
                }
                PedState::LeavingBuilding(b, ref int) => {
                    let len = map.get_b(b).driveway_geom.length();
                    bldg_driveway.insert(b, (*id, int.percent(now) * len));
                }
                PedState::EnteringBuilding(b, ref int) => {
                    let len = map.get_b(b).driveway_geom.length();
                    bldg_driveway.insert(b, (*id, (1.0 - int.percent(now)) * len));
                }
                PedState::LeavingParkingLot(pl, ref int) => {
                    let len = map.get_pl(pl).sidewalk_line.length();
                    lot_driveway.insert(pl, (*id, int.percent(now) * len));
                }
                PedState::EnteringParkingLot(pl, ref int) => {
                    let len = map.get_pl(pl).sidewalk_line.length();
                    lot_driveway.insert(pl, (*id, (1.0 - int.percent(now)) * len));
                }
                PedState::StartingToBike(_, _, _)
                | PedState::FinishingBiking(_, _, _)
                | PedState::WaitingForBus(_, _) => {
                    // The backwards half of the sidewalk is closer to the road.
                    backwards.push((*id, dist));
                }
            }
        }

        let mut crowds: Vec<DrawPedCrowdInput> = Vec::new();
        let mut loners: Vec<DrawPedestrianInput> = Vec::new();

        // For each group, sort by distance along. Attempt to bundle into intervals.
        for (mut group, location, on_len) in vec![
            (
                forwards,
                PedCrowdLocation::Sidewalk(on, false),
                on.length(map),
            ),
            (
                backwards,
                PedCrowdLocation::Sidewalk(on, true),
                on.length(map),
            ),
        ]
        .into_iter()
        .chain(bldg_driveway.consume().into_iter().map(|(b, set)| {
            (
                set.into_iter().collect::<Vec<_>>(),
                PedCrowdLocation::BldgDriveway(b),
                map.get_b(b).driveway_geom.length(),
            )
        }))
        .chain(lot_driveway.consume().into_iter().map(|(pl, set)| {
            (
                set.into_iter().collect::<Vec<_>>(),
                PedCrowdLocation::LotDriveway(pl),
                map.get_pl(pl).sidewalk_line.length(),
            )
        })) {
            if group.is_empty() {
                continue;
            }
            group.sort_by_key(|(_, dist)| *dist);
            let (individs, these_crowds) = find_crowds(group, location);
            for id in individs {
                loners.push(self.peds[&id].get_draw_ped(now, map));
            }
            for mut crowd in these_crowds {
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

    pub fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }

    pub fn find_trips_to_parking(&self, evicted_cars: Vec<ParkedCar>) -> Vec<(AgentID, TripID)> {
        let goals: BTreeSet<SidewalkPOI> = evicted_cars
            .into_iter()
            .map(|p| SidewalkPOI::ParkingSpot(p.spot))
            .collect();
        let mut affected = Vec::new();
        for ped in self.peds.values() {
            if goals.contains(&ped.goal.connection) {
                affected.push((AgentID::Pedestrian(ped.id), ped.trip));
            }
        }
        affected
    }

    pub fn all_waiting_people(&self, now: Time, delays: &mut BTreeMap<PersonID, Duration>) {
        for p in self.peds.values() {
            let delay = p.state.time_spent_waiting(now);
            if delay > Duration::ZERO {
                delays.insert(p.person, delay);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Pedestrian {
    id: PedestrianID,
    state: PedState,
    speed: Speed,
    total_blocked_time: Duration,
    // TODO organize analytics better.
    started_at: Time,

    path: Path,
    goal: SidewalkSpot,
    trip: TripID,
    person: PersonID,
}

impl Pedestrian {
    fn crossing_state(&self, start_dist: Distance, start_time: Time, map: &Map) -> PedState {
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

    fn get_dist_along(&self, now: Time, map: &Map) -> Distance {
        match self.state {
            PedState::Crossing(ref dist_int, ref time_int) => dist_int.lerp(time_int.percent(now)),
            PedState::WaitingToTurn(dist, _) => dist,
            PedState::LeavingBuilding(b, _) | PedState::EnteringBuilding(b, _) => {
                map.get_b(b).sidewalk_pos.dist_along()
            }
            PedState::LeavingParkingLot(pl, _) | PedState::EnteringParkingLot(pl, _) => {
                map.get_pl(pl).sidewalk_pos.dist_along()
            }
            PedState::StartingToBike(ref spot, _, _) => spot.sidewalk_pos.dist_along(),
            PedState::FinishingBiking(ref spot, _, _) => spot.sidewalk_pos.dist_along(),
            PedState::WaitingForBus(_, _) => self.goal.sidewalk_pos.dist_along(),
        }
    }

    fn get_draw_ped(&self, now: Time, map: &Map) -> DrawPedestrianInput {
        let on = self.path.current_step().as_traversable();
        let err = format!("at {}, {}'s position is broken", now, self.id);
        let angle_offset = if map.get_config().driving_side == DrivingSide::Right {
            90.0
        } else {
            270.0
        };
        let (pos, facing) = match self.state {
            PedState::Crossing(ref dist_int, ref time_int) => {
                let percent = if now > time_int.end {
                    1.0
                } else {
                    time_int.percent(now)
                };
                let (pos, orig_angle) = on.dist_along(dist_int.lerp(percent), map).expect(&err);
                let facing = if dist_int.start < dist_int.end {
                    orig_angle
                } else {
                    orig_angle.opposite()
                };
                (
                    pos.project_away(SIDEWALK_THICKNESS / 4.0, facing.rotate_degs(angle_offset)),
                    facing,
                )
            }
            PedState::WaitingToTurn(dist, _) => {
                let (pos, orig_angle) = on.dist_along(dist, map).expect(&err);
                let facing = if dist == Distance::ZERO {
                    orig_angle.opposite()
                } else {
                    orig_angle
                };
                (
                    pos.project_away(SIDEWALK_THICKNESS / 4.0, facing.rotate_degs(angle_offset)),
                    facing,
                )
            }
            PedState::LeavingBuilding(b, ref time_int) => {
                let pl = &map.get_b(b).driveway_geom;
                // If we're on some tiny line and percent_along fails, just fall back to to some
                // point on the line instead of crashing.
                if let Ok(pair) = pl.dist_along(time_int.percent(now) * pl.length()) {
                    pair
                } else {
                    (pl.first_pt(), pl.first_line().angle())
                }
            }
            PedState::EnteringBuilding(b, ref time_int) => {
                let pl = &map.get_b(b).driveway_geom;
                if let Ok((pt, angle)) = pl.dist_along((1.0 - time_int.percent(now)) * pl.length())
                {
                    (pt, angle.opposite())
                } else {
                    (pl.first_pt(), pl.first_line().angle().opposite())
                }
            }
            PedState::LeavingParkingLot(pl, ref time_int) => {
                let line = &map.get_pl(pl).sidewalk_line;
                (
                    line.percent_along(time_int.percent(now))
                        .unwrap_or(line.pt1()),
                    line.angle(),
                )
            }
            PedState::EnteringParkingLot(pl, ref time_int) => {
                let line = &map.get_pl(pl).sidewalk_line;
                (
                    line.reverse()
                        .percent_along(time_int.percent(now))
                        .unwrap_or(line.pt1()),
                    line.angle().opposite(),
                )
            }
            PedState::StartingToBike(_, ref line, ref time_int) => (
                line.percent_along(time_int.percent(now))
                    .unwrap_or(line.pt1()),
                line.angle(),
            ),
            PedState::FinishingBiking(_, ref line, ref time_int) => (
                line.percent_along(time_int.percent(now))
                    .unwrap_or(line.pt1()),
                line.angle(),
            ),
            PedState::WaitingForBus(_, _) => {
                let (pt, angle) = self.goal.sidewalk_pos.pt_and_angle(map);
                // Stand on the far side of the sidewalk (by the bus stop), facing the road
                (
                    pt.project_away(SIDEWALK_THICKNESS / 4.0, angle.rotate_degs(angle_offset)),
                    angle.rotate_degs(-angle_offset),
                )
            }
        };

        DrawPedestrianInput {
            id: self.id,
            pos,
            facing,
            waiting_for_turn: match self.state {
                PedState::WaitingToTurn(_, _) => Some(self.path.next_step().as_turn()),
                _ => None,
            },
            preparing_bike: matches!(self.state, PedState::StartingToBike(_, _, _) | PedState::FinishingBiking(_, _, _)),
            waiting_for_bus: matches!(self.state, PedState::WaitingForBus(_, _)),
            on,
        }
    }

    // True if we successfully continued to the next step of our path
    fn maybe_transition(
        &mut self,
        now: Time,
        map: &Map,
        intersections: &mut IntersectionSimState,
        peds_per_traversable: &mut MultiMap<Traversable, PedestrianID>,
        events: &mut Vec<Event>,
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
        events.push(Event::AgentEntersTraversable(
            AgentID::Pedestrian(self.id),
            self.path.current_step().as_traversable(),
            None,
        ));
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum PedState {
    Crossing(DistanceInterval, TimeInterval),
    // The Distance is either 0 or the current traversable's length. The Time is blocked_since.
    WaitingToTurn(Distance, Time),
    LeavingBuilding(BuildingID, TimeInterval),
    EnteringBuilding(BuildingID, TimeInterval),
    LeavingParkingLot(ParkingLotID, TimeInterval),
    EnteringParkingLot(ParkingLotID, TimeInterval),
    StartingToBike(SidewalkSpot, Line, TimeInterval),
    FinishingBiking(SidewalkSpot, Line, TimeInterval),
    WaitingForBus(BusRouteID, Time),
}

impl PedState {
    fn get_end_time(&self) -> Time {
        match self {
            PedState::Crossing(_, ref time_int) => time_int.end,
            PedState::WaitingToTurn(_, _) => unreachable!(),
            PedState::LeavingBuilding(_, ref time_int) => time_int.end,
            PedState::EnteringBuilding(_, ref time_int) => time_int.end,
            PedState::LeavingParkingLot(_, ref time_int) => time_int.end,
            PedState::EnteringParkingLot(_, ref time_int) => time_int.end,
            PedState::StartingToBike(_, _, ref time_int) => time_int.end,
            PedState::FinishingBiking(_, _, ref time_int) => time_int.end,
            PedState::WaitingForBus(_, _) => unreachable!(),
        }
    }

    fn time_spent_waiting(&self, now: Time) -> Duration {
        match self {
            PedState::WaitingToTurn(_, blocked_since)
            | PedState::WaitingForBus(_, blocked_since) => now - *blocked_since,
            _ => Duration::ZERO,
        }
    }
}

// The crowds returned here may have low/high values extending up to radius past the real geometry.
fn find_crowds(
    input: Vec<(PedestrianID, Distance)>,
    location: PedCrowdLocation,
) -> (Vec<PedestrianID>, Vec<DrawPedCrowdInput>) {
    let mut loners = Vec::new();
    let mut crowds = Vec::new();
    let radius = SIDEWALK_THICKNESS / 4.0;

    let mut current_crowd = DrawPedCrowdInput {
        low: input[0].1 - radius,
        high: input[0].1 + radius,
        members: vec![input[0].0],
        location: location.clone(),
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
                members: vec![id],
                location: location.clone(),
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
