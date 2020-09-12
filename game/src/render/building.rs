use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use geom::{Distance, Polygon, Pt2D};
use map_model::{Building, BuildingID, Map, OffstreetParking, NORMAL_LANE_THICKNESS};
use std::cell::RefCell;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Text};

pub struct DrawBuilding {
    pub id: BuildingID,
    label: RefCell<Option<Drawable>>,
}

impl DrawBuilding {
    pub fn new(
        ctx: &EventCtx,
        bldg: &Building,
        map: &Map,
        cs: &ColorScheme,
        bldg_batch: &mut GeomBatch,
        paths_batch: &mut GeomBatch,
        outlines_batch: &mut GeomBatch,
    ) -> DrawBuilding {
        // Trim the driveway away from the sidewalk's center line, so that it doesn't overlap. For
        // now, this cleanup is visual; it doesn't belong in the map_model layer.
        let orig_pl = &bldg.driveway_geom;
        let driveway = orig_pl
            .slice(
                Distance::ZERO,
                orig_pl.length() - map.get_l(bldg.sidewalk()).width / 2.0,
            )
            .map(|(pl, _)| pl)
            .unwrap_or_else(|_| orig_pl.clone());

        if bldg.amenities.is_empty() {
            bldg_batch.push(cs.residential_building, bldg.polygon.clone());
        } else {
            bldg_batch.push(cs.commerical_building, bldg.polygon.clone());
        }
        paths_batch.push(cs.sidewalk, driveway.make_polygons(NORMAL_LANE_THICKNESS));
        if let Ok(p) = bldg.polygon.to_outline(Distance::meters(0.1)) {
            outlines_batch.push(cs.building_outline, p);
        }

        let parking_icon = match bldg.parking {
            OffstreetParking::PublicGarage(_, _) => true,
            OffstreetParking::Private(_, garage) => garage,
        };
        if parking_icon {
            // Might need to scale down more for some buildings, but so far, this works everywhere.
            bldg_batch.append(
                GeomBatch::load_svg(ctx.prerender, "system/assets/map/parking.svg")
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
                let mut dy = 0.0;
                for (names, amenity_type) in &b.amenities {
                    if dy == 0.0 && false {
                        let txt = Text::from(
                            Line(names.get(app.opts.language.as_ref())).fg(Color::BLACK),
                        );
                        batch.append(
                            txt.render_to_batch(g.prerender)
                                .scale(0.1)
                                .centered_on(b.label_center),
                        );
                    }

                    if !abstutil::file_exists(abstutil::path(format!(
                        "system/assets/osm_amenities/{}.svg",
                        amenity_type
                    ))) {
                        println!("missing {}", amenity_type);
                        continue;
                    }
                    batch.append(
                        GeomBatch::load_svg(
                            g.prerender,
                            format!("system/assets/osm_amenities/{}.svg", amenity_type),
                        )
                        .scale(0.3)
                        .centered_on(b.label_center.offset(0.0, dy)),
                    );
                    // Original files are 14x14
                    dy += 16.0 * 0.3;
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
