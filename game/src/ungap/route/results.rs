use std::collections::HashSet;

use geom::{Circle, Distance, Duration, FindClosest, PolyLine};
use map_gui::tools::{PopupMsg, ToggleZoomed};
use map_model::{Path, PathStep, NORMAL_LANE_THICKNESS};
use sim::{TripEndpoint, TripMode};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, LinePlot, Outcome, Panel, PlotOptions,
    Series, Text, Widget,
};

use super::RoutingPreferences;
use crate::app::{App, Transition};

pub struct RouteResults {
    pub preferences: RoutingPreferences,

    // It's tempting to glue together all of the paths. But since some waypoints might force the
    // path to double back on itself, rendering the path as a single PolyLine would break.
    paths: Vec<(Path, Option<PolyLine>)>,
    // Match each polyline to the index in paths
    closest_path_segment: FindClosest<usize>,
    pub stats: RouteStats,

    hover_on_line_plot: Option<(Distance, Drawable)>,
    hover_on_route_tooltip: Option<Text>,
    draw_route: ToggleZoomed,

    draw_high_stress: Drawable,
    draw_traffic_signals: Drawable,
    draw_unprotected_turns: Drawable,

    // Possibly a bit large to stash
    elevation_pts: Vec<(Distance, Distance)>,
}

#[derive(PartialEq)]
pub struct RouteStats {
    total_distance: Distance,
    dist_along_high_stress_roads: Distance,
    total_time: Duration,
    num_traffic_signals: usize,
    num_unprotected_turns: usize,
    total_up: Distance,
    total_down: Distance,
}

impl RouteResults {
    /// "main" is determined by `app.session.routing_preferences`
    pub fn main_route(ctx: &mut EventCtx, app: &App, waypoints: Vec<TripEndpoint>) -> RouteResults {
        RouteResults::new(
            ctx,
            app,
            waypoints,
            Color::CYAN,
            None,
            app.session.routing_preferences,
        )
    }

