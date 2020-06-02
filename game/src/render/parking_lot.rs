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
    pub inferred_spots: usize,
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
        let inferred_spots = infer_spots(cs, lot, all_lots);

        paths_batch.push(
            cs.sidewalk,
            front_path_line.make_polygons(NORMAL_LANE_THICKNESS),
        );

        DrawParkingLot {
            id: lot.id,
            inferred_spots,
        }
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

fn line_valid(lot: &ParkingLot, line: &Line, finalized_lines: &Vec<Line>) -> bool {
    // Don't leak out of the parking lot
    // TODO Entire line
    if !lot.polygon.contains_pt(line.pt1()) || !lot.polygon.contains_pt(line.pt2()) {
        return false;
    }

    // Don't let this line hit another line
    if finalized_lines.iter().any(|other| line.crosses(other)) {
        return false;
    }

    // Don't hit an aisle
    if lot.aisles.iter().any(|pts| {
        PolyLine::unchecked_new(pts.clone())
            .intersection(&line.to_polyline())
            .is_some()
    }) {
        return false;
    }

    true
}

// Returns the number of spots
fn infer_spots(cs: &ColorScheme, lot: &ParkingLot, batch: &mut GeomBatch) -> usize {
    let mut total_spots = 0;
    let mut finalized_lines = Vec::new();

    for aisle in &lot.aisles {
        let aisle_thickness = NORMAL_LANE_THICKNESS / 2.0;
        let pl = PolyLine::unchecked_new(aisle.clone());
        batch.push(cs.parking_lane, pl.make_polygons(aisle_thickness));

        for rotate in vec![90.0, -90.0] {
            // Blindly generate all of the lines
            let lines = {
                let mut lines = Vec::new();
                let mut start = Distance::ZERO;
                while start + NORMAL_LANE_THICKNESS < pl.length() {
                    let (pt, angle) = pl.dist_along(start);
                    start += NORMAL_LANE_THICKNESS;
                    let theta = angle.rotate_degs(rotate);
                    lines.push(Line::new(
                        pt.project_away(aisle_thickness / 2.0, theta),
                        // The full PARKING_SPOT_LENGTH used for on-street is looking too
                        // conservative for some manually audited cases in Seattle
                        pt.project_away(aisle_thickness / 2.0 + 0.8 * PARKING_SPOT_LENGTH, theta),
                    ));
                }
                lines
            };

            for pair in lines.windows(2) {
                let l1 = &pair[0];
                let l2 = &pair[1];
                let back = Line::new(l1.pt2(), l2.pt2());
                if l1.intersection(&l2).is_none()
                    && l1.angle().approx_eq(l2.angle(), 5.0)
                    && line_valid(lot, l1, &finalized_lines)
                    && line_valid(lot, l2, &finalized_lines)
                    && line_valid(lot, &back, &finalized_lines)
                {
                    total_spots += 1;
                    batch.push(
                        cs.general_road_marking,
                        l1.make_polygons(Distance::meters(0.25)),
                    );
                    batch.push(
                        cs.general_road_marking,
                        l2.make_polygons(Distance::meters(0.25)),
                    );
                    batch.push(
                        ezgui::Color::RED,
                        back.make_polygons(Distance::meters(0.25)),
                    );
                    finalized_lines.push(l1.clone());
                    finalized_lines.push(l2.clone());
                    finalized_lines.push(back);
                }
            }
        }
    }
    total_spots
}
