// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use ezgui::GfxCtx;
use geom::{Angle, Polygon, Pt2D};
use graphics;
use kinematics::Vehicle;
use map_model::{geometry, Map, TurnID};
use {CarID, Distance};

const CAR_WIDTH: f64 = 2.0;

// TODO should this live in editor/render?
pub struct DrawCar {
    pub id: CarID,
    body_polygon: Polygon,
    window_polygons: Vec<Polygon>,
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
        vehicle: &Vehicle,
        waiting_for_turn: Option<TurnID>,
        map: &Map,
        front: Pt2D,
        angle: Angle,
        stopping_dist: Distance,
    ) -> DrawCar {
        let turn_arrow = if let Some(t) = waiting_for_turn {
            let angle = map.get_t(t).line.angle();
            let arrow_pt = front.project_away(vehicle.length.value_unsafe / 2.0, angle.opposite());
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

        let front_window_length_gap = 0.2;
        let front_window_thickness = 0.3;

        DrawCar {
            id: id,
            turn_arrow,
            // TODO the rounded corners from graphics::Line::new_round look kind of cool though
            body_polygon: geometry::thick_line_from_angle(
                CAR_WIDTH,
                vehicle.length.value_unsafe,
                front,
                // find the back of the car relative to the front
                angle.opposite(),
            ),
            // TODO it's way too hard to understand and tune this. just wait and stick in sprites
            // or something.
            window_polygons: vec![
                geometry::thick_line_from_angle(
                    front_window_thickness,
                    CAR_WIDTH - 2.0 * front_window_length_gap,
                    front.project_away(1.0, angle.opposite()).project_away(
                        CAR_WIDTH / 2.0 - front_window_length_gap,
                        angle.rotate_degs(-90.0),
                    ),
                    angle.rotate_degs(90.0),
                ),
                geometry::thick_line_from_angle(
                    front_window_thickness * 0.8,
                    CAR_WIDTH - 2.0 * front_window_length_gap,
                    front
                        .project_away(vehicle.length.value_unsafe - 1.0, angle.opposite())
                        .project_away(
                            CAR_WIDTH / 2.0 - front_window_length_gap,
                            angle.rotate_degs(-90.0),
                        ),
                    angle.rotate_degs(90.0),
                ),
            ],
            front_pt: front,
            stopping_buffer_arrow,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: graphics::types::Color) {
        g.draw_polygon(color, &self.body_polygon);
        for p in &self.window_polygons {
            g.draw_polygon([0.0, 0.0, 0.0, 1.0], p);
        }

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
        self.body_polygon.contains_pt(pt)
    }

    pub fn focus_pt(&self) -> Pt2D {
        self.front_pt
    }
}
