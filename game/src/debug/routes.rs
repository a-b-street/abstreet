use crate::ui::UI;
use ezgui::GfxCtx;
use geom::PolyLine;
use map_model::NORMAL_LANE_THICKNESS;
use sim::TripID;

pub enum AllRoutesViewer {
    Inactive,
    Active(Vec<PolyLine>),
}

impl AllRoutesViewer {
    pub fn toggle(&mut self, ui: &mut UI) {
        match self {
            AllRoutesViewer::Inactive => {
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
                        if let Some(trace) =
                            ui.primary.sim.trace_route(agent, &ui.primary.map, None)
                        {
                            traces.push(trace);
                        }
                    }
                }
                *self = AllRoutesViewer::Active(traces);
            }
            AllRoutesViewer::Active(_) => {
                *self = AllRoutesViewer::Inactive;
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let AllRoutesViewer::Active(ref traces) = self {
            for t in traces {
                g.draw_polygon(ui.cs.get("route"), &t.make_polygons(NORMAL_LANE_THICKNESS));
            }
        }
    }
}
