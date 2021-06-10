use crate::{
    Drawable, EventCtx, GeomBatch, GeomBatchStack, GfxCtx, RewriteColor, ScreenDims, ScreenPt,
    ScreenRectangle, Widget, WidgetImpl, WidgetOutput,
};

pub struct DragDrop<K: Clone> {
    members: Vec<(K, GeomBatch, ScreenDims)>,
    draw: Drawable,
    hovering: Option<usize>,
    dragging: Option<usize>,

    dims: ScreenDims,
    top_left: ScreenPt,
}

impl<K: 'static + Clone> DragDrop<K> {
    pub fn new_widget(ctx: &EventCtx, members: Vec<(K, GeomBatch)>) -> Widget {
        let mut dd = DragDrop {
            members: members
                .into_iter()
                .map(|(key, batch)| {
                    let dims = batch.get_dims();
                    (key, batch, dims)
                })
                .collect(),
            draw: Drawable::empty(ctx),
            hovering: None,
            dragging: None,

            dims: ScreenDims::square(0.0),
            top_left: ScreenPt::new(0.0, 0.0),
        };
        dd.recalc_draw(ctx);
        Widget::new(Box::new(dd))
    }
}

impl<K: 'static + Clone> DragDrop<K> {
    fn recalc_draw(&mut self, ctx: &EventCtx) {
        let mut stack = GeomBatchStack::horizontal(Vec::new());
        for (idx, (_, batch, _)) in self.members.iter().enumerate() {
            let mut batch = batch.clone();
            if let Some(drag_idx) = self.dragging {
                // If we're dragging, fade everything out except what we're dragging and where
                // we're maybe going to drop
                if idx == drag_idx {
                    // Leave it
                } else if self.hovering == Some(idx) {
                    // Possible drop
                    batch = batch.color(RewriteColor::ChangeAlpha(0.8));
                } else {
                    // Fade it out
                    batch = batch.color(RewriteColor::ChangeAlpha(0.5));
                }
            } else if self.hovering == Some(idx) {
                // If we're not dragging, show what we're hovering on
                batch = batch.color(RewriteColor::ChangeAlpha(0.5));
            }
            stack.push(batch);
        }
        let batch = stack.batch();
        self.dims = batch.get_dims();
        self.draw = batch.upload(ctx);
    }

    fn mouseover_card(&self, ctx: &EventCtx) -> Option<usize> {
        let pt = ctx.canvas.get_cursor_in_screen_space()?;
        let mut top_left = self.top_left;
        for (idx, (_, _, dims)) in self.members.iter().enumerate() {
            if ScreenRectangle::top_left(top_left, *dims).contains(pt) {
                return Some(idx);
            }
            top_left.x += dims.width;
        }
        None
    }
}

impl<K: 'static + Clone> WidgetImpl for DragDrop<K> {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, _: &mut WidgetOutput) {
        if let Some(old_idx) = self.dragging {
            if ctx.input.left_mouse_button_released() {
                self.dragging = None;
                if let Some(new_idx) = self.hovering {
                    if old_idx != new_idx {
                        // TODO Emit a Changed event, then the caller can go fetch the new ordering
                        self.members.swap(old_idx, new_idx);
                        self.recalc_draw(ctx);
                    }
                }
            }
        }
        if ctx.redo_mouseover() {
            let old = self.hovering.take();
            self.hovering = self.mouseover_card(ctx);
            if old != self.hovering {
                self.recalc_draw(ctx);
            }
        }
        if let Some(idx) = self.hovering {
            if ctx.input.left_mouse_button_pressed() {
                self.dragging = Some(idx);
                self.recalc_draw(ctx);
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
    }
}
