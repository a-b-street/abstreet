use crate::helpers::{ColorScheme, ID};
use crate::render::{DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, GeomBatch, GfxCtx, Line, Text};
use geom::{Circle, Distance, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, Map, LANE_THICKNESS};

pub struct DrawBuilding {
    pub id: BuildingID,
    label: Option<Text>,
    label_pos: Pt2D,
}

impl DrawBuilding {
    pub fn new(bldg: &Building, cs: &ColorScheme, batch: &mut GeomBatch) -> DrawBuilding {
        // Trim the front path line away from the sidewalk's center line, so that it doesn't
        // overlap. For now, this cleanup is visual; it doesn't belong in the map_model layer.
        let mut front_path_line = bldg.front_path.line.clone();
        let len = front_path_line.length();
        let trim_back = LANE_THICKNESS / 2.0;
        if len > trim_back && len - trim_back > geom::EPSILON_DIST {
            front_path_line = Line::new(
                front_path_line.pt1(),
                front_path_line.dist_along(len - trim_back),
            );
        }
        let front_path = front_path_line.make_polygons(Distance::meters(1.0));

        batch.push(
            cs.get_def("building", Color::rgb(196, 193, 188)),
            bldg.polygon.clone(),
        );
        batch.push(cs.get_def("building path", Color::grey(0.6)), front_path);

        // TODO Do similar trim_back for driveway
        if let Some(ref p) = bldg.parking {
            batch.push(
                cs.get_def("driveway", Color::grey(0.1)),
                p.driveway_line.make_polygons(LANE_THICKNESS),
            );
        }

        let label = bldg
            .osm_tags
            .get("addr:housenumber")
            .map(|num| Text::from(Line(num.to_string()).fg(Color::BLACK).size(50)));

        if bldg.parking.is_some() {
            let center = bldg.label_center;
            batch.push(
                cs.get_def("parking icon background", Color::BLACK),
                Circle::new(center, Distance::meters(5.0)).to_polygon(),
            );
            // Draw a 'P'
            // TODO The result here looks pretty bad and is quite tedious to define. Figure out a
            // reasonable way to import SVG sprites. Still need to programatically fill up the
            // circle with color, though.
            batch.push(
                cs.get_def("parking icon foreground", Color::WHITE),
                Polygon::rectangle(
                    center.offset(Distance::meters(-1.0), Distance::ZERO),
                    Distance::meters(1.5),
                    Distance::meters(4.5),
                ),
            );
            batch.push(
                cs.get("parking icon foreground"),
                Circle::new(
                    center.offset(Distance::meters(0.5), Distance::meters(-0.5)),
                    Distance::meters(1.5),
                )
                .to_polygon(),
            );
            batch.push(
                cs.get("parking icon background"),
                Circle::new(
                    center.offset(Distance::meters(0.5), Distance::meters(-0.5)),
                    Distance::meters(0.5),
                )
                .to_polygon(),
            );
        }

        // TODO Slow and looks silly, but it's a nice experiment.
        /*for poly in bldg.polygon.shrink(-3.0) {
            batch.push(cs.get_def("building roof", Color::rgb(150, 75, 0)), poly);
        }*/

        DrawBuilding {
            id: bldg.id,
            label,
            label_pos: bldg.label_center,
        }
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, ctx: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_polygon(color, &ctx.map.get_b(self.id).polygon);
        }
        if opts.label_buildings {
            if let Some(ref txt) = self.label {
                g.draw_text_at_mapspace(txt, self.label_pos);
            }
        }
    }

    // Some buildings cover up tunnels
    fn get_zorder(&self) -> isize {
        0
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        map.get_b(self.id).polygon.to_outline(OUTLINE_THICKNESS)
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_b(self.id).polygon.contains_pt(pt)
    }
}