    fn new(
        ctx: &mut EventCtx,
        app: &App,
        waypoints: Vec<TripEndpoint>,
        route_color: Color,
        outline_color: Option<Color>,
        preferences: RoutingPreferences,
    ) -> RouteResults {
        let mut draw_route = ToggleZoomed::builder();
        let mut draw_high_stress = GeomBatch::new();
        let mut draw_traffic_signals = GeomBatch::new();
        let mut draw_unprotected_turns = GeomBatch::new();
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

        let routing_params = preferences.routing_params();

        for pair in waypoints.windows(2) {
            if let Some(path) = TripEndpoint::path_req(pair[0], pair[1], TripMode::Bike, map)
                .and_then(|req| map.pathfind_with_params(req, &routing_params, true).ok())
            {
                total_distance += path.total_length();
                total_time += path.estimate_duration(map, Some(map_model::MAX_BIKE_SPEED));

                for step in path.get_steps() {
                    let this_pl = step.as_traversable().get_polyline(map);
                    match step {
                        PathStep::Lane(l) | PathStep::ContraflowLane(l) => {
                            if map.get_parent(*l).high_stress_for_bikes(map) {
                                dist_along_high_stress_roads += this_pl.length();

                                // TODO It'd be nicer to build up contiguous subsets of the path
                                // that're stressful, and use trace
                                draw_high_stress.push(
                                    Color::RED,
                                    this_pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS),
                                );
                            }
                        }
                        PathStep::Turn(t) => {
                            let i = map.get_i(t.parent);
                            elevation_pts.push((current_dist, i.elevation));
                            if i.is_traffic_signal() {
                                num_traffic_signals += 1;
                                draw_traffic_signals.push(Color::RED, i.polygon.clone());
                            }
                            if map.is_unprotected_turn(
                                t.src.road,
                                t.dst.road,
                                map.get_t(*t).turn_type,
                            ) {
                                num_unprotected_turns += 1;
                                draw_unprotected_turns.push(Color::RED, i.polygon.clone());
                            }
                        }
                    }
                    current_dist += this_pl.length();
                }

                let maybe_pl = path.trace(map);
                if let Some(ref pl) = maybe_pl {
                    let shape = pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS);
                    draw_route
                        .unzoomed
                        .push(route_color.alpha(0.8), shape.clone());
                    draw_route.zoomed.push(route_color.alpha(0.5), shape);

                    if let Some(color) = outline_color {
                        if let Some(outline) =
                            pl.to_thick_boundary(5.0 * NORMAL_LANE_THICKNESS, NORMAL_LANE_THICKNESS)
                        {
                            draw_route.unzoomed.push(color, outline.clone());
                            draw_route.zoomed.push(color.alpha(0.5), outline);
                        }
                    }

                    closest_path_segment.add(paths.len(), pl.points());
                }
                paths.push((path, maybe_pl));
            }
        }

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

        RouteResults {
            preferences,
            draw_route: draw_route.build(ctx),
            draw_high_stress: ctx.upload(draw_high_stress),
            draw_traffic_signals: ctx.upload(draw_traffic_signals),
            draw_unprotected_turns: ctx.upload(draw_unprotected_turns),
            paths,
            closest_path_segment,
            hover_on_line_plot: None,
            hover_on_route_tooltip: None,
            elevation_pts,
            stats: RouteStats {
                total_distance,
                dist_along_high_stress_roads,
                total_time,
                num_traffic_signals,
                num_unprotected_turns,
                total_up,
                total_down,
            },
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &App,
        outcome: &Outcome,
        panel: &mut Panel,
    ) -> Option<Transition> {
        if let Outcome::Clicked(x) = outcome {
            match x.as_ref() {
                "high-stress roads" => {
                    return Some(Transition::Push(PopupMsg::new_state(
                        ctx,
                        "High-stress roads",
                        vec![
                            "Roads are defined as high-stress for biking if:",
                            "- they're classified as arterials",
                            "- they lack dedicated space for biking",
                        ],
                    )));
                }
                // No effect. Maybe these should be toggles, so people can pan the map around and
                // see these in more detail?
                "traffic signals" | "unprotected turns" => {
                    return Some(Transition::Keep);
                }
                _ => {
                    return None;
                }
            }
        }

        let current_dist_along = panel
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

        if ctx.redo_mouseover() {
            self.hover_on_route_tooltip = None;
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
                            // The LinePlot doesn't hold onto the original Series, so it can't help
                            // us figure out elevation here. Let's match this point to the original
                            // path and guess elevation ourselves...
                            let map = &app.primary.map;
                            let elevation = match self.paths[idx]
                                .0
                                .get_step_at_dist_along(map, dist_here)
                                // We often seem to slightly exceed the total length, so just clamp
                                // here...
                                .unwrap_or_else(|_| self.paths[idx].0.last_step())
                            {
                                PathStep::Lane(l) | PathStep::ContraflowLane(l) => {
                                    // TODO Interpolate
                                    map.get_i(map.get_l(l).src_i).elevation
                                }
                                PathStep::Turn(t) => map.get_i(t.parent).elevation,
                            };
                            panel
                                .find_mut::<LinePlot<Distance, Distance>>("elevation")
                                .set_hovering(ctx, dist + dist_here, elevation);
                            self.hover_on_route_tooltip = Some(Text::from(Line(format!(
                                "Elevation: {}",
                                elevation.to_string(&app.opts.units)
                            ))));
                        }
                    }
                }
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App, panel: &Panel) {
        self.draw_route.draw(g, app);
        if let Some((_, ref draw)) = self.hover_on_line_plot {
            g.redraw(draw);
        }
        if let Some(ref txt) = self.hover_on_route_tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
        if panel.currently_hovering() == Some(&"high-stress roads".to_string()) {
            g.redraw(&self.draw_high_stress);
        }
        if panel.currently_hovering() == Some(&"traffic signals".to_string()) {
            g.redraw(&self.draw_traffic_signals);
        }
        if panel.currently_hovering() == Some(&"unprotected turns".to_string()) {
            g.redraw(&self.draw_unprotected_turns);
        }
    }

    pub fn to_widget(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        let pct_stressful = if self.stats.total_distance == Distance::ZERO {
            0.0
        } else {
            ((self.stats.dist_along_high_stress_roads / self.stats.total_distance) * 100.0).round()
        };

        let elevation_plot = LinePlot::new_widget(
            ctx,
            "elevation",
            vec![Series {
                label: "Elevation".to_string(),
                color: Color::RED,
                pts: self.elevation_pts.clone(),
            }],
            PlotOptions {
                filterable: false,
                max_x: Some(self.stats.total_distance.round_up_for_axis()),
                max_y: Some(app.primary.map.max_elevation().round_up_for_axis()),
                disabled: HashSet::new(),
            },
            app.opts.units,
        );

        Widget::col(vec![
            Line("Route details").small_heading().into_widget(ctx),
            Text::from_all(vec![
                Line("Distance: ").secondary(),
                Line(self.stats.total_distance.to_string(&app.opts.units)),
            ])
            .into_widget(ctx),
            Widget::row(vec![
                Text::from_all(vec![
                    Line(format!(
                        "  {} or {}%",
                        self.stats
                            .dist_along_high_stress_roads
                            .to_string(&app.opts.units),
                        pct_stressful
                    )),
                    Line(" along ").secondary(),
                ])
                .into_widget(ctx)
                .centered_vert(),
                ctx.style()
                    .btn_plain
                    .btn()
                    .label_underlined_text("high-stress roads")
                    .build_def(ctx),
            ]),
            Text::from_all(vec![
                Line("Estimated time: ").secondary(),
                Line(self.stats.total_time.to_string(&app.opts.units)),
            ])
            .into_widget(ctx),
            Widget::row(vec![
                Line("Traffic signals crossed: ")
                    .secondary()
                    .into_widget(ctx)
                    .centered_vert(),
                ctx.style()
                    .btn_plain
                    .btn()
                    .label_underlined_text(self.stats.num_traffic_signals.to_string())
                    .build_widget(ctx, "traffic signals"),
            ]),
            Widget::row(vec![
                Line("Unprotected left turns onto busy roads: ")
                    .secondary()
                    .into_widget(ctx)
                    .centered_vert(),
                ctx.style()
                    .btn_plain
                    .btn()
                    .label_underlined_text(self.stats.num_unprotected_turns.to_string())
                    .build_widget(ctx, "unprotected turns"),
            ]),
            Text::from_all(vec![
                Line("Elevation change: ").secondary(),
                Line(format!(
                    "{}↑, {}↓",
                    self.stats.total_up.to_string(&app.opts.units),
                    self.stats.total_down.to_string(&app.opts.units)
                )),
            ])
            .into_widget(ctx),
            elevation_plot,
        ])
    }
}

