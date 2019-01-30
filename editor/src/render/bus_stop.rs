use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Distance, PolyLine, Polygon, Pt2D};
use map_model::{BusStop, BusStopID, Map, LANE_THICKNESS};

pub struct DrawBusStop {
    pub id: BusStopID,
    polygon: Polygon,
    zorder: isize,
}

impl DrawBusStop {
    pub fn new(stop: &BusStop, map: &Map) -> DrawBusStop {
        let radius = Distance::meters(2.0);
        // TODO if this happens to cross a bend in the lane, it'll look weird. similar to the
        // lookahead arrows and center points / dashed white, we really want to render an Interval
        // or something.
        // Kinda sad that bus stops might be very close to the start of the lane, but it's
        // happening.
        let lane = map.get_l(stop.id.sidewalk);
        let polygon = PolyLine::new(vec![
            lane.safe_dist_along(stop.sidewalk_pos.dist_along() - radius)
                .map(|(pt, _)| pt)
                .unwrap_or_else(|| lane.first_pt()),
            lane.safe_dist_along(stop.sidewalk_pos.dist_along() + radius)
                .map(|(pt, _)| pt)
                .unwrap_or_else(|| lane.last_pt()),
        ])
        .make_polygons(0.8 * LANE_THICKNESS);
        DrawBusStop {
            id: stop.id,
            polygon,
            zorder: map.get_parent(lane.id).get_zorder(),
        }
    }
}

impl Renderable for DrawBusStop {
    fn get_id(&self) -> ID {
        ID::BusStop(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        g.draw_polygon(
            opts.color.unwrap_or_else(|| {
                ctx.cs
                    .get_def("bus stop marking", Color::rgba(220, 160, 220, 0.8))
            }),
            &self.polygon,
        );
    }

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
