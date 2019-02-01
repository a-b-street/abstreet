use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use crate::render::DrawTurn;
use ezgui::{Canvas, GfxCtx, ScreenPt, Text, UserInput};
use geom::{PolyLine, Pt2D};
use map_model::{IntersectionID, LaneID, Turn, TurnID, TurnType};

pub struct Legend {
    top_left: ScreenPt,
}

impl Legend {
    pub fn new(ctx: &mut PluginCtx) -> Option<Legend> {
        if ctx.input.action_chosen("show legend") {
            return Some(Legend::start(ctx.input, ctx.canvas));
        }
        None
    }

    pub fn start(input: &mut UserInput, canvas: &Canvas) -> Legend {
        Legend {
            // Size needed for the legend was manually tuned. :\
            top_left: input.set_mode_with_extra(
                "Legend",
                "Legend".to_string(),
                canvas,
                220.0,
                300.0,
            ),
        }
    }
}

impl Plugin for Legend {
    fn nonblocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        *self = Legend::start(ctx.input, ctx.canvas);

        if ctx.input.modal_action("quit") {
            return false;
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        let zoom = 10.0;
        g.fork(Pt2D::new(0.0, 0.0), self.top_left, zoom);

        // Create a fake turn.
        let mut turn = Turn {
            id: TurnID {
                parent: IntersectionID(0),
                src: LaneID(0),
                dst: LaneID(0),
            },
            turn_type: TurnType::Straight,
            lookup_idx: 0,
            // TODO Do we need to zoom here at all? For the arrows, sadly. Annoying to express the
            // fake geometry in terms of zoom, but oh well.
            geom: PolyLine::new(vec![
                Pt2D::new(10.0 / zoom, 10.0 / zoom),
                Pt2D::new(10.0 / zoom, 100.0 / zoom),
            ]),
        };

        DrawTurn::draw_full(
            &turn,
            g,
            ctx.cs.get("turns protected by traffic signal right now"),
        );
        g.draw_text_at_screenspace_topleft(
            Text::from_line("Protected turn".to_string()),
            ScreenPt::new(self.top_left.x + 20.0, self.top_left.y + 10.0),
        );

        turn.geom = PolyLine::new(vec![
            Pt2D::new(10.0 / zoom, 110.0 / zoom),
            Pt2D::new(10.0 / zoom, 200.0 / zoom),
        ]);
        DrawTurn::draw_dashed(
            &turn,
            g,
            ctx.cs
                .get("turns allowed with yielding by traffic signal right now"),
        );
        g.draw_text_at_screenspace_topleft(
            Text::from_line("Yield turn".to_string()),
            ScreenPt::new(self.top_left.x + 20.0, self.top_left.y + 110.0),
        );

        g.unfork();
    }
}
