mod ui;

use abstio::MapName;
use abstutil::{prettyprint_usize, Counter, Timer};
use geom::{Distance, Histogram, Statistic};
use map_gui::tools::{cmp_count, ColorNetwork, DivergingScale};
use map_model::{
    IntersectionID, Map, PathRequest, PathStepV2, PathfinderCaching, RoadID, RoutingParams,
};
use sim::{Scenario, TripEndpoint, TripMode};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{Color, EventCtx, GeomBatch, Line, Text};

pub use self::ui::ShowResults;
use crate::App;

// TODO Configurable main road penalty, like in the pathfinding tool
// TODO Don't allow crossing filters at all -- don't just disincentivize
// TODO Share structure or pieces with Ungap's predict mode
// ... can't we just produce data of a certain shape, and have a UI pretty tuned for that?

pub struct Results {
    pub map: MapName,
    // This changes per map
    all_driving_trips: Vec<PathRequest>,
    before_world: World<Obj>,
    pub before_road_counts: Counter<RoadID>,
    pub before_intersection_counts: Counter<IntersectionID>,

    // The rest need updating when this changes
    pub change_key: usize,
    after_world: World<Obj>,
    pub after_road_counts: Counter<RoadID>,
    pub after_intersection_counts: Counter<IntersectionID>,
    relative_world: World<Obj>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Road(RoadID),
    Intersection(IntersectionID),
}
impl ObjectID for Obj {}

impl Results {
    fn from_scenario(
        ctx: &mut EventCtx,
        app: &App,
        scenario: Scenario,
        timer: &mut Timer,
    ) -> Results {
        let map = &app.map;
        let all_driving_trips = timer
            .parallelize(
                "analyze trips",
                scenario
                    .all_trips()
                    .filter(|trip| trip.mode == TripMode::Drive)
                    .collect(),
                |trip| TripEndpoint::path_req(trip.origin, trip.destination, TripMode::Drive, map),
            )
            .into_iter()
            .flatten()
            .collect();

        let mut results = Results {
            map: app.map.get_name().clone(),
            all_driving_trips,
            before_world: World::unbounded(),
            before_road_counts: Counter::new(),
            before_intersection_counts: Counter::new(),

            change_key: 0,
            after_world: World::unbounded(),
            after_road_counts: Counter::new(),
            after_intersection_counts: Counter::new(),
            relative_world: World::unbounded(),
        };
        results.recalculate_impact(ctx, app, timer);
        results
    }

    fn recalculate_impact(&mut self, ctx: &mut EventCtx, app: &App, timer: &mut Timer) {
        self.change_key = app.session.modal_filters.change_key;
        let map = &app.map;

        // Before the filters. These don't change with no filters, so only calculate once per map
        if self.before_road_counts.is_empty() {
            let (roads, intersections) = count_throughput(
                &self.all_driving_trips,
                map,
                map.routing_params().clone(),
                PathfinderCaching::NoCache,
                timer,
            );
            self.before_road_counts = roads;
            self.before_intersection_counts = intersections;

            self.before_world = make_world(ctx, app);
            let mut colorer = ColorNetwork::no_fading(app);
            colorer.ranked_roads(self.before_road_counts.clone(), &app.cs.good_to_bad_red);
            colorer.ranked_intersections(
                self.before_intersection_counts.clone(),
                &app.cs.good_to_bad_red,
            );
            self.before_world
                .draw_master_batch_built(colorer.build(ctx));
        }

        // After the filters
        {
            let mut params = map.routing_params().clone();
            app.session.modal_filters.update_routing_params(&mut params);
            // Since we're making so many requests, it's worth it to rebuild a contraction
            // hierarchy. And since we're single-threaded, no complications there.
            let (roads, intersections) = count_throughput(
                &self.all_driving_trips,
                map,
                params,
                PathfinderCaching::CacheCH,
                timer,
            );
            self.after_road_counts = roads;
            self.after_intersection_counts = intersections;

            self.after_world = make_world(ctx, app);
            let mut colorer = ColorNetwork::no_fading(app);
            colorer.ranked_roads(self.after_road_counts.clone(), &app.cs.good_to_bad_red);
            colorer.ranked_intersections(
                self.after_intersection_counts.clone(),
                &app.cs.good_to_bad_red,
            );
            self.after_world.draw_master_batch_built(colorer.build(ctx));
        }

        self.recalculate_relative_diff(ctx, app);
    }

