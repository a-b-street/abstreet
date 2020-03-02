use crate::app::App;
use crate::helpers::{rotating_color_agents, ColorScheme, ID};
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Angle, Circle, Distance, PolyLine, Polygon};
use map_model::{Map, SIDEWALK_THICKNESS};
use sim::{DrawPedCrowdInput, DrawPedestrianInput, PedCrowdLocation, PedestrianID};

pub struct DrawPedestrian {
    pub id: PedestrianID,
    body_circle: Circle,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawPedestrian {
    pub fn new(
        input: DrawPedestrianInput,
        step_count: usize,
        map: &Map,
        prerender: &Prerender,
        cs: &ColorScheme,
    ) -> DrawPedestrian {
        // TODO Slight issues with rendering small pedestrians:
        // - route visualization is thick
        // - there are little skips when making turns
        // - front paths are too skinny
        let radius = SIDEWALK_THICKNESS / 4.0; // TODO make const after const fn is better
        let body_circle = Circle::new(input.pos, radius);

        let mut draw_default = GeomBatch::new();

        let foot_radius = 0.2 * radius;
        let hand_radius = 0.2 * radius;
        let left_foot_angle = 30.0;
        let right_foot_angle = -30.0;
        let left_hand_angle = 70.0;
        let right_hand_angle = -70.0;

        let left_foot = Circle::new(
            input
                .pos
                .project_away(radius, input.facing.rotate_degs(left_foot_angle)),
            foot_radius,
        );
        let right_foot = Circle::new(
            input
                .pos
                .project_away(radius, input.facing.rotate_degs(right_foot_angle)),
            foot_radius,
        );
        let left_hand = Circle::new(
            input
                .pos
                .project_away(radius, input.facing.rotate_degs(left_hand_angle)),
            hand_radius,
        );
        let right_hand = Circle::new(
            input
                .pos
                .project_away(radius, input.facing.rotate_degs(right_hand_angle)),
            hand_radius,
        );
        let foot_color = cs.get_def("pedestrian foot", Color::BLACK);
        let hand_color = cs.get("pedestrian head");
        // Jitter based on ID so we don't all walk synchronized.
        let jitter = input.id.0 % 2 == 0;
        let remainder = step_count % 6;
        if input.waiting_for_turn.is_some() || input.waiting_for_bus {
            draw_default.push(foot_color, left_foot.to_polygon());
            draw_default.push(foot_color, right_foot.to_polygon());
            draw_default.push(hand_color, left_hand.to_polygon());
            draw_default.push(hand_color, right_hand.to_polygon());
        } else if jitter == (remainder < 3) {
            draw_default.push(foot_color, left_foot.to_polygon());
            draw_default.push(
                foot_color,
                Circle::new(
                    input
                        .pos
                        .project_away(0.9 * radius, input.facing.rotate_degs(right_foot_angle)),
                    foot_radius,
                )
                .to_polygon(),
            );

            draw_default.push(hand_color, right_hand.to_polygon());
            draw_default.push(
                hand_color,
                Circle::new(
                    input
                        .pos
                        .project_away(0.9 * radius, input.facing.rotate_degs(left_hand_angle)),
                    hand_radius,
                )
                .to_polygon(),
            );
        } else {
            draw_default.push(foot_color, right_foot.to_polygon());
            draw_default.push(
                foot_color,
                Circle::new(
                    input
                        .pos
                        .project_away(0.9 * radius, input.facing.rotate_degs(left_foot_angle)),
                    foot_radius,
                )
                .to_polygon(),
            );

            draw_default.push(hand_color, left_hand.to_polygon());
            draw_default.push(
                hand_color,
                Circle::new(
                    input
                        .pos
                        .project_away(0.9 * radius, input.facing.rotate_degs(right_hand_angle)),
                    hand_radius,
                )
                .to_polygon(),
            );
        };

        let head_circle = Circle::new(input.pos, 0.5 * radius);
        draw_default.push(zoomed_color_ped(&input, cs), body_circle.to_polygon());
        draw_default.push(
            cs.get_def("pedestrian head", Color::rgb(139, 69, 19)),
            head_circle.to_polygon(),
        );

        if let Some(t) = input.waiting_for_turn {
            // A silly idea for peds... use hands to point at their turn?
            let angle = map.get_t(t).angle();
            draw_default.push(
                cs.get("turn arrow"),
                PolyLine::new(vec![
                    input.pos.project_away(radius / 2.0, angle.opposite()),
                    input.pos.project_away(radius / 2.0, angle),
                ])
                .make_arrow(Distance::meters(0.15))
                .unwrap(),
            );
        }

        DrawPedestrian {
            id: input.id,
            body_circle,
            zorder: input.on.get_zorder(map),
            draw_default: prerender.upload(draw_default),
        }
    }
}

impl Renderable for DrawPedestrian {
    fn get_id(&self) -> ID {
        ID::Pedestrian(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App, _: &DrawOptions) {
        g.redraw(&self.draw_default);
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        // TODO thin ring
        self.body_circle.to_polygon()
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

pub struct DrawPedCrowd {
    members: Vec<PedestrianID>,
    blob: Polygon,
    blob_pl: PolyLine,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawPedCrowd {
    pub fn new(
        input: DrawPedCrowdInput,
        map: &Map,
        prerender: &Prerender,
        cs: &ColorScheme,
    ) -> DrawPedCrowd {
        let pl_shifted = match input.location {
            PedCrowdLocation::Sidewalk(on, contraflow) => {
                let pl_slice = on.exact_slice(input.low, input.high, map);
                if contraflow {
                    pl_slice.shift_left(SIDEWALK_THICKNESS / 4.0).unwrap()
                } else {
                    pl_slice.shift_right(SIDEWALK_THICKNESS / 4.0).unwrap()
                }
            }
            PedCrowdLocation::FrontPath(b) => map
                .get_b(b)
                .front_path
                .line
                .to_polyline()
                .exact_slice(input.low, input.high),
        };
        let blob = pl_shifted.make_polygons(SIDEWALK_THICKNESS / 2.0);
        let mut batch = GeomBatch::new();
        batch.push(
            cs.get_def("pedestrian crowd", Color::rgb_f(0.2, 0.7, 0.7)),
            blob.clone(),
        );
        batch.add_transformed(
            Text::from(Line(format!("{}", input.members.len())).fg(Color::BLACK))
                .render_to_batch(prerender),
            blob.center(),
            0.02,
            Angle::ZERO,
        );

        DrawPedCrowd {
            members: input.members,
            blob_pl: pl_shifted,
            blob,
            zorder: match input.location {
                PedCrowdLocation::Sidewalk(on, _) => on.get_zorder(map),
                PedCrowdLocation::FrontPath(_) => 0,
            },
            draw_default: prerender.upload(batch),
        }
    }
}

impl Renderable for DrawPedCrowd {
    fn get_id(&self) -> ID {
        // Expensive! :(
        ID::PedCrowd(self.members.clone())
    }

    fn draw(&self, g: &mut GfxCtx, _: &App, _: &DrawOptions) {
        g.redraw(&self.draw_default);
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        self.blob_pl
            .to_thick_boundary(SIDEWALK_THICKNESS / 2.0, OUTLINE_THICKNESS)
            .unwrap_or_else(|| self.blob.clone())
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
fn zoomed_color_ped(input: &DrawPedestrianInput, cs: &ColorScheme) -> Color {
    if input.preparing_bike {
        cs.get_def("pedestrian preparing bike", Color::rgb(255, 0, 144))
    } else {
        rotating_color_agents(input.id.0)
    }
}
