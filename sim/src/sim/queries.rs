//! All sorts of read-only queries about a simulation

use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;

use abstutil::Counter;
use geom::{Distance, Duration, PolyLine, Pt2D, Time};
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, Lane, LaneID, Map, Path, PathConstraints,
    Position, Traversable, TurnID,
};

use crate::analytics::Window;
use crate::{
    AgentID, AgentType, Analytics, CarID, CommutersVehiclesCounts, DrawCarInput, DrawPedCrowdInput,
    DrawPedestrianInput, OrigPersonID, PandemicModel, ParkedCar, ParkingSim, PedestrianID, Person,
    PersonID, PersonState, Scenario, Sim, TripEndpoint, TripID, TripInfo, TripMode, TripResult,
    UnzoomedAgent, VehicleType,
};

// TODO Many of these just delegate to an inner piece. This is unorganized and hard to maintain.
impl Sim {
    pub fn time(&self) -> Time {
        self.time
    }

    pub fn is_done(&self) -> bool {
        self.trips.is_done()
    }

    pub fn is_empty(&self) -> bool {
        self.time == Time::START_OF_DAY && self.is_done()
    }

    /// (number of finished trips, number of unfinished trips)
    pub fn num_trips(&self) -> (usize, usize) {
        self.trips.num_trips()
    }
    pub fn num_agents(&self) -> Counter<AgentType> {
        self.trips.num_agents(&self.transit)
    }
    pub fn num_commuters_vehicles(&self) -> CommutersVehiclesCounts {
        self.trips
            .num_commuters_vehicles(&self.transit, &self.walking)
    }
    /// (total number of people, just in buildings, just off map)
    pub fn num_ppl(&self) -> (usize, usize, usize) {
        self.trips.num_ppl()
    }

    pub fn debug_ped(&self, id: PedestrianID) {
        self.walking.debug_ped(id);
        self.trips.debug_trip(AgentID::Pedestrian(id));
    }

    pub fn debug_car(&self, id: CarID) {
        self.driving.debug_car(id);
        self.trips.debug_trip(AgentID::Car(id));
    }

    pub fn debug_intersection(&self, id: IntersectionID, map: &Map) {
        self.intersections.debug(id, map);
    }

    pub fn debug_lane(&self, id: LaneID) {
        self.driving.debug_lane(id);
    }

    /// Only call for active agents, will panic otherwise
    pub fn agent_properties(&self, map: &Map, id: AgentID) -> AgentProperties {
        match id {
            AgentID::Pedestrian(id) => self.walking.agent_properties(map, id, self.time),
            AgentID::Car(id) => self.driving.agent_properties(id, self.time),
            // TODO Harder to measure some of this stuff
            AgentID::BusPassenger(_, _) => AgentProperties {
                total_time: Duration::ZERO,
                waiting_here: Duration::ZERO,
                total_waiting: Duration::ZERO,
                dist_crossed: Distance::ZERO,
                total_dist: Distance::meters(0.1),
            },
        }
    }

    pub fn num_transit_passengers(&self, car: CarID) -> usize {
        self.transit.get_passengers(car).len()
    }

    pub fn bus_route_id(&self, maybe_bus: CarID) -> Option<BusRouteID> {
        if maybe_bus.1 == VehicleType::Bus || maybe_bus.1 == VehicleType::Train {
            Some(self.transit.bus_route(maybe_bus))
        } else {
            None
        }
    }

    pub fn active_agents(&self) -> Vec<AgentID> {
        self.trips.active_agents()
    }
    pub fn num_active_agents(&self) -> usize {
        self.trips.num_active_agents()
    }

    pub fn agent_to_trip(&self, id: AgentID) -> Option<TripID> {
        self.trips.agent_to_trip(id)
    }

    pub fn trip_to_agent(&self, id: TripID) -> TripResult<AgentID> {
        self.trips.trip_to_agent(id)
    }

    pub fn trip_info(&self, id: TripID) -> TripInfo {
        self.trips.trip_info(id)
    }
    pub fn all_trip_info(&self) -> Vec<(TripID, TripInfo)> {
        self.trips.all_trip_info()
    }
    /// If trip is finished, returns (total time, total waiting time, total distance)
    pub fn finished_trip_details(&self, id: TripID) -> Option<(Duration, Duration, Distance)> {
        self.trips.finished_trip_details(id)
    }
    // Returns the total time a trip was blocked for
    pub fn trip_blocked_time(&self, id: TripID) -> Duration {
        self.trips.trip_blocked_time(id)
    }

