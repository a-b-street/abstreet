use crate::helpers::{ColorScheme, ID};
use crate::render::{AgentColorScheme, DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Circle, Distance, PolyLine, Polygon};
use map_model::{Map, LANE_THICKNESS};
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
        acs: AgentColorScheme,
    ) -> DrawPedestrian {
        // TODO Slight issues with rendering small pedestrians:
        // - route visualization is thick
        // - there are little skips when making turns
        // - front paths are too skinny
        let radius = LANE_THICKNESS / 4.0; // TODO make const after const fn is better
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
        if input.waiting_for_turn.is_some() {
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
        draw_default.push(acs.zoomed_color_ped(&input, cs), body_circle.to_polygon());
        draw_default.push(
            cs.get_def("pedestrian head", Color::rgb(139, 69, 19)),
            head_circle.to_polygon(),
        );

        if let Some(t) = input.waiting_for_turn {
            // A silly idea for peds... use hands to point at their turn?
            let angle = map.get_t(t).angle();
            draw_default.push(
                cs.get("blinker on"),
                PolyLine::new(vec![
                    input.pos.project_away(radius / 2.0, angle.opposite()),
                    input.pos.project_away(radius / 2.0, angle),
                ])
                .make_arrow(Distance::meters(0.25))
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

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_circle(color, &self.body_circle);
        } else {
            g.redraw(&self.draw_default);
        }
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
    label: Text,
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
                    pl_slice.shift_left(LANE_THICKNESS / 4.0).unwrap()
                } else {
                    pl_slice.shift_right(LANE_THICKNESS / 4.0).unwrap()
                }
            }
            PedCrowdLocation::FrontPath(b) => map
                .get_b(b)
                .front_path
                .line
                .to_polyline()
                .exact_slice(input.low, input.high),
        };
        let blob = pl_shifted.make_polygons(LANE_THICKNESS / 2.0);
        let draw_default = prerender.upload_borrowed(vec![(cs.get("pedestrian"), &blob)]);

        // Ideally "pedestrian head" color, but it looks really faded...
        let label = Text::from(
            Line(format!("{}", input.members.len()))
                .fg(Color::BLACK)
                .size(15),
        );

        DrawPedCrowd {
            members: input.members,
            blob_pl: pl_shifted,
            blob,
            zorder: match input.location {
                PedCrowdLocation::Sidewalk(on, _) => on.get_zorder(map),
                PedCrowdLocation::FrontPath(_) => 0,
            },
            draw_default,
            label,
        }
    }
}

impl Renderable for DrawPedCrowd {
    fn get_id(&self) -> ID {
        // Expensive! :(
        ID::PedCrowd(self.members.clone())
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_polygon(color, &self.blob);
        } else {
            g.redraw(&self.draw_default);
        }
        g.draw_text_at_mapspace(&self.label, self.blob.center());
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        self.blob_pl
            .to_thick_boundary(LANE_THICKNESS / 2.0, OUTLINE_THICKNESS)
            .unwrap_or_else(|| self.blob.clone())
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
