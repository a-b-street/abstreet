use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use abstutil::Counter;
use geom::{Duration, Time};
use map_model::{
    BusRouteID, BusStopID, CompressedMovementID, IntersectionID, LaneID, Map, MovementID,
    ParkingLotID, Path, PathRequest, RoadID, Traversable, TurnType,
};

use crate::{
    AgentID, AgentType, AlertLocation, CarID, Event, ParkingSpot, TripID, TripMode, TripPhaseType,
};

/// As a simulation runs, different pieces emit Events. The Analytics object listens to these,
/// organizing and storing some information from them. The UI queries Analytics to draw time-series
/// and display statistics.
///
/// For all maps whose weekday scenario fully runs, the game's release includes some "prebaked
/// results." These are just serialized Analytics after running the simulation on a map without any
/// edits for the full day. This is the basis of A/B testing -- the player can edit the map, start
/// running the simulation, and compare the live Analytics to the prebaked baseline Analytics.
#[derive(Clone, Serialize, Deserialize)]
pub struct Analytics {
    pub road_thruput: TimeSeriesCount<RoadID>,
    pub intersection_thruput: TimeSeriesCount<IntersectionID>,
    // TODO For traffic signals, intersection_thruput could theoretically use this. But that
    // requires occasionally expensive or complicated summing or merging over all directions of an
    // intersection. So for now, eat the file size cost.
    pub traffic_signal_thruput: TimeSeriesCount<CompressedMovementID>,

    /// Most fields in Analytics are cumulative over time, but this is just for the current moment
    /// in time.
    pub demand: BTreeMap<MovementID, usize>,

    // TODO Reconsider this one
    pub bus_arrivals: Vec<(Time, CarID, BusRouteID, BusStopID)>,
    /// For each passenger boarding, how long did they wait at the stop?
    pub passengers_boarding: BTreeMap<BusStopID, Vec<(Time, BusRouteID, Duration)>>,
    pub passengers_alighting: BTreeMap<BusStopID, Vec<(Time, BusRouteID)>>,

    pub started_trips: BTreeMap<TripID, Time>,
    /// Finish time, ID, mode, trip duration if successful (or None if cancelled)
    pub finished_trips: Vec<(Time, TripID, TripMode, Option<Duration>)>,

    /// Record different problems that each trip encounters.
    pub problems_per_trip: BTreeMap<TripID, Vec<(Time, Problem)>>,

    // TODO This subsumes finished_trips
    pub trip_log: Vec<(Time, TripID, Option<PathRequest>, TripPhaseType)>,

    // TODO Transit riders aren't represented here yet, just the vehicle they're riding.
    /// Only for traffic signals. The u8 is the movement index from a CompressedMovementID.
    pub intersection_delays: BTreeMap<IntersectionID, Vec<(u8, Time, Duration, AgentType)>>,

    /// Per parking lane or lot, when does a spot become filled (true) or free (false)
    pub parking_lane_changes: BTreeMap<LaneID, Vec<(Time, bool)>>,
    pub parking_lot_changes: BTreeMap<ParkingLotID, Vec<(Time, bool)>>,

    pub(crate) alerts: Vec<(Time, AlertLocation, String)>,

    /// For benchmarking, we may want to disable collecting data.
    record_anything: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Problem {
    /// A vehicle waited >30s, or a pedestrian waited >15s.
    IntersectionDelay(IntersectionID, Duration),
    /// A cyclist crossed an intersection with >4 connecting roads.
    LargeIntersectionCrossing(IntersectionID),
    /// Another vehicle wanted to over-take this cyclist somewhere on this lane or turn.
    OvertakeDesired(Traversable),
}

impl Analytics {
    pub fn new(record_anything: bool) -> Analytics {
        Analytics {
            road_thruput: TimeSeriesCount::new(),
            intersection_thruput: TimeSeriesCount::new(),
            traffic_signal_thruput: TimeSeriesCount::new(),
            demand: BTreeMap::new(),
            bus_arrivals: Vec::new(),
            passengers_boarding: BTreeMap::new(),
            passengers_alighting: BTreeMap::new(),
            started_trips: BTreeMap::new(),
            finished_trips: Vec::new(),
            problems_per_trip: BTreeMap::new(),
            trip_log: Vec::new(),
            intersection_delays: BTreeMap::new(),
            parking_lane_changes: BTreeMap::new(),
            parking_lot_changes: BTreeMap::new(),
            alerts: Vec::new(),
            record_anything,
        }
    }

