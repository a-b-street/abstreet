use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{GeomBatch, GfxCtx, Prerender, RewriteColor};
use geom::{Angle, Line, Polygon, Pt2D};
use map_model::{Map, ParkingLot, ParkingLotID, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS};

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
