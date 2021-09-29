use widgetry::{EventCtx, GfxCtx, Outcome, Panel, Slider, State, TextExt, Widget};

use self::results::{AltRouteResults, RouteResults};
use crate::app::{App, Transition};
use crate::common::InputWaypoints;
use crate::ungap::{Layers, Tab, TakeLayers};

mod files;
mod results;

const MAX_AVOID_PARAM: f64 = 2.0;

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

    fn update_input_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        self.main_route = RouteResults::main_route(ctx, app, self.waypoints.get_waypoints());
        self.alt_routes.clear();
        let low_stress =
            AltRouteResults::low_stress(ctx, app, self.waypoints.get_waypoints(), &self.main_route);
        if low_stress.results.stats != self.main_route.stats {
            self.alt_routes.push(low_stress);
        }

        let col = Widget::col(vec![
            self.files.get_panel_widget(ctx),
            Widget::col(vec![
                Widget::row(vec![
                    "Avoid steep hills (> 8% incline)".text_widget(ctx),
                    Slider::area(
                        ctx,
                        100.0,
                        self.main_route.params.avoid_steep_incline_penalty / MAX_AVOID_PARAM,
                        "avoid_steep_incline_penalty",
                    ),
                ]),
                Widget::row(vec![
                    "Avoid high-stress roads".text_widget(ctx),
                    Slider::area(
                        ctx,
                        100.0,
                        self.main_route.params.avoid_high_stress / MAX_AVOID_PARAM,
                        "avoid_high_stress",
                    ),
                ]),
            ])
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
                println!("SWITCH");
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
            if x == "avoid_steep_incline_penalty" || x == "avoid_high_stress" {
                app.session.routing_params.avoid_steep_incline_penalty = MAX_AVOID_PARAM
                    * self
                        .input_panel
                        .slider("avoid_steep_incline_penalty")
                        .get_percent();
                app.session.routing_params.avoid_high_stress =
                    MAX_AVOID_PARAM * self.input_panel.slider("avoid_high_stress").get_percent();
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
            self.update_input_panel(ctx, app);
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
