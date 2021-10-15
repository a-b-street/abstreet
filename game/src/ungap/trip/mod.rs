use map_model::RoutingParams;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{EventCtx, GfxCtx, Outcome, Panel, State, Toggle, Widget};

use self::results::RouteDetails;
use crate::app::{App, Transition};
use crate::common::{InputWaypoints, WaypointID};
use crate::ungap::{Layers, Tab, TakeLayers};

mod files;
mod results;

pub struct TripPlanner {
    layers: Layers,

    input_panel: Panel,
    waypoints: InputWaypoints,
    main_route: RouteDetails,
    files: files::TripManagement,
    // TODO We really only need to store preferences and stats, but...
    alt_routes: Vec<RouteDetails>,
    world: World<ID>,

    thickness: f64,
}

impl TakeLayers for TripPlanner {
    fn take_layers(self) -> Layers {
        self.layers
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ID {
    MainRoute,
    AltRoute(usize),
    Waypoint(WaypointID),
}
impl ObjectID for ID {}

impl TripPlanner {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, layers: Layers) -> Box<dyn State<App>> {
        ctx.loading_screen("apply edits", |_, mut timer| {
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
        });

        let mut rp = TripPlanner {
            layers,

            input_panel: Panel::empty(ctx),
            waypoints: InputWaypoints::new(app),
            main_route: RouteDetails::main_route(ctx, app, Vec::new(), 0.0).details,
            files: files::TripManagement::new(app),
            alt_routes: Vec::new(),
            world: World::bounded(app.primary.map.get_bounds()),
            thickness: 0.0,
        };

        if let Some(current_name) = &app.session.ungap_current_trip_name {
            rp.files.set_current(current_name);
        }
        rp.sync_from_file_management(ctx, app);
        Box::new(rp)
    }

    // Use the current session settings to determine "main" and alts
    fn recalculate_routes(&mut self, ctx: &mut EventCtx, app: &mut App) {
        self.thickness = zoom_to_thickness(ctx);
        let mut world = World::bounded(app.primary.map.get_bounds());

        let main_route =
            RouteDetails::main_route(ctx, app, self.waypoints.get_waypoints(), self.thickness);
        self.main_route = main_route.details;
        world
            .add(ID::MainRoute)
            .hitbox(main_route.hitbox)
            .zorder(1)
            .draw(main_route.draw)
            .build(ctx);

        self.files.autosave(app);
        // This doesn't depend on the alt routes, so just do it here
        self.update_input_panel(ctx, app, main_route.details_widget);

        self.alt_routes.clear();
        // Just show one alternate trip by default, unless the user enables one checkbox but not
        // the other. We could show more variations, but it makes the view too messy.
        for preferences in [
            RoutingPreferences {
                avoid_hills: false,
                avoid_stressful_roads: false,
            },
            RoutingPreferences {
                avoid_hills: true,
                avoid_stressful_roads: true,
            },
        ] {
            if app.session.routing_preferences == preferences {
                continue;
            }
            let mut alt = RouteDetails::alt_route(
                ctx,
                app,
                self.waypoints.get_waypoints(),
                &self.main_route,
                preferences,
                self.thickness,
            );
            // Dedupe equivalent routes based on their stats, which is usually detailed enough
            if alt.details.stats != self.main_route.stats
                && self.alt_routes.iter().all(|x| alt.details.stats != x.stats)
            {
                self.alt_routes.push(alt.details);
                world
                    .add(ID::AltRoute(self.alt_routes.len() - 1))
                    .hitbox(alt.hitbox)
                    .zorder(0)
                    .draw(alt.draw)
                    .hover_alpha(0.8)
                    .tooltip(alt.tooltip_for_alt.take().unwrap())
                    .clickable()
                    .build(ctx);
            }
        }

        // TODO Oh no, even these have to scale with zoom!?
        self.waypoints
            .rebuild_world(ctx, &mut world, |id| ID::Waypoint(id), 2, self.thickness);

        world.initialize_hover(ctx);
        world.rebuilt_during_drag(&self.world);
        self.world = world;
    }

    fn update_input_panel(&mut self, ctx: &mut EventCtx, app: &App, main_route: Widget) {
        let mut sections = vec![Widget::col(vec![
            self.files.get_panel_widget(ctx),
            Widget::horiz_separator(ctx, 1.0),
            self.waypoints.get_panel_widget(ctx),
        ])
        .section(ctx)];
        if self.waypoints.len() >= 2 {
            sections.push(
                Widget::row(vec![
                    Toggle::checkbox(
                        ctx,
                        "Avoid steep hills",
                        None,
                        app.session.routing_preferences.avoid_hills,
                    ),
                    Toggle::checkbox(
                        ctx,
                        "Avoid stressful roads",
                        None,
                        app.session.routing_preferences.avoid_stressful_roads,
                    ),
                ])
                .section(ctx),
            );
            sections.push(main_route.section(ctx));
        }

        let col = Widget::col(sections);
        let mut new_panel = Tab::Trip.make_left_panel(ctx, app, col);

        // TODO After scrolling down and dragging a slider, sometimes releasing the slider
        // registers as clicking "X" on the waypoints! Maybe just replace() in that case?
        new_panel.restore_scroll(ctx, &self.input_panel);
        self.input_panel = new_panel;
    }

    fn sync_from_file_management(&mut self, ctx: &mut EventCtx, app: &mut App) {
        self.waypoints
            .overwrite(app, self.files.current.waypoints.clone());
        self.recalculate_routes(ctx, app);
    }
}

impl State<App> for TripPlanner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let world_outcome_for_waypoints = match self.world.event(ctx) {
            WorldOutcome::ClickedObject(ID::AltRoute(idx)) => {
                // Switch routes
                app.session.routing_preferences = self.alt_routes[idx].preferences;
                self.recalculate_routes(ctx, app);
                return Transition::Keep;
            }
            x => x.map_id(|id| match id {
                ID::Waypoint(id) => id,
                _ => unreachable!(),
            }),
        };

