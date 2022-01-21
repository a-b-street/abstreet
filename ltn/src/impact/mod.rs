mod ui;

use abstio::MapName;
use abstutil::{prettyprint_usize, Counter, Timer};
use map_gui::tools::{cmp_count, ColorScale, DivergingScale};
use map_model::{IntersectionID, Map, PathRequest, PathStepV2, PathfinderCaching, RoadID};
use sim::{Scenario, TripEndpoint, TripMode};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{Color, EventCtx, Line, Text};

pub use self::ui::ShowResults;
use crate::{App, NeighborhoodID};

// TODO Configurable main road penalty, like in the pathfinding tool
// TODO Don't allow crossing filters at all -- don't just disincentivize
// TODO Share structure or pieces with Ungap's predict mode
// ... can't we just produce data of a certain shape, and have a UI pretty tuned for that?

pub struct Results {
    pub map: MapName,
    // This changes per map
    all_driving_trips: Vec<PathRequest>,
    before_world: World<Obj>,
    before_road_counts: Counter<RoadID>,
    before_intersection_counts: Counter<IntersectionID>,

    // The rest need updating when this changes
    pub change_key: usize,
    after_world: World<Obj>,
    after_road_counts: Counter<RoadID>,
    after_intersection_counts: Counter<IntersectionID>,
    relative_world: World<Obj>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Road(RoadID),
    Intersection(IntersectionID),
    Neighborhood(NeighborhoodID),
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
            self.before_road_counts = Counter::new();
            self.before_intersection_counts = Counter::new();
            for path in timer
                .parallelize(
                    "calculate routes before filters",
                    self.all_driving_trips.clone(),
                    |req| map.pathfind_v2(req),
                )
                .into_iter()
                .flatten()
            {
                for step in path.get_steps() {
                    // No Contraflow steps for driving paths
                    match step {
                        PathStepV2::Along(dr) => {
                            self.before_road_counts.inc(dr.road);
                        }
                        PathStepV2::Movement(m) => {
                            self.before_intersection_counts.inc(m.parent);
                        }
                        _ => {}
                    }
                }
            }
            self.before_world = make_world(ctx, app);
            ranked_roads(
                ctx,
                map,
                &mut self.before_world,
                &self.before_road_counts,
                &app.cs.good_to_bad_red,
            );
            ranked_intersections(
                ctx,
                map,
                &mut self.before_world,
                &self.before_intersection_counts,
                &app.cs.good_to_bad_red,
            );
        }

        // After the filters
        self.after_road_counts = Counter::new();
        self.after_intersection_counts = Counter::new();
        let mut params = map.routing_params().clone();
        app.session.modal_filters.update_routing_params(&mut params);
        for path in timer
            .parallelize(
                "calculate routes after filters",
                self.all_driving_trips.clone(),
                |req| map.pathfind_v2_with_params(req, &params, PathfinderCaching::CacheDijkstra),
            )
            .into_iter()
            .flatten()
        {
            for step in path.get_steps() {
                // No Contraflow steps for driving paths
                match step {
                    PathStepV2::Along(dr) => {
                        self.after_road_counts.inc(dr.road);
                    }
                    PathStepV2::Movement(m) => {
                        self.after_intersection_counts.inc(m.parent);
                    }
                    _ => {}
                }
            }
        }
        self.after_world = make_world(ctx, app);
        ranked_roads(
            ctx,
            map,
            &mut self.after_world,
            &self.after_road_counts,
            &app.cs.good_to_bad_red,
        );
        ranked_intersections(
            ctx,
            map,
            &mut self.after_world,
            &self.after_intersection_counts,
            &app.cs.good_to_bad_red,
        );

        self.recalculate_relative_diff(ctx, app);
    }

    fn recalculate_relative_diff(&mut self, ctx: &mut EventCtx, app: &App) {
        let map = &app.map;
        self.relative_world = make_world(ctx, app);
        // TODO I really need help understanding how to do this. If the average isn't 1.0 (meaning
        // no change), then the colors are super wacky.
        let scale = DivergingScale::new(Color::hex("#5D9630"), Color::WHITE, Color::hex("#A32015"))
            .range(0.0, 2.0);

        let mut min_ratio: f64 = 100000.0;
        let mut max_ratio: f64 = 0.0;

        for (r, before, after) in self
            .before_road_counts
            .clone()
            .compare(self.after_road_counts.clone())
        {
            let ratio = (after as f64) / (before as f64);
            if let Some(color) = scale.eval(ratio) {
                let mut txt = Text::from_multiline(vec![
                    Line(format!("Before: {}", prettyprint_usize(before))),
                    Line(format!("After: {}", prettyprint_usize(after))),
                ]);
                cmp_count(&mut txt, before, after);
                txt.add_line(Line(format!("After/before: {:.2}", ratio)));
                self.relative_world
                    .add(Obj::Road(r))
                    .hitbox(map.get_r(r).get_thick_polygon())
                    .draw_color(color)
                    .hover_alpha(0.9)
                    .tooltip(txt)
                    .build(ctx);
            }
            min_ratio = min_ratio.min(ratio);
            max_ratio = max_ratio.max(ratio);
        }
        info!("The ratios were between {min_ratio:.2} and {max_ratio:.2}");

        for (i, before, after) in self
            .before_intersection_counts
            .clone()
            .compare(self.after_intersection_counts.clone())
        {
            let ratio = (after as f64) / (before as f64);
            if let Some(color) = scale.eval(ratio) {
                let mut txt = Text::from_multiline(vec![
                    Line(format!("Before: {}", prettyprint_usize(before))),
                    Line(format!("After: {}", prettyprint_usize(after))),
                ]);
                cmp_count(&mut txt, before, after);
                txt.add_line(Line(format!("After/before: {:.2}", ratio)));
                self.relative_world
                    .add(Obj::Intersection(i))
                    .hitbox(map.get_i(i).polygon.clone())
                    .draw_color(color)
                    .hover_alpha(0.9)
                    .tooltip(txt)
                    .build(ctx);
            }
        }
    }
}

