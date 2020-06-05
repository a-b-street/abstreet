use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, RewriteColor, Text};
use geom::{Angle, Distance, Line, Polygon, Pt2D};
use map_model::{Building, BuildingID, Map, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS};
use std::cell::RefCell;

pub struct DrawBuilding {
    pub id: BuildingID,
    label: RefCell<Option<Drawable>>,
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

        DrawBuilding {
            id: bldg.id,
            label: RefCell::new(None),
        }
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &App, opts: &DrawOptions) {
        if opts.label_buildings {
            // Labels are expensive to compute up-front, so do it lazily, since we don't really
            // zoom in on all buildings in a single session anyway
            let mut label = self.label.borrow_mut();
            if label.is_none() {
                let mut batch = GeomBatch::new();
                let b = app.primary.map.get_b(self.id);
                if let Some((name, _)) = b.amenities.iter().next() {
                    let mut txt = Text::from(Line(name).fg(Color::BLACK));
                    if b.amenities.len() > 1 {
                        txt.append(Line(format!(" (+{})", b.amenities.len() - 1)).fg(Color::BLACK));
                    }
                    batch.append(
                        txt.render_to_batch(g.prerender)
                            .scale(0.1)
                            .centered_on(b.label_center),
                    );
                }
                *label = Some(g.prerender.upload(batch));
            }
            g.redraw(label.as_ref().unwrap());
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
