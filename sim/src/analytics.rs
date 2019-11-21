use crate::{AgentID, CarID, Event, TripID, TripMode, VehicleType};
use abstutil::Counter;
use derivative::Derivative;
use geom::{Duration, DurationHistogram};
use map_model::{BusRouteID, BusStopID, IntersectionID, Map, RoadID, Traversable};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

// Embed a deeper structure with its own impl when that makes sense, or feel free to just inline
// things.
#[derive(Serialize, Deserialize, Derivative)]
pub struct Analytics {
    pub thruput_stats: ThruputStats,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) test_expectations: VecDeque<Event>,
    pub bus_arrivals: Vec<(Duration, CarID, BusRouteID, BusStopID)>,
    #[serde(skip_serializing, skip_deserializing)]
    pub total_bus_passengers: Counter<BusRouteID>,
    // TODO Hack: No TripMode means aborted
    // Finish time, ID, mode (or None as aborted), trip duration
    pub finished_trips: Vec<(Duration, TripID, Option<TripMode>, Duration)>,
}

#[derive(Serialize, Deserialize, Derivative)]
pub struct ThruputStats {
    #[serde(skip_serializing, skip_deserializing)]
    pub count_per_road: Counter<RoadID>,
    #[serde(skip_serializing, skip_deserializing)]
    pub count_per_intersection: Counter<IntersectionID>,

    raw_per_road: Vec<(Duration, TripMode, RoadID)>,
    raw_per_intersection: Vec<(Duration, TripMode, IntersectionID)>,
}

impl Analytics {
    pub fn new() -> Analytics {
        Analytics {
            thruput_stats: ThruputStats {
                count_per_road: Counter::new(),
                count_per_intersection: Counter::new(),
                raw_per_road: Vec::new(),
                raw_per_intersection: Vec::new(),
            },
            test_expectations: VecDeque::new(),
            bus_arrivals: Vec::new(),
            total_bus_passengers: Counter::new(),
            finished_trips: Vec::new(),
        }
    }

