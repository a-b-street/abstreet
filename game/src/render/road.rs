use crate::helpers::{ColorScheme, ID};
use crate::render::{
    osm_rank_to_road_center_line_color, DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS,
};
use ezgui::{Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Distance, Polygon, Pt2D};
use map_model::{LaneType, Map, Road, RoadID, LANE_THICKNESS};

pub struct DrawRoad {
    pub id: RoadID,
    zorder: isize,

    draw_center_line: Drawable,
    label: Text,
    label_pos: Pt2D,
}

impl DrawRoad {
    pub fn new(r: &Road, map: &Map, cs: &ColorScheme, prerender: &Prerender) -> DrawRoad {
        let mut draw = GeomBatch::new();
        // The road's original center_pts don't account for contraflow lane edits.
        let center = map
            .get_l(if !r.children_forwards.is_empty() {
                r.children_forwards[0].0
            } else {
                r.children_backwards[0].0
            })
            .lane_center_pts
            .shift_left(LANE_THICKNESS / 2.0)
            .unwrap();
        let color = osm_rank_to_road_center_line_color(cs, r.get_rank());

        // Only one yellow line if the road is a oneway
        // No center line at all if there's a shared left turn lane
        if r.children_backwards
            .iter()
            .all(|(_, lt)| *lt == LaneType::Parking || *lt == LaneType::Sidewalk)
        {
            draw.push(color, center.make_polygons(Distance::meters(0.25)));
        } else if r.children_forwards.is_empty()
            || r.children_forwards[0].1 != LaneType::SharedLeftTurn
        {
            draw.push(
                color,
                center
                    .shift_left(Distance::meters(0.25))
                    .unwrap()
                    .make_polygons(Distance::meters(0.25)),
            );
            draw.push(
                color,
                center
                    .shift_right(Distance::meters(0.25))
                    .unwrap()
                    .make_polygons(Distance::meters(0.25)),
            );
        }

        let mut label = Text::new();
        label.add(Line(r.get_name()).size(50));

        DrawRoad {
            id: r.id,
            zorder: r.get_zorder(),
            draw_center_line: prerender.upload(draw),
            label,
            label_pos: r.center_pts.middle(),
        }
    }
}

impl Renderable for DrawRoad {
    fn get_id(&self) -> ID {
        ID::Road(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        g.redraw(&self.draw_center_line);
        if opts.label_roads {
            g.draw_text_at_mapspace(&self.label, self.label_pos);
        }
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        let (pl, width) = map.get_r(self.id).get_thick_polyline().unwrap();
        pl.to_thick_boundary(width, OUTLINE_THICKNESS)
            .unwrap_or_else(|| map.get_r(self.id).get_thick_polygon().unwrap())
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_r(self.id)
            .get_thick_polygon()
            .unwrap()
            .contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