    pub fn trip_to_person(&self, id: TripID) -> Option<PersonID> {
        self.trips.trip_to_person(id)
    }
    // TODO This returns None for parked cars owned by people! That's confusing. Dedupe with
    // get_owner_of_car.
    pub fn agent_to_person(&self, id: AgentID) -> Option<PersonID> {
        self.agent_to_trip(id)
            .map(|t| self.trip_to_person(t).unwrap())
    }
    pub fn person_to_agent(&self, id: PersonID) -> Option<AgentID> {
        if let PersonState::Trip(t) = self.trips.get_person(id)?.state {
            self.trip_to_agent(t).ok()
        } else {
            None
        }
    }
    pub fn get_owner_of_car(&self, id: CarID) -> Option<PersonID> {
        self.driving
            .get_owner_of_car(id)
            .or_else(|| self.parking.get_owner_of_car(id))
    }
    pub fn lookup_parked_car(&self, id: CarID) -> Option<&ParkedCar> {
        self.parking.lookup_parked_car(id)
    }
    /// For every parked car, (position of parking spot, position of owner)
    pub fn all_parked_car_positions(&self, map: &Map) -> Vec<(Position, Position)> {
        self.parking
            .all_parked_car_positions(map)
            .into_iter()
            .filter_map(|(car_pos, owner)| {
                // TODO Should include people off-map and in the middle of a non-car trip too
                match self.trips.get_person(owner)?.state {
                    PersonState::Inside(b) => Some((car_pos, map.get_b(b).sidewalk_pos)),
                    PersonState::Trip(_) => None,
                    PersonState::OffMap => None,
                }
            })
            .collect()
    }

    pub fn lookup_person(&self, id: PersonID) -> Option<&Person> {
        self.trips.get_person(id)
    }
    pub fn get_person(&self, id: PersonID) -> &Person {
        self.trips.get_person(id).unwrap()
    }
    pub fn find_person_by_orig_id(&self, id: OrigPersonID) -> Option<PersonID> {
        for p in self.get_all_people() {
            if p.orig_id == Some(id) {
                return Some(p.id);
            }
        }
        None
    }
    pub fn get_all_people(&self) -> &Vec<Person> {
        self.trips.get_all_people()
    }

    pub fn lookup_car_id(&self, idx: usize) -> Option<CarID> {
        for vt in &[
            VehicleType::Car,
            VehicleType::Bike,
            VehicleType::Bus,
            VehicleType::Train,
        ] {
            let id = CarID(idx, *vt);
            if self.driving.does_car_exist(id) {
                return Some(id);
            }
        }

        let id = CarID(idx, VehicleType::Car);
        // Only cars can be parked.
        if self.parking.lookup_parked_car(id).is_some() {
            return Some(id);
        }

        None
    }

    pub fn get_path(&self, id: AgentID) -> Option<&Path> {
        match id {
            AgentID::Car(car) => self.driving.get_path(car),
            AgentID::Pedestrian(ped) => self.walking.get_path(ped),
            AgentID::BusPassenger(_, _) => None,
        }
    }
    pub fn get_all_driving_paths(&self) -> Vec<&Path> {
        self.driving.get_all_driving_paths()
    }

    pub fn trace_route(&self, id: AgentID, map: &Map) -> Option<PolyLine> {
        match id {
            AgentID::Car(car) => self.driving.trace_route(self.time, car, map),
            AgentID::Pedestrian(ped) => self.walking.trace_route(self.time, ped, map),
            AgentID::BusPassenger(_, _) => None,
        }
    }

    pub fn get_canonical_pt_per_trip(&self, trip: TripID, map: &Map) -> TripResult<Pt2D> {
        let agent = match self.trips.trip_to_agent(trip) {
            TripResult::Ok(a) => a,
            x => {
                return x.propagate_error();
            }
        };
        if let Some(pt) = self.canonical_pt_for_agent(agent, map) {
            return TripResult::Ok(pt);
        }
        TripResult::ModeChange
    }
    pub fn get_canonical_pt_per_person(&self, p: PersonID, map: &Map) -> Option<Pt2D> {
        match self.trips.get_person(p)?.state {
            PersonState::Inside(b) => Some(map.get_b(b).polygon.center()),
            PersonState::Trip(t) => self.get_canonical_pt_per_trip(t, map).ok(),
            PersonState::OffMap => None,
        }
    }

    pub fn canonical_pt_for_agent(&self, id: AgentID, map: &Map) -> Option<Pt2D> {
        match id {
            AgentID::Car(id) => self
                .parking
                .canonical_pt(id, map)
                .or_else(|| Some(self.get_draw_car(id, map)?.body.last_pt())),
            AgentID::Pedestrian(id) => Some(self.get_draw_ped(id, map)?.pos),
            AgentID::BusPassenger(_, bus) => Some(self.get_draw_car(bus, map)?.body.last_pt()),
        }
    }

