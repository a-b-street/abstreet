use std::cmp::Ordering;

use geom::{Circle, Distance, Duration, FindClosest, PolyLine, Polygon};
use map_gui::tools::PopupMsg;
use map_model::{Path, PathStep, NORMAL_LANE_THICKNESS};
use sim::{TripEndpoint, TripMode};
use widgetry::mapspace::{ToggleZoomed, ToggleZoomedBuilder};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, LinePlot, Outcome, Panel, PlotOptions,
    ScreenDims, Series, Text, Widget,
};

use super::{before_after_button, RoutingPreferences};
use crate::app::{App, Transition};
use crate::common::{cmp_dist, cmp_duration};

/// A temporary structure that the caller should unpack and use as needed.
pub struct BuiltRoute {
    pub details: RouteDetails,
    pub details_widget: Widget,
    pub draw: ToggleZoomedBuilder,
    pub hitbox: Polygon,
    pub tooltip_for_alt: Option<Text>,
}

pub struct RouteDetails {
    pub preferences: RoutingPreferences,
    pub stats: RouteStats,

    // It's tempting to glue together all of the paths. But since some waypoints might force the
    // path to double back on itself, rendering the path as a single PolyLine would break.
    paths: Vec<(Path, Option<PolyLine>)>,
    // Match each polyline to the index in paths
    closest_path_segment: FindClosest<usize>,

    hover_on_line_plot: Option<(Distance, Drawable)>,
    hover_on_route_tooltip: Option<Text>,

    draw_high_stress: Drawable,
    draw_traffic_signals: Drawable,
    draw_unprotected_turns: Drawable,
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

impl RouteDetails {
    /// "main" is determined by `app.session.routing_preferences`
    pub fn main_route(ctx: &mut EventCtx, app: &App, waypoints: Vec<TripEndpoint>) -> BuiltRoute {
        RouteDetails::new_route(
            ctx,
            app,
            waypoints,
            Color::RED,
            None,
            app.session.routing_preferences,
        )
    }

    pub fn alt_route(
        ctx: &mut EventCtx,
        app: &App,
        waypoints: Vec<TripEndpoint>,
        main: &RouteDetails,
        preferences: RoutingPreferences,
    ) -> BuiltRoute {
        let mut built = RouteDetails::new_route(
            ctx,
            app,
            waypoints,
            Color::grey(0.3),
            Some(Color::RED),
            preferences,
        );
        built.tooltip_for_alt = Some(compare_routes(
            app,
            &main.stats,
            &built.details.stats,
            preferences,
        ));
        built
    }