    pub fn event(&mut self, ev: Event, time: Time, map: &Map) {
        if !self.record_anything {
            return;
        }

        // Throughput
        if let Event::AgentEntersTraversable(a, _, to, passengers) = ev {
            match to {
                Traversable::Lane(l) => {
                    self.road_thruput
                        .record(time, map.get_l(l).parent, a.to_type(), 1);
                    if let Some(n) = passengers {
                        self.road_thruput.record(
                            time,
                            map.get_l(l).parent,
                            AgentType::TransitRider,
                            n,
                        );
                    }
                }
                Traversable::Turn(t) => {
                    self.intersection_thruput
                        .record(time, t.parent, a.to_type(), 1);
                    if let Some(n) = passengers {
                        self.intersection_thruput.record(
                            time,
                            t.parent,
                            AgentType::TransitRider,
                            n,
                        );
                    }

                    if let Some(id) = map.get_movement(t) {
                        *self.demand.entry(id).or_insert(0) -= 1;

                        let m = map.get_traffic_signal(t.parent).compressed_id(t);
                        self.traffic_signal_thruput.record(time, m, a.to_type(), 1);
                        if let Some(n) = passengers {
                            self.traffic_signal_thruput
                                .record(time, m, AgentType::TransitRider, n);
                        }
                    }
                }
            };
        }
        match ev {
            Event::PersonLeavesMap(_, Some(a), i) => {
                // Ignore cancelled trips
                self.intersection_thruput.record(time, i, a.to_type(), 1);
            }
            Event::PersonEntersMap(_, a, i) => {
                self.intersection_thruput.record(time, i, a.to_type(), 1);
            }
            _ => {}
        }

        // Bus arrivals
        if let Event::BusArrivedAtStop(bus, route, stop) = ev {
            self.bus_arrivals.push((time, bus, route, stop));
        }

        // Passengers boarding/alighting
        if let Event::PassengerBoardsTransit(_, _, route, stop, waiting) = ev {
            self.passengers_boarding
                .entry(stop)
                .or_insert_with(Vec::new)
                .push((time, route, waiting));
        }
        if let Event::PassengerAlightsTransit(_, _, route, stop) = ev {
            self.passengers_alighting
                .entry(stop)
                .or_insert_with(Vec::new)
                .push((time, route));
        }

        // Started trips
        if let Event::TripPhaseStarting(id, _, _, _) = ev {
            self.started_trips.entry(id).or_insert(time);
        }

        // Finished trips
        if let Event::TripFinished {
            trip,
            mode,
            total_time,
            ..
        } = ev
        {
            self.finished_trips
                .push((time, trip, mode, Some(total_time)));
        } else if let Event::TripCancelled(id, mode) = ev {
            self.started_trips.entry(id).or_insert(time);
            self.finished_trips.push((time, id, mode, None));
        }

        // Intersection delay
        if let Event::IntersectionDelayMeasured(trip_id, turn_id, agent, delay) = ev {
            let threshold = match agent {
                AgentID::Car(_) => Duration::seconds(30.0),
                AgentID::Pedestrian(_) => Duration::seconds(15.0),
                // Don't record for riders
                AgentID::BusPassenger(_, _) => Duration::hours(24),
            };
            if delay > threshold {
                self.problems_per_trip
                    .entry(trip_id)
                    .or_insert_with(Vec::new)
                    .push((time, Problem::IntersectionDelay(turn_id.parent, delay)));
            }

            // SharedSidewalkCorner are always no-conflict, immediate turns; they're not
            // interesting.
            if map.get_t(turn_id).turn_type != TurnType::SharedSidewalkCorner {
                // Save memory and space by only storing these measurements at traffic signals.
                if let Some(ts) = map.maybe_get_traffic_signal(turn_id.parent) {
                    self.intersection_delays
                        .entry(turn_id.parent)
                        .or_insert_with(Vec::new)
                        .push((ts.compressed_id(turn_id).idx, time, delay, agent.to_type()));
                }
            }
        }

        // Parking spot changes
        if let Event::CarReachedParkingSpot(_, spot) = ev {
            if let ParkingSpot::Onstreet(l, _) = spot {
                self.parking_lane_changes
                    .entry(l)
                    .or_insert_with(Vec::new)
                    .push((time, true));
            } else if let ParkingSpot::Lot(pl, _) = spot {
                self.parking_lot_changes
                    .entry(pl)
                    .or_insert_with(Vec::new)
                    .push((time, true));
            }
        }
        if let Event::CarLeftParkingSpot(_, spot) = ev {
            if let ParkingSpot::Onstreet(l, _) = spot {
                self.parking_lane_changes
                    .entry(l)
                    .or_insert_with(Vec::new)
                    .push((time, false));
            } else if let ParkingSpot::Lot(pl, _) = spot {
                self.parking_lot_changes
                    .entry(pl)
                    .or_insert_with(Vec::new)
                    .push((time, false));
            }
        }

        // Safety metrics
        if let Event::AgentEntersTraversable(a, Some(trip), Traversable::Turn(t), _) = ev {
            if a.to_type() == AgentType::Bike && map.get_i(t.parent).roads.len() > 4 {
                // Defining a "large intersection" is tricky. If a road is split into two one-ways,
                // should we count it as two roads? If we haven't consolidated some crazy
                // intersection, we won't see it.
                self.problems_per_trip
                    .entry(trip)
                    .or_insert_with(Vec::new)
                    .push((time, Problem::LargeIntersectionCrossing(t.parent)));
            }
        }

        // TODO Kinda hacky, but these all consume the event, so kinda bundle em.
        match ev {
            Event::TripPhaseStarting(id, _, maybe_req, phase_type) => {
                self.trip_log.push((time, id, maybe_req, phase_type));
            }
            Event::TripCancelled(id, _) => {
                self.trip_log
                    .push((time, id, None, TripPhaseType::Cancelled));
            }
            Event::TripFinished { trip, .. } => {
                self.trip_log
                    .push((time, trip, None, TripPhaseType::Finished));
            }
            Event::PathAmended(path) => {
                self.record_demand(&path, map);
            }
            Event::Alert(loc, msg) => {
                self.alerts.push((time, loc, msg));
            }
            Event::ProblemEncountered(trip, problem) => {
                self.problems_per_trip
                    .entry(trip)
                    .or_insert_with(Vec::new)
                    .push((time, problem));
            }
            _ => {}
        }
    }