    pub fn event(&mut self, ev: Event, time: Duration, map: &Map) {
        // TODO Plumb a flag
        let raw_thruput = true;

        // Throughput
        if let Event::AgentEntersTraversable(a, to) = ev {
            let mode = match a {
                AgentID::Pedestrian(_) => TripMode::Walk,
                AgentID::Car(c) => match c.1 {
                    VehicleType::Car => TripMode::Drive,
                    VehicleType::Bike => TripMode::Bike,
                    VehicleType::Bus => TripMode::Transit,
                },
            };

            match to {
                Traversable::Lane(l) => {
                    let r = map.get_l(l).parent;
                    self.thruput_stats.count_per_road.inc(r);
                    if raw_thruput {
                        self.thruput_stats.raw_per_road.push((time, mode, r));
                    }
                }
                Traversable::Turn(t) => {
                    self.thruput_stats.count_per_intersection.inc(t.parent);
                    if raw_thruput {
                        self.thruput_stats
                            .raw_per_intersection
                            .push((time, mode, t.parent));
                    }
                }
            };
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
        if let Event::PedEntersBus(_, _, route) = ev {
            self.total_bus_passengers.inc(route);
        }

        // Finished trips
        if let Event::TripFinished(id, mode, dt) = ev {
            self.finished_trips.push((time, id, Some(mode), dt));
        } else if let Event::TripAborted(id) = ev {
            self.finished_trips.push((time, id, None, Duration::ZERO));
        }
    }

    // TODO If these ever need to be speeded up, just cache the histogram and index in the events
    // list.

    pub fn finished_trips(&self, now: Duration, mode: TripMode) -> DurationHistogram {
        let mut distrib = DurationHistogram::new();
        for (t, _, m, dt) in &self.finished_trips {
            if *t > now {
                break;
            }
            if *m == Some(mode) {
                distrib.add(*dt);
            }
        }
        distrib
    }

    // Returns (all trips except aborted, number of aborted trips, trips by mode)
    pub fn all_finished_trips(
        &self,
        now: Duration,
    ) -> (
        DurationHistogram,
        usize,
        BTreeMap<TripMode, DurationHistogram>,
    ) {
        let mut per_mode = TripMode::all()
            .into_iter()
            .map(|m| (m, DurationHistogram::new()))
            .collect::<BTreeMap<_, _>>();
        let mut all = DurationHistogram::new();
        let mut num_aborted = 0;
        for (t, _, m, dt) in &self.finished_trips {
            if *t > now {
                break;
            }
            if let Some(mode) = *m {
                all.add(*dt);
                per_mode.get_mut(&mode).unwrap().add(*dt);
            } else {
                num_aborted += 1;
            }
        }
        (all, num_aborted, per_mode)
    }

    pub fn bus_arrivals(
        &self,
        now: Duration,
        r: BusRouteID,
    ) -> BTreeMap<BusStopID, DurationHistogram> {
        let mut per_bus: BTreeMap<CarID, Vec<(Duration, BusStopID)>> = BTreeMap::new();
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
        let mut delay_to_stop: BTreeMap<BusStopID, DurationHistogram> = BTreeMap::new();
        for events in per_bus.values() {
            for pair in events.windows(2) {
                delay_to_stop
                    .entry(pair[1].1)
                    .or_insert_with(DurationHistogram::new)
                    .add(pair[1].0 - pair[0].0);
            }
        }
        delay_to_stop
    }

    // TODO Refactor!
    // For each stop, a list of (time, delay)
    pub fn bus_arrivals_over_time(
        &self,
        now: Duration,
        r: BusRouteID,
    ) -> BTreeMap<BusStopID, Vec<(Duration, Duration)>> {
        let mut per_bus: BTreeMap<CarID, Vec<(Duration, BusStopID)>> = BTreeMap::new();
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
        let mut delays_to_stop: BTreeMap<BusStopID, Vec<(Duration, Duration)>> = BTreeMap::new();
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

    // Slightly misleading -- TripMode::Transit means buses, not pedestrians taking transit
    pub fn throughput_road(
        &self,
        now: Duration,
        road: RoadID,
        bucket: Duration,
    ) -> BTreeMap<TripMode, Vec<(Duration, usize)>> {
        let mut max_this_bucket = now.min(bucket);
        let mut per_mode = TripMode::all()
            .into_iter()
            .map(|m| (m, vec![(Duration::ZERO, 0), (max_this_bucket, 0)]))
            .collect::<BTreeMap<_, _>>();
        for (t, m, r) in &self.thruput_stats.raw_per_road {
            if *r != road {
                continue;
            }
            if *t > now {
                break;
            }
            if *t > max_this_bucket {
                max_this_bucket = now.min(max_this_bucket + bucket);
                for vec in per_mode.values_mut() {
                    vec.push((max_this_bucket, 0));
                }
            }
            per_mode.get_mut(m).unwrap().last_mut().unwrap().1 += 1;
        }
        per_mode
    }

    // TODO Refactor!
    pub fn throughput_intersection(
        &self,
        now: Duration,
        intersection: IntersectionID,
        bucket: Duration,
    ) -> BTreeMap<TripMode, Vec<(Duration, usize)>> {
        let mut per_mode = TripMode::all()
            .into_iter()
            .map(|m| (m, vec![(Duration::ZERO, 0)]))
            .collect::<BTreeMap<_, _>>();
        let mut max_this_bucket = bucket;
        for (t, m, i) in &self.thruput_stats.raw_per_intersection {
            if *i != intersection {
                continue;
            }
            if *t > now {
                break;
            }
            if *t > max_this_bucket {
                max_this_bucket = now.min(max_this_bucket + bucket);
                for vec in per_mode.values_mut() {
                    vec.push((max_this_bucket, 0));
                }
            }
            per_mode.get_mut(m).unwrap().last_mut().unwrap().1 += 1;
        }
        per_mode
    }
}
