use crate::app::App;
use ezgui::GfxCtx;
use geom::PolyLine;
use map_model::NORMAL_LANE_THICKNESS;
use sim::TripID;

pub enum AllRoutesViewer {
    Inactive,
    Active(Vec<PolyLine>),
}

impl AllRoutesViewer {
    pub fn toggle(&mut self, app: &mut App) {
        match self {
            AllRoutesViewer::Inactive => {
                let mut traces: Vec<PolyLine> = Vec::new();
                let trips: Vec<TripID> = app
                    .primary
                    .sim
                    .get_trip_positions(&app.primary.map)
                    .canonical_pt_per_trip
                    .keys()
                    .cloned()
                    .collect();
                for trip in trips {
                    if let Some(agent) = app.primary.sim.trip_to_agent(trip).ok() {
                        if let Some(trace) =
                            app.primary.sim.trace_route(agent, &app.primary.map, None)
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

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        if let AllRoutesViewer::Active(ref traces) = self {
            for t in traces {
                g.draw_polygon(app.cs.get("route"), &t.make_polygons(NORMAL_LANE_THICKNESS));
            }
        }
    }
}
