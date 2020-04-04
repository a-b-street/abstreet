use crate::{
    text, Btn, Button, EventCtx, GeomBatch, GfxCtx, Line, Outcome, ScreenDims, ScreenPt, Text,
    Widget, WidgetImpl,
};
use geom::{Polygon, Pt2D};

// TODO MAX_CHAR_WIDTH is a hardcoded nonsense value
const TEXT_WIDTH: f64 = 2.0 * text::MAX_CHAR_WIDTH;

// TODO Allow text entry
// TODO Allow click and hold
// TODO Grey out the buttons when we're maxed out
pub struct Spinner {
    low: usize,
    high: usize,
    pub current: usize,

    up: Button,
    down: Button,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Spinner {
    pub fn new(ctx: &EventCtx, (low, high): (usize, usize), current: usize) -> Widget {
        let up = Btn::text_fg("▲")
            .build(ctx, "increase value", None)
            .take_btn();
        let down = Btn::text_fg("▼")
            .build(ctx, "decrease value", None)
            .take_btn();

        let dims = ScreenDims::new(
            TEXT_WIDTH + up.get_dims().width,
            up.get_dims().height + down.get_dims().height,
        );

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
}

impl WidgetImpl for Spinner {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        // TODO This works, but it'd be kind of cool if we could construct a tiny little Composite
        // here and use that. Wait, why can't we? ...
        self.top_left = top_left;
        self.up
            .set_pos(ScreenPt::new(top_left.x + TEXT_WIDTH, top_left.y));
        self.down.set_pos(ScreenPt::new(
            top_left.x + TEXT_WIDTH,
            top_left.y + self.up.get_dims().height,
        ));
    }

    fn event(&mut self, ctx: &mut EventCtx, redo_layout: &mut bool) -> Option<Outcome> {
        if self.up.event(ctx, redo_layout).is_some() {
            if self.current != self.high {
                self.current += 1;
            }
        } else if self.down.event(ctx, redo_layout).is_some() {
            if self.current != self.low {
                self.current -= 1;
            }
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        // TODO Cache
        let mut batch = GeomBatch::from(vec![(
            text::BG_COLOR,
            Polygon::rounded_rectangle(self.dims.width, self.dims.height, Some(5.0)),
        )]);
        batch.add_centered(
            Text::from(Line(self.current.to_string())).render_to_batch(g.prerender),
            Pt2D::new(TEXT_WIDTH / 2.0, self.dims.height / 2.0),
        );
        let draw = g.upload(batch);
        g.redraw_at(self.top_left, &draw);

        self.up.draw(g);
        self.down.draw(g);
    }
}
