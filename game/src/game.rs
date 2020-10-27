//! Everything here should ideally be lifted to widgetry as common states.

use widgetry::{
    hotkeys, Btn, Choice, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Menu, Outcome, Panel, ScreenRectangle, State, Text,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::helpers::grey_out_map;

pub type Transition = widgetry::Transition<App>;

pub struct ChooseSomething<T> {
    panel: Panel,
    cb: Box<dyn Fn(T, &mut EventCtx, &mut App) -> Transition>,
}

impl<T: 'static> ChooseSomething<T> {
    pub fn new(
        ctx: &mut EventCtx,
        query: &str,
        choices: Vec<Choice<T>>,
        cb: Box<dyn Fn(T, &mut EventCtx, &mut App) -> Transition>,
    ) -> Box<dyn State<App>> {
        Box::new(ChooseSomething {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line(query).small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                Menu::new(ctx, choices).named("menu"),
            ]))
            .build(ctx),
            cb,
        })
    }

    pub fn new_below(
        ctx: &mut EventCtx,
        rect: &ScreenRectangle,
        choices: Vec<Choice<T>>,
        cb: Box<dyn Fn(T, &mut EventCtx, &mut App) -> Transition>,
    ) -> Box<dyn State<App>> {
        Box::new(ChooseSomething {
            panel: Panel::new(Menu::new(ctx, choices).named("menu").container())
                .aligned(
                    HorizontalAlignment::Centered(rect.center().x),
                    VerticalAlignment::Below(rect.y2 + 15.0),
                )
                .build(ctx),
            cb,
        })
    }
}

impl<T: 'static> State<App> for ChooseSomething<T> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                _ => {
                    let data = self.panel.take_menu_choice::<T>("menu");
                    (self.cb)(data, ctx, app)
                }
            },
            _ => {
                if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
                    return Transition::Pop;
                }
                // new_below doesn't make an X button
                if ctx.input.pressed(Key::Escape) {
                    return Transition::Pop;
                }
                Transition::Keep
            }
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

pub struct PromptInput {
    panel: Panel,
    cb: Box<dyn Fn(String, &mut EventCtx, &mut App) -> Transition>,
}

impl PromptInput {
    pub fn new(
        ctx: &mut EventCtx,
        query: &str,
        cb: Box<dyn Fn(String, &mut EventCtx, &mut App) -> Transition>,
    ) -> Box<dyn State<App>> {
        Box::new(PromptInput {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line(query).small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                Widget::text_entry(ctx, String::new(), true).named("input"),
                Btn::text_fg("confirm").build_def(ctx, Key::Enter),
            ]))
            .build(ctx),
            cb,
        })
    }
}

impl State<App> for PromptInput {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                "confirm" => {
                    let data = self.panel.text_box("input");
                    (self.cb)(data, ctx, app)
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

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

pub struct PopupMsg {
    panel: Panel,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl PopupMsg {
    pub fn new<I: Into<String>>(
        ctx: &mut EventCtx,
        title: &str,
        lines: Vec<I>,
    ) -> Box<dyn State<App>> {
        PopupMsg::also_draw(
            ctx,
            title,
            lines,
            ctx.upload(GeomBatch::new()),
            ctx.upload(GeomBatch::new()),
        )
    }

    pub fn also_draw<I: Into<String>>(
        ctx: &mut EventCtx,
        title: &str,
        lines: Vec<I>,
        unzoomed: Drawable,
        zoomed: Drawable,
    ) -> Box<dyn State<App>> {
        let mut txt = Text::new();
        txt.add(Line(title).small_heading());
        for l in lines {
            txt.add(Line(l));
        }
        Box::new(PopupMsg {
            panel: Panel::new(Widget::col(vec![
                txt.draw(ctx),
                Btn::text_bg2("OK").build_def(ctx, hotkeys(vec![Key::Enter, Key::Escape])),
            ]))
            .build(ctx),
            unzoomed,
            zoomed,
        })
    }
}

impl State<App> for PopupMsg {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
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

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
}
