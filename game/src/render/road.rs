use crate::helpers::{ColorScheme, ID};
use crate::render::{dashed_lines, DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
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
        let width = Distance::meters(0.25);
        // If the road is a one-way (only parking and sidewalk on the off-side), draw a solid line
        // No center line at all if there's a shared left turn lane
        if r.children_backwards
            .iter()
            .all(|(_, lt)| *lt == LaneType::Parking || *lt == LaneType::Sidewalk)
        {
            draw.push(cs.get("road center line"), center.make_polygons(width));
        } else if r.children_forwards.is_empty()
            || r.children_forwards[0].1 != LaneType::SharedLeftTurn
        {
            draw.extend(
                cs.get_def("road center line", Color::YELLOW),
                dashed_lines(&center, width, Distance::meters(2.0), Distance::meters(1.0)),
            );
        }

        let mut label = Text::new().with_bg();
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
