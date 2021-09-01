use std::collections::HashSet;

use geom::{Circle, Distance, Duration, FindClosest, PolyLine};
use map_model::{Path, PathStep, NORMAL_LANE_THICKNESS};
use sim::{TripEndpoint, TripMode};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, LinePlot, Outcome,
    Panel, PlotOptions, Series, State, Text, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::InputWaypoints;
use crate::ungap::{Layers, Tab, TakeLayers};

pub struct RoutePlanner {
    layers: Layers,
    once: bool,

    input_panel: Panel,
    waypoints: InputWaypoints,
    results: RouteResults,
}

impl TakeLayers for RoutePlanner {
    fn take_layers(self) -> Layers {
        self.layers
    }
}

impl RoutePlanner {
    pub fn new_state(ctx: &mut EventCtx, app: &App, layers: Layers) -> Box<dyn State<App>> {
        let mut rp = RoutePlanner {
            layers,
            once: true,

            input_panel: Panel::empty(ctx),
            waypoints: InputWaypoints::new(ctx, app),
            results: RouteResults::new(ctx, app, Vec::new()),
        };
        rp.update_input_panel(ctx, app);
        Box::new(rp)
    }

    fn update_input_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        self.input_panel = Panel::new_builder(Widget::col(vec![
            Tab::Route.make_header(ctx, app),
            self.waypoints.get_panel_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
    }
}

impl State<App> for RoutePlanner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if self.once {
            self.once = false;
            ctx.loading_screen("apply edits", |_, mut timer| {
                app.primary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);
            });
        }

        match self.input_panel.event(ctx) {
            // TODO Inverting control is hard. Who should try to handle the outcome first?
            Outcome::Clicked(x) if !x.starts_with("delete waypoint ") => {
                return Tab::Route.handle_action::<RoutePlanner>(ctx, app, &x);
            }
            outcome => {
                if self.waypoints.event(ctx, app, outcome) {
                    self.update_input_panel(ctx, app);
                    self.results = RouteResults::new(ctx, app, self.waypoints.get_waypoints());
                }
            }
        }

        self.results.event(ctx);

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.layers.draw(g, app);
        self.input_panel.draw(g);
        self.waypoints.draw(g);
        self.results.draw(g);
    }
}

struct RouteResults {
    // It's tempting to glue together all of the paths. But since some waypoints might force the
    // path to double back on itself, rendering the path as a single PolyLine would break.
    paths: Vec<(Path, Option<PolyLine>)>,
    // Match each polyline to the index in paths
    closest_path_segment: FindClosest<usize>,

    hover_on_line_plot: Option<(Distance, Drawable)>,
    draw_route: Drawable,
    panel: Panel,
}

impl RouteResults {
    fn new(ctx: &mut EventCtx, app: &App, waypoints: Vec<TripEndpoint>) -> RouteResults {
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;

        let mut total_distance = Distance::ZERO;
        let mut total_time = Duration::ZERO;

        let mut dist_along_high_stress_roads = Distance::ZERO;
        let mut num_traffic_signals = 0;
        let mut num_unprotected_turns = 0;

        let mut elevation_pts: Vec<(Distance, Distance)> = Vec::new();
        let mut current_dist = Distance::ZERO;

        let mut paths = Vec::new();
        let mut closest_path_segment = FindClosest::new(map.get_bounds());

        for pair in waypoints.windows(2) {
            if let Some(path) = TripEndpoint::path_req(pair[0], pair[1], TripMode::Bike, map)
                .and_then(|req| map.pathfind(req).ok())
            {
                total_distance += path.total_length();
                total_time += path.estimate_duration(map, Some(map_model::MAX_BIKE_SPEED));

                for step in path.get_steps() {
                    let this_dist = step.as_traversable().get_polyline(map).length();
                    match step {
                        PathStep::Lane(l) | PathStep::ContraflowLane(l) => {
                            if map.get_parent(*l).high_stress_for_bikes(map) {
                                dist_along_high_stress_roads += this_dist;
                            }
                        }
                        PathStep::Turn(t) => {
                            let i = map.get_i(t.parent);
                            elevation_pts.push((current_dist, i.elevation));
                            if i.is_traffic_signal() {
                                num_traffic_signals += 1;
                            }
                            if map.is_unprotected_turn(
                                map.get_l(t.src).parent,
                                map.get_l(t.dst).parent,
                                map.get_t(*t).turn_type,
                            ) {
                                num_unprotected_turns += 1;
                            }
                        }
                    }
                    current_dist += this_dist;
                }

                let maybe_pl = path.trace(map);
                if let Some(ref pl) = maybe_pl {
                    batch.push(Color::CYAN, pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS));
                    closest_path_segment.add(paths.len(), pl.points());
                }
                paths.push((path, maybe_pl));
            }
        }
        let draw_route = ctx.upload(batch);

