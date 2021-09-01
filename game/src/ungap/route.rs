use std::collections::HashSet;

use geom::{Distance, Duration};
use map_model::{PathStep, NORMAL_LANE_THICKNESS};
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

    // Routing
    draw_route: Drawable,
    results_panel: Panel,
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

            draw_route: Drawable::empty(ctx),
            results_panel: Panel::empty(ctx),
        };
        rp.update_input_panel(ctx, app);
        rp.update_route(ctx, app);
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

    fn update_route(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;

        let mut total_distance = Distance::ZERO;
        let mut total_time = Duration::ZERO;

        let mut dist_along_high_stress_roads = Distance::ZERO;
        let mut num_traffic_signals = 0;
        let mut num_unprotected_turns = 0;

        let mut elevation_pts: Vec<(Distance, Distance)> = Vec::new();
        let mut current_dist = Distance::ZERO;

        for pair in self.waypoints.get_waypoints().windows(2) {
            if let Some((path, draw_path)) =
                TripEndpoint::path_req(pair[0], pair[1], TripMode::Bike, map)
                    .and_then(|req| map.pathfind(req).ok())
                    .and_then(|path| {
                        path.trace(&app.primary.map)
                            .map(|pl| (path, pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS)))
                    })
            {
                batch.push(Color::CYAN, draw_path);
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
            }
        }

        self.draw_route = ctx.upload(batch);

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

        self.results_panel = Panel::new_builder(Widget::col(vec![
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
                    self.update_route(ctx, app);
                }
            }
        }

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.layers.draw(g, app);
        self.input_panel.draw(g);
        self.waypoints.draw(g);

        self.results_panel.draw(g);
        g.redraw(&self.draw_route);
    }
}
