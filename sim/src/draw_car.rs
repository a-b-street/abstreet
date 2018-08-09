// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use ezgui::GfxCtx;
use geom::{Angle, Polygon, Pt2D};
use graphics;
use map_model::{geometry, Map, TurnID};
use std;
use {CarID, Distance};

const CAR_WIDTH: f64 = 2.0;

pub const CAR_LENGTH: Distance = si::Meter {
    value_unsafe: 4.5,
    _marker: std::marker::PhantomData,
};

// TODO should this live in editor/render?
pub struct DrawCar {
    pub id: CarID,
    polygon: Polygon,
    // TODO ideally, draw the turn icon inside the car quad. how can we do that easily?
    turn_arrow: Option<[f64; 4]>,
    front_pt: Pt2D,
    // TODO maybe also draw lookahead buffer to know what the car is considering
    // TODO it would be really neat to project the stopping buffer onto the actual route that'll be
    // taken
    stopping_buffer_arrow: Option<[f64; 4]>,
}

impl DrawCar {
    pub fn new(
        id: CarID,
        waiting_for_turn: Option<TurnID>,
        map: &Map,
        front: Pt2D,
        angle: Angle,
        stopping_dist: Distance,
    ) -> DrawCar {
        let turn_arrow = if let Some(t) = waiting_for_turn {
            let angle = map.get_t(t).line.angle();
            let arrow_pt = front.project_away(CAR_LENGTH.value_unsafe / 2.0, angle.opposite());
            Some([arrow_pt.x(), arrow_pt.y(), front.x(), front.y()])
        } else {
            None
        };

        let stopping_buffer_arrow = if stopping_dist == 0.0 * si::M {
            None
        } else {
            let arrow_pt = front.project_away(stopping_dist.value_unsafe, angle);
            Some([front.x(), front.y(), arrow_pt.x(), arrow_pt.y()])
        };

        DrawCar {
            id: id,
            turn_arrow,
            // TODO the rounded corners from graphics::Line::new_round look kind of cool though
            polygon: geometry::thick_line_from_angle(
                CAR_WIDTH,
                CAR_LENGTH.value_unsafe,
                front,
                // find the back of the car relative to the front
                angle.opposite(),
            ),
            front_pt: front,
            stopping_buffer_arrow,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: graphics::types::Color) {
        g.draw_polygon(color, &self.polygon);
        // TODO tune color, sizes
        if let Some(a) = self.turn_arrow {
            g.draw_arrow(
                &graphics::Line::new_round([0.0, 1.0, 1.0, 1.0], 0.25),
                a,
                1.0,
            );
        }

        if let Some(a) = self.stopping_buffer_arrow {
            g.draw_arrow(
                &graphics::Line::new_round([1.0, 0.0, 0.0, 0.7], 0.25),
                a,
                1.0,
            );
        }
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }

    pub fn focus_pt(&self) -> Pt2D {
        self.front_pt
    }
}
