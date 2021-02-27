use geom::{CornerRadii, Distance, Polygon, Pt2D};

use crate::{
    include_labeled_bytes, text, Button, Drawable, EdgeInsets, EventCtx, GeomBatch, GfxCtx, Line,
    Outcome, OutlineStyle, Prerender, ScreenDims, ScreenPt, ScreenRectangle, Style, StyledButtons,
    Text, Widget, WidgetImpl, WidgetOutput,
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
    outline: OutlineStyle,
    drawable: Drawable,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Spinner {
    pub fn widget(ctx: &EventCtx, (low, high): (isize, isize), current: isize) -> Widget {
        Widget::new(Box::new(Self::new(ctx, (low, high), current)))
    }

    pub fn new(ctx: &EventCtx, (low, high): (isize, isize), mut current: isize) -> Self {
        let button_builder = ctx
            .style()
            .btn_plain()
            .padding(EdgeInsets {
                top: 2.0,
                bottom: 2.0,
                left: 4.0,
                right: 4.0,
            })
            .image_dims(17.0);

        let up = button_builder
            .clone()
            .image_bytes(include_labeled_bytes!("../../icons/arrow_up.svg"))
            .corner_rounding(CornerRadii {
                top_left: 0.0,
                top_right: 5.0,
                bottom_right: 0.0,
                bottom_left: 5.0,
            })
            .build(ctx, "increase value");

        let down = button_builder
            .image_bytes(include_labeled_bytes!("../../icons/arrow_down.svg"))
            .corner_rounding(CornerRadii {
                top_left: 5.0,
                top_right: 0.0,
                bottom_right: 5.0,
                bottom_left: 0.0,
            })
            .build(ctx, "decrease value");

        let outline = ctx.style().btn_outline.outline;
        let dims = ScreenDims::new(
            TEXT_WIDTH + up.get_dims().width,
            up.get_dims().height + down.get_dims().height + 1.0,
        );
        if current < low {
            current = low;
            warn!("Spinner current value is out of bounds!");
        } else if high < current {
            current = high;
            warn!("Spinner current value is out of bounds!");
        }

        let mut spinner = Spinner {
            low,
            high,
            current,

            up,
            down,
            drawable: Drawable::empty(ctx),
            outline,
            top_left: ScreenPt::new(0.0, 0.0),
            dims,
        };
        spinner.drawable = spinner.drawable(ctx.prerender, ctx.style());
        spinner
    }

    pub fn modify(&mut self, ctx: &EventCtx, delta: isize) {
        self.current += delta;
        self.current = self.current.min(self.high);
        self.current = self.current.max(self.low);
        self.drawable = self.drawable(ctx.prerender, ctx.style());
    }

    fn drawable(&self, prerender: &Prerender, style: &Style) -> Drawable {
        let mut batch = GeomBatch::from(vec![(
            style.field_bg,
            Polygon::rounded_rectangle(self.dims.width, self.dims.height, 5.0),
        )]);
        batch.append(
            Text::from(Line(self.current.to_string()))
                .render_autocropped(prerender)
                .centered_on(Pt2D::new(TEXT_WIDTH / 2.0, self.dims.height / 2.0)),
        );
        batch.push(
            self.outline.1,
            Polygon::rounded_rectangle(self.dims.width, self.dims.height, 5.0)
                .to_outline(Distance::meters(self.outline.0))
                .unwrap(),
        );
        prerender.upload(batch)
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
            self.drawable = self.drawable(&ctx.prerender, ctx.style());
            ctx.no_op_event(true, |ctx| self.up.event(ctx, output));
            return;
        }

        self.down.event(ctx, output);
        if let Outcome::Clicked(_) = output.outcome {
            output.outcome = Outcome::Changed;
            self.current = (self.current - 1).max(self.low);
            self.drawable = self.drawable(&ctx.prerender, ctx.style());
            ctx.no_op_event(true, |ctx| self.down.event(ctx, output));
            return;
        }

        if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
            if ScreenRectangle::top_left(self.top_left, self.dims).contains(pt) {
                if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                    if dy > 0.0 && self.current != self.high {
                        self.current += 1;
                        output.outcome = Outcome::Changed;
                        self.drawable = self.drawable(&ctx.prerender, ctx.style());
                    }
                    if dy < 0.0 && self.current != self.low {
                        self.current -= 1;
                        output.outcome = Outcome::Changed;
                        self.drawable = self.drawable(&ctx.prerender, ctx.style());
                    }
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.drawable);

        self.up.draw(g);
        self.down.draw(g);
    }
}