    pub fn record_demand(&mut self, path: &Path, map: &Map) {
        for step in path.get_steps() {
            if let Traversable::Turn(t) = step.as_traversable() {
                if let Some(id) = map.get_movement(t) {
                    *self.demand.entry(id).or_insert(0) += 1;
                }
            }
        }
    }

    // TODO If these ever need to be speeded up, just cache the histogram and index in the events
    // list.

    /// Ignores the current time. Returns None for cancelled trips.
    pub fn finished_trip_time(&self, trip: TripID) -> Option<Duration> {
        // TODO This is so inefficient!
        for (_, id, _, maybe_dt) in &self.finished_trips {
            if *id == trip {
                return *maybe_dt;
            }
        }
        None
    }

    /// Returns pairs of trip times for finished trips in both worlds. (ID, before, after, mode)
    pub fn both_finished_trips(
        &self,
        now: Time,
        before: &Analytics,
    ) -> Vec<(TripID, Duration, Duration, TripMode)> {
        let mut a = BTreeMap::new();
        for (t, id, _, maybe_dt) in &self.finished_trips {
            if *t > now {
                break;
            }
            if let Some(dt) = maybe_dt {
                a.insert(*id, *dt);
            }
        }

        let mut results = Vec::new();
        for (t, id, mode, maybe_dt) in &before.finished_trips {
            if *t > now {
                break;
            }
            if let Some(dt) = maybe_dt {
                if let Some(dt1) = a.remove(id) {
                    results.push((*id, *dt, dt1, *mode));
                }
            }
        }
        results
    }

    /// If calling on prebaked Analytics, be careful to pass in an unedited map, to match how the
    /// simulation was originally run. Otherwise the paths may be nonsense.
    pub fn get_trip_phases(&self, trip: TripID, map: &Map) -> Vec<TripPhase> {
        let mut phases: Vec<TripPhase> = Vec::new();
        for (t, id, maybe_req, phase_type) in &self.trip_log {
            if *id != trip {
                continue;
            }
            if let Some(ref mut last) = phases.last_mut() {
                last.end_time = Some(*t);
            }
            if *phase_type == TripPhaseType::Finished || *phase_type == TripPhaseType::Cancelled {
                break;
            }
            phases.push(TripPhase {
                start_time: *t,
                end_time: None,
                path: maybe_req.clone().and_then(|req| map.pathfind(req).ok()),
                has_path_req: maybe_req.is_some(),
                phase_type: *phase_type,
            })
        }
        phases
    }

