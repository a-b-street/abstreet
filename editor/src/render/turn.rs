// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::Colors;
use dimensioned::si;
use ezgui::GfxCtx;
use geom::Pt2D;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::{geometry, Map, Turn, TurnID};
use objects::{Ctx, ID};
use render::{
    RenderOptions, Renderable, BIG_ARROW_TIP_LENGTH, TURN_ICON_ARROW_LENGTH,
    TURN_ICON_ARROW_THICKNESS, TURN_ICON_ARROW_TIP_LENGTH,
};
use std::f64;

#[derive(Debug)]
pub struct DrawTurn {
    pub id: TurnID,
    src_pt: Vec2d,
    pub dst_pt: Vec2d,
    icon_circle: [f64; 4],
    icon_arrow: [f64; 4],
}

impl DrawTurn {
    pub fn new(map: &Map, turn: &Turn, offset_along_lane: usize) -> DrawTurn {
        let offset_along_lane = offset_along_lane as f64;
        let src_pt = turn.line.pt1();
        let dst_pt = turn.line.pt2();
        let angle = turn.line.angle();

        let end_line = map.get_l(turn.src).end_line(turn.parent);
        // Start the distance from the intersection
        let icon_center = end_line
            .reverse()
            .unbounded_dist_along((offset_along_lane + 0.5) * TURN_ICON_ARROW_LENGTH * si::M);

        let icon_src = icon_center
            .project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle.opposite())
            .to_vec();
        let icon_dst = icon_center
            .project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle)
            .to_vec();

        let icon_circle = geometry::make_circle(icon_center, TURN_ICON_ARROW_LENGTH / 2.0);

        let icon_arrow = [icon_src[0], icon_src[1], icon_dst[0], icon_dst[1]];

        DrawTurn {
            id: turn.id,
            src_pt: src_pt.to_vec(),
            dst_pt: dst_pt.to_vec(),
            icon_circle,
            icon_arrow,
        }
    }

    pub fn draw_full(&self, g: &mut GfxCtx, color: Color) {
        g.draw_arrow(
            &graphics::Line::new_round(color, geometry::BIG_ARROW_THICKNESS),
            [
                self.src_pt[0],
                self.src_pt[1],
                self.dst_pt[0],
                self.dst_pt[1],
            ],
            BIG_ARROW_TIP_LENGTH,
        );
    }
}

// Little weird, but this is focused on the turn icon, not the full visualization
impl Renderable for DrawTurn {
    fn get_id(&self) -> ID {
        ID::Turn(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        g.draw_ellipse(ctx.cs.get(Colors::TurnIconCircle), self.icon_circle);

        g.draw_arrow(
            &graphics::Line::new_round(
                opts.color.unwrap_or(ctx.cs.get(Colors::TurnIconInactive)),
                TURN_ICON_ARROW_THICKNESS,
            ),
            self.icon_arrow,
            TURN_ICON_ARROW_TIP_LENGTH,
        );
    }

    fn get_bbox(&self) -> Rect {
        geometry::circle_to_bbox(&self.icon_circle)
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        geometry::point_in_circle(&self.icon_circle, pt)
    }

    fn tooltip_lines(&self, map: &Map) -> Vec<String> {
        vec![
            format!("{}", self.id),
            format!("Angle {}", map.get_t(self.id).turn_angle(map)),
        ]
    }
}
