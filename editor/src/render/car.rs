use aabb_quadtree::geom::Rect;
use colors::Colors;
use dimensioned::si;
use ezgui::{shift_color, GfxCtx};
use geom::{Polygon, Pt2D};
use graphics;
use map_model::{geometry, Map};
use objects::{Ctx, ID};
use render::{get_bbox, RenderOptions, Renderable};
use sim::{CarID, CarState, DrawCarInput};

const CAR_WIDTH: f64 = 2.0;

pub struct DrawCar {
    pub id: CarID,
    body_polygon: Polygon,
    window_polygons: Vec<Polygon>,
    // TODO ideally, draw the turn icon inside the car quad. how can we do that easily?
    turn_arrow: Option<[f64; 4]>,
    // TODO maybe also draw lookahead buffer to know what the car is considering
    // TODO it would be really neat to project the stopping buffer onto the actual route that'll be
    // taken
    stopping_buffer_arrow: Option<[f64; 4]>,
}

impl DrawCar {
    pub fn new(input: DrawCarInput, map: &Map) -> DrawCar {
        let turn_arrow = if let Some(t) = input.waiting_for_turn {
            let angle = map.get_t(t).line.angle();
            let arrow_pt = input
                .front
                .project_away(input.vehicle_length.value_unsafe / 2.0, angle.opposite());
            Some([arrow_pt.x(), arrow_pt.y(), input.front.x(), input.front.y()])
        } else {
            None
        };

        let stopping_buffer_arrow = if input.stopping_dist == 0.0 * si::M {
            None
        } else {
            let arrow_pt = input
                .front
                .project_away(input.stopping_dist.value_unsafe, input.angle);
            Some([input.front.x(), input.front.y(), arrow_pt.x(), arrow_pt.y()])
        };

        let front_window_length_gap = 0.2;
        let front_window_thickness = 0.3;

        DrawCar {
            id: input.id,
            turn_arrow,
            // TODO the rounded corners from graphics::Line::new_round look kind of cool though
            body_polygon: geometry::thick_line_from_angle(
                CAR_WIDTH,
                input.vehicle_length.value_unsafe,
                input.front,
                // find the back of the car relative to the front
                input.angle.opposite(),
            ),
            // TODO it's way too hard to understand and tune this. just wait and stick in sprites
            // or something.
            window_polygons: vec![
                geometry::thick_line_from_angle(
                    front_window_thickness,
                    CAR_WIDTH - 2.0 * front_window_length_gap,
                    input
                        .front
                        .project_away(1.0, input.angle.opposite())
                        .project_away(
                            CAR_WIDTH / 2.0 - front_window_length_gap,
                            input.angle.rotate_degs(-90.0),
                        ),
                    input.angle.rotate_degs(90.0),
                ),
                geometry::thick_line_from_angle(
                    front_window_thickness * 0.8,
                    CAR_WIDTH - 2.0 * front_window_length_gap,
                    input
                        .front
                        .project_away(
                            input.vehicle_length.value_unsafe - 1.0,
                            input.angle.opposite(),
                        )
                        .project_away(
                            CAR_WIDTH / 2.0 - front_window_length_gap,
                            input.angle.rotate_degs(-90.0),
                        ),
                    input.angle.rotate_degs(90.0),
                ),
            ],
            stopping_buffer_arrow,
        }
    }
}

impl Renderable for DrawCar {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        let color = opts.color.unwrap_or_else(|| {
            // TODO if it's a bus, color it differently -- but how? :\
            match ctx.sim.get_car_state(self.id) {
                CarState::Debug => shift_color(ctx.cs.get(Colors::DebugCar), self.id.0),
                CarState::Moving => shift_color(ctx.cs.get(Colors::MovingCar), self.id.0),
                CarState::Stuck => shift_color(ctx.cs.get(Colors::StuckCar), self.id.0),
                CarState::Parked => shift_color(ctx.cs.get(Colors::ParkedCar), self.id.0),
            }
        });
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

    fn get_bbox(&self) -> Rect {
        get_bbox(&self.body_polygon.get_bounds())
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.body_polygon.contains_pt(pt)
    }

    fn tooltip_lines(&self, _map: &Map) -> Vec<String> {
        vec![self.id.to_string()]
    }
}
