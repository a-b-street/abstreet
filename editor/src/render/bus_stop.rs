use aabb_quadtree::geom::Rect;
use colors::Colors;
use dimensioned::si;
use ezgui::GfxCtx;
use geom::{PolyLine, Polygon, Pt2D};
use map_model::{geometry, BusStop, BusStopID, Map};
use objects::{Ctx, ID};
use render::{get_bbox, RenderOptions, Renderable};

pub struct DrawBusStop {
    pub id: BusStopID,
    polygon: Polygon,
}

impl DrawBusStop {
    pub fn new(stop: &BusStop, map: &Map) -> DrawBusStop {
        let radius = 2.0 * si::M;
        // TODO if this happens to cross a bend in the lane, it'll look weird. similar to the
        // lookahead arrows and center points / dashed white, we really want to render an Interval
        // or something.
        // Kinda sad that bus stops might be very close to the start of the lane, but it's
        // happening.
        let lane = map.get_l(stop.id.sidewalk);
        let polygon = PolyLine::new(vec![
            lane.safe_dist_along(stop.dist_along - radius)
                .map(|(pt, _)| pt)
                .unwrap_or(lane.first_pt()),
            lane.safe_dist_along(stop.dist_along + radius)
                .map(|(pt, _)| pt)
                .unwrap_or(lane.last_pt()),
        ]).make_polygons_blindly(0.8 * geometry::LANE_THICKNESS);
        DrawBusStop {
            id: stop.id,
            polygon,
        }
    }
}

impl Renderable for DrawBusStop {
    fn get_id(&self) -> ID {
        ID::BusStop(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        g.draw_polygon(
            opts.color.unwrap_or(ctx.cs.get(Colors::BusStopMarking)),
            &self.polygon,
        );
    }

    fn get_bbox(&self) -> Rect {
        get_bbox(&self.polygon.get_bounds())
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }

    fn tooltip_lines(&self, _map: &Map) -> Vec<String> {
        vec![self.id.to_string()]
    }
}
