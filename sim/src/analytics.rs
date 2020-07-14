use crate::{AlertLocation, CarID, Event, ParkingSpot, TripID, TripMode, TripPhaseType};
use abstutil::Counter;
use geom::{Distance, Duration, Histogram, Time};
use map_model::{
    BusRouteID, BusStopID, IntersectionID, LaneID, Map, ParkingLotID, Path, PathRequest, RoadID,
    Traversable, TurnGroupID,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Serialize, Deserialize)]
pub struct Analytics {
    pub road_thruput: TimeSeriesCount<RoadID>,
    pub intersection_thruput: TimeSeriesCount<IntersectionID>,

    // Unlike everything else in Analytics, this is just for a moment in time.
    pub demand: BTreeMap<TurnGroupID, usize>,
    pub bus_arrivals: Vec<(Time, CarID, BusRouteID, BusStopID)>,
    pub bus_passengers_waiting: Vec<(Time, BusStopID, BusRouteID)>,
    pub started_trips: BTreeMap<TripID, Time>,
    // TODO Hack: No TripMode means aborted
    // Finish time, ID, mode (or None as aborted), trip duration
    pub finished_trips: Vec<(Time, TripID, Option<TripMode>, Duration)>,
    // TODO This subsumes finished_trips
    pub trip_log: Vec<(Time, TripID, Option<PathRequest>, TripPhaseType)>,
    pub intersection_delays: BTreeMap<IntersectionID, Vec<(Time, Duration, TripMode)>>,
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
            demand: BTreeMap::new(),
            bus_arrivals: Vec::new(),
            bus_passengers_waiting: Vec::new(),
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
        if let Event::IntersectionDelayMeasured(id, delay, mode) = ev {
            self.intersection_delays
                .entry(id)
                .or_insert_with(Vec::new)
                .push((time, delay, mode));
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
                for (t, dt, _) in list1 {
                    if *t > now {
                        break;
                    }
                    sum1 += *dt;
                }

                let mut sum2 = Duration::ZERO;
                for (t, dt, _) in list2 {
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
    ) -> impl Iterator<Item = (BusStopID, Histogram<Duration>)> {
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

        waiting_per_stop.into_iter().filter_map(move |(k, v)| {
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

// Slightly misleading -- TripMode::Transit means buses, not pedestrians taking transit
#[derive(Clone, Serialize, Deserialize)]
pub struct TimeSeriesCount<X: Ord + Clone> {
    // (Road or intersection, mode, hour block) -> count for that hour
    pub counts: BTreeMap<(X, TripMode, usize), usize>,

    // Very expensive to store, so it's optional. But useful to flag on to experiment with
    // representations better than the hour count above.
    pub raw: Vec<(Time, TripMode, X)>,
}

impl<X: Ord + Clone> TimeSeriesCount<X> {
    fn new() -> TimeSeriesCount<X> {
        TimeSeriesCount {
            counts: BTreeMap::new(),
            raw: Vec::new(),
        }
    }

    fn record(&mut self, time: Time, id: X, mode: TripMode) {
        // TODO Manually change flag
        if false {
            self.raw.push((time, mode, id.clone()));
        }

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

    pub fn count_per_hour(&self, id: X, time: Time) -> Vec<(TripMode, Vec<(Time, usize)>)> {
        let hour = time.get_hours();
        let mut results = Vec::new();
        for mode in TripMode::all() {
            let mut pts = Vec::new();
            for hour in 0..=hour {
                let cnt = self
                    .counts
                    .get(&(id.clone(), mode, hour))
                    .cloned()
                    .unwrap_or(0);
                pts.push((Time::START_OF_DAY + Duration::hours(hour), cnt));
                pts.push((Time::START_OF_DAY + Duration::hours(hour + 1), cnt));
            }
            pts.pop();
            results.push((mode, pts));
        }
        results
    }

    pub fn raw_throughput(&self, now: Time, id: X) -> Vec<(TripMode, Vec<(Time, usize)>)> {
        let window_size = Duration::hours(1);
        let mut pts_per_mode: BTreeMap<TripMode, Vec<(Time, usize)>> = BTreeMap::new();
        let mut windows_per_mode: BTreeMap<TripMode, Window> = BTreeMap::new();
        for mode in TripMode::all() {
            pts_per_mode.insert(mode, vec![(Time::START_OF_DAY, 0)]);
            windows_per_mode.insert(mode, Window::new(window_size));
        }

        for (t, m, x) in &self.raw {
            if *x != id {
                continue;
            }
            if *t > now {
                break;
            }

            let count = windows_per_mode.get_mut(m).unwrap().add(*t);
            pts_per_mode.get_mut(m).unwrap().push((*t, count));
        }

        for (m, pts) in pts_per_mode.iter_mut() {
            let mut window = windows_per_mode.remove(m).unwrap();

            // Add a drop-off after window_size (+ a little epsilon!)
            let t = (pts.last().unwrap().0 + window_size + Duration::seconds(0.1)).min(now);
            if pts.last().unwrap().0 != t {
                pts.push((t, window.count(t)));
            }

            if pts.last().unwrap().0 != now {
                pts.push((now, window.count(now)));
            }
        }

        pts_per_mode.into_iter().collect()
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