    pub fn get_accepted_agents(&self, id: IntersectionID) -> Vec<(AgentID, TurnID)> {
        self.intersections.get_accepted_agents(id)
    }
    pub fn get_waiting_agents(&self, id: IntersectionID) -> Vec<(AgentID, TurnID, Time)> {
        self.intersections.get_waiting_agents(id)
    }

    /// For every agent that's currently not moving, figure out how long they've been waiting and
    /// why they're blocked.
    pub fn get_blocked_by_graph(&self, map: &Map) -> BTreeMap<AgentID, (Duration, DelayCause)> {
        // Pedestrians can only be blocked at intersections, which is handled inside this call
        self.driving
            .get_blocked_by_graph(self.time, map, &self.intersections)
    }

    /// (bus, stop index it's coming from, percent to next stop, location)
    pub fn status_of_buses(
        &self,
        route: BusRouteID,
        map: &Map,
    ) -> Vec<(CarID, Option<usize>, f64, Pt2D)> {
        let mut results = Vec::new();
        for (bus, stop_idx) in self.transit.buses_for_route(route) {
            results.push((
                bus,
                stop_idx,
                self.driving.percent_along_route(bus),
                self.canonical_pt_for_agent(AgentID::Car(bus), map).unwrap(),
            ));
        }
        results
    }

    pub fn get_analytics(&self) -> &Analytics {
        &self.analytics
    }

    /// For intersections with an agent waiting beyond some threshold, return when they started
    /// waiting. Sorted by earliest waiting (likely the root cause of gridlock).
    pub fn delayed_intersections(&self, threshold: Duration) -> Vec<(IntersectionID, Time)> {
        self.intersections
            .delayed_intersections(self.time, threshold)
    }

    pub fn bldg_to_people(&self, b: BuildingID) -> Vec<PersonID> {
        self.trips.bldg_to_people(b)
    }

    pub fn get_pandemic_model(&self) -> Option<&PandemicModel> {
        self.pandemic.as_ref()
    }

    pub fn get_end_of_day(&self) -> Time {
        // Always count at least 24 hours
        self.scheduler
            .get_last_time()
            .max(Time::START_OF_DAY + Duration::hours(24))
    }

    pub fn current_stage_and_remaining_time(&self, i: IntersectionID) -> (usize, Duration) {
        self.intersections
            .current_stage_and_remaining_time(self.time, i)
    }

    // TODO This is an awkward copy of raw_throughput
    // TODO And it does NOT count buses/trains spawning
    pub fn all_arrivals_at_border(
        &self,
        i: IntersectionID,
    ) -> Vec<(AgentType, Vec<(Time, usize)>)> {
        let window_size = Duration::hours(1);
        let mut pts_per_type: BTreeMap<AgentType, Vec<(Time, usize)>> = BTreeMap::new();
        let mut windows_per_type: BTreeMap<AgentType, Window> = BTreeMap::new();
        for agent_type in AgentType::all() {
            pts_per_type.insert(agent_type, vec![(Time::START_OF_DAY, 0)]);
            windows_per_type.insert(agent_type, Window::new(window_size));
        }

        for (t, agent_type) in self.trips.all_arrivals_at_border(i) {
            let count = windows_per_type.get_mut(&agent_type).unwrap().add(t);
            pts_per_type.get_mut(&agent_type).unwrap().push((t, count));
        }

        for (agent_type, pts) in pts_per_type.iter_mut() {
            let mut window = windows_per_type.remove(agent_type).unwrap();

            // Add a drop-off after window_size (+ a little epsilon!)
            let end = self.get_end_of_day();
            let t = (pts.last().unwrap().0 + window_size + Duration::seconds(0.1)).min(end);
            if pts.last().unwrap().0 != t {
                pts.push((t, window.count(t)));
            }

            if pts.last().unwrap().0 != end {
                pts.push((end, window.count(end)));
            }
        }

        pts_per_type.into_iter().collect()
    }

    /// (number of vehicles in the lane, penalty if a bike or other slow vehicle is present)
    pub fn target_lane_penalty(&self, lane: &Lane) -> (usize, usize) {
        if lane.is_walkable() {
            (0, 0)
        } else {
            self.driving.target_lane_penalty(lane.id)
        }
    }

    pub fn get_people_waiting_at_stop(
        &self,
        at: BusStopID,
    ) -> &Vec<(PedestrianID, BusRouteID, Option<BusStopID>, Time)> {
        self.transit.get_people_waiting_at_stop(at)
    }

    pub fn generate_scenario(&self, map: &Map, name: String) -> Scenario {
        self.trips.generate_scenario(map, name)
    }

    pub fn get_cap_counter(&self, l: LaneID) -> usize {
        self.cap.get_cap_counter(l)
    }

