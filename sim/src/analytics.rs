use crate::{CarID, Event, TripMode};
use abstutil::Counter;
use derivative::Derivative;
use geom::{Duration, DurationHistogram, DurationStats};
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
    // Finish time, mode (or None as aborted), trip duration
    pub finished_trips: Vec<(Duration, Option<TripMode>, Duration)>,
}

#[derive(Serialize, Deserialize, Derivative)]
pub struct ThruputStats {
    #[serde(skip_serializing, skip_deserializing)]
    pub count_per_road: Counter<RoadID>,
    #[serde(skip_serializing, skip_deserializing)]
    pub count_per_intersection: Counter<IntersectionID>,
}

impl Analytics {
    pub fn new() -> Analytics {
        Analytics {
            thruput_stats: ThruputStats {
                count_per_road: Counter::new(),
                count_per_intersection: Counter::new(),
            },
            test_expectations: VecDeque::new(),
            bus_arrivals: Vec::new(),
            total_bus_passengers: Counter::new(),
            finished_trips: Vec::new(),
        }
    }

    pub fn event(&mut self, ev: Event, time: Duration, map: &Map) {
        // Throughput
        if let Event::AgentEntersTraversable(_, to) = ev {
            match to {
                Traversable::Lane(l) => self.thruput_stats.count_per_road.inc(map.get_l(l).parent),
                Traversable::Turn(t) => self.thruput_stats.count_per_intersection.inc(t.parent),
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
        if let Event::TripFinished(_, mode, dt) = ev {
            self.finished_trips.push((time, Some(mode), dt));
        } else if let Event::TripAborted(_) = ev {
            self.finished_trips.push((time, None, Duration::ZERO));
        }
    }

    // TODO If these ever need to be speeded up, just cache the histogram and index in the events
    // list.

    pub fn finished_trips(&self, now: Duration, mode: TripMode) -> DurationStats {
        let mut distrib = DurationHistogram::new();
        for (t, m, dt) in &self.finished_trips {
            if *t > now {
                break;
            }
            if *m == Some(mode) {
                distrib.add(*dt);
            }
        }
        distrib.to_stats()
    }

    pub fn bus_arrivals(&self, now: Duration, r: BusRouteID) -> BTreeMap<BusStopID, DurationStats> {
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
            .into_iter()
            .map(|(k, v)| (k, v.to_stats()))
            .collect()
    }
}