    pub fn get_all_trip_phases(&self) -> BTreeMap<TripID, Vec<TripPhase>> {
        let mut trips = BTreeMap::new();
        for (t, id, maybe_req, phase_type) in &self.trip_log {
            let phases: &mut Vec<TripPhase> = trips.entry(*id).or_insert_with(Vec::new);
            if let Some(ref mut last) = phases.last_mut() {
                last.end_time = Some(*t);
            }
            if *phase_type == TripPhaseType::Finished {
                continue;
            }
            // Remove cancelled trips
            if *phase_type == TripPhaseType::Cancelled {
                trips.remove(id);
                continue;
            }
            phases.push(TripPhase {
                start_time: *t,
                end_time: None,
                // Don't compute any paths
                path: None,
                has_path_req: maybe_req.is_some(),
                phase_type: *phase_type,
            })
        }
        trips
    }

    pub fn active_agents(&self, now: Time) -> Vec<(Time, usize)> {
        let mut starts_stops: Vec<(Time, bool)> = Vec::new();
        for t in self.started_trips.values() {
            if *t <= now {
                starts_stops.push((*t, false));
            }
        }
        for (t, _, _, _) in &self.finished_trips {
            if *t > now {
                break;
            }
            starts_stops.push((*t, true));
        }
        // Make sure the start events get sorted before the stops.
        starts_stops.sort();

        let mut pts = Vec::new();
        let mut cnt = 0;
        let mut last_t = Time::START_OF_DAY;
        for (t, ended) in starts_stops {
            if t != last_t {
                // Step functions. Don't interpolate.
                pts.push((last_t, cnt));
            }
            last_t = t;
            if ended {
                // release mode disables this check, so...
                if cnt == 0 {
                    panic!("active_agents at {} has more ended trips than started", t);
                }
                cnt -= 1;
            } else {
                cnt += 1;
            }
        }
        pts.push((last_t, cnt));
        if last_t != now {
            pts.push((now, cnt));
        }
        pts
    }

    /// Returns the free spots over time
    pub fn parking_lane_availability(
        &self,
        now: Time,
        l: LaneID,
        capacity: usize,
    ) -> Vec<(Time, usize)> {
        if let Some(changes) = self.parking_lane_changes.get(&l) {
            Analytics::parking_spot_availability(now, changes, capacity)
        } else {
            vec![(Time::START_OF_DAY, capacity), (now, capacity)]
        }
    }
    pub fn parking_lot_availability(
        &self,
        now: Time,
        pl: ParkingLotID,
        capacity: usize,
    ) -> Vec<(Time, usize)> {
        if let Some(changes) = self.parking_lot_changes.get(&pl) {
            Analytics::parking_spot_availability(now, changes, capacity)
        } else {
            vec![(Time::START_OF_DAY, capacity), (now, capacity)]
        }
    }

    fn parking_spot_availability(
        now: Time,
        changes: &Vec<(Time, bool)>,
        capacity: usize,
    ) -> Vec<(Time, usize)> {
        let mut pts = Vec::new();
        let mut cnt = capacity;
        let mut last_t = Time::START_OF_DAY;

        for (t, filled) in changes {
            if *t > now {
                break;
            }
            if *t != last_t {
                // Step functions. Don't interpolate.
                pts.push((last_t, cnt));
            }
            last_t = *t;
            if *filled {
                if cnt == 0 {
                    panic!("parking_spot_availability at {} went below 0", t);
                }
                cnt -= 1;
            } else {
                cnt += 1;
            }
        }
        pts.push((last_t, cnt));
        if last_t != now {
            pts.push((now, cnt));
        }
        pts
    }
}

impl Default for Analytics {
    fn default() -> Analytics {
        Analytics::new(false)
    }
}

#[derive(Debug)]
pub struct TripPhase {
    pub start_time: Time,
    pub end_time: Option<Time>,
    pub path: Option<Path>,
    pub has_path_req: bool,
    pub phase_type: TripPhaseType,
}

/// See https://github.com/a-b-street/abstreet/issues/85
#[derive(Clone, Serialize, Deserialize)]
pub struct TimeSeriesCount<X: Ord + Clone> {
    /// (Road or intersection, type, hour block) -> count for that hour
    pub counts: BTreeMap<(X, AgentType, usize), usize>,

    /// Very expensive to store, so it's optional. But useful to flag on to experiment with
    /// representations better than the hour count above.
    pub raw: Vec<(Time, AgentType, X)>,
}

impl<X: Ord + Clone> TimeSeriesCount<X> {
    fn new() -> TimeSeriesCount<X> {
        TimeSeriesCount {
            counts: BTreeMap::new(),
            raw: Vec::new(),
        }
    }

