use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, RewriteColor, Text};
use geom::{Angle, Distance, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, Map, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS};

pub struct DrawBuilding {
    pub id: BuildingID,
    label: Option<Drawable>,
}

impl DrawBuilding {
    pub fn new(
        bldg: &Building,
        cs: &ColorScheme,
        bldg_batch: &mut GeomBatch,
        paths_batch: &mut GeomBatch,
        outlines_batch: &mut GeomBatch,
        prerender: &Prerender,
    ) -> DrawBuilding {
        // Trim the front path line away from the sidewalk's center line, so that it doesn't
        // overlap. For now, this cleanup is visual; it doesn't belong in the map_model layer.
        let mut front_path_line = bldg.front_path.line.clone();
        let len = front_path_line.length();
        let trim_back = SIDEWALK_THICKNESS / 2.0;
        if len > trim_back && len - trim_back > geom::EPSILON_DIST {
            front_path_line = Line::new(
                front_path_line.pt1(),
                front_path_line.dist_along(len - trim_back),
            );
        }

        bldg_batch.push(cs.building, bldg.polygon.clone());
        paths_batch.push(
            cs.sidewalk,
            front_path_line.make_polygons(NORMAL_LANE_THICKNESS),
        );
        if let Some(p) = bldg.polygon.maybe_to_outline(Distance::meters(0.1)) {
            outlines_batch.push(cs.building_outline, p);
        }

        if bldg
            .parking
            .as_ref()
            .map(|p| p.public_garage_name.is_some())
            .unwrap_or(false)
        {
            // Might need to scale down more for some buildings, but so far, this works everywhere.
            bldg_batch.add_svg(
                prerender,
                "../data/system/assets/map/parking.svg",
                bldg.label_center,
                0.1,
                Angle::ZERO,
                RewriteColor::NoOp,
                true,
            );
        }

        // TODO Uh oh, this is lots of work upfront. Disable by default. :(
        let label = if false {
            bldg.house_number().map(|num| {
                let mut lbl = GeomBatch::new();
                lbl.add_transformed(
                    Text::from(Line(num.to_string()).fg(Color::BLACK)).render_to_batch(prerender),
                    bldg.label_center,
                    0.1,
                    Angle::ZERO,
                    RewriteColor::NoOp,
                );
                prerender.upload(lbl)
            })
        } else {
            None
        };

        // TODO Slow and looks silly, but it's a nice experiment.
        /*for poly in bldg.polygon.shrink(-3.0) {
            bldg_batch.push(color, poly);
        }*/

        DrawBuilding { id: bldg.id, label }
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App, opts: &DrawOptions) {
        if opts.label_buildings {
            if let Some(ref lbl) = self.label {
                g.redraw(lbl);
            }
        }
    }

    // Some buildings cover up tunnels
    fn get_zorder(&self) -> isize {
        0
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        let b = map.get_b(self.id);
        if let Some(p) = b.polygon.maybe_to_outline(OUTLINE_THICKNESS) {
            p
        } else {
            b.polygon.clone()
        }
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_b(self.id).polygon.contains_pt(pt)
    }
}
