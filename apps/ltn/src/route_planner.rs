use geom::{Distance, Duration};
use map_gui::tools::{
    cmp_dist, cmp_duration, DrawRoadLabels, InputWaypoints, TripManagement, TripManagementState,
    WaypointID,
};
use map_model::{PathfinderCaching, NORMAL_LANE_THICKNESS};
use synthpop::{TripEndpoint, TripMode};
use widgetry::mapspace::{ToggleZoomed, World};
use widgetry::{
    EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, RoundedF64, Spinner, State, Text,
    Widget,
};

use crate::{colors, App, BrowseNeighborhoods, Transition};

pub struct RoutePlanner {
    top_panel: Panel,
    left_panel: Panel,
    waypoints: InputWaypoints,
    files: TripManagement<App, RoutePlanner>,
    world: World<WaypointID>,
    draw_routes: ToggleZoomed,
    labels: DrawRoadLabels,
}

impl TripManagementState<App> for RoutePlanner {
    fn mut_files(&mut self) -> &mut TripManagement<App, Self> {
        &mut self.files
    }

    fn app_session_current_trip_name(app: &mut App) -> &mut Option<String> {
        &mut app.session.current_trip_name
    }

    fn sync_from_file_management(&mut self, ctx: &mut EventCtx, app: &mut App) {
        self.waypoints
            .overwrite(app, self.files.current.waypoints.clone());
        self.update_everything(ctx, app);
    }
}

impl RoutePlanner {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let mut rp = RoutePlanner {
            top_panel: crate::common::app_top_panel(ctx, app),
            left_panel: Panel::empty(ctx),
            waypoints: InputWaypoints::new(app),
            files: TripManagement::new(app),
            world: World::unbounded(),
            draw_routes: ToggleZoomed::empty(ctx),
            labels: DrawRoadLabels::only_major_roads(),
        };

        if let Some(current_name) = &app.session.current_trip_name {
            rp.files.set_current(current_name);
        }
        rp.sync_from_file_management(ctx, app);

