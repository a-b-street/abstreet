use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{prettyprint_usize, Counter, Timer};
use geom::Distance;
use map_model::{IntersectionID, Map, PathRequest, PathStepV2, PathV2, Pathfinder, RoadID};

/// This represents the number of vehicles (or trips, or something else) crossing roads and
/// intersections over some span of time. The data could represent real observations or something
/// from a simulation.
///
/// There's some nice UIs in other crates to compare counts.
#[derive(Clone, Serialize, Deserialize)]
pub struct TrafficCounts {
    pub map: MapName,
    // TODO For now, squeeze everything into this -- mode, weekday/weekend, time of day, data
    // source, etc
    pub description: String,
    // TODO Maybe per direction, movement
    pub per_road: Counter<RoadID>,
    pub per_intersection: Counter<IntersectionID>,
}

impl Default for TrafficCounts {
    fn default() -> Self {
        Self {
            map: MapName::new("zz", "place", "holder"),
            description: String::new(),
            per_road: Counter::new(),
            per_intersection: Counter::new(),
        }
    }
}

impl TrafficCounts {
    /// Run pathfinding on all of the requests, then count the throughput on every road and
    /// intersection. Each request has the count it should contribute -- use
    /// `PathRequest::deduplicate` to easily generate this.
    pub fn from_path_requests(
        map: &Map,
        description: String,
        requests: &[(PathRequest, usize)],
        pathfinder: &Pathfinder,
        timer: &mut Timer,
    ) -> Self {
        let mut counts = Self {
            map: map.get_name().clone(),
            description,
            per_road: Counter::new(),
            per_intersection: Counter::new(),
        };

        // Statistic::Min will be wrong later for roads that're 0. So explicitly start with 0 for every
        // road/intersection.
        for r in map.all_roads() {
            counts.per_road.add(r.id, 0);
        }
        for i in map.all_intersections() {
            counts.per_intersection.add(i.id, 0);
        }

        // It's very memory intensive to calculate all of the paths in one chunk, then process them to
        // get counts. Increment the counters as we go.
        //
        // TODO But that makes it hard to use timer.parallelize for this. We could make a thread-local
        // Counter and aggregte them at the end, but the way timer.parallelize uses scoped_threadpool
        // right now won't let that work. Stick to single-threaded for now.

        timer.start_iter("calculate routes", requests.len());
        for (req, count) in requests {
            timer.next();
            if let Some(path) = pathfinder.pathfind_v2(req.clone(), map) {
                counts.update_with_path(path, *count, map);
            }
        }
        counts
    }

    pub fn update_with_path(&mut self, path: PathV2, count: usize, map: &Map) {
        for step in path.get_steps() {
            match step {
                PathStepV2::Along(dr) | PathStepV2::Contraflow(dr) => {
                    self.per_road.add(dr.road, count);
                }
                PathStepV2::Movement(m) | PathStepV2::ContraflowMovement(m) => {
                    self.per_intersection.add(m.parent, count);
                }
            }
        }

        // If we're starting or ending at a border, count it
        let req = path.get_req();
        if req.start.dist_along() == Distance::ZERO {
            // TODO src_i and dst_i may not work for pedestrians on contraflow sidewalks
            let i = map.get_l(req.start.lane()).src_i;
            if map.get_i(i).is_border() {
                self.per_intersection.add(i, count);
            }
        } else {
            let i = map.get_l(req.end.lane()).dst_i;
            if map.get_i(i).is_border() {
                self.per_intersection.add(i, count);
            }
        }
    }

    /// Print a comparison of counts. Only look at roads/intersections in `self`.
    pub fn quickly_compare(&self, other: &TrafficCounts) {
        // TODO Easy ASCII art table without huge dependencies?
        println!("{} vs {}", self.description, other.description);
        let mut sum = 0.0;
        let mut n = 0;
        for (r, cnt1) in self.per_road.borrow() {
            let cnt1 = *cnt1;
            let cnt2 = other.per_road.get(*r);
            println!(
                "{}: {} vs {}",
                r,
                prettyprint_usize(cnt1),
                prettyprint_usize(cnt2)
            );
            sum += (cnt1 as f64 - cnt2 as f64).powi(2);
            n += 1;
        }
        for (i, cnt1) in self.per_intersection.borrow() {
            let cnt1 = *cnt1;
            let cnt2 = other.per_intersection.get(*i);
            println!(
                "{}: {} vs {}",
                i,
                prettyprint_usize(cnt1),
                prettyprint_usize(cnt2)
            );
            sum += (cnt1 as f64 - cnt2 as f64).powi(2);
            n += 1;
        }
        println!("RMSE = {:.2}", (sum / n as f64).sqrt());
    }
}
