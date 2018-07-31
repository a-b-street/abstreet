// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use ezgui::GfxCtx;
use geom::{Angle, Pt2D};
use graphics;
use graphics::math::Vec2d;
use map_model::{geometry, Map, TurnID};
use std;
use CarID;

const CAR_WIDTH: f64 = 2.0;

pub const CAR_LENGTH: si::Meter<f64> = si::Meter {
    value_unsafe: 4.5,
    _marker: std::marker::PhantomData,
};

// TODO should this live in editor/render?
pub struct DrawCar {
    pub id: CarID,
    polygons: Vec<Vec<Vec2d>>,
    // TODO ideally, draw the turn icon inside the car quad. how can we do that easily?
    turn_arrow: Option<[f64; 4]>,
    front_pt: Pt2D,
}

impl DrawCar {
    pub fn new(
        id: CarID,
        waiting_for_turn: Option<TurnID>,
        map: &Map,
        front: Pt2D,
        angle: Angle,
    ) -> DrawCar {
        let turn_arrow = if let Some(t) = waiting_for_turn {
            let angle = map.get_t(t).line.angle();
            let arrow_pt = front.project_away(CAR_LENGTH.value_unsafe / 2.0, angle.opposite());
            Some([arrow_pt.x(), arrow_pt.y(), front.x(), front.y()])
        } else {
            None
        };

        DrawCar {
            id: id,
            turn_arrow,
            // TODO the rounded corners from graphics::Line::new_round look kind of cool though
            polygons: geometry::thick_line_from_angle(
                CAR_WIDTH,
                CAR_LENGTH.value_unsafe,
                front,
                // find the back of the car relative to the front
                angle.opposite(),
            ),
            front_pt: front,
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

    pub fn focus_pt(&self) -> Pt2D {
        self.front_pt
    }
}