    fn new_route(
        ctx: &mut EventCtx,
        app: &App,
        waypoints: Vec<TripEndpoint>,
        route_color: Color,
        // Only used for alts
        outline_color: Option<Color>,
        preferences: RoutingPreferences,
    ) -> BuiltRoute {
        let mut draw_route = ToggleZoomed::builder();
        let mut hitbox_pieces = Vec::new();
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
                            let road = map.get_parent(*l);
                            if road.high_stress_for_bikes(map, road.lanes[l.offset].dir) {
                                dist_along_high_stress_roads += this_pl.length();

                                // TODO It'd be nicer to build up contiguous subsets of the path
                                // that're stressful, and use trace
                                draw_high_stress.push(
                                    Color::YELLOW,
                                    this_pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS),
                                );
                            }
                        }
                        PathStep::Turn(t) | PathStep::ContraflowTurn(t) => {
                            let i = map.get_i(t.parent);
                            elevation_pts.push((current_dist, i.elevation));
                            if i.is_traffic_signal() {
                                num_traffic_signals += 1;
                                draw_traffic_signals.push(Color::YELLOW, i.polygon.clone());
                            }
                            if map.is_unprotected_turn(
                                t.src.road,
                                t.dst.road,
                                map.get_t(*t).turn_type,
                            ) {
                                num_unprotected_turns += 1;
                                draw_unprotected_turns.push(Color::YELLOW, i.polygon.clone());
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
                    draw_route
                        .zoomed
                        .push(route_color.alpha(0.5), shape.clone());

                    hitbox_pieces.push(shape);

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
        let stats = RouteStats {
            total_distance,
            dist_along_high_stress_roads,
            total_time,
            num_traffic_signals,
            num_unprotected_turns,
            total_up,
            total_down,
        };

        let details_widget = make_detail_widget(ctx, app, &stats, elevation_pts);

        BuiltRoute {
            details: RouteDetails {
                preferences,
                draw_high_stress: ctx.upload(draw_high_stress),
                draw_traffic_signals: ctx.upload(draw_traffic_signals),
                draw_unprotected_turns: ctx.upload(draw_unprotected_turns),
                paths,
                closest_path_segment,
                hover_on_line_plot: None,
                hover_on_route_tooltip: None,
                stats,
            },
            details_widget,
            draw: draw_route,
            hitbox: if hitbox_pieces.is_empty() {
                // Dummy tiny hitbox
                Polygon::rectangle(0.0001, 0.0001)
            } else {
                Polygon::union_all(hitbox_pieces)
            },
            tooltip_for_alt: None,
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

        if let Some(line_plot) = panel.maybe_find::<LinePlot<Distance, Distance>>("elevation") {
            let current_dist_along = line_plot.get_hovering().get(0).map(|pair| pair.0);
            if self.hover_on_line_plot.as_ref().map(|pair| pair.0) != current_dist_along {
                self.hover_on_line_plot = current_dist_along.map(|mut dist| {
                    let mut batch = GeomBatch::new();
                    // Find this position on the trip
                    for (path, maybe_pl) in &self.paths {
                        if dist > path.total_length() {
                            dist -= path.total_length();
                            continue;
                        }
                        if let Some(ref pl) = maybe_pl {
                            if let Ok((pt, _)) = pl.dist_along(dist) {
                                batch.push(
                                    Color::YELLOW,
                                    Circle::new(pt, Distance::meters(30.0)).to_polygon(),
                                );
                            }
                        }
                        break;
                    }

                    (dist, batch.upload(ctx))
                });
            }
        }

        if ctx.redo_mouseover() {
            self.hover_on_route_tooltip = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if let Some((idx, pt)) = self
                    .closest_path_segment
                    .closest_pt(pt, 10.0 * NORMAL_LANE_THICKNESS)
                {
                    // Find the total distance along the trip
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
                                PathStep::Turn(t) | PathStep::ContraflowTurn(t) => {
                                    map.get_i(t.parent).elevation
                                }
                            };
                            panel
                                .find_mut::<LinePlot<Distance, Distance>>("elevation")
                                .set_hovering(ctx, "Elevation", dist + dist_here, elevation);
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

    pub fn draw(&self, g: &mut GfxCtx, panel: &Panel) {
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
}

fn make_detail_widget(
    ctx: &mut EventCtx,
    app: &App,
    stats: &RouteStats,
    elevation_pts: Vec<(Distance, Distance)>,
) -> Widget {
    let pct_stressful = if stats.total_distance == Distance::ZERO {
        0.0
    } else {
        ((stats.dist_along_high_stress_roads / stats.total_distance) * 100.0).round()
    };

    Widget::col(vec![
        Line("Route details").small_heading().into_widget(ctx),
        before_after_button(ctx, app),
        Text::from_all(vec![
            Line("Distance: ").secondary(),
            Line(stats.total_distance.to_string(&app.opts.units)),
        ])
        .into_widget(ctx),
        Widget::row(vec![
            Text::from_all(vec![
                Line(format!(
                    "  {} or {}%",
                    stats
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
            Line(stats.total_time.to_string(&app.opts.units)),
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
                .label_underlined_text(stats.num_traffic_signals.to_string())
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
                .label_underlined_text(stats.num_unprotected_turns.to_string())
                .build_widget(ctx, "unprotected turns"),
        ]),
        Text::from_all(vec![
            Line("Elevation change: ").secondary(),
            Line(format!(
                "{}↑, {}↓",
                stats.total_up.to_string(&app.opts.units),
                stats.total_down.to_string(&app.opts.units)
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
                max_x: Some(stats.total_distance.round_up_for_axis()),
                max_y: Some(app.primary.map.max_elevation().round_up_for_axis()),
                dims: Some(ScreenDims {
                    width: 400.0,
                    height: 200.0,
                }),
                ..Default::default()
            },
            app.opts.units,
        ),
    ])
}

fn compare_routes(
    app: &App,
    main: &RouteStats,
    alt: &RouteStats,
    preferences: RoutingPreferences,
) -> Text {
    let mut txt = Text::new();
    txt.add_line(Line(format!("Click to use {} trip", preferences.name())));

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
        match up.cmp(&Distance::ZERO) {
            Ordering::Less => {
                txt.append(
                    Line(format!("{} less ↑", (-up).to_string(&app.opts.units))).fg(Color::GREEN),
                );
            }
            Ordering::Greater => {
                txt.append(
                    Line(format!("{} more ↑", up.to_string(&app.opts.units))).fg(Color::RED),
                );
            }
            Ordering::Equal => {}
        }
    }

    txt
}