        if zoom_to_thickness(ctx) != self.thickness {
            self.recalculate_routes(ctx, app);
        }

        let panel_outcome = self.input_panel.event(ctx);
        if let Outcome::Clicked(ref x) = panel_outcome {
            if let Some(t) = Tab::Trip.handle_action::<TripPlanner>(ctx, app, x) {
                return t;
            }
            if let Some(t) = self.files.on_click(ctx, app, x) {
                // Bit hacky...
                if matches!(t, Transition::Keep) {
                    self.sync_from_file_management(ctx, app);
                }
                return t;
            }
        }
        if let Outcome::Changed(ref x) = panel_outcome {
            if x == "Avoid steep hills" || x == "Avoid stressful roads" {
                app.session.routing_preferences = RoutingPreferences {
                    avoid_hills: self.input_panel.is_checked("Avoid steep hills"),
                    avoid_stressful_roads: self.input_panel.is_checked("Avoid stressful roads"),
                };
                self.recalculate_routes(ctx, app);
                return Transition::Keep;
            }
        }
        // Send all other outcomes here
        // TODO This routing of outcomes and the brittle ordering totally breaks encapsulation :(
        if let Some(t) = self
            .main_route
            .event(ctx, app, &panel_outcome, &mut self.input_panel)
        {
            return t;
        }

        if self
            .waypoints
            .event(app, panel_outcome, world_outcome_for_waypoints)
        {
            // Sync from waypoints to file management
            // TODO Maaaybe this directly live in the InputWaypoints system?
            self.files.current.waypoints = self.waypoints.get_waypoints();
            self.recalculate_routes(ctx, app);
        }

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.layers.draw(g, app);
        self.input_panel.draw(g);
        self.world.draw(g);
        self.main_route.draw(g, &self.input_panel);
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct RoutingPreferences {
    avoid_hills: bool,
    avoid_stressful_roads: bool,
}

impl RoutingPreferences {
    // TODO Consider changing this now, and also for the mode shift calculation
    pub fn default() -> Self {
        Self {
            avoid_hills: false,
            avoid_stressful_roads: false,
        }
    }

    fn name(self) -> &'static str {
        match (self.avoid_hills, self.avoid_stressful_roads) {
            (false, false) => "fastest",
            (true, false) => "flat",
            (false, true) => "low-stress",
            (true, true) => "flat & low-stress",
        }
    }

    fn routing_params(self) -> RoutingParams {
        RoutingParams {
            avoid_steep_incline_penalty: if self.avoid_hills { 2.0 } else { 1.0 },
            avoid_high_stress: if self.avoid_stressful_roads { 2.0 } else { 1.0 },
            ..Default::default()
        }
    }
}

fn zoom_to_thickness(ctx: &EventCtx) -> f64 {
    let zoom = ctx.canvas.cam_zoom;
    let bucketed_zoom = if zoom >= 1.0 {
        1.0
    } else {
        (zoom * 10.0).round() / 10.0
    };

    // Thicker lines as we zoom out. Scale up to 5x. Never shrink past the road's actual width.
    let thickness = (0.5 / bucketed_zoom).max(1.0);
    // And on gigantic maps, zoom may approach 0, so avoid NaNs.
    if thickness.is_finite() {
        thickness
    } else {
        5.0
    }
}
