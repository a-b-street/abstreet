use crate::{EventCtx, GfxCtx, Outcome, ScreenDims, ScreenPt, Widget, WidgetImpl, WidgetOutput};

pub struct Nothing {}

impl WidgetImpl for Nothing {
    fn get_dims(&self) -> ScreenDims {
        unreachable!()
    }

    fn set_pos(&mut self, _top_left: ScreenPt) {
        unreachable!()
    }

    fn event(&mut self, _: &mut EventCtx, _: &mut WidgetOutput) {
        unreachable!()
    }
    fn draw(&self, _g: &mut GfxCtx) {
        unreachable!()
    }
}

pub struct Container {
    // false means column
    pub is_row: bool,
    pub members: Vec<Widget>,
}

impl Container {
    pub fn new(is_row: bool, mut members: Vec<Widget>) -> Container {
        members.retain(|w| !w.widget.is::<Nothing>());
        Container { is_row, members }
    }
}

impl WidgetImpl for Container {
    fn get_dims(&self) -> ScreenDims {
        // TODO This impl isn't correct, but it works for the one use case of
        // get_width_for_forcing.
        let mut width: f64 = 0.0;
        for x in &self.members {
            width = width.max(x.get_width_for_forcing());
        }
        ScreenDims::new(width, 0.0)
    }
    fn set_pos(&mut self, _top_left: ScreenPt) {
        unreachable!()
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        for w in &mut self.members {
            // If both are filled out, they'll be the same
            if let Some(id) = output
                .prev_focus_owned_by
                .as_ref()
                .or(output.current_focus_owned_by.as_ref())
            {
                // Container is the only place that needs to actually enforce focus. If a Panel
                // consists of only one top-level widget, then there's nothing else to conflict
                // with focus. And in the common case, we have a tree of Containers, with
                // non-Container leaves.
                if w.id.as_ref() != Some(id) && !w.widget.is::<Container>() {
                    continue;
                }
            }
            w.widget.event(ctx, output);
            // If the widget produced an outcome or currently has focus, short-circuit.
            if !matches!(output.outcome, Outcome::Nothing)
                || output.current_focus_owned_by.is_some()
            {
                return;
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        for w in &self.members {
            w.draw(g);
        }
    }
}
