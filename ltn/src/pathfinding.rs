use geom::{Distance, Duration};
use map_model::NORMAL_LANE_THICKNESS;
use sim::{TripEndpoint, TripMode};
use widgetry::mapspace::{ObjectID, ToggleZoomed, World};
use widgetry::{
    Color, EventCtx, GfxCtx, Line, Outcome, Panel, RoundedF64, Spinner, State, Text, Widget,
};

use super::per_neighborhood::{FilterableObj, Tab};
use super::{Neighborhood, NeighborhoodID};
use crate::common::{cmp_dist, cmp_duration, InputWaypoints, WaypointID};
use crate::{App, Transition};

pub struct RoutePlanner {
    panel: Panel,
    waypoints: InputWaypoints,
    world: World<Obj>,
    draw_routes: ToggleZoomed,

    neighborhood: Neighborhood,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Obj {
    Waypoint(WaypointID),
    Filterable(FilterableObj),
}
impl ObjectID for Obj {}

impl RoutePlanner {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighborhoodID) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::new(ctx, app, id);

        let mut rp = RoutePlanner {
            panel: Panel::empty(ctx),
            waypoints: InputWaypoints::new(app),
            world: World::unbounded(),
            draw_routes: ToggleZoomed::empty(ctx),
            neighborhood,
        };
        rp.update(ctx, app);
        Box::new(rp)
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        let contents = Widget::col(vec![
            self.waypoints.get_panel_widget(ctx),
            Widget::row(vec![
                Line("Slow-down factor for main roads:")
                    .into_widget(ctx)
                    .centered_vert(),
                Spinner::f64_widget(ctx, "main road penalty", (1.0, 10.0), 1.0, 0.5),
            ]),
            Text::from_multiline(vec![
                Line("1 means free-flow traffic conditions").secondary(),
                Line("Increase to see how vehicles may try to detour in heavy traffic").secondary(),
            ])
            .into_widget(ctx),
            Text::new().into_widget(ctx).named("results"),
        ]);
        let mut panel = Tab::Pathfinding
            .panel_builder(ctx, app, contents)
            // Hovering on waypoint cards
            .ignore_initial_events()
            .build(ctx);
        panel.restore(ctx, &self.panel);
        self.panel = panel;

        let mut world = World::bounded(app.map.get_bounds());
        super::per_neighborhood::populate_world(
            ctx,
            app,
            &self.neighborhood,
            &mut world,
            Obj::Filterable,
            0,
        );
        self.waypoints
            .rebuild_world(ctx, &mut world, Obj::Waypoint, 1);
        world.initialize_hover(ctx);
        world.rebuilt_during_drag(&self.world);
        self.world = world;

        self.recalculate_paths(ctx, app);
    }

    // Updates the panel and draw_routes
    fn recalculate_paths(&mut self, ctx: &mut EventCtx, app: &App) {
        let map = &app.map;
        let mut results = Text::new();
        let mut draw = ToggleZoomed::builder();

        // First the route respecting the filters
        let (total_time_after, total_dist_after) = {
            let mut params = map.routing_params().clone();
            app.session.modal_filters.update_routing_params(&mut params);
            params.main_road_penalty = self.panel.spinner::<RoundedF64>("main road penalty").0;
            let cache_custom = true;

            let mut total_time = Duration::ZERO;
            let mut total_dist = Distance::ZERO;
            for pair in self.waypoints.get_waypoints().windows(2) {
                if let Some((path, pl)) =
                    TripEndpoint::path_req(pair[0], pair[1], TripMode::Drive, map)
                        .and_then(|req| map.pathfind_with_params(req, &params, cache_custom).ok())
                        .and_then(|path| path.trace(map).map(|pl| (path, pl)))
                {
                    let shape = pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS);
                    draw.unzoomed.push(Color::RED.alpha(0.8), shape.clone());
                    draw.zoomed.push(Color::RED.alpha(0.5), shape);

                    // Use estimate_duration and not the original cost from pathfinding, since that
                    // includes huge penalties when the route is forced to cross a filter
                    total_time += path.estimate_duration(map, None);
                    total_dist += path.total_length();
                }
            }
            if total_dist != Distance::ZERO {
                results.add_line(Line("Route respecting the new modal filters").fg(Color::BLUE));
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
            let mut params = map.routing_params().clone();
            params.main_road_penalty = self.panel.spinner::<RoundedF64>("main road penalty").0;
            let cache_custom = true;
            for pair in self.waypoints.get_waypoints().windows(2) {
                if let Some((path, pl)) =
                    TripEndpoint::path_req(pair[0], pair[1], TripMode::Drive, map)
                        .and_then(|req| map.pathfind_with_params(req, &params, cache_custom).ok())
                        .and_then(|path| path.trace(map).map(|pl| (path, pl)))
                {
                    let shape = pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS);
                    draw_old_route
                        .unzoomed
                        .push(Color::BLUE.alpha(0.8), shape.clone());
                    draw_old_route.zoomed.push(Color::BLUE.alpha(0.5), shape);

                    total_time += path.estimate_duration(map, None);
                    total_dist += path.total_length();
                }
            }
            if total_dist != Distance::ZERO {
                // If these two stats are the same, assume the two paths are equivalent
                if total_time == total_time_after && total_dist == total_dist_after {
                    draw = draw_old_route;
                    results = Text::new();
                    results.add_line(Line(
                        "The route is the same before/after the new modal filters",
                    ));
                    results.add_line(Line(format!("Time: {}", total_time)));
                    results.add_line(Line(format!("Distance: {}", total_dist)));
                } else {
                    draw.append(draw_old_route);
                    results.add_line(Line("Route before the new modal filters").fg(Color::RED));
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
        let label = results.into_widget(ctx);
        self.panel.replace(ctx, "results", label);
    }
}

impl State<App> for RoutePlanner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let world_outcome = self.world.event(ctx);
        // TODO map_id can only extract one case. Do a bit of a hack to handle filter managament
        // first.
        if let Some(outcome) = world_outcome.clone().maybe_map_id(|id| match id {
            Obj::Filterable(id) => Some(id),
            _ => None,
        }) {
            if super::per_neighborhood::handle_world_outcome(ctx, app, outcome) {
                self.neighborhood = Neighborhood::new(ctx, app, self.neighborhood.id);
                self.update(ctx, app);
                return Transition::Keep;
            }
            // Fall through. Clicking free space and other ID-less outcomes will match here, but we
            // don't want them to.
        }
        let world_outcome_for_waypoints = world_outcome.map_id(|id| match id {
            Obj::Waypoint(id) => id,
            _ => unreachable!(),
        });

        let panel_outcome = self.panel.event(ctx);
        if let Outcome::Clicked(ref x) = panel_outcome {
            if let Some(t) = Tab::Pathfinding.handle_action(ctx, app, x, self.neighborhood.id) {
                return t;
            }
        }

        if let Outcome::Changed(ref x) = panel_outcome {
            if x == "main road penalty" {
                self.recalculate_paths(ctx, app);
            }
        }

        if self
            .waypoints
            .event(app, panel_outcome, world_outcome_for_waypoints)
        {
            self.update(ctx, app);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);

        g.redraw(&self.neighborhood.fade_irrelevant);
        self.draw_routes.draw(g);
        self.neighborhood.draw_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }

        self.world.draw(g);
    }

    fn on_destroy(&mut self, _: &mut EventCtx, app: &mut App) {
        // We'll cache a custom pathfinder per set of avoided roads. Avoid leaking memory by
        // clearing this out
        app.map.clear_custom_pathfinder_cache();
    }
}