    fn recalculate_relative_diff(&mut self, ctx: &mut EventCtx, app: &App) {
        let map = &app.map;

        // First just understand the counts...
        let mut hgram_before = Histogram::new();
        for (_, cnt) in self.before_road_counts.borrow() {
            hgram_before.add(*cnt);
        }
        let mut hgram_after = Histogram::new();
        for (_, cnt) in self.after_road_counts.borrow() {
            hgram_after.add(*cnt);
        }
        info!("Road counts before: {}", hgram_before.describe());
        info!("Road counts after: {}", hgram_after.describe());

        // What's physical road width look like?
        let mut hgram_width = Histogram::new();
        for r in app.map.all_roads() {
            hgram_width.add(r.get_width());
        }
        info!("Physical road widths: {}", hgram_width.describe());

        // TODO This is still a bit arbitrary
        let scale = DivergingScale::new(Color::hex("#5D9630"), Color::WHITE, Color::hex("#A32015"))
            .range(0.0, 2.0);

        // Draw road width based on the count before
        // TODO unwrap will crash on an empty demand model
        let min_count = hgram_before.select(Statistic::Min).unwrap();
        let max_count = hgram_before.select(Statistic::Max).unwrap();

        let mut draw_roads = GeomBatch::new();
        for (r, before, after) in self
            .before_road_counts
            .clone()
            .compare(self.after_road_counts.clone())
        {
            let ratio = (after as f64) / (before as f64);
            let color = if let Some(c) = scale.eval(ratio) {
                c
            } else {
                continue;
            };

            // TODO Refactor histogram helpers
            let pct_count = (before - min_count) as f64 / (max_count - min_count) as f64;
            // TODO Pretty arbitrary. Ideally we'd hide roads and intersections underneath...
            let width = Distance::meters(2.0) + pct_count * Distance::meters(10.0);

            draw_roads.push(color, map.get_r(r).center_pts.make_polygons(width));
        }
        self.relative_world = make_world(ctx, app);
        self.relative_world.draw_master_batch(ctx, draw_roads);
    }

    pub fn relative_road_tooltip(&self, r: RoadID) -> Text {
        let before = self.before_road_counts.get(r);
        let after = self.after_road_counts.get(r);
        let ratio = (after as f64) / (before as f64);

        let mut txt = Text::from_multiline(vec![
            Line(format!("Before: {}", prettyprint_usize(before))),
            Line(format!("After: {}", prettyprint_usize(after))),
        ]);
        cmp_count(&mut txt, before, after);
        txt.add_line(Line(format!("After/before: {:.2}", ratio)));
        txt
    }
}

fn count_throughput(
    requests: &[PathRequest],
    map: &Map,
    params: RoutingParams,
    cache_custom: PathfinderCaching,
    timer: &mut Timer,
) -> (Counter<RoadID>, Counter<IntersectionID>) {
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
                // No Contraflow steps for driving paths
                match step {
                    PathStepV2::Along(dr) => {
                        road_counts.inc(dr.road);
                    }
                    PathStepV2::Movement(m) => {
                        intersection_counts.inc(m.parent);
                    }
                    _ => {}
                }
            }
        }
    }

    (road_counts, intersection_counts)
}

// Creates a world that just has placeholders for hovering on roads and intersections. The caller
// manually handles drawing and tooltips.
//
// TODO This is necessary to avoid running out of video memory. World should be able to lazily
// create tooltips and more easily merge drawn objects together in a master batch.
// https://github.com/a-b-street/abstreet/issues/763
fn make_world(ctx: &mut EventCtx, app: &App) -> World<Obj> {
    let mut world = World::bounded(app.map.get_bounds());
    for r in app.map.all_roads() {
        world
            .add(Obj::Road(r.id))
            .hitbox(r.get_thick_polygon())
            .drawn_in_master_batch()
            .invisibly_hoverable()
            .build(ctx);
    }
    for i in app.map.all_intersections() {
        world
            .add(Obj::Intersection(i.id))
            .hitbox(i.polygon.clone())
            .drawn_in_master_batch()
            .invisibly_hoverable()
            .build(ctx);
    }
    world
}