        let mut total_up = Distance::ZERO;
        let mut total_down = Distance::ZERO;
        for pair in elevation_pts.windows(2) {
            let dy = pair[1].1 - pair[0].1;
            if dy < Distance::ZERO {
                total_down -= dy;
            } else {
                total_up += dy;
            }
        }

        let pct_stressful = if total_distance == Distance::ZERO {
            0.0
        } else {
            ((dist_along_high_stress_roads / total_distance) * 100.0).round()
        };
        let mut txt = Text::from(Line("Your route").small_heading());
        txt.add_appended(vec![
            Line("Distance: ").secondary(),
            Line(total_distance.to_string(&app.opts.units)),
        ]);
        // TODO Hover to see definition of high-stress, and also highlight those segments
        txt.add_appended(vec![
            Line(format!(
                "  {} or {}%",
                dist_along_high_stress_roads.to_string(&app.opts.units),
                pct_stressful
            )),
            Line(" along high-stress roads").secondary(),
        ]);
        txt.add_appended(vec![
            Line("Estimated time: ").secondary(),
            Line(total_time.to_string(&app.opts.units)),
        ]);
        txt.add_appended(vec![
            Line("Traffic signals crossed: ").secondary(),
            Line(num_traffic_signals.to_string()),
        ]);
        // TODO Need tooltips and highlighting to explain and show where these are
        txt.add_appended(vec![
            Line("Unprotected left turns onto busy roads: ").secondary(),
            Line(num_unprotected_turns.to_string()),
        ]);

        let panel = Panel::new_builder(Widget::col(vec![
            txt.into_widget(ctx),
            Text::from_all(vec![
                Line("Elevation change: ").secondary(),
                Line(format!(
                    "{}↑, {}↓",
                    total_up.to_string(&app.opts.units),
                    total_down.to_string(&app.opts.units)
                )),
            ])
            .into_widget(ctx),
            LinePlot::new_widget(
                ctx,
                "elevation",
                vec![Series {
                    label: "Elevation".to_string(),
                    color: Color::RED,
                    pts: elevation_pts,
                }],
                PlotOptions {
                    filterable: false,
                    max_x: Some(current_dist.round_up_for_axis()),
                    max_y: Some(map.max_elevation().round_up_for_axis()),
                    disabled: HashSet::new(),
                },
            ),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx);

        RouteResults {
            draw_route,
            panel,
            paths,
            closest_path_segment,
            hover_on_line_plot: None,
        }
    }

    fn event(&mut self, ctx: &mut EventCtx) {
        // No outcomes, just trigger the LinePlot to update hover state
        self.panel.event(ctx);

        let current_dist_along = self
            .panel
            .find::<LinePlot<Distance, Distance>>("elevation")
            .get_hovering()
            .get(0)
            .map(|pair| pair.0);
        if self.hover_on_line_plot.as_ref().map(|pair| pair.0) != current_dist_along {
            self.hover_on_line_plot = current_dist_along.map(|mut dist| {
                let mut batch = GeomBatch::new();
                // Find this position on the route
                for (path, maybe_pl) in &self.paths {
                    if dist > path.total_length() {
                        dist -= path.total_length();
                        continue;
                    }
                    if let Some(ref pl) = maybe_pl {
                        if let Ok((pt, _)) = pl.dist_along(dist) {
                            batch.push(
                                Color::CYAN,
                                Circle::new(pt, Distance::meters(30.0)).to_polygon(),
                            );
                        }
                    }
                    break;
                }

                (dist, batch.upload(ctx))
            });
        }

        if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            if let Some((idx, pt)) = self
                .closest_path_segment
                .closest_pt(pt, 10.0 * NORMAL_LANE_THICKNESS)
            {
                // Find the total distance along the route
                let mut dist = Distance::ZERO;
                for (path, _) in &self.paths[0..idx] {
                    dist += path.total_length();
                }
                if let Some(ref pl) = self.paths[idx].1 {
                    if let Some((dist_here, _)) = pl.dist_along_of_point(pt) {
                        // TODO We know we're dist + dist_here along the entire route. The LinePlot
                        // would need to store the original Series to look up elevation for us. So
                        // I guess we can do it ourselves here, then send in the X and Y and make
                        // it map that to its own screen-space...
                        println!("We're {} along the route...", dist + dist_here);
                    }
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.panel.draw(g);
        g.redraw(&self.draw_route);
        if let Some((_, ref draw)) = self.hover_on_line_plot {
            g.redraw(draw);
        }
    }
}
