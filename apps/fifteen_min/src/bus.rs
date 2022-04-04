use abstutil::prettyprint_usize;
use geom::Duration;
use map_gui::tools::{InputWaypoints, WaypointID};
use map_model::connectivity::WalkingOptions;
use synthpop::{TripEndpoint, TripMode};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text,
    Transition, VerticalAlignment, Widget,
};

use crate::isochrone::{Isochrone, MovementOptions, Options};
use crate::App;

pub struct BusExperiment {
    panel: Panel,
    waypoints: InputWaypoints,
    world: World<ID>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ID {
    Waypoint(WaypointID),
    // Starting from this waypoint and going to the next
    BusRoute(usize),
}
impl ObjectID for ID {}

impl BusExperiment {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = BusExperiment {
            panel: Panel::empty(ctx),
            waypoints: InputWaypoints::new(app),
            world: World::unbounded(),
        };
        state.recalculate_everything(ctx, app);
        Box::new(state)
    }

    fn recalculate_everything(&mut self, ctx: &mut EventCtx, app: &App) {
        let map = &app.map;
        let mut world = World::bounded(map.get_bounds());
        self.waypoints
            .rebuild_world(ctx, &mut world, ID::Waypoint, 1);

        for (idx, pair) in self.waypoints.get_waypoints().windows(2).enumerate() {
            // TODO Pathfind for buses
            if let Some(path) = TripEndpoint::path_req(pair[0], pair[1], TripMode::Drive, map)
                .and_then(|req| map.pathfind(req).ok())
            {
                let duration = path.estimate_duration(map, None);
                if let Ok(hitbox) = path.trace_v2(map) {
                    world
                        .add(ID::BusRoute(idx))
                        .hitbox(hitbox)
                        .zorder(0)
                        .draw_color(self.waypoints.get_waypoint_color(idx))
                        .hover_alpha(0.8)
                        .tooltip(Text::from(Line(format!("Freeflow time is {duration}"))))
                        .build(ctx);
                }
            }
        }

        let stops = self
            .waypoints
            .get_waypoints()
            .into_iter()
            .filter_map(|endpt| match endpt {
                TripEndpoint::Building(b) => Some(b),
                _ => None,
            })
            .collect::<Vec<_>>();
        let isochrone = Isochrone::new(
            ctx,
            app,
            stops,
            Options {
                movement: MovementOptions::Walking(WalkingOptions::default()),
                thresholds: vec![(Duration::minutes(15), Color::grey(0.3).alpha(0.5))],
                // TODO The inner colors overlap the outer; this doesn't look right yet
                /*thresholds: vec![
                    (Duration::minutes(5), Color::grey(0.3).alpha(0.5)),
                    (Duration::minutes(10), Color::grey(0.3).alpha(0.3)),
                    (Duration::minutes(15), Color::grey(0.3).alpha(0.2)),
                ],*/
            },
        );
        world.draw_master_batch_built(isochrone.draw);

        world.initialize_hover(ctx);
        world.rebuilt_during_drag(&self.world);
        self.world = world;

        self.panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Bus planner"),
            ctx.style()
                .btn_back("15-minute neighborhoods")
                .hotkey(Key::Escape)
                .build_def(ctx),
            Text::from_multiline(vec![
                Line("Within a 15 min walk of all stops:"),
                Line(format!(
                    "Population: {}",
                    prettyprint_usize(isochrone.population)
                )),
                Line(format!(
                    "Shops: {}",
                    prettyprint_usize(
                        isochrone
                            .amenities_reachable
                            .borrow()
                            .values()
                            .map(|x| x.len())
                            .sum()
                    )
                )),
            ])
            .into_widget(ctx),
            self.waypoints.get_panel_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .ignore_initial_events()
        .build(ctx);
    }
}

impl State<App> for BusExperiment {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        let panel_outcome = self.panel.event(ctx);
        if let Outcome::Clicked(ref x) = panel_outcome {
            if x == "15-minute neighborhoods" {
                return Transition::Pop;
            }
        }

        let world_outcome = self.world.event(ctx);
        let world_outcome_for_waypoints = world_outcome
            .maybe_map_id(|id| match id {
                ID::Waypoint(id) => Some(id),
                _ => None,
            })
            .unwrap_or(WorldOutcome::Nothing);

        if self
            .waypoints
            .event(app, panel_outcome, world_outcome_for_waypoints)
        {
            self.recalculate_everything(ctx, app);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.world.draw(g);
    }
}
