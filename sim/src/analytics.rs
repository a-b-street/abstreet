use crate::{AgentType, AlertLocation, CarID, Event, ParkingSpot, TripID, TripMode, TripPhaseType};
use abstutil::Counter;
use geom::{Distance, Duration, Time};
use map_model::{
    BusRouteID, BusStopID, CompressedTurnGroupID, IntersectionID, LaneID, Map, ParkingLotID, Path,
    PathRequest, RoadID, Traversable, TurnGroupID,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Serialize, Deserialize)]
pub struct Analytics {
    pub road_thruput: TimeSeriesCount<RoadID>,
    pub intersection_thruput: TimeSeriesCount<IntersectionID>,
    // TODO For traffic signals, intersection_thruput could theoretically use this. But that
    // requires occasionally expensive or complicated summing or merging over all directions of an
    // intersection. So for now, eat the file size cost.
    pub traffic_signal_thruput: TimeSeriesCount<CompressedTurnGroupID>,

    // Unlike everything else in Analytics, this is just for a moment in time.
    pub demand: BTreeMap<TurnGroupID, usize>,

    // TODO Reconsider this one
    pub bus_arrivals: Vec<(Time, CarID, BusRouteID, BusStopID)>,
    // For each passenger boarding, how long did they wait at the stop?
    pub passengers_boarding: BTreeMap<BusStopID, Vec<(Time, BusRouteID, Duration)>>,
    pub passengers_alighting: BTreeMap<BusStopID, Vec<(Time, BusRouteID)>>,

    pub started_trips: BTreeMap<TripID, Time>,
    // TODO Hack: No TripMode means aborted
    // Finish time, ID, mode (or None as aborted), trip duration
    pub finished_trips: Vec<(Time, TripID, Option<TripMode>, Duration)>,
    // TODO This subsumes finished_trips
    pub trip_log: Vec<(Time, TripID, Option<PathRequest>, TripPhaseType)>,

    // TODO Transit riders aren't represented here yet, just the vehicle they're riding.
    // Only for traffic signals. The u8 is the turn group index from a CompressedTurnGroupID.
    pub intersection_delays: BTreeMap<IntersectionID, Vec<(u8, Time, Duration, AgentType)>>,

    // Per parking lane or lot, when does a spot become filled (true) or free (false)
    pub parking_lane_changes: BTreeMap<LaneID, Vec<(Time, bool)>>,
    pub parking_lot_changes: BTreeMap<ParkingLotID, Vec<(Time, bool)>>,

    pub(crate) alerts: Vec<(Time, AlertLocation, String)>,

    // After we restore from a savestate, don't record anything. This is only going to make sense
    // if savestates are only used for quickly previewing against prebaked results, where we have
    // the full Analytics anyway.
    record_anything: bool,
}

