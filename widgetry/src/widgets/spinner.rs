use geom::{CornerRadii, Distance, Polygon, Pt2D};

use crate::{
    include_labeled_bytes, Button, Drawable, EdgeInsets, EventCtx, GeomBatch, GfxCtx, Outcome,
    OutlineStyle, Prerender, ScreenDims, ScreenPt, ScreenRectangle, Style, Text, Widget,
    WidgetImpl, WidgetOutput,
};

// Manually tuned
const TEXT_WIDTH: f64 = 80.0;

pub trait SpinnerValue:
    Copy
    + PartialOrd
    + std::fmt::Display
    + std::ops::Add<Output = Self>
    + std::ops::AddAssign
    + std::ops::Sub<Output = Self>
    + std::ops::SubAssign
where
    Self: std::marker::Sized,
{
}

impl<T> SpinnerValue for T where
    T: Copy
        + PartialOrd
        + std::fmt::Display
        + std::ops::Add<Output = Self>
        + std::ops::AddAssign
        + std::ops::Sub<Output = Self>
        + std::ops::SubAssign
{
}

// TODO Allow text entry
// TODO Allow click and hold
// TODO Grey out the buttons when we're maxed out
pub struct Spinner<T> {
    low: T,
    high: T,
    step_size: T,
    pub current: T,
    label: String,

    up: Button,
    down: Button,
    outline: OutlineStyle,
    drawable: Drawable,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl<T: 'static + SpinnerValue> Spinner<T> {
    pub fn widget(
        ctx: &EventCtx,
        label: impl Into<String>,
        (low, high): (T, T),
        current: T,
        step_size: T,
    ) -> Widget {
        let label = label.into();
        Widget::new(Box::new(Self::new(
            ctx,
            label.clone(),
            (low, high),
            current,
            step_size,
        )))
        .named(label)
    }

    fn new(
        ctx: &EventCtx,
        label: String,
        (low, high): (T, T),
        mut current: T,
        step_size: T,
    ) -> Self {
        let button_builder = ctx
            .style()
            .btn_plain
            .btn()
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
            warn!(
                "Spinner's initial value is out of bounds! {}, bounds ({}, {})",
                current, low, high
            );
            current = low;
        } else if high < current {
            warn!(
                "Spinner's initial value is out of bounds! {}, bounds ({}, {})",
                current, low, high
            );
            current = high;
        }

        let mut spinner = Spinner {
            low,
            high,
            current,
            step_size,
            label,

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

    pub fn modify(&mut self, ctx: &EventCtx, delta: T) {
        self.current += delta;
        self.clamp();
        self.drawable = self.drawable(ctx.prerender, ctx.style());
    }

    fn clamp(&mut self) {
        if self.current > self.high {
            self.current = self.high;
        }
        if self.current < self.low {
            self.current = self.low;
        }
    }

    fn drawable(&self, prerender: &Prerender, style: &Style) -> Drawable {
        let mut batch = GeomBatch::from(vec![(
            style.field_bg,
            Polygon::rounded_rectangle(self.dims.width, self.dims.height, 5.0),
        )]);
        batch.append(
            Text::from(self.current.to_string())
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

impl<T: 'static + SpinnerValue> WidgetImpl for Spinner<T> {
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
            output.outcome = Outcome::Changed(self.label.clone());
            self.current += self.step_size;
            self.clamp();
            self.drawable = self.drawable(&ctx.prerender, ctx.style());
            ctx.no_op_event(true, |ctx| self.up.event(ctx, output));
            return;
        }

        self.down.event(ctx, output);
        if let Outcome::Clicked(_) = output.outcome {
            output.outcome = Outcome::Changed(self.label.clone());
            self.current -= self.step_size;
            self.clamp();
            self.drawable = self.drawable(&ctx.prerender, ctx.style());
            ctx.no_op_event(true, |ctx| self.down.event(ctx, output));
            return;
        }

        if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
            if ScreenRectangle::top_left(self.top_left, self.dims).contains(pt) {
                if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                    if dy > 0.0 && self.current < self.high {
                        self.current += self.step_size;
                        self.clamp();
                        output.outcome = Outcome::Changed(self.label.clone());
                        self.drawable = self.drawable(&ctx.prerender, ctx.style());
                    }
                    if dy < 0.0 && self.current > self.low {
                        self.current -= self.step_size;
                        self.clamp();
                        output.outcome = Outcome::Changed(self.label.clone());
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