        Box::new(rp)
    }

    // Updates the panel and draw_routes
    fn update_everything(&mut self, ctx: &mut EventCtx, app: &mut App) {
        self.files.autosave(app);
        let results_widget = self.recalculate_paths(ctx, app);

        let contents = Widget::col(vec![
            app.session.alt_proposals.to_widget(ctx, app),
            ctx.style()
                .btn_back("Browse neighborhoods")
                .hotkey(Key::Escape)
                .build_def(ctx),
            Line("Plan a route").small_heading().into_widget(ctx),
            Widget::col(vec![
                self.files.get_panel_widget(ctx),
                Widget::horiz_separator(ctx, 1.0),
                self.waypoints.get_panel_widget(ctx),
            ])
            .section(ctx),
            Widget::row(vec![
                Line("Slow-down factor for main roads:")
                    .into_widget(ctx)
                    .centered_vert(),
                Spinner::f64_widget(
                    ctx,
                    "main road penalty",
                    (1.0, 10.0),
                    app.session.main_road_penalty,
                    0.5,
                ),
            ]),
            Text::from_multiline(vec![
                Line("1 means free-flow traffic conditions").secondary(),
                Line("Increase to see how vehicles may try to detour in heavy traffic").secondary(),
            ])
            .into_widget(ctx),
            results_widget,
        ]);
        let mut panel = crate::common::left_panel_builder(ctx, &self.top_panel, contents)
            // Hovering on waypoint cards
            .ignore_initial_events()
            .build(ctx);
        panel.restore(ctx, &self.left_panel);
        self.left_panel = panel;

        // Fade all neighborhood interiors, so it's very clear when a route cuts through
        let mut batch = GeomBatch::new();
        for (block, _) in app.session.partitioning.all_neighborhoods().values() {
            batch.push(app.cs.fade_map_dark, block.polygon.clone());
        }

        let mut world = World::bounded(app.map.get_bounds());
        world.draw_master_batch(ctx, batch);
        self.waypoints.rebuild_world(ctx, &mut world, |x| x, 0);
        world.initialize_hover(ctx);
        world.rebuilt_during_drag(&self.world);
        self.world = world;
    }

    // Returns a widget to display
    fn recalculate_paths(&mut self, ctx: &mut EventCtx, app: &App) -> Widget {
        let map = &app.map;
        let mut results = Text::new();
        let mut draw = ToggleZoomed::builder();

        // First the route respecting the filters
        let (total_time_after, total_dist_after) = {
            let mut params = map.routing_params().clone();
            app.session.modal_filters.update_routing_params(&mut params);
            params.main_road_penalty = app.session.main_road_penalty;

            let mut total_time = Duration::ZERO;
            let mut total_dist = Distance::ZERO;
            let color = colors::PLAN_ROUTE_AFTER;
            for pair in self.waypoints.get_waypoints().windows(2) {
                if let Some((path, pl)) =
                    TripEndpoint::path_req(pair[0], pair[1], TripMode::Drive, map)
                        .and_then(|req| {
                            map.pathfind_with_params(req, &params, PathfinderCaching::CacheDijkstra)
                                .ok()
                        })
                        .and_then(|path| path.trace(map).map(|pl| (path, pl)))
                {
                    let shape = pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS);
                    draw.unzoomed.push(color.alpha(0.8), shape.clone());
                    draw.zoomed.push(color.alpha(0.5), shape);

                    // We use PathV1 (lane-based) for tracing. It doesn't preserve the cost
                    // calculated while pathfinding, so just estimate_duration.
                    //
                    // The original reason for using estimate_duration here was to exclude the large
                    // penalty if the route crossed a filter. But now that's impossible at the
                    // pathfinding layer.
                    total_time += path.estimate_duration(map, None);
                    total_dist += path.total_length();
                }
            }
            if total_dist != Distance::ZERO {
                results.add_line(Line("Route respecting modal filters").fg(color));
                results.add_line(Line(format!("Time: {}", total_time)));
                results.add_line(Line(format!("Distance: {}", total_dist)));
            }

            (total_time, total_dist)
        };

        // Then the one ignoring filters
        {
            let mut draw_old_route = ToggleZoomed::builder();
            let mut total_time = Duration::ZERO;
            let mut total_dist = Distance::ZERO;
            let color = colors::PLAN_ROUTE_BEFORE;
            let mut params = map.routing_params().clone();
            params.main_road_penalty = app.session.main_road_penalty;
            for pair in self.waypoints.get_waypoints().windows(2) {
                if let Some((path, pl)) =
                    TripEndpoint::path_req(pair[0], pair[1], TripMode::Drive, map)
                        .and_then(|req| {
                            map.pathfind_with_params(req, &params, PathfinderCaching::CacheDijkstra)
                                .ok()
                        })
                        .and_then(|path| path.trace(map).map(|pl| (path, pl)))
                {
                    let shape = pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS);
                    draw_old_route
                        .unzoomed
                        .push(color.alpha(0.8), shape.clone());
                    draw_old_route.zoomed.push(color.alpha(0.5), shape);

                    total_time += path.estimate_duration(map, None);
                    total_dist += path.total_length();
                }
            }
            if total_dist != Distance::ZERO {
                // If these two stats are the same, assume the two paths are equivalent
                if total_time == total_time_after && total_dist == total_dist_after {
                    draw = draw_old_route;
                    results = Text::new();
                    results.add_line(Line("The route is the same before/after modal filters"));
                    results.add_line(Line(format!(
                        "Time: {}",
                        total_time.to_string(&app.opts.units)
                    )));
                    results.add_line(Line(format!(
                        "Distance: {}",
                        total_dist.to_string(&app.opts.units)
                    )));
                } else {
                    draw.append(draw_old_route);
                    results.add_line(
                        Line("Route before any modal filters (existing or new)").fg(color),
                    );
                    cmp_duration(
                        &mut results,
                        app,
                        total_time - total_time_after,
                        "shorter",
                        "longer",
                    );
                    // Remove formatting -- red/green gets confusing with the blue/red of the two
                    // routes
                    results.remove_colors_from_last_line();
                    cmp_dist(
                        &mut results,
                        app,
                        total_dist - total_dist_after,
                        "shorter",
                        "longer",
                    );
                    results.remove_colors_from_last_line();
                }
            }
        }

        self.draw_routes = draw.build(ctx);
        results.into_widget(ctx)
    }
}

impl State<App> for RoutePlanner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) = crate::common::handle_top_panel(ctx, app, &mut self.top_panel) {
            return t;
        }

        let panel_outcome = self.left_panel.event(ctx);
        if let Outcome::Clicked(ref x) = panel_outcome {
            if x == "Browse neighborhoods" {
                return Transition::Replace(BrowseNeighborhoods::new_state(ctx, app));
            }
            if let Some(t) = self.files.on_click(ctx, app, x) {
                // Bit hacky...
                if matches!(t, Transition::Keep) {
                    self.sync_from_file_management(ctx, app);
                }
                return t;
            }
            if let Some(t) = crate::save::AltProposals::handle_action(
                ctx,
                app,
                crate::save::PreserveState::Route,
                x,
            ) {
                return t;
            }
        }

        if let Outcome::Changed(ref x) = panel_outcome {
            if x == "main road penalty" {
                app.session.main_road_penalty =
                    self.left_panel.spinner::<RoundedF64>("main road penalty").0;
                self.update_everything(ctx, app);
            }
        }

        if self
            .waypoints
            .event(app, panel_outcome, self.world.event(ctx))
        {
            // Sync from waypoints to file management
            // TODO Maaaybe this directly live in the InputWaypoints system?
            self.files.current.waypoints = self.waypoints.get_waypoints();
            self.update_everything(ctx, app);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.left_panel.draw(g);

        self.world.draw(g);

        self.draw_routes.draw(g);
        app.session.draw_all_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
        }
    }

    fn on_destroy(&mut self, _: &mut EventCtx, app: &mut App) {
        // We'll cache a custom pathfinder per set of avoided roads. Avoid leaking memory by
        // clearing this out
        app.map.clear_custom_pathfinder_cache();
    }
}
