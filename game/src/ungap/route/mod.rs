use map_model::RoutingParams;
use widgetry::{Choice, EventCtx, GfxCtx, Outcome, Panel, State, TextExt, Widget};

use self::results::{AltRouteResults, RouteResults};
use crate::app::{App, Transition};
use crate::common::InputWaypoints;
use crate::ungap::{Layers, Tab, TakeLayers};

mod files;
mod results;

pub struct RoutePlanner {
    layers: Layers,
    once: bool,

    input_panel: Panel,
    waypoints: InputWaypoints,
    main_route: RouteResults,
    files: files::RouteManagement,

    alt_routes: Vec<AltRouteResults>,
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
            main_route: RouteResults::main_route(ctx, app, Vec::new()),
            files: files::RouteManagement::new(app),

            alt_routes: Vec::new(),
        };
        rp.update_input_panel(ctx, app);
        Box::new(rp)
    }

    fn recalculate_routes(&mut self, ctx: &mut EventCtx, app: &App) {
        // Use the current session settings to determine "main" and alts
        self.main_route = RouteResults::main_route(ctx, app, self.waypoints.get_waypoints());

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
            let alt = AltRouteResults::new(
                ctx,
                app,
                self.waypoints.get_waypoints(),
                &self.main_route,
                preferences,
            );
            // Dedupe equivalent routes based on their stats, which is usually detailed enough
            if alt.results.stats != self.main_route.stats
                && self
                    .alt_routes
                    .iter()
                    .all(|x| alt.results.stats != x.results.stats)
            {
                self.alt_routes.push(alt);
            }
        }
    }

    fn update_input_panel(&mut self, ctx: &mut EventCtx, app: &App) {
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
            self.main_route.to_widget(ctx, app).section(ctx),
        ]);

        let mut new_panel = Tab::Route.make_left_panel(ctx, app, col);

        // TODO After scrolling down and dragging a slider, sometimes releasing the slider
        // registers as clicking "X" on the waypoints! Maybe just replace() in that case?
        new_panel.restore_scroll(ctx, &self.input_panel);
        self.input_panel = new_panel;
    }

    fn sync_from_file_management(&mut self, ctx: &mut EventCtx, app: &App) {
        self.waypoints
            .overwrite(ctx, app, self.files.current.waypoints.clone());
        self.recalculate_routes(ctx, app);
        self.update_input_panel(ctx, app);
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

        let mut focused_on_alt_route = false;
        for r in &mut self.alt_routes {
            r.event(ctx);
            focused_on_alt_route |= r.has_focus();
            if r.has_focus() && ctx.normal_left_click() {
                // Switch routes
                app.session.routing_preferences = r.results.preferences;
                self.recalculate_routes(ctx, app);
                self.update_input_panel(ctx, app);
                return Transition::Keep;
            }
        }

        let outcome = self.input_panel.event(ctx);
        if let Outcome::Clicked(ref x) = outcome {
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
        if let Outcome::Changed(ref x) = outcome {
            if x == "steep hills" || x == "stressful roads" {
                app.session.routing_preferences = RoutingPreferences {
                    hills: self.input_panel.dropdown_value("steep hills"),
                    stressful_roads: self.input_panel.dropdown_value("stressful roads"),
                };
                self.recalculate_routes(ctx, app);
                self.update_input_panel(ctx, app);
                return Transition::Keep;
            }
        }
        // Send all other outcomes here
        // TODO This routing of outcomes and the brittle ordering totally breaks encapsulation :(
        if let Some(t) = self
            .main_route
            .event(ctx, app, &outcome, &mut self.input_panel)
        {
            return t;
        }
        // Dragging behavior inside here only works if we're not hovering on an alternate route
        // TODO But then that prevents dragging some waypoints! Can we give waypoints precedence
        // instead?
        if !focused_on_alt_route && self.waypoints.event(ctx, app, outcome) {
            // Sync from waypoints to file management
            // TODO Maaaybe this directly live in the InputWaypoints system?
            self.files.current.waypoints = self.waypoints.get_waypoints();
            self.recalculate_routes(ctx, app);
            self.update_input_panel(ctx, app);
        }
        if focused_on_alt_route {
            // Still allow zooming
            if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                ctx.canvas.zoom(dy, ctx.canvas.get_cursor());
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
        self.main_route.draw(g, app, &self.input_panel);
        for r in &self.alt_routes {
            r.draw(g, app);
        }
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
