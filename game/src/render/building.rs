use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Distance, Polygon, Pt2D};
use map_model::{Building, BuildingID, Map, NORMAL_LANE_THICKNESS};
use std::cell::RefCell;

pub struct DrawBuilding {
    pub id: BuildingID,
    label: RefCell<Option<Drawable>>,
}

impl DrawBuilding {
    pub fn new(
        bldg: &Building,
        map: &Map,
        cs: &ColorScheme,
        bldg_batch: &mut GeomBatch,
        paths_batch: &mut GeomBatch,
        outlines_batch: &mut GeomBatch,
        prerender: &Prerender,
    ) -> DrawBuilding {
        // Trim the front path line away from the sidewalk's center line, so that it doesn't
        // overlap. For now, this cleanup is visual; it doesn't belong in the map_model layer.
        let orig_line = &bldg.front_path.line;
        let front_path_line = orig_line
            .slice(
                Distance::ZERO,
                orig_line.length() - map.get_l(bldg.sidewalk()).width / 2.0,
            )
            .unwrap_or_else(|| orig_line.clone());

        if bldg.amenities.is_empty() {
            bldg_batch.push(cs.residential_building, bldg.polygon.clone());
        } else {
            bldg_batch.push(cs.commerical_building, bldg.polygon.clone());
        }
        paths_batch.push(
            cs.sidewalk,
            front_path_line.make_polygons(NORMAL_LANE_THICKNESS),
        );
        if let Ok(p) = bldg.polygon.to_outline(Distance::meters(0.1)) {
            outlines_batch.push(cs.building_outline, p);
        }

        if bldg
            .parking
            .as_ref()
            .map(|p| p.public_garage_name.is_some())
            .unwrap_or(false)
        {
            // Might need to scale down more for some buildings, but so far, this works everywhere.
            bldg_batch.append(
                GeomBatch::mapspace_svg(prerender, "system/assets/map/parking.svg")
                    .scale(0.1)
                    .centered_on(bldg.label_center),
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
        if let Ok(p) = b.polygon.to_outline(OUTLINE_THICKNESS) {
            p
        } else {
            b.polygon.clone()
        }
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_b(self.id).polygon.contains_pt(pt)
    }
}