// Just add the base layer of non-clickable neighborhoods
fn make_world(ctx: &mut EventCtx, app: &App) -> World<Obj> {
    let mut world = World::bounded(app.map.get_bounds());
    for (id, (block, color)) in &app.session.partitioning.neighborhoods {
        world
            .add(Obj::Neighborhood(*id))
            .hitbox(block.polygon.clone())
            .draw_color(color.alpha(0.2))
            .build(ctx);
    }
    world
}

// TODO Duplicates some logic from ColorNetwork
fn ranked_roads(
    ctx: &mut EventCtx,
    map: &Map,
    world: &mut World<Obj>,
    counter: &Counter<RoadID>,
    scale: &ColorScale,
) {
    let roads = counter.sorted_asc();
    let len = roads.len() as f64;
    for (idx, list) in roads.into_iter().enumerate() {
        let color = scale.eval((idx as f64) / len);
        for r in list {
            world
                .add(Obj::Road(r))
                .hitbox(map.get_r(r).get_thick_polygon())
                .draw_color(color)
                .hover_alpha(0.9)
                .tooltip(Text::from(Line(prettyprint_usize(counter.get(r)))))
                .build(ctx);
        }
    }
}

fn ranked_intersections(
    ctx: &mut EventCtx,
    map: &Map,
    world: &mut World<Obj>,
    counter: &Counter<IntersectionID>,
    scale: &ColorScale,
) {
    let intersections = counter.sorted_asc();
    let len = intersections.len() as f64;
    for (idx, list) in intersections.into_iter().enumerate() {
        let color = scale.eval((idx as f64) / len);
        for i in list {
            world
                .add(Obj::Intersection(i))
                .hitbox(map.get_i(i).polygon.clone())
                .draw_color(color)
                .hover_alpha(0.9)
                .tooltip(Text::from(Line(prettyprint_usize(counter.get(i)))))
                .build(ctx);
        }
    }
}
