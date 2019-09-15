use crate::ui::UI;
use ezgui::{GfxCtx, ModalMenu};
use geom::{Duration, PolyLine};
use map_model::LANE_THICKNESS;
use sim::TripID;

pub enum AllRoutesViewer {
    Inactive,
    Active(Duration, Vec<PolyLine>),
}

impl AllRoutesViewer {
    pub fn event(&mut self, ui: &mut UI, menu: &mut ModalMenu) {
        match self {
            AllRoutesViewer::Inactive => {
                if menu.action("show/hide route for all agents") {
                    *self = debug_all_routes(ui);
                }
            }
            AllRoutesViewer::Active(time, _) => {
                if menu.action("show/hide route for all agents") {
                    *self = AllRoutesViewer::Inactive;
                } else if *time != ui.primary.sim.time() {
                    *self = debug_all_routes(ui);
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let AllRoutesViewer::Active(_, ref traces) = self {
            for t in traces {
                g.draw_polygon(ui.cs.get("route"), &t.make_polygons(LANE_THICKNESS));
            }
        }
    }
}

fn debug_all_routes(ui: &mut UI) -> AllRoutesViewer {
    let mut traces: Vec<PolyLine> = Vec::new();
    let trips: Vec<TripID> = ui
        .primary
        .sim
        .get_trip_positions(&ui.primary.map)
        .canonical_pt_per_trip
        .keys()
        .cloned()
        .collect();
    for trip in trips {
        if let Some(agent) = ui.primary.sim.trip_to_agent(trip).ok() {
            if let Some(trace) = ui.primary.sim.trace_route(agent, &ui.primary.map, None) {
                traces.push(trace);
            }
        }
    }
    AllRoutesViewer::Active(ui.primary.sim.time(), traces)
}
