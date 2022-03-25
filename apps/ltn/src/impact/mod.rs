mod ui;

use std::collections::BTreeSet;

use abstio::MapName;
use abstutil::Timer;
use geom::{Duration, Time};
use map_gui::tools::compare_counts::CompareCounts;
use map_model::{Path, PathConstraints, PathRequest, Pathfinder, RoadID};
use synthpop::{Scenario, TrafficCounts, TripEndpoint, TripMode};
use widgetry::EventCtx;

pub use self::ui::ShowResults;
use crate::filters::ChangeKey;
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
    // A subset of all_trips, and the number of times somebody takes the same trip
    filtered_trips: Vec<(PathRequest, usize)>,

    pub compare_counts: CompareCounts,
    pub change_key: ChangeKey,
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
            change_key: ChangeKey::default(),
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
        impact.change_key = app.session.modal_filters.get_change_key();
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
        self.filtered_trips = PathRequest::deduplicate(
            map,
            self.all_trips
                .iter()
                .filter(|req| constraints.contains(&req.constraints))
                .cloned()
                .collect(),
        );

        let counts_a = TrafficCounts::from_path_requests(
            map,
            // Don't bother describing all the trip filtering
            "before filters".to_string(),
            &self.filtered_trips,
            map.get_pathfinder(),
            timer,
        );

        let counts_b = self.counts_b(app, timer);

        let clickable_roads = true;
        self.compare_counts = CompareCounts::new(
            ctx,
            app,
            counts_a,
            counts_b,
            self.compare_counts.layer,
            clickable_roads,
        );
    }

    fn map_edits_changed(&mut self, ctx: &mut EventCtx, app: &App, timer: &mut Timer) {
        self.change_key = app.session.modal_filters.get_change_key();
        let counts_b = self.counts_b(app, timer);
        self.compare_counts.recalculate_b(ctx, app, counts_b);
    }

    fn counts_b(&self, app: &App, timer: &mut Timer) -> TrafficCounts {
        let constraints: BTreeSet<PathConstraints> = self
            .filters
            .modes
            .iter()
            .map(|m| m.to_constraints())
            .collect();

        let map = &app.map;
        let mut params = map.routing_params().clone();
        app.session.modal_filters.update_routing_params(&mut params);
        // Since we're making so many requests, it's worth it to rebuild a contraction hierarchy.
        // This depends on the current map edits, so no need to cache
        let pathfinder_after =
            Pathfinder::new_ch(map, params, constraints.into_iter().collect(), timer);

        // We can't simply use TrafficCounts::from_path_requests. Due to spurious diffs with paths,
        // we need to skip cases where the path before and after have the same cost. It's easiest
        // (code-wise) to just repeat some calculation here.
        let mut counts = TrafficCounts::from_path_requests(
            map,
            // Don't bother describing all the trip filtering
            "after filters".to_string(),
            &vec![],
            &pathfinder_after,
            timer,
        );

        timer.start_iter("calculate routes", self.filtered_trips.len());
        for (req, count) in &self.filtered_trips {
            timer.next();
            if let (Some(path1), Some(path2)) = (
                map.get_pathfinder().pathfind_v2(req.clone(), map),
                pathfinder_after.pathfind_v2(req.clone(), map),
            ) {
                if path1.get_cost() == path2.get_cost() {
                    // When the path maybe changed but the cost is the same, just count it the same
                    // as the original path
                    counts.update_with_path(path1, *count, map);
                } else {
                    counts.update_with_path(path2, *count, map);
                }
            }
        }

        counts
    }

    /// Returns routes that start or stop crossing the given road. Returns paths (before filters,
    /// after)
    pub fn find_changed_routes(
        &self,
        app: &App,
        r: RoadID,
        timer: &mut Timer,
    ) -> Vec<(Path, Path)> {
        let map = &app.map;
        // TODO Cache the pathfinder. It depends both on the change_key and modes belonging to
        // filtered_trips.
        let pathfinder_after = {
            let constraints: BTreeSet<PathConstraints> = self
                .filters
                .modes
                .iter()
                .map(|m| m.to_constraints())
                .collect();
            let mut params = map.routing_params().clone();
            app.session.modal_filters.update_routing_params(&mut params);
            Pathfinder::new_ch(map, params, constraints.into_iter().collect(), timer)
        };

        let mut changed = Vec::new();
        timer.start_iter("find changed routes", self.filtered_trips.len());
        for (req, _) in &self.filtered_trips {
            timer.next();
            if let (Some(path1), Some(path2)) = (
                map.get_pathfinder().pathfind_v2(req.clone(), map),
                pathfinder_after.pathfind_v2(req.clone(), map),
            ) {
                // Skip spurious changes where the cost matches.
                if path1.get_cost() == path2.get_cost() {
                    continue;
                }

                if path1.crosses_road(r) != path2.crosses_road(r) {
                    if let (Ok(path1), Ok(path2)) = (path1.into_v1(map), path2.into_v1(map)) {
                        changed.push((path1, path2));
                    }
                }
            }
        }
        changed
    }
}

// TODO Fixed, and sadly not const
fn end_of_day() -> Time {
    Time::START_OF_DAY + Duration::hours(24)
}
