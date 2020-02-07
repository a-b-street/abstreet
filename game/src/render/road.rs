use crate::helpers::{ColorScheme, ID};
use crate::render::{dashed_lines, DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Angle, Distance, Polygon, Pt2D};
use map_model::{LaneType, Map, Road, RoadID};

pub struct DrawRoad {
    pub id: RoadID,
    zorder: isize,

    draw_center_line: Drawable,
    label: Drawable,
}

impl DrawRoad {
    pub fn new(r: &Road, map: &Map, cs: &ColorScheme, prerender: &Prerender) -> DrawRoad {
        let mut draw = GeomBatch::new();
        let center = r.get_current_center(map);
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

        let mut txt = Text::new().with_bg();
        txt.add(Line(r.get_name()));
        let mut lbl = GeomBatch::new();
        // TODO Disabled because it's slow up-front cost
        if false {
            lbl.add_transformed(
                txt.render_to_batch(prerender),
                r.center_pts.middle(),
                0.1,
                Angle::ZERO,
            );
        }

        DrawRoad {
            id: r.id,
            zorder: r.get_zorder(),
            draw_center_line: prerender.upload(draw),
            label: prerender.upload(lbl),
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
            g.redraw(&self.label);
        }
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        let (pl, width) = map.get_r(self.id).get_thick_polyline(map).unwrap();
        pl.to_thick_boundary(width, OUTLINE_THICKNESS)
            .unwrap_or_else(|| map.get_r(self.id).get_thick_polygon(map).unwrap())
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_r(self.id)
            .get_thick_polygon(map)
            .unwrap()
            .contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
