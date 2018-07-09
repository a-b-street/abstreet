// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use CarID;
use ezgui::GfxCtx;
use geom::{Angle, Pt2D};
use graphics;
use graphics::math::Vec2d;
use map_model::{geometry, Map};
use straw_model::{Car, On};

const CAR_WIDTH: f64 = 2.0;
const CAR_LENGTH: f64 = 4.5;

// TODO should this live in editor/render?
pub struct DrawCar {
    pub id: CarID,
    polygons: Vec<Vec<Vec2d>>,
    // TODO ideally, draw the turn icon inside the car quad. how can we do that easily?
    turn_arrow: Option<[f64; 4]>,
}

impl DrawCar {
    pub(crate) fn new(car: &Car, map: &Map, front: Pt2D, angle: Angle) -> DrawCar {
        let turn_arrow = if let Some(On::Turn(on)) = car.waiting_for {
            let angle = map.get_t(on).line.angle();
            let arrow_pt = front.project_away(CAR_LENGTH / 2.0, angle.opposite());
            Some([arrow_pt.x(), arrow_pt.y(), front.x(), front.y()])
        } else {
            None
        };

        DrawCar {
            id: car.id,
            turn_arrow,
            // TODO the rounded corners from graphics::Line::new_round look kind of cool though
            polygons: geometry::thick_line_from_angle(
                CAR_WIDTH,
                CAR_LENGTH,
                front,
                // find the back of the car relative to the front
                angle.opposite(),
            ),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: graphics::types::Color) {
        for p in &self.polygons {
            g.draw_polygon(color, p);
        }
        // TODO tune color, sizes
        if let Some(a) = self.turn_arrow {
            g.draw_arrow(
                &graphics::Line::new_round([0.0, 1.0, 1.0, 1.0], 0.25),
                a,
                1.0,
            );
        }
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        for p in &self.polygons {
            if geometry::point_in_polygon(x, y, p) {
                return true;
            }
        }
        false
    }
}
