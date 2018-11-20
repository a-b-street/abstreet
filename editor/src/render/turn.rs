// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Circle, Line, Pt2D};
use map_model::{Map, Turn, TurnID};
use objects::{Ctx, ID};
use render::{
    RenderOptions, Renderable, BIG_ARROW_THICKNESS, BIG_ARROW_TIP_LENGTH, TURN_ICON_ARROW_LENGTH,
    TURN_ICON_ARROW_THICKNESS, TURN_ICON_ARROW_TIP_LENGTH,
};
use std::f64;

#[derive(Debug)]
pub struct DrawTurn {
    pub id: TurnID,
    line: Line,
    icon_circle: Circle,
    icon_arrow: Line,
}

impl DrawTurn {
    pub fn new(map: &Map, turn: &Turn, offset_along_lane: usize) -> DrawTurn {
        let offset_along_lane = offset_along_lane as f64;
        let angle = turn.line.angle();

        let end_line = map.get_l(turn.id.src).end_line(turn.id.parent);
        // Start the distance from the intersection
        let icon_center = end_line
            .reverse()
            .unbounded_dist_along((offset_along_lane + 0.5) * TURN_ICON_ARROW_LENGTH * si::M);

        let icon_circle = Circle::new(icon_center, TURN_ICON_ARROW_LENGTH / 2.0);

        let icon_src = icon_center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle.opposite());
        let icon_dst = icon_center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle);
        let icon_arrow = Line::new(icon_src, icon_dst);

        DrawTurn {
            id: turn.id,
            line: turn.line.clone(),
            icon_circle,
            icon_arrow,
        }
    }

    pub fn draw_full(&self, g: &mut GfxCtx, color: Color) {
        g.draw_rounded_arrow(color, BIG_ARROW_THICKNESS, BIG_ARROW_TIP_LENGTH, &self.line);
    }
}

// Little weird, but this is focused on the turn icon, not the full visualization
impl Renderable for DrawTurn {
    fn get_id(&self) -> ID {
        ID::Turn(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        g.draw_circle(
            ctx.cs.get("turn icon circle", Color::grey(0.3)),
            &self.icon_circle,
        );

        g.draw_arrow(
            opts.color
                .unwrap_or_else(|| ctx.cs.get("inactive turn icon", Color::grey(0.7))),
            TURN_ICON_ARROW_THICKNESS,
            TURN_ICON_ARROW_TIP_LENGTH,
            &self.icon_arrow,
        );
    }

    fn get_bounds(&self) -> Bounds {
        self.icon_circle.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.icon_circle.contains_pt(pt)
    }
}
