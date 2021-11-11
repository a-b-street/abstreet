use geom::{Distance, Polygon};
use map_model::NORMAL_LANE_THICKNESS;
use sim::{TripEndpoint, TripMode};
use widgetry::mapspace::{ObjectID, ToggleZoomed, World};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, VerticalAlignment,
    Widget,
};

use super::Neighborhood;
use crate::app::{App, Transition};
use crate::common::{InputWaypoints, WaypointID};

pub struct RoutePlanner {
    panel: Panel,
    waypoints: InputWaypoints,
    world: World<ID>,

    neighborhood: Neighborhood,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ID {
    MainRoute,
    Waypoint(WaypointID),
}
impl ObjectID for ID {}

impl RoutePlanner {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        neighborhood: Neighborhood,
    ) -> Box<dyn State<App>> {
        let mut rp = RoutePlanner {
            panel: Panel::empty(ctx),
            waypoints: InputWaypoints::new(app),
            world: World::bounded(app.primary.map.get_bounds()),
            neighborhood,
        };
        rp.update(ctx, app);
        Box::new(rp)
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        self.panel = Panel::new_builder(Widget::col(vec![
            ctx.style()
                .btn_outline
                .text("Back to editing modal filters")
                .hotkey(Key::Escape)
                .build_def(ctx),
            self.waypoints.get_panel_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        // Hovering on waypoint cards
        .ignore_initial_events()
        .build(ctx);

        let mut world = self.calculate_paths(ctx, app);
        self.waypoints
            .rebuild_world(ctx, &mut world, ID::Waypoint, 1);
        world.initialize_hover(ctx);
        world.rebuilt_during_drag(&self.world);
        self.world = world;
    }

    fn calculate_paths(&self, ctx: &mut EventCtx, app: &App) -> World<ID> {
        let map = &app.primary.map;

        let mut world = World::bounded(map.get_bounds());

        let mut params = map.routing_params().clone();
        params
            .avoid_roads
            .extend(app.session.modal_filters.roads.keys().cloned());
        let cache_custom = true;

        let mut draw_route = ToggleZoomed::builder();
        let mut hitbox_pieces = Vec::new();
        for pair in self.waypoints.get_waypoints().windows(2) {
            if let Some(pl) = TripEndpoint::path_req(pair[0], pair[1], TripMode::Drive, map)
                .and_then(|req| map.pathfind_with_params(req, &params, cache_custom).ok())
                .and_then(|path| path.trace(map))
            {
                let shape = pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS);
                draw_route
                    .unzoomed
                    .push(Color::RED.alpha(0.8), shape.clone());
                draw_route.zoomed.push(Color::RED.alpha(0.5), shape.clone());
                hitbox_pieces.push(shape);
            }
        }
        if !hitbox_pieces.is_empty() {
            world
                .add(ID::MainRoute)
                .hitbox(Polygon::union_all(hitbox_pieces))
                .draw(draw_route)
                .hover_outline(Color::BLACK, Distance::meters(2.0))
                .build(ctx);
        }

        world
    }
}

impl State<App> for RoutePlanner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let world_outcome_for_waypoints = match self.world.event(ctx) {
            x => x.map_id(|id| match id {
                ID::Waypoint(id) => id,
                _ => unreachable!(),
            }),
        };

        let panel_outcome = self.panel.event(ctx);
        if let Outcome::Clicked(ref x) = panel_outcome {
            if x == "Back to editing modal filters" {
                return Transition::ConsumeState(Box::new(|state, ctx, app| {
                    let state = state.downcast::<RoutePlanner>().ok().unwrap();
                    vec![super::viewer::Viewer::new_state(
                        ctx,
                        app,
                        state.neighborhood,
                    )]
                }));
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
        g.redraw(&self.neighborhood.draw_filters);
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }

        self.world.draw(g);
    }
}
