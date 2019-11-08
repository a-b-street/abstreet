use crate::helpers::ID;
use crate::render::{dashed_lines, MIN_ZOOM_FOR_DETAIL};
use crate::ui::UI;
use ezgui::{hotkey, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, ModalMenu};
use geom::{Distance, Duration};
use sim::{AgentID, TripID, TripResult};

pub enum RouteViewer {
    Inactive,
    Hovering(Duration, AgentID, Drawable),
    // (zoomed, unzoomed)
    Active(Duration, TripID, Option<(Drawable, Drawable)>),
}

impl RouteViewer {
    fn recalc(ctx: &EventCtx, ui: &UI) -> RouteViewer {
        if let Some(agent) = ui
            .primary
            .current_selection
            .as_ref()
            .and_then(|id| id.agent_id())
        {
            if let Some(trace) = ui.primary.sim.trace_route(agent, &ui.primary.map, None) {
                let mut batch = GeomBatch::new();
                batch.extend(
                    ui.cs.get_def("route", Color::ORANGE.alpha(0.5)),
                    dashed_lines(
                        &trace,
                        Distance::meters(0.75),
                        Distance::meters(1.0),
                        Distance::meters(0.4),
                    ),
                );
                return RouteViewer::Hovering(
                    ui.primary.sim.time(),
                    agent,
                    ctx.prerender.upload(batch),
                );
            }
        }
        RouteViewer::Inactive
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI, menu: &mut ModalMenu) {
        match self {
            RouteViewer::Inactive => {
                *self = RouteViewer::recalc(ctx, ui);
            }
            RouteViewer::Hovering(time, agent, _) => {
                if *time != ui.primary.sim.time()
                    || ui.primary.current_selection != Some(ID::from_agent(*agent))
                {
                    *self = RouteViewer::recalc(ctx, ui);
                }

                if let RouteViewer::Hovering(_, agent, _) = self {
                    // If there's a current route, then there must be a trip.
                    let trip = ui.primary.sim.agent_to_trip(*agent).unwrap();
                    if ctx
                        .input
                        .contextual_action(Key::R, format!("show {}'s route", agent))
                    {
                        *self = show_route(trip, ui, ctx);
                        menu.push_action(hotkey(Key::R), "stop showing agent's route", ctx);
                    }
                }
            }
            RouteViewer::Active(time, trip, _) => {
                if menu.consume_action("stop showing agent's route", ctx) {
                    *self = RouteViewer::Inactive;
                } else if *time != ui.primary.sim.time() {
                    *self = show_route(*trip, ui, ctx);
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            RouteViewer::Hovering(_, _, ref route) => {
                g.redraw(route);
            }
            RouteViewer::Active(_, _, Some((ref zoomed, ref unzoomed))) => {
                if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                    g.redraw(unzoomed);
                } else {
                    g.redraw(zoomed);
                }
            }
            _ => {}
        }
    }
}

fn show_route(trip: TripID, ui: &UI, ctx: &EventCtx) -> RouteViewer {
    let time = ui.primary.sim.time();
    match ui.primary.sim.trip_to_agent(trip) {
        TripResult::Ok(agent) => RouteViewer::Active(
            time,
            trip,
            ui.primary
                .sim
                .trace_route(agent, &ui.primary.map, None)
                .map(|trace| {
                    let mut zoomed = GeomBatch::new();
                    zoomed.extend(
                        ui.cs.get("route").alpha(0.8),
                        dashed_lines(
                            &trace,
                            Distance::meters(0.75),
                            Distance::meters(1.0),
                            Distance::meters(0.4),
                        ),
                    );

                    let mut unzoomed = GeomBatch::new();
                    unzoomed.push(
                        ui.cs.get_def("unzoomed route", Color::CYAN),
                        trace.make_polygons(Distance::meters(10.0)),
                    );

                    (ctx.prerender.upload(zoomed), ctx.prerender.upload(unzoomed))
                }),
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
