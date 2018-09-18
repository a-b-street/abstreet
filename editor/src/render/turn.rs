// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::Colors;
use dimensioned::si;
use ezgui::GfxCtx;
use geom::{Line, Pt2D};
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
    src_pt: Pt2D,
    dst_pt: Pt2D,
    icon_circle: [f64; 4],
    icon_arrow: Line,
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

        let icon_circle = geometry::make_circle(icon_center, TURN_ICON_ARROW_LENGTH / 2.0);

        let icon_src = icon_center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle.opposite());
        let icon_dst = icon_center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle);
        let icon_arrow = Line::new(icon_src, icon_dst);

        DrawTurn {
            id: turn.id,
            src_pt,
            dst_pt,
            icon_circle,
            icon_arrow,
        }
    }

    pub fn draw_full(&self, g: &mut GfxCtx, color: Color) {
        g.draw_rounded_arrow(
            color,
            geometry::BIG_ARROW_THICKNESS,
            BIG_ARROW_TIP_LENGTH,
            &Line::new(self.src_pt, self.dst_pt),
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
            opts.color.unwrap_or(ctx.cs.get(Colors::TurnIconInactive)),
            TURN_ICON_ARROW_THICKNESS,
            TURN_ICON_ARROW_TIP_LENGTH,
            &self.icon_arrow,
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
