use crate::{
    Drawable, EventCtx, GeomBatch, GeomBatchStack, GfxCtx, Outcome, RewriteColor, ScreenDims,
    ScreenPt, ScreenRectangle, Widget, WidgetImpl, WidgetOutput,
};

pub struct DragDrop {
    label: String,
    members: Vec<(GeomBatch, ScreenDims)>,
    draw: Drawable,
    state: State,

    dims: ScreenDims,
    top_left: ScreenPt,
}

#[derive(PartialEq)]
enum State {
    Idle {
        hovering: Option<usize>,
    },
    Dragging {
        orig_idx: usize,
        drag_from: ScreenPt,
        cursor_at: ScreenPt,
        new_idx: usize,
    },
}

impl DragDrop {
    pub fn new_widget(ctx: &EventCtx, label: &str, members: Vec<GeomBatch>) -> Widget {
        let mut dd = DragDrop {
            label: label.to_string(),
            members: members
                .into_iter()
                .map(|batch| {
                    let dims = batch.get_dims();
                    (batch, dims)
                })
                .collect(),
            draw: Drawable::empty(ctx),
            state: State::Idle { hovering: None },

            dims: ScreenDims::square(0.0),
            top_left: ScreenPt::new(0.0, 0.0),
        };
        dd.recalc_draw(ctx);
        Widget::new(Box::new(dd))
    }
}

impl DragDrop {
    fn recalc_draw(&mut self, ctx: &EventCtx) {
        let batch = match self.state {
            State::Idle { hovering } => {
                let mut stack = GeomBatchStack::horizontal(Vec::new());
                for (idx, (batch, _)) in self.members.iter().enumerate() {
                    let mut batch = batch.clone();
                    if hovering == Some(idx) {
                        batch = batch.color(RewriteColor::ChangeAlpha(0.5));
                    }
                    stack.push(batch);
                }
                stack.batch()
            }
            State::Dragging {
                orig_idx,
                drag_from,
                cursor_at,
                new_idx,
            } => {
                let mut members = self.members.iter().collect::<Vec<_>>();
                // TODO Swap isn't what we want... we want to remove the old thing and insert at
                // the new position
                members.swap(orig_idx, new_idx);

                let mut stack = GeomBatchStack::horizontal(Vec::new());
                let mut width = 0.0;
                for (idx, (batch, _)) in members.into_iter().enumerate() {
                    let mut batch = batch.clone();
                    if new_idx == idx {
                        batch =
                            batch.translate(cursor_at.x - drag_from.x - width, cursor_at.y - drag_from.y);
                        batch = batch.color(RewriteColor::ChangeAlpha(0.5));
                    } else if idx < new_idx {
                        // TODO Not correct
                        width += batch.get_dims().width;
                    }
                    stack.push(batch);
                }
                stack.batch()
            }
        };
        self.dims = batch.get_dims();
        self.draw = batch.upload(ctx);
    }

    fn mouseover_card(&self, ctx: &EventCtx) -> Option<usize> {
        let pt = ctx.canvas.get_cursor_in_screen_space()?;
        let mut top_left = self.top_left;
        for (idx, (_, dims)) in self.members.iter().enumerate() {
            if ScreenRectangle::top_left(top_left, *dims).contains(pt) {
                return Some(idx);
            }
            top_left.x += dims.width;
        }
        None
    }
}

impl WidgetImpl for DragDrop {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        let mut state = std::mem::replace(&mut self.state, State::Idle { hovering: None });
        match state {
            State::Idle { ref mut hovering } => {
                if ctx.redo_mouseover() {
                    let new = self.mouseover_card(ctx);
                    if *hovering != new {
                        *hovering = new;
                    }
                }
                if let Some(idx) = hovering {
                    if ctx.input.left_mouse_button_pressed() {
                        let cursor = ctx.canvas.get_cursor_in_screen_space().unwrap();
                        state = State::Dragging {
                            orig_idx: *idx,
                            drag_from: cursor,
                            cursor_at: cursor,
                            new_idx: *idx,
                        };
                    }
                }
            }
            State::Dragging {
                orig_idx,
                ref mut cursor_at,
                ref mut new_idx,
                ..
            } => {
                if ctx.redo_mouseover() {
                    if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                        *cursor_at = pt;
                    }
                    // TODO https://jqueryui.com/sortable/ only swaps once you cross the center of
                    // the new card
                    if let Some(idx) = self.mouseover_card(ctx) {
                        *new_idx = idx;
                    }
                }
                if ctx.input.left_mouse_button_released() {
                    let new_idx = *new_idx;
                    state = State::Idle {
                        hovering: Some(new_idx),
                    };

                    if orig_idx != new_idx {
                        output.outcome =
                            Outcome::DragDropReordered(self.label.clone(), orig_idx, new_idx);
                        self.members.swap(orig_idx, new_idx);
                    }
                }
            }
        }
        let changed = self.state != state;
        self.state = state;
        if changed {
            self.recalc_draw(ctx);
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
    }
}
