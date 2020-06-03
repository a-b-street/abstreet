use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Drawable, GeomBatch, GfxCtx, Prerender, RewriteColor};
use geom::{Angle, Distance, Line, PolyLine, Polygon, Pt2D};
use map_model::{
    Map, ParkingLot, ParkingLotID, NORMAL_LANE_THICKNESS, PARKING_SPOT_LENGTH, SIDEWALK_THICKNESS,
};

pub struct DrawParkingLot {
    pub id: ParkingLotID,
    draw: Drawable,
}

impl DrawParkingLot {
    pub fn new(
        lot: &ParkingLot,
        cs: &ColorScheme,
        unzoomed_batch: &mut GeomBatch,
        prerender: &Prerender,
    ) -> DrawParkingLot {
        unzoomed_batch.push(cs.parking_lot, lot.polygon.clone());
        for aisle in &lot.aisles {
            let aisle_thickness = NORMAL_LANE_THICKNESS / 2.0;
            unzoomed_batch.push(
                cs.unzoomed_residential,
                PolyLine::unchecked_new(aisle.clone()).make_polygons(aisle_thickness),
            );
        }
        unzoomed_batch.add_svg(
            prerender,
            "../data/system/assets/map/parking.svg",
            lot.polygon.polylabel(),
            0.05,
            Angle::ZERO,
            RewriteColor::NoOp,
            true,
        );

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

        let mut batch = GeomBatch::new();
        // TODO This isn't getting clipped to the parking lot boundary properly, so just stick this
        // on the lowest order for now.
        batch.push(
            cs.sidewalk,
            front_path_line.make_polygons(NORMAL_LANE_THICKNESS),
        );
        batch.push(cs.parking_lot, lot.polygon.clone());
        for aisle in &lot.aisles {
            let aisle_thickness = NORMAL_LANE_THICKNESS / 2.0;
            batch.push(
                cs.driving_lane,
                PolyLine::unchecked_new(aisle.clone()).make_polygons(aisle_thickness),
            );
        }
        let width = NORMAL_LANE_THICKNESS;
        let height = 0.8 * PARKING_SPOT_LENGTH;
        for (pt, angle) in &lot.spots {
            let left = pt.project_away(width / 2.0, angle.rotate_degs(90.0));
            let right = pt.project_away(width / 2.0, angle.rotate_degs(-90.0));

            batch.push(
                cs.general_road_marking,
                PolyLine::new(vec![
                    left.project_away(height, *angle),
                    left,
                    right,
                    right.project_away(height, *angle),
                ])
                .make_polygons(Distance::meters(0.25)),
            );
        }

        DrawParkingLot {
            id: lot.id,
            draw: prerender.upload(batch),
        }
    }
}

impl Renderable for DrawParkingLot {
    fn get_id(&self) -> ID {
        ID::ParkingLot(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App, _: &DrawOptions) {
        g.redraw(&self.draw);
    }

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
