use map_model::RoutingParams;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{Choice, EventCtx, GfxCtx, Outcome, Panel, State, TextExt, Widget};

use self::results::RouteDetails;
use crate::app::{App, Transition};
use crate::common::{InputWaypoints, WaypointID};
use crate::ungap::{Layers, Tab, TakeLayers};

mod files;
mod results;

pub struct RoutePlanner {
    layers: Layers,
    once: bool,

    input_panel: Panel,
    waypoints: InputWaypoints,
    main_route: RouteDetails,
    files: files::RouteManagement,
    // TODO We really only need to store preferences and stats, but...
    alt_routes: Vec<RouteDetails>,
    world: World<ID>,
}

impl TakeLayers for RoutePlanner {
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

impl RoutePlanner {
    pub fn new_state(ctx: &mut EventCtx, app: &App, layers: Layers) -> Box<dyn State<App>> {
        let mut rp = RoutePlanner {
            layers,
            once: true,

            input_panel: Panel::empty(ctx),
            waypoints: InputWaypoints::new(app),
            main_route: RouteDetails::main_route(ctx, app, Vec::new()).details,
            files: files::RouteManagement::new(app),
            alt_routes: Vec::new(),
            world: World::bounded(app.primary.map.get_bounds()),
        };
        rp.recalculate_routes(ctx, app);
        Box::new(rp)
    }

    // Use the current session settings to determine "main" and alts
    fn recalculate_routes(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut world = World::bounded(app.primary.map.get_bounds());

        let main_route = RouteDetails::main_route(ctx, app, self.waypoints.get_waypoints());
        self.main_route = main_route.details;
        world
            .add(ID::MainRoute)
            .hitbox(main_route.hitbox)
            .zorder(1)
            .draw(main_route.draw)
            .build(ctx);
        // This doesn't depend on the alt routes, so just do it here
        self.update_input_panel(ctx, app, main_route.details_widget);

        self.alt_routes.clear();
        // Just a few fixed variations... all 9 combos seems overwhelming
        for preferences in [
            RoutingPreferences {
                hills: Preference::Neutral,
                stressful_roads: Preference::Neutral,
            },
            RoutingPreferences {
                hills: Preference::Avoid,
                stressful_roads: Preference::Avoid,
            },
            // TODO Too many alts cover up the main route awkwardly
            /*RoutingPreferences {
                hills: Preference::SeekOut,
                stressful_roads: Preference::SeekOut,
            },*/
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

        self.waypoints
            .rebuild_world(ctx, &mut world, |id| ID::Waypoint(id), 2);

        world.initialize_hover(ctx);
        world.rebuilt_during_drag(&self.world);
        self.world = world;
    }

    fn update_input_panel(&mut self, ctx: &mut EventCtx, app: &App, main_route: Widget) {
        let col = Widget::col(vec![
            self.files.get_panel_widget(ctx),
            Widget::col(vec![Widget::row(vec![
                "Steep hills".text_widget(ctx).centered_vert(),
                Widget::dropdown(
                    ctx,
                    "steep hills",
                    app.session.routing_preferences.hills,
                    vec![
                        Choice::new("avoid", Preference::Avoid),
                        // TODO Wording for these
                        Choice::new("neutral", Preference::Neutral),
                        Choice::new("fitness mode!", Preference::SeekOut),
                    ],
                ),
                "High-stress roads".text_widget(ctx).centered_vert(),
                Widget::dropdown(
                    ctx,
                    "stressful roads",
                    app.session.routing_preferences.stressful_roads,
                    vec![
                        Choice::new("avoid", Preference::Avoid),
                        // TODO Wording for these
                        Choice::new("neutral", Preference::Neutral),
                        Choice::new("danger zone!", Preference::SeekOut),
                    ],
                ),
            ])])
            .section(ctx),
            self.waypoints.get_panel_widget(ctx).section(ctx),
            main_route.section(ctx),
        ]);

        let mut new_panel = Tab::Route.make_left_panel(ctx, app, col);

        // TODO After scrolling down and dragging a slider, sometimes releasing the slider
        // registers as clicking "X" on the waypoints! Maybe just replace() in that case?
        new_panel.restore_scroll(ctx, &self.input_panel);
        self.input_panel = new_panel;
    }

    fn sync_from_file_management(&mut self, ctx: &mut EventCtx, app: &App) {
        self.waypoints
            .overwrite(app, self.files.current.waypoints.clone());
        self.recalculate_routes(ctx, app);
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

        let panel_outcome = self.input_panel.event(ctx);
        if let Outcome::Clicked(ref x) = panel_outcome {
            if let Some(t) = Tab::Route.handle_action::<RoutePlanner>(ctx, app, x) {
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
            if x == "steep hills" || x == "stressful roads" {
                app.session.routing_preferences = RoutingPreferences {
                    hills: self.input_panel.dropdown_value("steep hills"),
                    stressful_roads: self.input_panel.dropdown_value("stressful roads"),
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
    hills: Preference,
    stressful_roads: Preference,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Preference {
    Avoid,
    Neutral,
    SeekOut,
}

impl RoutingPreferences {
    // TODO Consider changing this now, and also for the mode shift calculation
    pub fn default() -> Self {
        Self {
            hills: Preference::Neutral,
            stressful_roads: Preference::Neutral,
        }
    }

    fn name(self) -> String {
        let words = vec![
            match self.hills {
                Preference::Avoid => Some("flat"),
                Preference::Neutral => None,
                Preference::SeekOut => Some("steep"),
            },
            match self.stressful_roads {
                Preference::Avoid => Some("low-stress"),
                Preference::Neutral => None,
                Preference::SeekOut => Some("high-stress"),
            },
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if words.is_empty() {
            "default".to_string()
        } else if words.len() == 1 {
            words[0].to_string()
        } else {
            format!("{}, {}", words[0], words[1])
        }
    }

    fn routing_params(self) -> RoutingParams {
        RoutingParams {
            avoid_steep_incline_penalty: match self.hills {
                Preference::Avoid => 2.0,
                Preference::Neutral => 1.0,
                Preference::SeekOut => 0.1,
            },
            avoid_high_stress: match self.stressful_roads {
                Preference::Avoid => 2.0,
                Preference::Neutral => 1.0,
                Preference::SeekOut => 0.1,
            },
            ..Default::default()
        }
    }
}
