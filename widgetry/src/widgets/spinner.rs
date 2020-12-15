use geom::{Polygon, Pt2D};

use crate::{
    text, Btn, Button, EventCtx, GeomBatch, GfxCtx, Line, Outcome, ScreenDims, ScreenPt,
    ScreenRectangle, Text, Widget, WidgetImpl, WidgetOutput,
};

// TODO MAX_CHAR_WIDTH is a hardcoded nonsense value
const TEXT_WIDTH: f64 = 2.0 * text::MAX_CHAR_WIDTH;

// TODO Allow text entry
// TODO Allow click and hold
// TODO Grey out the buttons when we're maxed out
pub struct Spinner {
    low: isize,
    high: isize,
    pub current: isize,

    up: Button,
    down: Button,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Spinner {
    pub fn new(ctx: &EventCtx, (low, high): (isize, isize), mut current: isize) -> Widget {
        let up = Btn::text_fg("↑")
            .build(ctx, "increase value", None)
            .take_btn();
        let down = Btn::text_fg("↓")
            .build(ctx, "decrease value", None)
            .take_btn();

        let dims = ScreenDims::new(
            TEXT_WIDTH + up.get_dims().width,
            up.get_dims().height + down.get_dims().height,
        );
        if current < low {
            current = low;
            warn!("Spinner current value is out of bounds!");
        } else if high < current {
            current = high;
            warn!("Spinner current value is out of bounds!");
        }
        Widget::new(Box::new(Spinner {
            low,
            high,
            current,

            up,
            down,

            top_left: ScreenPt::new(0.0, 0.0),
            dims,
        }))
    }

    pub fn modify(&mut self, delta: isize) {
        self.current += delta;
        self.current = self.current.min(self.high);
        self.current = self.current.max(self.low);
    }
}

impl WidgetImpl for Spinner {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        // TODO This works, but it'd be kind of cool if we could construct a tiny little Panel
        // here and use that. Wait, why can't we? ...
        self.top_left = top_left;
        self.up
            .set_pos(ScreenPt::new(top_left.x + TEXT_WIDTH, top_left.y));
        self.down.set_pos(ScreenPt::new(
            top_left.x + TEXT_WIDTH,
            top_left.y + self.up.get_dims().height,
        ));
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        self.up.event(ctx, output);
        if let Outcome::Clicked(_) = output.outcome {
            output.outcome = Outcome::Changed;
            self.current = (self.current + 1).min(self.high);
            ctx.no_op_event(true, |ctx| self.up.event(ctx, output));
            return;
        }

        self.down.event(ctx, output);
        if let Outcome::Clicked(_) = output.outcome {
            output.outcome = Outcome::Changed;
            self.current = (self.current - 1).max(self.low);
            ctx.no_op_event(true, |ctx| self.down.event(ctx, output));
            return;
        }

        if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
            if ScreenRectangle::top_left(self.top_left, self.dims).contains(pt) {
                if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                    if dy > 0.0 && self.current != self.high {
                        self.current += 1;
                        output.outcome = Outcome::Changed;
                    }
                    if dy < 0.0 && self.current != self.low {
                        self.current -= 1;
                        output.outcome = Outcome::Changed;
                    }
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        // TODO Cache
        let mut batch = GeomBatch::from(vec![(
            text::BG_COLOR,
            Polygon::rounded_rectangle(self.dims.width, self.dims.height, Some(5.0)),
        )]);
        batch.append(
            Text::from(Line(self.current.to_string()))
                .render_autocropped(g)
                .centered_on(Pt2D::new(TEXT_WIDTH / 2.0, self.dims.height / 2.0)),
        );
        let draw = g.upload(batch);
        g.redraw_at(self.top_left, &draw);

        self.up.draw(g);
        self.down.draw(g);
    }
}