    fn record(&mut self, time: Time, id: X, agent_type: AgentType, count: usize) {
        // TODO Manually change flag
        if false {
            // TODO Woo, handling transit passengers is even more expensive in this already
            // expensive representation...
            for _ in 0..count {
                self.raw.push((time, agent_type, id.clone()));
            }
        }

        *self
            .counts
            .entry((id, agent_type, time.get_hours()))
            .or_insert(0) += count;
    }

    pub fn total_for(&self, id: X) -> usize {
        self.total_for_with_agent_types(id, AgentType::all().into_iter().collect())
    }

    pub fn total_for_with_agent_types(&self, id: X, agent_types: BTreeSet<AgentType>) -> usize {
        let mut cnt = 0;
        for agent_type in agent_types {
            // TODO Hmm
            for hour in 0..24 {
                cnt += self
                    .counts
                    .get(&(id.clone(), agent_type, hour))
                    .cloned()
                    .unwrap_or(0);
            }
        }
        cnt
    }

    pub fn total_for_by_time(&self, id: X, now: Time) -> usize {
        let mut cnt = 0;
        for agent_type in AgentType::all() {
            for hour in 0..=now.get_hours() {
                cnt += self
                    .counts
                    .get(&(id.clone(), agent_type, hour))
                    .cloned()
                    .unwrap_or(0);
            }
        }
        cnt
    }

    pub fn all_total_counts(&self, agent_types: &BTreeSet<AgentType>) -> Counter<X> {
        let mut cnt = Counter::new();
        for ((id, agent_type, _), value) in &self.counts {
            if agent_types.contains(&agent_type) {
                cnt.add(id.clone(), *value);
            }
        }
        cnt
    }

    pub fn count_per_hour(&self, id: X, time: Time) -> Vec<(AgentType, Vec<(Time, usize)>)> {
        let hour = time.get_hours();
        let mut results = Vec::new();
        for agent_type in AgentType::all() {
            let mut pts = Vec::new();
            for hour in 0..=hour {
                let cnt = self
                    .counts
                    .get(&(id.clone(), agent_type, hour))
                    .cloned()
                    .unwrap_or(0);
                pts.push((Time::START_OF_DAY + Duration::hours(hour), cnt));
                pts.push((Time::START_OF_DAY + Duration::hours(hour + 1), cnt));
            }
            pts.pop();
            results.push((agent_type, pts));
        }
        results
    }

    pub fn raw_throughput(&self, now: Time, id: X) -> Vec<(AgentType, Vec<(Time, usize)>)> {
        let window_size = Duration::hours(1);
        let mut pts_per_type: BTreeMap<AgentType, Vec<(Time, usize)>> = BTreeMap::new();
        let mut windows_per_type: BTreeMap<AgentType, Window> = BTreeMap::new();
        for agent_type in AgentType::all() {
            pts_per_type.insert(agent_type, vec![(Time::START_OF_DAY, 0)]);
            windows_per_type.insert(agent_type, Window::new(window_size));
        }

        for (t, agent_type, x) in &self.raw {
            if *x != id {
                continue;
            }
            if *t > now {
                break;
            }

            let count = windows_per_type.get_mut(agent_type).unwrap().add(*t);
            pts_per_type.get_mut(agent_type).unwrap().push((*t, count));
        }

        for (agent_type, pts) in pts_per_type.iter_mut() {
            let mut window = windows_per_type.remove(agent_type).unwrap();

            // Add a drop-off after window_size (+ a little epsilon!)
            let t = (pts.last().unwrap().0 + window_size + Duration::seconds(0.1)).min(now);
            if pts.last().unwrap().0 != t {
                pts.push((t, window.count(t)));
            }

            if pts.last().unwrap().0 != now {
                pts.push((now, window.count(now)));
            }
        }

        pts_per_type.into_iter().collect()
    }
}

pub struct Window {
    times: VecDeque<Time>,
    window_size: Duration,
}

impl Window {
    pub fn new(window_size: Duration) -> Window {
        Window {
            times: VecDeque::new(),
            window_size,
        }
    }

    /// Returns the count at time
    pub fn add(&mut self, time: Time) -> usize {
        self.times.push_back(time);
        self.count(time)
    }

    /// Grab the count at this time, but don't add a new time
    pub fn count(&mut self, end: Time) -> usize {
        while !self.times.is_empty() && end - *self.times.front().unwrap() > self.window_size {
            self.times.pop_front();
        }
        self.times.len()
    }
}