impl Analytics {
    pub fn new() -> Analytics {
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
            trip_log: Vec::new(),
            intersection_delays: BTreeMap::new(),
            parking_lane_changes: BTreeMap::new(),
            parking_lot_changes: BTreeMap::new(),
            alerts: Vec::new(),
            record_anything: true,
        }
    }

    pub fn event(&mut self, ev: Event, time: Time, map: &Map) {
        if !self.record_anything {
            return;
        }

        // Throughput
        if let Event::AgentEntersTraversable(a, to, passengers) = ev {
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

                    if let Some(id) = map.get_turn_group(t) {
                        *self.demand.entry(id).or_insert(0) -= 1;

                        let tg = map.get_traffic_signal(t.parent).compressed_id(t);
                        self.traffic_signal_thruput.record(time, tg, a.to_type(), 1);
                        if let Some(n) = passengers {
                            self.traffic_signal_thruput.record(
                                time,
                                tg,
                                AgentType::TransitRider,
                                n,
                            );
                        }
                    }
                }
            };
        }
        match ev {
            Event::PersonLeavesMap(_, maybe_a, i, _) => {
                // Ignore aborted trips
                if let Some(a) = maybe_a {
                    self.intersection_thruput.record(time, i, a.to_type(), 1);
                }
            }
            Event::PersonEntersMap(_, a, i, _) => {
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
                .push((time, trip, Some(mode), total_time));
        } else if let Event::TripAborted(id) = ev {
            self.started_trips.entry(id).or_insert(time);
            self.finished_trips.push((time, id, None, Duration::ZERO));
        }

        // Intersection delays
        if let Event::IntersectionDelayMeasured(id, delay, agent) = ev {
            self.intersection_delays
                .entry(id.i)
                .or_insert_with(Vec::new)
                .push((id.idx, time, delay, agent.to_type()));
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

        // TODO Kinda hacky, but these all consume the event, so kinda bundle em.
        match ev {
            Event::TripPhaseStarting(id, _, maybe_req, phase_type) => {
                self.trip_log.push((time, id, maybe_req, phase_type));
            }
            Event::TripAborted(id) => {
                self.trip_log.push((time, id, None, TripPhaseType::Aborted));
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
            _ => {}
        }
    }

    pub fn record_demand(&mut self, path: &Path, map: &Map) {
        for step in path.get_steps() {
            if let Traversable::Turn(t) = step.as_traversable() {
                if let Some(id) = map.get_turn_group(t) {
                    *self.demand.entry(id).or_insert(0) += 1;
                }
            }
        }
    }

    // TODO If these ever need to be speeded up, just cache the histogram and index in the events
    // list.

    // Ignores the current time. Returns None for aborted trips.
    pub fn finished_trip_time(&self, trip: TripID) -> Option<Duration> {
        // TODO This is so inefficient!
        for (_, id, maybe_mode, dt) in &self.finished_trips {
            if *id == trip {
                if maybe_mode.is_some() {
                    return Some(*dt);
                } else {
                    return None;
                }
            }
        }
        None
    }

    // Returns pairs of trip times for finished trips in both worlds. (before, after, mode)
    pub fn both_finished_trips(
        &self,
        now: Time,
        before: &Analytics,
    ) -> Vec<(Duration, Duration, TripMode)> {
        let mut a = BTreeMap::new();
        for (t, id, maybe_mode, dt) in &self.finished_trips {
            if *t > now {
                break;
            }
            if maybe_mode.is_some() {
                a.insert(*id, *dt);
            }
        }

        let mut results = Vec::new();
        for (t, id, maybe_mode, dt) in &before.finished_trips {
            if *t > now {
                break;
            }
            if let Some(mode) = maybe_mode {
                if let Some(dt1) = a.remove(id) {
                    results.push((*dt, dt1, *mode));
                }
            }
        }
        results
    }

    // Find intersections where the cumulative sum of delay has changed. Negative means faster.
    pub fn compare_delay(&self, now: Time, before: &Analytics) -> Vec<(IntersectionID, Duration)> {
        let mut results = Vec::new();
        for (i, list1) in &self.intersection_delays {
            if let Some(list2) = before.intersection_delays.get(i) {
                let mut sum1 = Duration::ZERO;
                for (_, t, dt, _) in list1 {
                    if *t > now {
                        break;
                    }
                    sum1 += *dt;
                }

                let mut sum2 = Duration::ZERO;
                for (_, t, dt, _) in list2 {
                    if *t > now {
                        break;
                    }
                    sum2 += *dt;
                }

                if sum1 != sum2 {
                    results.push((*i, sum1 - sum2));
                }
            }
        }
        results
    }

    pub fn get_trip_phases(&self, trip: TripID, map: &Map) -> Vec<TripPhase> {
        let mut phases: Vec<TripPhase> = Vec::new();
        for (t, id, maybe_req, phase_type) in &self.trip_log {
            if *id != trip {
                continue;
            }
            if let Some(ref mut last) = phases.last_mut() {
                last.end_time = Some(*t);
            }
            if *phase_type == TripPhaseType::Finished || *phase_type == TripPhaseType::Aborted {
                break;
            }
            phases.push(TripPhase {
                start_time: *t,
                end_time: None,
                // Unwrap should be safe, because this is the request that was actually done...
                // TODO Not if this is prebaked data and we've made edits. Woops.
                path: maybe_req.as_ref().and_then(|req| {
                    map.pathfind(req.clone())
                        .map(|path| (req.start.dist_along(), path))
                }),
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
            // Remove aborted trips
            if *phase_type == TripPhaseType::Aborted {
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

    // Returns the free spots over time
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
        let mut a = Analytics::new();
        a.record_anything = false;
        a
    }
}

#[derive(Debug)]
pub struct TripPhase {
    pub start_time: Time,
    pub end_time: Option<Time>,
    // Plumb along start distance
    pub path: Option<(Distance, Path)>,
    pub has_path_req: bool,
    pub phase_type: TripPhaseType,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TimeSeriesCount<X: Ord + Clone> {
    // (Road or intersection, type, hour block) -> count for that hour
    pub counts: BTreeMap<(X, AgentType, usize), usize>,

    // Very expensive to store, so it's optional. But useful to flag on to experiment with
    // representations better than the hour count above.
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

        let hour = time.get_parts().0;
        *self.counts.entry((id, agent_type, hour)).or_insert(0) += count;
    }

    pub fn total_for(&self, id: X) -> usize {
        let mut cnt = 0;
        for agent_type in AgentType::all() {
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

    pub fn all_total_counts(&self) -> Counter<X> {
        let mut cnt = Counter::new();
        for ((id, _, _), value) in &self.counts {
            cnt.add(id.clone(), *value);
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

    // Returns the count at time
    pub fn add(&mut self, time: Time) -> usize {
        self.times.push_back(time);
        self.count(time)
    }

    // Grab the count at this time, but don't add a new time
    pub fn count(&mut self, end: Time) -> usize {
        while !self.times.is_empty() && end - *self.times.front().unwrap() > self.window_size {
            self.times.pop_front();
        }
        self.times.len()
    }
}
