use crate::{AlertLocation, CarID, Event, ParkingSpot, TripID, TripMode, TripPhaseType};
use abstutil::Counter;
use geom::{Distance, Duration, Histogram, Statistic, Time};
use map_model::{
    BusRouteID, BusStopID, IntersectionID, LaneID, Map, Path, PathRequest, RoadID, Traversable,
    TurnGroupID,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Serialize, Deserialize)]
pub struct Analytics {
    pub road_thruput: TimeSeriesCount<RoadID>,
    pub intersection_thruput: TimeSeriesCount<IntersectionID>,

    // Unlike everything else in Analytics, this is just for a moment in time.
    pub demand: BTreeMap<TurnGroupID, usize>,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) test_expectations: VecDeque<Event>,
    pub bus_arrivals: Vec<(Time, CarID, BusRouteID, BusStopID)>,
    pub bus_passengers_waiting: Vec<(Time, BusStopID, BusRouteID)>,
    pub started_trips: BTreeMap<TripID, Time>,
    // TODO Hack: No TripMode means aborted
    // Finish time, ID, mode (or None as aborted), trip duration
    pub finished_trips: Vec<(Time, TripID, Option<TripMode>, Duration)>,
    // TODO This subsumes finished_trips
    pub trip_log: Vec<(Time, TripID, Option<PathRequest>, TripPhaseType)>,
    pub intersection_delays: BTreeMap<IntersectionID, Vec<(Time, Duration)>>,
    // Per parking lane, when does a spot become filled (true) or free (false)
    pub parking_spot_changes: BTreeMap<LaneID, Vec<(Time, bool)>>,
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
            demand: BTreeMap::new(),
            test_expectations: VecDeque::new(),
            bus_arrivals: Vec::new(),
            bus_passengers_waiting: Vec::new(),
            started_trips: BTreeMap::new(),
            finished_trips: Vec::new(),
            trip_log: Vec::new(),
            intersection_delays: BTreeMap::new(),
            parking_spot_changes: BTreeMap::new(),
            alerts: Vec::new(),
            record_anything: true,
        }
    }

    pub fn event(&mut self, ev: Event, time: Time, map: &Map) {
        if !self.record_anything {
            return;
        }

        // Throughput
        if let Event::AgentEntersTraversable(a, to) = ev {
            let mode = TripMode::from_agent(a);
            match to {
                Traversable::Lane(l) => {
                    self.road_thruput.record(time, map.get_l(l).parent, mode);
                }
                Traversable::Turn(t) => {
                    self.intersection_thruput.record(time, t.parent, mode);

                    if let Some(id) = map.get_turn_group(t) {
                        *self.demand.entry(id).or_insert(0) -= 1;
                    }
                }
            };
        }
        match ev {
            Event::PersonLeavesMap(_, mode, i, _) | Event::PersonEntersMap(_, mode, i, _) => {
                self.intersection_thruput.record(time, i, mode);
            }
            _ => {}
        }

        // Test expectations
        if !self.test_expectations.is_empty() && &ev == self.test_expectations.front().unwrap() {
            println!("At {}, met expectation {:?}", time, ev);
            self.test_expectations.pop_front();
        }

        // Bus arrivals
        if let Event::BusArrivedAtStop(bus, route, stop) = ev {
            self.bus_arrivals.push((time, bus, route, stop));
        }

        // Bus passengers
        if let Event::TripPhaseStarting(_, _, _, ref tpt) = ev {
            if let TripPhaseType::WaitingForBus(route, stop) = tpt {
                self.bus_passengers_waiting.push((time, *stop, *route));
            }
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
        if let Event::IntersectionDelayMeasured(id, delay) = ev {
            self.intersection_delays
                .entry(id)
                .or_insert_with(Vec::new)
                .push((time, delay));
        }

        // Parking spot changes
        if let Event::CarReachedParkingSpot(_, spot) = ev {
            if let ParkingSpot::Onstreet(l, _) = spot {
                self.parking_spot_changes
                    .entry(l)
                    .or_insert_with(Vec::new)
                    .push((time, true));
            }
        }
        if let Event::CarLeftParkingSpot(_, spot) = ev {
            if let ParkingSpot::Onstreet(l, _) = spot {
                self.parking_spot_changes
                    .entry(l)
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

    pub fn bus_arrivals(
        &self,
        now: Time,
        r: BusRouteID,
    ) -> BTreeMap<BusStopID, Histogram<Duration>> {
        let mut per_bus: BTreeMap<CarID, Vec<(Time, BusStopID)>> = BTreeMap::new();
        for (t, car, route, stop) in &self.bus_arrivals {
            if *t > now {
                break;
            }
            if *route == r {
                per_bus
                    .entry(*car)
                    .or_insert_with(Vec::new)
                    .push((*t, *stop));
            }
        }
        let mut delay_to_stop: BTreeMap<BusStopID, Histogram<Duration>> = BTreeMap::new();
        for events in per_bus.values() {
            for pair in events.windows(2) {
                delay_to_stop
                    .entry(pair[1].1)
                    .or_insert_with(Histogram::new)
                    .add(pair[1].0 - pair[0].0);
            }
        }
        delay_to_stop
    }

    // TODO Refactor!
    // For each stop, a list of (time, delay)
    pub fn bus_arrivals_over_time(
        &self,
        now: Time,
        r: BusRouteID,
    ) -> BTreeMap<BusStopID, Vec<(Time, Duration)>> {
        let mut per_bus: BTreeMap<CarID, Vec<(Time, BusStopID)>> = BTreeMap::new();
        for (t, car, route, stop) in &self.bus_arrivals {
            if *t > now {
                break;
            }
            if *route == r {
                per_bus
                    .entry(*car)
                    .or_insert_with(Vec::new)
                    .push((*t, *stop));
            }
        }
        let mut delays_to_stop: BTreeMap<BusStopID, Vec<(Time, Duration)>> = BTreeMap::new();
        for events in per_bus.values() {
            for pair in events.windows(2) {
                delays_to_stop
                    .entry(pair[1].1)
                    .or_insert_with(Vec::new)
                    .push((pair[1].0, pair[1].0 - pair[0].0));
            }
        }
        delays_to_stop
    }

    // At some moment in time, what's the distribution of passengers waiting for a route like?
    pub fn bus_passenger_delays(
        &self,
        now: Time,
        r: BusRouteID,
    ) -> BTreeMap<BusStopID, Histogram<Duration>> {
        let mut waiting_per_stop = BTreeMap::new();
        for (t, stop, route) in &self.bus_passengers_waiting {
            if *t > now {
                break;
            }
            if *route == r {
                waiting_per_stop
                    .entry(*stop)
                    .or_insert_with(Vec::new)
                    .push(*t);
            }
        }

        for (t, _, route, stop) in &self.bus_arrivals {
            if *t > now {
                break;
            }
            if *route == r {
                if let Some(ref mut times) = waiting_per_stop.get_mut(stop) {
                    times.retain(|time| *time > *t);
                }
            }
        }

        waiting_per_stop
            .into_iter()
            .filter_map(|(k, v)| {
                let mut delays = Histogram::new();
                for t in v {
                    delays.add(now - t);
                }
                if delays.count() == 0 {
                    None
                } else {
                    Some((k, delays))
                }
            })
            .collect()
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

    // TODO Unused right now!
    pub fn intersection_delays_all_day(&self, stat: Statistic) -> Vec<(IntersectionID, Duration)> {
        let mut results = Vec::new();
        for (i, delays) in &self.intersection_delays {
            let mut hgram = Histogram::new();
            for (_, dt) in delays {
                hgram.add(*dt);
            }
            results.push((*i, hgram.select(stat)));
        }
        results
    }

    pub fn intersection_delays_bucketized(
        &self,
        now: Time,
        i: IntersectionID,
        bucket: Duration,
    ) -> Vec<(Time, Histogram<Duration>)> {
        let mut max_this_bucket = now.min(Time::START_OF_DAY + bucket);
        let mut results = vec![
            (Time::START_OF_DAY, Histogram::new()),
            (max_this_bucket, Histogram::new()),
        ];
        if let Some(list) = self.intersection_delays.get(&i) {
            for (t, dt) in list {
                if *t > now {
                    break;
                }
                if *t > max_this_bucket {
                    max_this_bucket = now.min(max_this_bucket + bucket);
                    results.push((max_this_bucket, Histogram::new()));
                }
                results.last_mut().unwrap().1.add(*dt);
            }
        }
        results
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
    pub fn parking_spot_availability(
        &self,
        now: Time,
        l: LaneID,
        capacity: usize,
    ) -> Vec<(Time, usize)> {
        let changes = if let Some(changes) = self.parking_spot_changes.get(&l) {
            changes
        } else {
            return vec![(Time::START_OF_DAY, capacity), (now, capacity)];
        };

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

// Slightly misleading -- TripMode::Transit means buses, not pedestrians taking transit
#[derive(Clone, Serialize, Deserialize)]
pub struct TimeSeriesCount<X: Ord + Clone> {
    // (Road or intersection, mode, hour block) -> count for that hour
    pub counts: BTreeMap<(X, TripMode, usize), usize>,
}

impl<X: Ord + Clone> TimeSeriesCount<X> {
    fn new() -> TimeSeriesCount<X> {
        TimeSeriesCount {
            counts: BTreeMap::new(),
        }
    }

    fn record(&mut self, time: Time, id: X, mode: TripMode) {
        let hour = time.get_parts().0;
        *self.counts.entry((id, mode, hour)).or_insert(0) += 1;
    }

    pub fn total_for(&self, id: X) -> usize {
        let mut cnt = 0;
        for mode in TripMode::all() {
            // TODO Hmm
            for hour in 0..24 {
                cnt += self
                    .counts
                    .get(&(id.clone(), mode, hour))
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

    pub fn count_per_hour(&self, id: X) -> Vec<(TripMode, Vec<(Time, usize)>)> {
        let mut results = Vec::new();
        for mode in TripMode::all() {
            let mut pts = Vec::new();
            // TODO Hmm
            for hour in 0..24 {
                let cnt = self
                    .counts
                    .get(&(id.clone(), mode, hour))
                    .cloned()
                    .unwrap_or(0);
                pts.push((Time::START_OF_DAY + Duration::hours(hour), cnt));
                pts.push((Time::START_OF_DAY + Duration::hours(hour + 1), cnt));
            }
            results.push((mode, pts));
        }
        results
    }
}
