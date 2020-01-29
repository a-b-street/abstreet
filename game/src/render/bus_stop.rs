use crate::helpers::{ColorScheme, ID};
use crate::render::{DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Distance, PolyLine, Polygon, Pt2D};
use map_model::{BusStop, BusStopID, Map};

pub struct DrawBusStop {
    pub id: BusStopID,
    polyline: PolyLine,
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
        let main_pl = lane
            .lane_center_pts
            .exact_slice(
                Distance::ZERO.max(stop.sidewalk_pos.dist_along() - radius),
                lane.length().min(stop.sidewalk_pos.dist_along() + radius),
            )
            .shift_right(lane.width * 0.3)
            .unwrap();
        let polyline = PolyLine::new(vec![
            main_pl.first_pt().project_away(
                lane.width * 0.5,
                main_pl.first_line().angle().rotate_degs(-90.0),
            ),
            main_pl.first_pt(),
        ])
        .extend(main_pl.clone())
        .extend(PolyLine::new(vec![
            main_pl.last_pt(),
            main_pl.last_pt().project_away(
                lane.width * 0.5,
                main_pl.last_line().angle().rotate_degs(-90.0),
            ),
        ]));

        let polygon = polyline.make_polygons(lane.width * 0.25);
        let draw_default = prerender.upload_borrowed(vec![(
            cs.get_def("bus stop marking", Color::CYAN),
            &polygon,
        )]);

        DrawBusStop {
            id: stop.id,
            polyline,
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

    fn draw(&self, g: &mut GfxCtx, _: &DrawOptions, _: &DrawCtx) {
        g.redraw(&self.draw_default);
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        let lane = map.get_l(self.id.sidewalk);
        self.polyline
            .to_thick_boundary(lane.width * 0.25, OUTLINE_THICKNESS)
            .unwrap_or_else(|| self.polygon.clone())
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        self.polygon.contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
