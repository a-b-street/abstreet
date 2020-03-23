use crate::{EventCtx, GfxCtx, Outcome, ScreenDims, ScreenPt, Widget, WidgetImpl};

pub struct Nothing {}

impl WidgetImpl for Nothing {
    fn get_dims(&self) -> ScreenDims {
        unreachable!()
    }

    fn set_pos(&mut self, _top_left: ScreenPt) {
        unreachable!()
    }

    fn event(&mut self, _ctx: &mut EventCtx, _redo_layout: &mut bool) -> Option<Outcome> {
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
        unreachable!()
    }
    fn set_pos(&mut self, _top_left: ScreenPt) {
        unreachable!()
    }

    fn event(&mut self, ctx: &mut EventCtx, redo_layout: &mut bool) -> Option<Outcome> {
        for w in &mut self.members {
            if let Some(o) = w.widget.event(ctx, redo_layout) {
                return Some(o);
            }
        }
        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        for w in &self.members {
            w.draw(g);
        }
    }
}
