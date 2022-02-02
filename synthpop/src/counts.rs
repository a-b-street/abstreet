use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{Counter, Timer};
use map_model::{
    IntersectionID, Map, PathRequest, PathStepV2, PathfinderCaching, RoadID, RoutingParams,
};

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
        params: RoutingParams,
        cache_custom: PathfinderCaching,
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
            if let Ok(path) = map.pathfind_v2_with_params(req.clone(), &params, cache_custom) {
                let count = *count;
                for step in path.get_steps() {
                    match step {
                        PathStepV2::Along(dr) | PathStepV2::Contraflow(dr) => {
                            counts.per_road.add(dr.road, count);
                        }
                        PathStepV2::Movement(m) | PathStepV2::ContraflowMovement(m) => {
                            counts.per_intersection.add(m.parent, count);
                        }
                    }
                }
            }
        }
        counts
    }
}