pub struct AltRouteResults {
    pub results: RouteResults,
    hovering: bool,
    tooltip: Text,
}

impl AltRouteResults {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        waypoints: Vec<TripEndpoint>,
        main: &RouteResults,
        preferences: RoutingPreferences,
    ) -> AltRouteResults {
        let results = RouteResults::new(
            ctx,
            app,
            waypoints,
            Color::grey(0.3),
            Some(Color::CYAN),
            preferences,
        );
        let tooltip = compare_routes(app, &main.stats, &results.stats, preferences);
        AltRouteResults {
            results,
            hovering: false,
            tooltip,
        }
    }

    pub fn has_focus(&self) -> bool {
        self.hovering
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if ctx.redo_mouseover() {
            self.hovering = false;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if self
                    .results
                    .closest_path_segment
                    .closest_pt(pt, 10.0 * NORMAL_LANE_THICKNESS)
                    .is_some()
                {
                    self.hovering = true;
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.results.draw_route.draw(g, app);

        if self.hovering {
            g.draw_mouse_tooltip(self.tooltip.clone());
        }
    }
}

fn compare_routes(
    app: &App,
    main: &RouteStats,
    alt: &RouteStats,
    preferences: RoutingPreferences,
) -> Text {
    let mut txt = Text::new();
    txt.add_line(Line(format!("Click to use {} route", preferences.name())));

    cmp_dist(
        &mut txt,
        app,
        alt.total_distance - main.total_distance,
        "shorter",
        "longer",
    );
    cmp_duration(
        &mut txt,
        app,
        alt.total_time - main.total_time,
        "shorter",
        "longer",
    );
    cmp_dist(
        &mut txt,
        app,
        alt.dist_along_high_stress_roads - main.dist_along_high_stress_roads,
        "less on high-stress roads",
        "more on high-stress roads",
    );

    if alt.total_up != main.total_up || alt.total_down != main.total_down {
        txt.add_line(Line("Elevation change: "));
        let up = alt.total_up - main.total_up;
        if up < Distance::ZERO {
            txt.append(
                Line(format!("{} less ↑", (-up).to_string(&app.opts.units))).fg(Color::GREEN),
            );
            txt.append(Line(", "));
        } else if up > Distance::ZERO {
            txt.append(Line(format!("{} more ↑", up.to_string(&app.opts.units))).fg(Color::RED));
            txt.append(Line(", "));
        }

        // Unclear if more down should be "good" or "bad", so we'll omit color
        let down = alt.total_down - main.total_down;
        if down < Distance::ZERO {
            txt.append(Line(format!(
                "{} less ↓",
                (-down).to_string(&app.opts.units)
            )));
        } else if down > Distance::ZERO {
            txt.append(Line(format!("{} more ↓", down.to_string(&app.opts.units))));
        }
    }

    txt
}

fn cmp_dist(txt: &mut Text, app: &App, dist: Distance, shorter: &str, longer: &str) {
    if dist < Distance::ZERO {
        txt.add_line(
            Line(format!(
                "{} {}",
                (-dist).to_string(&app.opts.units),
                shorter
            ))
            .fg(Color::GREEN),
        );
    } else if dist > Distance::ZERO {
        txt.add_line(
            Line(format!("{} {}", dist.to_string(&app.opts.units), longer)).fg(Color::RED),
        );
    }
}

fn cmp_duration(txt: &mut Text, app: &App, duration: Duration, shorter: &str, longer: &str) {
    if duration < Duration::ZERO {
        txt.add_line(
            Line(format!(
                "{} {}",
                (-duration).to_string(&app.opts.units),
                shorter
            ))
            .fg(Color::GREEN),
        );
    } else if duration > Duration::ZERO {
        txt.add_line(
            Line(format!(
                "{} {}",
                duration.to_string(&app.opts.units),
                longer
            ))
            .fg(Color::RED),
        );
    }
}
