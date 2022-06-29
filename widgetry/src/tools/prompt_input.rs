use crate::{
    DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, TextBox, Transition, Widget,
};

/// Prompt for arbitrary text input, then feed the answer to a callback.
pub struct PromptInput<A> {
    panel: Panel,
    cb: Option<Box<dyn FnOnce(String, &mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: 'static> PromptInput<A> {
    pub fn new_state(
        ctx: &mut EventCtx,
        query: &str,
        initial: String,
        cb: Box<dyn FnOnce(String, &mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        Box::new(PromptInput {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line(query).small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                TextBox::default_widget(ctx, "input", initial),
                ctx.style()
                    .btn_outline
                    .text("confirm")
                    .hotkey(Key::Enter)
                    .build_def(ctx),
            ]))
            .build(ctx),
            cb: Some(cb),
        })
    }
}

impl<A: 'static> State<A> for PromptInput<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                "confirm" => {
                    let data = self.panel.text_box("input");
                    (self.cb.take().unwrap())(data, ctx, app)
                }
                _ => unreachable!(),
            },
            _ => {
                if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
                    return Transition::Pop;
                }
                Transition::Keep
            }
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        super::grey_out_map(g);
        self.panel.draw(g);
    }
}
