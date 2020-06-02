use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{GeomBatch, GfxCtx, Prerender, RewriteColor};
use geom::{Angle, Distance, Line, PolyLine, Polygon, Pt2D};
use map_model::{
    Map, ParkingLot, ParkingLotID, NORMAL_LANE_THICKNESS, PARKING_SPOT_LENGTH, SIDEWALK_THICKNESS,
};

pub struct DrawParkingLot {
    pub id: ParkingLotID,
}

impl DrawParkingLot {
    pub fn new(
        lot: &ParkingLot,
        cs: &ColorScheme,
        all_lots: &mut GeomBatch,
        paths_batch: &mut GeomBatch,
        prerender: &Prerender,
    ) -> DrawParkingLot {
        // Trim the front path line away from the sidewalk's center line, so that it doesn't
        // overlap. For now, this cleanup is visual; it doesn't belong in the map_model layer.
        let mut front_path_line = lot.sidewalk_line.clone();
        let len = front_path_line.length();
        let trim_back = SIDEWALK_THICKNESS / 2.0;
        if len > trim_back && len - trim_back > geom::EPSILON_DIST {
            front_path_line = Line::new(
                front_path_line.pt1(),
                front_path_line.dist_along(len - trim_back),
            );
        }

        all_lots.push(cs.parking_lot, lot.polygon.clone());
        all_lots.add_svg(
            prerender,
            "../data/system/assets/map/parking.svg",
            lot.polygon.polylabel(),
            0.05,
            Angle::ZERO,
            RewriteColor::NoOp,
            true,
        );

        let mut lines = Vec::new();
        for aisle in &lot.aisles {
            let aisle_thickness = NORMAL_LANE_THICKNESS / 2.0;
            let pl = PolyLine::unchecked_new(aisle.clone());
            all_lots.push(cs.parking_lane, pl.make_polygons(aisle_thickness));

            let mut start = Distance::ZERO;
            while start < pl.length() {
                let (pt, angle) = pl.dist_along(start);
                for rotate in vec![90.0, -90.0] {
                    let theta = angle.rotate_degs(rotate);
                    let line = Line::new(
                        pt.project_away(aisle_thickness / 2.0, theta),
                        // The full PARKING_SPOT_LENGTH used for on-street is looking too
                        // conservative for some manually audited cases in Seattle
                        pt.project_away(aisle_thickness / 2.0 + 0.8 * PARKING_SPOT_LENGTH, theta),
                    );

                    // Don't leak out of the parking lot
                    // TODO Entire line
                    if !lot.polygon.contains_pt(line.pt1()) || !lot.polygon.contains_pt(line.pt2())
                    {
                        continue;
                    }

                    // Don't let this line hit another line
                    if lines.iter().any(|other| line.intersection(other).is_some()) {
                        continue;
                    }

                    // Don't hit an aisle
                    if lot.aisles.iter().any(|pts| {
                        PolyLine::unchecked_new(pts.clone())
                            .intersection(&line.to_polyline())
                            .is_some()
                    }) {
                        continue;
                    }

                    all_lots.push(
                        cs.general_road_marking,
                        line.make_polygons(Distance::meters(0.25)),
                    );
                    lines.push(line);
                }
                start += NORMAL_LANE_THICKNESS;
            }
        }

        paths_batch.push(
            cs.sidewalk,
            front_path_line.make_polygons(NORMAL_LANE_THICKNESS),
        );

        DrawParkingLot { id: lot.id }
    }
}

impl Renderable for DrawParkingLot {
    fn get_id(&self) -> ID {
        ID::ParkingLot(self.id)
    }

    fn draw(&self, _: &mut GfxCtx, _: &App, _: &DrawOptions) {}

    fn get_zorder(&self) -> isize {
        0
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        let pl = map.get_pl(self.id);
        if let Some(p) = pl.polygon.maybe_to_outline(OUTLINE_THICKNESS) {
            p
        } else {
            pl.polygon.clone()
        }
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_pl(self.id).polygon.contains_pt(pt)
    }
}
