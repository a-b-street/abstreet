use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::options::{CameraAngle, Options};
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use geom::{Angle, Distance, Line, Polygon, Pt2D, Ring};
use map_model::{Building, BuildingID, Map, OffstreetParking, NORMAL_LANE_THICKNESS};
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
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
        opts: &Options,
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

        let bldg_color = if bldg.amenities.is_empty() {
            cs.residential_building
        } else {
            cs.commerical_building
        };

        match &opts.camera_angle {
            CameraAngle::TopDown => {
                bldg_batch.push(bldg_color, bldg.polygon.clone());
                if let Ok(p) = bldg.polygon.to_outline(Distance::meters(0.1)) {
                    outlines_batch.push(cs.building_outline, p);
                }

                let parking_icon = match bldg.parking {
                    OffstreetParking::PublicGarage(_, _) => true,
                    OffstreetParking::Private(_, garage) => garage,
                };
                if parking_icon {
                    // Might need to scale down more for some buildings, but so far, this works
                    // everywhere.
                    bldg_batch.append(
                        GeomBatch::load_svg(ctx.prerender, "system/assets/map/parking.svg")
                            .scale(0.1)
                            .centered_on(bldg.label_center),
                    );
                }
            }
            x => {
                let angle = match x {
                    CameraAngle::IsometricNE => Angle::new_degs(-45.0),
                    CameraAngle::IsometricNW => Angle::new_degs(-135.0),
                    CameraAngle::IsometricSE => Angle::new_degs(45.0),
                    CameraAngle::IsometricSW => Angle::new_degs(135.0),
                    _ => unreachable!(),
                };

                // TODO For now, blindly guess the building height
                let max_height = 15.0;
                let mut rng = XorShiftRng::seed_from_u64(bldg.id.0 as u64);
                let height = Distance::meters(rng.gen_range(1.0, max_height));

                let map_bounds = map.get_gps_bounds().to_bounds();
                let (map_width, map_height) = (map_bounds.width(), map_bounds.height());
                let map_max = Pt2D::new(map_width, map_height);
                let map_length = Pt2D::new(0.0, 0.0).dist_to(map_max).inner_meters();

                let distance = |pt: &Pt2D| {
                    let projection_origin = match x {
                        CameraAngle::IsometricNE => Pt2D::new(0.0, map_height),
                        CameraAngle::IsometricNW => Pt2D::new(map_width, map_height),
                        CameraAngle::IsometricSE => Pt2D::new(0.0, 0.0),
                        CameraAngle::IsometricSW => Pt2D::new(map_width, 0.0),
                        CameraAngle::TopDown => unreachable!(),
                    };

                    let abs_pt = Pt2D::new(
                        (pt.x() - projection_origin.x()).abs(),
                        (pt.y() - projection_origin.y()).abs(),
                    );

                    let a = f64::hypot(abs_pt.x(), abs_pt.y());
                    let theta = f64::atan(abs_pt.y() / abs_pt.x());
                    let distance = a * f64::sin(theta + std::f64::consts::PI / 4.0);
                    Distance::meters(distance)
                };

                // Things closer to the isometric projection origin should appear in front of
                // things farther away, so we give them a higher z-index.
                //
                // Naively, we compute the entire building's distance as the distance from it's
                // closest point. This is simple and usually works, but will likely fail on more
                // complex building arrangements, e.g. if a building were tightly encircled by a
                // large building.
                let closest_pt = bldg
                    .polygon
                    .points()
                    .into_iter()
                    .min_by(|a, b| distance(a).cmp(&distance(b)));

                let distance_from_projection_origin = closest_pt
                    .map(|pt| distance(pt).inner_meters())
                    .unwrap_or(0.0);

                // smaller z renders above larger
                let scale_factor = map_length + max_height;
                let z = (distance_from_projection_origin) / scale_factor - 1.0;

                println!(
                    "closest_pt: {:?}, map_size: {}, map_length: {}, distance: {}, z_offset: {}",
                    closest_pt, map_max, map_length, distance_from_projection_origin, z
                );

                // TODO Some buildings have holes in them
                if let Ok(roof) = Ring::new(
                    bldg.polygon
                        .points()
                        .iter()
                        .map(|pt| pt.project_away(height, angle))
                        .collect(),
                ) {
                    if let Ok(p) = bldg.polygon.to_outline(Distance::meters(0.3)) {
                        bldg_batch.push(Color::BLACK, p);
                    }

                    let mut wall_beams = Vec::new();
                    for (low, high) in bldg.polygon.points().iter().zip(roof.points().iter()) {
                        wall_beams.push(Line::must_new(*low, *high));
                    }
                    let wall_color = Color::hex("#BBBEC3");
                    for (wall1, wall2) in wall_beams.iter().zip(wall_beams.iter().skip(1)) {
                        bldg_batch.push_with_z(
                            wall_color,
                            Ring::must_new(vec![
                                wall1.pt1(),
                                wall1.pt2(),
                                wall2.pt2(),
                                wall2.pt1(),
                                wall1.pt1(),
                            ])
                            .to_polygon(),
                            z,
                        );
                    }
                    for wall in wall_beams {
                        bldg_batch.push_with_z(
                            Color::BLACK,
                            wall.make_polygons(Distance::meters(0.1)),
                            z,
                        );
                    }

                    let roof_z = z - height.inner_meters() / scale_factor;
                    bldg_batch.push_with_z(bldg_color, roof.clone().to_polygon(), roof_z);
                    bldg_batch.push_with_z(
                        Color::BLACK,
                        roof.to_outline(Distance::meters(0.3)),
                        roof_z,
                    );
                } else {
                    bldg_batch.push(bldg_color, bldg.polygon.clone());
                    if let Ok(p) = bldg.polygon.to_outline(Distance::meters(0.1)) {
                        outlines_batch.push(cs.building_outline, p);
                    }
                }
            }
        }
        paths_batch.push(cs.sidewalk, driveway.make_polygons(NORMAL_LANE_THICKNESS));

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
                if let Some((names, _)) = b.amenities.iter().next() {
                    let mut txt =
                        Text::from(Line(names.get(app.opts.language.as_ref())).fg(Color::BLACK));
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