    pub fn infinite_parking(&self) -> bool {
        self.parking.is_infinite()
    }

    pub fn all_waiting_people(&self) -> BTreeMap<PersonID, Duration> {
        let mut delays = BTreeMap::new();
        self.walking.all_waiting_people(self.time, &mut delays);
        self.driving.all_waiting_people(self.time, &mut delays);
        delays
    }

    pub fn describe_internal_stats(&self) -> Vec<String> {
        let mut stats = self.scheduler.describe_stats();
        stats.push(String::new());
        stats.extend(self.intersections.describe_stats());
        stats
    }

    pub fn debug_queue_lengths(&self, l: LaneID) -> Option<(Distance, Distance)> {
        self.driving.debug_queue_lengths(l)
    }

    /// Returns the best-case time for a trip in a world with no traffic or intersection delays.
    /// Might fail in some cases where the real trip succeeds, but the single-mode path can't be
    /// found. Assumes the TripID exists.
    pub fn get_trip_time_lower_bound(&self, map: &Map, id: TripID) -> Result<Duration> {
        let info = self.trips.trip_info(id);
        match TripEndpoint::path_req(info.start, info.end, info.mode, map) {
            Some(req) => {
                let path = map.pathfind(req)?;
                let person = self
                    .trips
                    .get_person(self.trips.trip_to_person(id).unwrap())
                    .unwrap();
                let mut constraints = info.mode.to_constraints();
                // TODO Fix TripMode.to_constraints
                if info.mode == TripMode::Transit {
                    constraints = PathConstraints::Pedestrian;
                }
                let max_speed = match info.mode {
                    TripMode::Walk | TripMode::Transit => Some(person.ped_speed),
                    // TODO We should really search the vehicles and grab it from there
                    TripMode::Drive => None,
                    // Assume just one bike
                    TripMode::Bike => {
                        person
                            .vehicles
                            .iter()
                            .find(|v| v.vehicle_type == VehicleType::Bike)
                            .unwrap()
                            .max_speed
                    }
                };
                Ok(path.estimate_duration(map, constraints, max_speed))
            }
            None => bail!(
                "can't figure out PathRequest from {:?} to {:?} via {}",
                info.start,
                info.end,
                info.mode.ongoing_verb()
            ),
        }
    }
}

// Drawing
impl Sim {
    pub fn step_count(&self) -> usize {
        self.step_count
    }

    pub fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        self.parking.get_draw_car(id, map).or_else(|| {
            self.driving
                .get_single_draw_car(id, self.time, map, &self.transit)
        })
    }

    pub fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrianInput> {
        self.walking.get_draw_ped(id, self.time, map)
    }

    pub fn get_draw_cars(&self, on: Traversable, map: &Map) -> Vec<DrawCarInput> {
        let mut results = Vec::new();
        if let Traversable::Lane(l) = on {
            if map.get_l(l).is_parking() {
                return self.parking.get_draw_cars(l, map);
            }
            results.extend(self.parking.get_draw_cars_in_lots(l, map));
        }
        results.extend(
            self.driving
                .get_draw_cars_on(self.time, on, map, &self.transit),
        );
        results
    }

    pub fn get_draw_peds(
        &self,
        on: Traversable,
        map: &Map,
    ) -> (Vec<DrawPedestrianInput>, Vec<DrawPedCrowdInput>) {
        self.walking.get_draw_peds_on(self.time, on, map)
    }

    pub fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        let mut result = self
            .driving
            .get_all_draw_cars(self.time, map, &self.transit);
        result.extend(self.parking.get_all_draw_cars(map));
        result
    }

    pub fn get_all_draw_peds(&self, map: &Map) -> Vec<DrawPedestrianInput> {
        self.walking.get_all_draw_peds(self.time, map)
    }

    pub fn get_unzoomed_agents(&self, map: &Map) -> Vec<UnzoomedAgent> {
        let mut result = self.driving.get_unzoomed_agents(self.time, map);
        result.extend(self.walking.get_unzoomed_agents(self.time, map));
        result
    }
}

pub struct AgentProperties {
    // TODO Of this leg of the trip only!
    pub total_time: Duration,
    pub waiting_here: Duration,
    pub total_waiting: Duration,

    pub dist_crossed: Distance,
    pub total_dist: Distance,
}

/// Why is an agent delayed? If there are multiple reasons, arbitrarily pick one -- ie, somebody
/// could be blocked by two conflicting turns.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize)]
pub enum DelayCause {
    /// Queued behind someone, or someone's doing a conflicting turn, or someone's eating up space
    /// in a target queue
    Agent(AgentID),
    /// Waiting on a traffic signal to change, or pausing at a stop sign before proceeding
    Intersection(IntersectionID),
}
