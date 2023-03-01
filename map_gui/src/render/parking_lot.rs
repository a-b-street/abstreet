use std::cell::RefCell;

use geom::{Bounds, Distance, PolyLine, Pt2D, Tessellation};
use map_model::{
    osm, LaneType, Map, ParkingLot, ParkingLotID, NORMAL_LANE_THICKNESS, PARKING_LOT_SPOT_LENGTH,
};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, RewriteColor};

use crate::colors::ColorScheme;
use crate::options::Options;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use crate::{AppLike, ID};

pub struct DrawParkingLot {
    pub id: ParkingLotID,
    draw: RefCell<Option<Drawable>>,
}

impl DrawParkingLot {
    pub fn new(
        ctx: &EventCtx,
        lot: &ParkingLot,
        cs: &ColorScheme,
        opts: &Options,
        unzoomed_batch: &mut GeomBatch,
    ) -> DrawParkingLot {
        unzoomed_batch.push(cs.parking_lot, lot.polygon.clone());
        for aisle in &lot.aisles {
            let aisle_thickness = NORMAL_LANE_THICKNESS / 2.0;
            unzoomed_batch.push(
                cs.unzoomed_road_surface(osm::RoadRank::Local),
                PolyLine::unchecked_new(aisle.clone()).thicken_tessellation(aisle_thickness),
            );
        }
        unzoomed_batch.append(
            GeomBatch::load_svg(ctx, "system/assets/map/parking.svg")
                .scale(0.05)
                .centered_on(lot.polygon.polylabel())
                .color(if opts.simplify_basemap {
                    RewriteColor::Change(Color::hex("#204A87"), cs.parking_lot)
                } else {
                    RewriteColor::NoOp
                }),
        );

        DrawParkingLot {
            id: lot.id,
            draw: RefCell::new(None),
        }
    }

    pub fn render(&self, app: &dyn AppLike) -> GeomBatch {
        let lot = app.map().get_pl(self.id);

        // Trim the front path line away from the sidewalk's center line, so that it doesn't
        // overlap. For now, this cleanup is visual; it doesn't belong in the map_model layer.
        let orig_line = &lot.sidewalk_line;
        let front_path_line = orig_line
            .slice(
                Distance::ZERO,
                orig_line.length() - app.map().get_l(lot.sidewalk_pos.lane()).width / 2.0,
            )
            .unwrap_or_else(|_| orig_line.clone());

        let mut batch = GeomBatch::new();
        // TODO This isn't getting clipped to the parking lot boundary properly, so just stick
        // this on the lowest order for now.
        let rank = app.map().get_parent(lot.sidewalk_pos.lane()).get_rank();
        batch.push(
            app.cs().zoomed_road_surface(LaneType::Sidewalk, rank),
            front_path_line.make_polygons(NORMAL_LANE_THICKNESS),
        );
        batch.push(app.cs().parking_lot, lot.polygon.clone());
        for aisle in &lot.aisles {
            let aisle_thickness = NORMAL_LANE_THICKNESS / 2.0;
            batch.push(
                app.cs()
                    .zoomed_road_surface(LaneType::Driving, osm::RoadRank::Local),
                PolyLine::unchecked_new(aisle.clone()).thicken_tessellation(aisle_thickness),
            );
        }
        let width = NORMAL_LANE_THICKNESS;
        let height = PARKING_LOT_SPOT_LENGTH;
        for (pt, angle) in &lot.spots {
            let left = pt.project_away(width / 2.0, angle.rotate_degs(90.0));
            let right = pt.project_away(width / 2.0, angle.rotate_degs(-90.0));

            batch.push(
                app.cs().general_road_marking,
                PolyLine::must_new(vec![
                    left.project_away(height, *angle),
                    left,
                    right,
                    right.project_away(height, *angle),
                ])
                .make_polygons(Distance::meters(0.25)),
            );
        }

        batch
    }

    pub fn clear_rendering(&self) {
        *self.draw.borrow_mut() = None;
    }
}

impl Renderable for DrawParkingLot {
    fn get_id(&self) -> ID {
        ID::ParkingLot(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, _: &DrawOptions) {
        let mut draw = self.draw.borrow_mut();
        if draw.is_none() {
            *draw = Some(g.upload(self.render(app)));
        }
        g.redraw(draw.as_ref().unwrap());
    }

    fn get_zorder(&self) -> isize {
        0
    }

    fn get_outline(&self, map: &Map) -> Tessellation {
        map.get_pl(self.id).polygon.to_outline(OUTLINE_THICKNESS)
    }

    fn get_bounds(&self, map: &Map) -> Bounds {
        map.get_pl(self.id).polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_pl(self.id).polygon.contains_pt(pt)
    }
}
