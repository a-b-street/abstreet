use std::cell::RefCell;

use geom::{Angle, Bounds, Distance, Line, Polygon, Pt2D, Ring, Tessellation};
use map_model::{Building, BuildingID, Map, OffstreetParking};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Text};

use crate::colors::ColorScheme;
use crate::options::{CameraAngle, Options};
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use crate::{AppLike, ID};

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
        outlines_batch: &mut GeomBatch,
    ) -> DrawBuilding {
        let bldg_color = if bldg.amenities.is_empty() {
            cs.residential_building
        } else {
            cs.commercial_building
        };

        match &opts.camera_angle {
            CameraAngle::TopDown => {
                bldg_batch.push(bldg_color, bldg.polygon.clone());
                if opts.show_building_outlines {
                    outlines_batch.push(
                        cs.building_outline,
                        bldg.polygon.to_outline(Distance::meters(0.1)),
                    );
                }

                let parking_icon = match bldg.parking {
                    OffstreetParking::PublicGarage(_, _) => true,
                    OffstreetParking::Private(_, garage) => garage,
                };
                if parking_icon {
                    // Might need to scale down more for some buildings, but so far, this works
                    // everywhere.
                    bldg_batch.append(
                        GeomBatch::load_svg(ctx, "system/assets/map/parking.svg")
                            .scale(0.1)
                            .centered_on(bldg.label_center),
                    );
                }
            }
            CameraAngle::Abstract => {
                // TODO The hitbox needs to change too
                bldg_batch.push(
                    bldg_color,
                    Polygon::rectangle_centered(
                        bldg.polygon.center(),
                        Distance::meters(5.0),
                        Distance::meters(5.0),
                    ),
                );
            }
            x => {
                let angle = match x {
                    CameraAngle::IsometricNE => Angle::degrees(-45.0),
                    CameraAngle::IsometricNW => Angle::degrees(-135.0),
                    CameraAngle::IsometricSE => Angle::degrees(45.0),
                    CameraAngle::IsometricSW => Angle::degrees(135.0),
                    CameraAngle::TopDown | CameraAngle::Abstract => unreachable!(),
                };

                let bldg_height_per_level = 3.5;
                // In downtown areas, really tall buildings look kind of ridculous next to
                // everything else. So we artificially compress the number of levels a bit.
                let bldg_rendered_meters = bldg_height_per_level * bldg.levels.powf(0.8);
                let height = Distance::meters(bldg_rendered_meters);

                let map_bounds = map.get_gps_bounds().to_bounds();
                let (map_width, map_height) = (map_bounds.width(), map_bounds.height());
                let map_length = map_width.hypot(map_height);

                let distance = |pt: &Pt2D| {
                    // some normalization so we can compute the distance to the corner of the
                    // screen from which the orthographic projection is based.
                    let projection_origin = match x {
                        CameraAngle::IsometricNE => Pt2D::new(0.0, map_height),
                        CameraAngle::IsometricNW => Pt2D::new(map_width, map_height),
                        CameraAngle::IsometricSE => Pt2D::new(0.0, 0.0),
                        CameraAngle::IsometricSW => Pt2D::new(map_width, 0.0),
                        CameraAngle::TopDown | CameraAngle::Abstract => unreachable!(),
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

                // Things closer to the isometric axis should appear in front of things farther
                // away, so we give them a higher z-index.
                //
                // Naively, we compute the entire building's distance as the distance from its
                // closest point. This is simple and usually works, but will likely fail on more
                // complex building arrangements, e.g. if a building were tightly encircled by a
                // large building.
                let closest_pt = bldg
                    .polygon
                    .get_outer_ring()
                    .points()
                    .iter()
                    .min_by(|a, b| distance(a).cmp(&distance(b)))
                    .cloned();

                let distance_from_projection_axis = closest_pt
                    .map(|pt| distance(&pt).inner_meters())
                    .unwrap_or(0.0);

                // smaller z renders above larger
                let scale_factor = map_length;
                let groundfloor_z = distance_from_projection_axis / scale_factor - 1.0;
                let roof_z = groundfloor_z - height.inner_meters() / scale_factor;

                // TODO Some buildings have holes in them
                if let Ok(roof) = Ring::new(
                    bldg.polygon
                        .get_outer_ring()
                        .points()
                        .iter()
                        .map(|pt| pt.project_away(height, angle))
                        .collect(),
                ) {
                    bldg_batch.push(Color::BLACK, bldg.polygon.to_outline(Distance::meters(0.3)));

                    // In actuality, the z of the walls should start at groundfloor_z and end at
                    // roof_z, but since we aren't dealing with actual 3d geometries, we have to
                    // pick one value. Anecdotally, picking a value between the two seems to
                    // usually looks right, but probably breaks down in certain overlap scenarios.
                    let wall_z = (groundfloor_z + roof_z) / 2.0;

                    let mut wall_beams = Vec::new();
                    for (low, high) in bldg
                        .polygon
                        .get_outer_ring()
                        .points()
                        .iter()
                        .zip(roof.points().iter())
                    {
                        // Sometimes building height is 0!
                        // https://www.openstreetmap.org/way/390547658
                        if let Ok(l) = Line::new(*low, *high) {
                            wall_beams.push(l);
                        }
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
                            .into_polygon(),
                            wall_z,
                        );
                    }
                    for wall in wall_beams {
                        bldg_batch.push_with_z(
                            Color::BLACK,
                            wall.make_polygons(Distance::meters(0.1)),
                            wall_z,
                        );
                    }

                    bldg_batch.push_with_z(bldg_color, roof.clone().into_polygon(), roof_z);
                    bldg_batch.push_with_z(
                        Color::BLACK,
                        roof.to_outline(Distance::meters(0.3)),
                        roof_z,
                    );
                } else {
                    bldg_batch.push(bldg_color, bldg.polygon.clone());
                    outlines_batch.push(
                        cs.building_outline,
                        bldg.polygon.to_outline(Distance::meters(0.1)),
                    );
                }
            }
        }

        DrawBuilding {
            id: bldg.id,
            label: RefCell::new(None),
        }
    }

    pub fn clear_rendering(&mut self) {
        *self.label.borrow_mut() = None;
    }
}

impl Renderable for DrawBuilding {
    fn get_id(&self) -> ID {
        ID::Building(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, opts: &DrawOptions) {
        if opts.label_buildings {
            // Labels are expensive to compute up-front, so do it lazily, since we don't really
            // zoom in on all buildings in a single session anyway
            let mut label = self.label.borrow_mut();
            if label.is_none() {
                let mut batch = GeomBatch::new();
                let b = app.map().get_b(self.id);
                if let Some(a) = b.amenities.get(0) {
                    let mut txt = Text::from(
                        Line(a.names.get(app.opts().language.as_ref())).fg(Color::BLACK),
                    );
                    if b.amenities.len() > 1 {
                        txt.append(Line(format!(" (+{})", b.amenities.len() - 1)).fg(Color::BLACK));
                    }
                    batch.append(
                        txt.render_autocropped(g)
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

    fn get_outline(&self, map: &Map) -> Tessellation {
        map.get_b(self.id).polygon.to_outline(OUTLINE_THICKNESS)
    }

    fn get_bounds(&self, map: &Map) -> Bounds {
        map.get_b(self.id).polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_b(self.id).polygon.contains_pt(pt)
    }
}
