use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu};
use geom::{Duration, PolyLine};
use map_model::LANE_THICKNESS;
use sim::{AgentID, TripID};

pub enum RouteViewer {
    Inactive,
    Hovering(Duration, AgentID, PolyLine),
    Active(Duration, TripID, Option<PolyLine>),
}

impl RouteViewer {
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI, menu: &mut ModalMenu) {
        match self {
            RouteViewer::Inactive => {
                if let Some(agent) = ui
                    .primary
                    .current_selection
                    .as_ref()
                    .and_then(|id| id.agent_id())
                {
                    if let Some(trace) = ui.primary.sim.trace_route(agent, &ui.primary.map, None) {
                        *self = RouteViewer::Hovering(ui.primary.sim.time(), agent, trace);
                    }
                }
            }
            RouteViewer::Hovering(time, agent, _) => {
                // Argh, borrow checker.
                let mut agent = *agent;

                if *time != ui.primary.sim.time()
                    || ui.primary.current_selection != Some(ID::from_agent(agent))
                {
                    *self = RouteViewer::Inactive;
                    if let Some(new_agent) = ui
                        .primary
                        .current_selection
                        .as_ref()
                        .and_then(|id| id.agent_id())
                    {
                        // Gross.
                        agent = new_agent;
                        if let Some(trace) =
                            ui.primary.sim.trace_route(new_agent, &ui.primary.map, None)
                        {
                            *self = RouteViewer::Hovering(ui.primary.sim.time(), new_agent, trace);
                        }
                    }
                }

                // If there's a current route, then there must be a trip.
                let trip = ui.primary.sim.agent_to_trip(agent).unwrap();
                if ctx
                    .input
                    .contextual_action(Key::R, &format!("show {}'s route", agent))
                {
                    *self = show_route(trip, ui);
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
    if let Some(agent) = ui.primary.sim.trip_to_agent(trip) {
        // Trace along the entire route by passing in max distance
        if let Some(trace) = ui.primary.sim.trace_route(agent, &ui.primary.map, None) {
            RouteViewer::Active(time, trip, Some(trace))
        } else {
            println!("{} has no trace right now", agent);
            RouteViewer::Active(time, trip, None)
        }
    } else {
        println!(
            "{} has no agent associated right now; is the trip done?",
            trip
        );
        RouteViewer::Active(time, trip, None)
    }
}
