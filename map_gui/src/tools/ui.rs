//! Generic UI tools. Some of this should perhaps be lifted to widgetry.

use widgetry::{
    hotkeys, Choice, DrawBaselayer, Drawable, EventCtx, GfxCtx, Key, Line, Menu, Outcome, Panel,
    State, Text, TextBox, Transition, Widget,
};

use crate::tools::grey_out_map;
use crate::AppLike;

/// Choose something from a menu, then feed the answer to a callback.
pub struct ChooseSomething<A: AppLike, T> {
    panel: Panel,
    // Wrapped in an Option so that we can consume it once
    cb: Option<Box<dyn FnOnce(T, &mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static, T: 'static> ChooseSomething<A, T> {
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

impl<A: AppLike + 'static, T: 'static> State<A> for ChooseSomething<A, T> {
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

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

/// Prompt for arbitrary text input, then feed the answer to a callback.
pub struct PromptInput<A: AppLike> {
    panel: Panel,
    cb: Option<Box<dyn FnOnce(String, &mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static> PromptInput<A> {
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

impl<A: AppLike + 'static> State<A> for PromptInput<A> {
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

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

/// Display a message dialog.
pub struct PopupMsg {
    panel: Panel,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl PopupMsg {
    pub fn new_state<A: AppLike>(
        ctx: &mut EventCtx,
        title: &str,
        lines: Vec<impl AsRef<str>>,
    ) -> Box<dyn State<A>> {
        PopupMsg::also_draw(
            ctx,
            title,
            lines,
            Drawable::empty(ctx),
            Drawable::empty(ctx),
        )
    }

    pub fn also_draw<A: AppLike>(
        ctx: &mut EventCtx,
        title: &str,
        lines: Vec<impl AsRef<str>>,
        unzoomed: Drawable,
        zoomed: Drawable,
    ) -> Box<dyn State<A>> {
        let mut txt = Text::new();
        txt.add_line(Line(title).small_heading());
        for l in lines {
            txt.add_line(l);
        }
        Box::new(PopupMsg {
            panel: Panel::new_builder(Widget::col(vec![
                txt.into_widget(ctx),
                ctx.style()
                    .btn_solid_primary
                    .text("OK")
                    .hotkey(hotkeys(vec![Key::Enter, Key::Escape]))
                    .build_def(ctx),
            ]))
            .build(ctx),
            unzoomed,
            zoomed,
        })
    }
}

impl<A: AppLike> State<A> for PopupMsg {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "OK" => Transition::Pop,
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

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts().min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
}
