use crate::{
    Choice, DrawBaselayer, EventCtx, GfxCtx, Line, Menu, Outcome, Panel, State, Transition, Widget,
};

/// Choose something from a menu, then feed the answer to a callback.
pub struct ChooseSomething<A, T> {
    panel: Panel,
    // Wrapped in an Option so that we can consume it once
    cb: Option<Box<dyn FnOnce(T, &mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: 'static, T: 'static> ChooseSomething<A, T> {
    pub fn new_state<I: Into<String>>(
        ctx: &mut EventCtx,
        query: I,
        choices: Vec<Choice<T>>,
        cb: Box<dyn FnOnce(T, &mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        Box::new(ChooseSomething {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line(query).small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Menu::widget(ctx, choices).named("menu"),
            ]))
            .build(ctx),
            cb: Some(cb),
        })
    }
}

impl<A: 'static, T: 'static> State<A> for ChooseSomething<A, T> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                _ => {
                    let data = self.panel.take_menu_choice::<T>("menu");
                    // If the callback doesn't replace or pop this ChooseSomething state, then
                    // it'll break when the user tries to interact with the menu again.
                    (self.cb.take().unwrap())(data, ctx, app)
                }
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
