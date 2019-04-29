use crate::helpers::{ColorScheme, DrawCtx, ID};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Distance, Polygon};
use map_model::{BusStop, BusStopID, Map, LANE_THICKNESS};

pub struct DrawBusStop {
    pub id: BusStopID,
    polygon: Polygon,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawBusStop {
    pub fn new(stop: &BusStop, map: &Map, cs: &ColorScheme, prerender: &Prerender) -> DrawBusStop {
        let radius = Distance::meters(2.0);
        // Kinda sad that bus stops might be very close to the start of the lane, but it's
        // happening.
        let lane = map.get_l(stop.id.sidewalk);
        let polygon = lane
            .lane_center_pts
            .exact_slice(
                Distance::ZERO.max(stop.sidewalk_pos.dist_along() - radius),
                lane.length().min(stop.sidewalk_pos.dist_along() + radius),
            )
            .make_polygons(LANE_THICKNESS * 0.8);
        let draw_default = prerender.upload_borrowed(vec![(
            cs.get_def("bus stop marking", Color::rgba(220, 160, 220, 0.8)),
            &polygon,
        )]);

        DrawBusStop {
            id: stop.id,
            polygon,
            zorder: map.get_parent(lane.id).get_zorder(),
            draw_default,
        }
    }
}

impl Renderable for DrawBusStop {
    fn get_id(&self) -> ID {
        ID::BusStop(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, _ctx: &DrawCtx) {
        if let Some(color) = opts.color {
            g.draw_polygon(color, &self.polygon);
        } else {
            g.redraw(&self.draw_default);
        }
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        self.polygon.clone()
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
