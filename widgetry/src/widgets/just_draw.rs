use geom::Polygon;

use crate::{
    Drawable, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt, ScreenRectangle, Text, Widget,
    WidgetImpl, WidgetOutput,
};

// Just draw something, no interaction.
pub struct JustDraw {
    pub draw: Drawable,

    pub top_left: ScreenPt,
    pub dims: ScreenDims,
}

impl JustDraw {
    pub(crate) fn wrap(ctx: &EventCtx, batch: GeomBatch) -> Widget {
        Widget::new(Box::new(JustDraw {
            dims: batch.get_dims(),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        }))
    }
}

impl WidgetImpl for JustDraw {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _: &mut EventCtx, _: &mut WidgetOutput) {}

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
    }
}

pub struct DrawWithTooltips {
    draw: Drawable,
    tooltips: Vec<(Polygon, Text)>,
    hover: Box<dyn Fn(&Polygon) -> GeomBatch>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl DrawWithTooltips {
    /// `batch`: the `GeomBatch` to draw
    /// `tooltips`: (hitbox, text) tuples where each `text` is shown when the user hovers over
    ///             the respective `hitbox`
    /// `hover`: returns a GeomBatch to render upon hovering. Return an `GeomBox::new()` if
    ///          you want hovering to be a no-op
    pub fn new(
        ctx: &EventCtx,
        batch: GeomBatch,
        tooltips: Vec<(Polygon, Text)>,
        hover: Box<dyn Fn(&Polygon) -> GeomBatch>,
    ) -> Widget {
        Widget::new(Box::new(DrawWithTooltips {
            dims: batch.get_dims(),
            top_left: ScreenPt::new(0.0, 0.0),
            hover,
            draw: ctx.upload(batch),
            tooltips,
        }))
    }
}

impl WidgetImpl for DrawWithTooltips {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _: &mut EventCtx, _: &mut WidgetOutput) {}

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);

        if let Some(cursor) = g.canvas.get_cursor_in_screen_space() {
            if !ScreenRectangle::top_left(self.top_left, self.dims).contains(cursor) {
                return;
            }
            let translated =
                ScreenPt::new(cursor.x - self.top_left.x, cursor.y - self.top_left.y).to_pt();
            // TODO Assume regions are non-overlapping
            for (region, txt) in &self.tooltips {
                if region.contains_pt(translated) {
                    let extra = g.upload((self.hover)(region));
                    g.redraw_at(self.top_left, &extra);
                    g.draw_mouse_tooltip(txt.clone());
                    return;
                }
            }
        }
    }
}

// TODO Name is bad. Lay out JustDraw stuff with flexbox, just to consume it and produce one big
// GeomBatch.
pub struct DeferDraw {
    pub batch: GeomBatch,

    pub top_left: ScreenPt,
    dims: ScreenDims,
}

impl DeferDraw {
    pub fn new(batch: GeomBatch) -> Widget {
        Widget::new(Box::new(DeferDraw {
            dims: batch.get_dims(),
            batch,
            top_left: ScreenPt::new(0.0, 0.0),
        }))
    }
}

impl WidgetImpl for DeferDraw {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _: &mut EventCtx, _: &mut WidgetOutput) {
        unreachable!()
    }

    fn draw(&self, _: &mut GfxCtx) {
        unreachable!()
    }
}
