mod ui;

use std::collections::BTreeSet;

use abstio::MapName;
use abstutil::{Counter, Timer};
use geom::{Duration, Time};
use map_gui::tools::compare_counts::{CompareCounts, Counts};
use map_model::{Map, PathConstraints, PathRequest, PathStepV2, PathfinderCaching, RoutingParams};
use sim::{Scenario, TripEndpoint, TripMode};
use widgetry::EventCtx;

pub use self::ui::ShowResults;
use crate::App;

// TODO Configurable main road penalty, like in the pathfinding tool
// TODO Share structure or pieces with Ungap's predict mode
// ... can't we just produce data of a certain shape, and have a UI pretty tuned for that?

// This gets incrementally recalculated when stuff changes.
//
// - all_trips and everything else depends just on the map (we only have one scenario per map now)
// - filtered_trips depend on filters
// - the 'b' and 'relative' parts of compare_counts depend on change_key (for when the map is edited)
pub struct Impact {
    pub map: MapName,
    pub filters: Filters,

    all_trips: Vec<PathRequest>,
    filtered_trips: Vec<PathRequest>,

    pub compare_counts: CompareCounts,
    pub change_key: usize,
}

#[derive(PartialEq)]
pub struct Filters {
    pub modes: BTreeSet<TripMode>,
    // TODO Has no effect yet. Do we need to store the TripEndpoints / can we detect from the
    // PathRequest reasonably?
    pub include_borders: bool,
    pub departure_time: (Time, Time),
}

impl Impact {
    pub fn empty(ctx: &EventCtx) -> Self {
        Self {
            map: MapName::new("zz", "place", "holder"),
            filters: Filters {
                modes: vec![TripMode::Drive].into_iter().collect(),
                include_borders: true,
                departure_time: (Time::START_OF_DAY, end_of_day()),
            },

            all_trips: Vec::new(),
            filtered_trips: Vec::new(),

            compare_counts: CompareCounts::empty(ctx),
            change_key: 0,
        }
    }

    fn from_scenario(
        ctx: &mut EventCtx,
        app: &App,
        scenario: Scenario,
        timer: &mut Timer,
    ) -> Impact {
        let mut impact = Impact::empty(ctx);
        let map = &app.map;

        impact.map = app.map.get_name().clone();
        impact.change_key = app.session.modal_filters.change_key;
        impact.all_trips = timer
            .parallelize("analyze trips", scenario.all_trips().collect(), |trip| {
                TripEndpoint::path_req(trip.origin, trip.destination, trip.mode, map)
            })
            .into_iter()
            .flatten()
            .collect();
        impact.trips_changed(ctx, app, timer);
        impact.compare_counts.autoselect_layer();
        impact
    }

    fn trips_changed(&mut self, ctx: &mut EventCtx, app: &App, timer: &mut Timer) {
        let map = &app.map;
        let constraints: BTreeSet<PathConstraints> = self
            .filters
            .modes
            .iter()
            .map(|m| m.to_constraints())
            .collect();
        self.filtered_trips = self
            .all_trips
            .iter()
            .filter(|req| constraints.contains(&req.constraints))
            .cloned()
            .collect();

        let counts_a = count_throughput(
            map,
            // Don't bother describing all the trip filtering
            "before filters".to_string(),
            &self.filtered_trips,
            map.routing_params().clone(),
            PathfinderCaching::NoCache,
            timer,
        );

        let counts_b = {
            let mut params = map.routing_params().clone();
            app.session.modal_filters.update_routing_params(&mut params);
            // Since we're making so many requests, it's worth it to rebuild a contraction
            // hierarchy. And since we're single-threaded, no complications there.
            count_throughput(
                map,
                // Don't bother describing all the trip filtering
                "after filters".to_string(),
                &self.filtered_trips,
                params,
                PathfinderCaching::CacheCH,
                timer,
            )
        };

        self.compare_counts =
            CompareCounts::new(ctx, app, counts_a, counts_b, self.compare_counts.layer);
    }

    fn map_edits_changed(&mut self, ctx: &mut EventCtx, app: &App, timer: &mut Timer) {
        self.change_key = app.session.modal_filters.change_key;
        let map = &app.map;

        let counts_b = {
            let mut params = map.routing_params().clone();
            app.session.modal_filters.update_routing_params(&mut params);
            // Since we're making so many requests, it's worth it to rebuild a contraction
            // hierarchy. And since we're single-threaded, no complications there.
            count_throughput(
                map,
                // Don't bother describing all the trip filtering
                "after filters".to_string(),
                &self.filtered_trips,
                params,
                PathfinderCaching::CacheCH,
                timer,
            )
        };
        self.compare_counts.recalculate_b(ctx, app, counts_b);
    }
}

fn count_throughput(
    map: &Map,
    description: String,
    requests: &[PathRequest],
    params: RoutingParams,
    cache_custom: PathfinderCaching,
    timer: &mut Timer,
) -> Counts {
    let mut road_counts = Counter::new();
    let mut intersection_counts = Counter::new();

    // Statistic::Min will be wrong later for roads that're 0. So explicitly start with 0 for every
    // road/intersection.
    for r in map.all_roads() {
        road_counts.add(r.id, 0);
    }
    for i in map.all_intersections() {
        intersection_counts.add(i.id, 0);
    }

    // It's very memory intensive to calculate all of the paths in one chunk, then process them to
    // get counts. Increment the counters as we go.
    //
    // TODO But that makes it hard to use timer.parallelize for this. We could make a thread-local
    // Counter and aggregte them at the end, but the way timer.parallelize uses scoped_threadpool
    // right now won't let that work. Stick to single-threaded for now.

    timer.start_iter("calculate routes", requests.len());
    for req in requests {
        timer.next();
        if let Ok(path) = map.pathfind_v2_with_params(req.clone(), &params, cache_custom) {
            for step in path.get_steps() {
                match step {
                    PathStepV2::Along(dr) | PathStepV2::Contraflow(dr) => {
                        road_counts.inc(dr.road);
                    }
                    PathStepV2::Movement(m) | PathStepV2::ContraflowMovement(m) => {
                        intersection_counts.inc(m.parent);
                    }
                }
            }
        }
    }

    Counts {
        map: map.get_name().clone(),
        description,
        per_road: road_counts.consume().into_iter().collect(),
        per_intersection: intersection_counts.consume().into_iter().collect(),
    }
}

// TODO Fixed, and sadly not const
fn end_of_day() -> Time {
    Time::START_OF_DAY + Duration::hours(24)
}
