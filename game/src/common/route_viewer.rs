use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu};
use geom::{Duration, PolyLine};
use map_model::LANE_THICKNESS;
use sim::{AgentID, TripID, TripResult};

pub enum RouteViewer {
    Inactive,
    Hovering(Duration, AgentID, PolyLine),
    Active(Duration, TripID, Option<PolyLine>),
}

impl RouteViewer {
    fn recalc(ui: &UI) -> RouteViewer {
        if let Some(agent) = ui
            .primary
            .current_selection
            .as_ref()
            .and_then(|id| id.agent_id())
        {
            if let Some(trace) = ui.primary.sim.trace_route(agent, &ui.primary.map, None) {
                return RouteViewer::Hovering(ui.primary.sim.time(), agent, trace);
            }
        }
        RouteViewer::Inactive
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI, menu: &mut ModalMenu) {
        match self {
            RouteViewer::Inactive => {
                *self = RouteViewer::recalc(ui);
            }
            RouteViewer::Hovering(time, agent, _) => {
                if *time != ui.primary.sim.time()
                    || ui.primary.current_selection != Some(ID::from_agent(*agent))
                {
                    *self = RouteViewer::recalc(ui);
                }

                if let RouteViewer::Hovering(_, agent, _) = self {
                    // If there's a current route, then there must be a trip.
                    let trip = ui.primary.sim.agent_to_trip(*agent).unwrap();
                    if ctx
                        .input
                        .contextual_action(Key::R, format!("show {}'s route", agent))
                    {
                        *self = show_route(trip, ui);
                    }
                }
            }
            RouteViewer::Active(time, trip, _) => {
                // TODO Using the modal menu from parent is weird...
                if menu.action("stop showing agent's route") {
                    *self = RouteViewer::Inactive;
                } else if *time != ui.primary.sim.time() {
                    *self = show_route(*trip, ui);
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        match self {
            RouteViewer::Hovering(_, _, ref trace) => {
                g.draw_polygon(
                    ui.cs.get("route").alpha(0.5),
                    &trace.make_polygons(LANE_THICKNESS),
                );
            }
            RouteViewer::Active(_, _, Some(ref trace)) => {
                g.draw_polygon(
                    ui.cs.get_def("route", Color::RED.alpha(0.8)),
                    &trace.make_polygons(LANE_THICKNESS),
                );
            }
            _ => {}
        }
    }
}

fn show_route(trip: TripID, ui: &UI) -> RouteViewer {
    let time = ui.primary.sim.time();
    match ui.primary.sim.trip_to_agent(trip) {
        TripResult::Ok(agent) => RouteViewer::Active(
            time,
            trip,
            ui.primary.sim.trace_route(agent, &ui.primary.map, None),
        ),
        TripResult::ModeChange => {
            println!("{} is doing a mode change", trip);
            RouteViewer::Active(time, trip, None)
        }
        TripResult::TripDone => {
            println!("{} is done or aborted, so no more showing route", trip);
            RouteViewer::Inactive
        }
        TripResult::TripDoesntExist => {
            println!("{} doesn't exist yet, so not showing route", trip);
            RouteViewer::Inactive
        }
    }
}
