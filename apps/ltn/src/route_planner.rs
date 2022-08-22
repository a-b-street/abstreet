use geom::Duration;
use map_gui::tools::{
    DrawSimpleRoadLabels, InputWaypoints, TripManagement, TripManagementState, WaypointID,
};
use map_model::{PathV2, PathfinderCache};
use synthpop::{TripEndpoint, TripMode};
use widgetry::mapspace::World;
use widgetry::{
    ButtonBuilder, Color, ControlState, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome,
    Panel, RoundedF64, Spinner, State, Text, Widget,
};

use crate::components::Mode;
use crate::{colors, App, BrowseNeighbourhoods, Transition};

pub struct RoutePlanner {
    top_panel: Panel,
    left_panel: Panel,
    waypoints: InputWaypoints,
    files: TripManagement<App, RoutePlanner>,
    world: World<WaypointID>,
    draw_routes: Drawable,
    // TODO We could save the no-filter variations map-wide
    pathfinder_cache: PathfinderCache,
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
        if app.session.draw_all_road_labels.is_none() {
            app.session.draw_all_road_labels = Some(DrawSimpleRoadLabels::all_roads(
                ctx,
                app,
                colors::ROAD_LABEL,
            ));
        }

        let mut rp = RoutePlanner {
            top_panel: crate::components::TopPanel::panel(ctx, app),
            left_panel: Panel::empty(ctx),
            waypoints: InputWaypoints::new_max_2(app),
            files: TripManagement::new(app),
            world: World::unbounded(),
            draw_routes: Drawable::empty(ctx),
            pathfinder_cache: PathfinderCache::new(),
        };

        if let Some(current_name) = &app.session.current_trip_name {
            rp.files.set_current(current_name);
        }
        rp.sync_from_file_management(ctx, app);

        Box::new(rp)
    }

    pub fn button(ctx: &EventCtx) -> Widget {
        ctx.style()
            .btn_outline
            .text("Plan a route")
            .hotkey(Key::R)
            .build_def(ctx)
    }

    // Updates the panel and draw_routes
    fn update_everything(&mut self, ctx: &mut EventCtx, app: &mut App) {
        self.files.autosave(app);
        let results_widget = self.recalculate_paths(ctx, app);

        let contents = Widget::col(vec![
            app.session.alt_proposals.to_widget(ctx, app),
            BrowseNeighbourhoods::button(ctx, app),
            ctx.style()
                .btn_back("Analyze neighbourhood")
                .hotkey(Key::Escape)
                .build_def(ctx)
                .hide(app.session.consultation.is_none()),
            Line("Plan a route").small_heading().into_widget(ctx),
            Widget::col(vec![
                self.files.get_panel_widget(ctx),
                Widget::horiz_separator(ctx, 1.0),
                self.waypoints.get_panel_widget(ctx),
            ])
            .section(ctx),
            Widget::col(vec![
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
                    Line("Increase to see how drivers may try to detour in heavy traffic")
                        .secondary(),
                ])
                .into_widget(ctx),
            ])
            .section(ctx),
            results_widget.section(ctx),
        ]);
        let mut panel = crate::components::LeftPanel::builder(ctx, &self.top_panel, contents)
            // Hovering on waypoint cards
            .ignore_initial_events()
            .build(ctx);
        panel.restore(ctx, &self.left_panel);
        self.left_panel = panel;

        // Fade all neighbourhood interiors, so it's very clear when a route cuts through
        let mut batch = GeomBatch::new();
        for info in app.session.partitioning.all_neighbourhoods().values() {
            batch.push(app.cs.fade_map_dark, info.block.polygon.clone());
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

        let mut paths: Vec<(PathV2, Color)> = Vec::new();

        let driving_before_changes_time = {
            let mut total_time = Duration::ZERO;
            let mut params = app.session.routing_params_before_changes.clone();
            params.main_road_penalty = app.session.main_road_penalty;

            for pair in self.waypoints.get_waypoints().windows(2) {
                if let Some(path) = TripEndpoint::path_req(pair[0], pair[1], TripMode::Drive, map)
                    .and_then(|req| {
                        self.pathfinder_cache
                            .pathfind_with_params(map, req, params.clone())
                    })
                {
                    total_time += path.get_cost();
                    paths.push((path, *colors::PLAN_ROUTE_BEFORE));
                }
            }

            total_time
        };

        // The route respecting the filters
        let driving_after_changes_time = {
            let mut params = map.routing_params().clone();
            app.session.edits.update_routing_params(&mut params);
            params.main_road_penalty = app.session.main_road_penalty;

            let mut total_time = Duration::ZERO;
            let mut paths_after = Vec::new();
            for pair in self.waypoints.get_waypoints().windows(2) {
                if let Some(path) = TripEndpoint::path_req(pair[0], pair[1], TripMode::Drive, map)
                    .and_then(|req| {
                        self.pathfinder_cache
                            .pathfind_with_params(map, req, params.clone())
                    })
                {
                    total_time += path.get_cost();
                    paths_after.push((path, *colors::PLAN_ROUTE_AFTER));
                }
            }
            // To simplify colors, don't draw this path when it's the same as the baseline
            if total_time != driving_before_changes_time {
                paths.append(&mut paths_after);
            }

            total_time
        };

        let biking_time = {
            // No custom params, but don't use the map's built-in bike CH. Changes to one-way
            // streets haven't been reflected, and it's cheap enough to use Dijkstra's for
            // calculating one path at a time anyway.
            let mut total_time = Duration::ZERO;
            for pair in self.waypoints.get_waypoints().windows(2) {
                if let Some(path) = TripEndpoint::path_req(pair[0], pair[1], TripMode::Bike, map)
                    .and_then(|req| {
                        self.pathfinder_cache.pathfind_with_params(
                            map,
                            req,
                            map.routing_params().clone(),
                        )
                    })
                {
                    total_time += path.get_cost();
                    paths.push((path, *colors::PLAN_ROUTE_BIKE));
                }
            }
            total_time
        };

        let walking_time = {
            // Same as above -- don't use the built-in CH.
            let mut total_time = Duration::ZERO;
            for pair in self.waypoints.get_waypoints().windows(2) {
                if let Some(path) = TripEndpoint::path_req(pair[0], pair[1], TripMode::Walk, map)
                    .and_then(|req| {
                        self.pathfinder_cache.pathfind_with_params(
                            map,
                            req,
                            map.routing_params().clone(),
                        )
                    })
                {
                    total_time += path.get_cost();
                    paths.push((path, *colors::PLAN_ROUTE_WALK));
                }
            }
            total_time
        };

        self.draw_routes = map_gui::tools::draw_overlapping_paths(app, paths)
            .unzoomed
            .upload(ctx);

        Widget::col(vec![
            Widget::row(vec![
                card(
                    ctx,
                    "Driving before any changes",
                    "",
                    driving_before_changes_time,
                    *colors::PLAN_ROUTE_BEFORE,
                ),
                if driving_before_changes_time == driving_after_changes_time {
                    Widget::col(vec![
                        Line("Driving after changes")
                            .fg(colors::PLAN_ROUTE_BEFORE.invert())
                            .into_widget(ctx),
                        Line("No difference")
                            .fg(colors::PLAN_ROUTE_BEFORE.invert())
                            .into_widget(ctx),
                    ])
                    .bg(*colors::PLAN_ROUTE_BEFORE)
                    .padding(16)
                } else {
                    card(
                        ctx,
                        "Driving after changes",
                        "",
                        driving_after_changes_time,
                        *colors::PLAN_ROUTE_AFTER,
                    )
                },
            ])
            .evenly_spaced(),
            Widget::row(vec![
                card(ctx, "Cycling", "This cycling route doesn't avoid high-stress roads or hills, and assumes an average 10mph pace", biking_time, *colors::PLAN_ROUTE_BIKE),
                card(ctx, "Walking", "This walking route doesn't avoid high-stress roads or hills, and assumes an average 3 mph pace", walking_time, *colors::PLAN_ROUTE_WALK),
            ])
            .evenly_spaced(),
        ])
    }
}

impl State<App> for RoutePlanner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) = crate::components::TopPanel::event(ctx, app, &mut self.top_panel, help) {
            return t;
        }
        if let Some(t) = app.session.layers.event(ctx, &app.cs, Mode::RoutePlanner) {
            return t;
        }

        let panel_outcome = self.left_panel.event(ctx);
        if let Outcome::Clicked(ref x) = panel_outcome {
            if x == "Browse neighbourhoods" {
                return Transition::Replace(BrowseNeighbourhoods::new_state(ctx, app));
            }
            if x == "Analyze neighbourhood" {
                return Transition::Pop;
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
        app.session.layers.draw(g);

        self.world.draw(g);
        self.draw_routes.draw(g);
        app.session.draw_all_road_labels.as_ref().unwrap().draw(g);
        app.session.draw_all_filters.draw(g);
        app.session.draw_poi_icons.draw(g);
    }
}

fn help() -> Vec<&'static str> {
    vec![
        "You can test how different driving routes are affected by proposed LTNs.",
        "",
        "The fastest route may not cut through neighbourhoods normally,",
        "but you can adjust the slow-down factor to mimic rush hour conditions",
    ]
}

fn card(
    ctx: &EventCtx,
    label: &'static str,
    tooltip: &'static str,
    time: Duration,
    color: Color,
) -> Widget {
    // TODO Convoluted way to add tooltips to text with a background
    let mut txt = Text::new();
    txt.add_line(Line(label).fg(color.invert()));
    txt.add_line(Line(time.to_rounded_string(0)).fg(color.invert()));
    let (batch, _) = txt
        .render_autocropped(ctx)
        .batch()
        .container()
        .bg(color)
        .padding(16)
        .into_geom(ctx, None);
    let btn = ButtonBuilder::new()
        .custom_batch(batch, ControlState::Default)
        .disabled(true);
    if tooltip.is_empty() {
        btn.build_widget(ctx, label)
    } else {
        btn.disabled_tooltip(tooltip).build_widget(ctx, label)
    }
}
