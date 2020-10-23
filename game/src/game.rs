use widgetry::{
    hotkeys, Btn, Choice, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Menu, Outcome, Panel, ScreenRectangle, State, Text,
    VerticalAlignment, Widget,
};

use crate::app::{App, Flags};
use crate::helpers::grey_out_map;
use crate::options::Options;
use crate::pregame::TitleScreen;
use crate::sandbox::{GameplayMode, SandboxMode};

pub struct Game;

pub type Transition = widgetry::Transition<App>;

impl Game {
    pub fn new(
        flags: Flags,
        opts: Options,
        start_with_edits: Option<String>,
        maybe_mode: Option<GameplayMode>,
        ctx: &mut EventCtx,
    ) -> (App, Vec<Box<dyn State<App>>>) {
        let title = !opts.dev
            && !flags.sim_flags.load.contains("player/save")
            && !flags.sim_flags.load.contains("system/scenarios")
            && maybe_mode.is_none();
        let mut app = App::new(flags, opts, ctx, title);

        // Handle savestates
        let savestate = if app
            .primary
            .current_flags
            .sim_flags
            .load
            .contains("player/saves/")
        {
            assert!(maybe_mode.is_none());
            Some(app.primary.clear_sim())
        } else {
            None
        };

        // Just apply this here, don't plumb to SimFlags or anything else. We recreate things using
        // these flags later, but we don't want to keep applying the same edits.
        if let Some(edits_name) = start_with_edits {
            // TODO Maybe loading screen
            let mut timer = abstutil::Timer::new("apply initial edits");
            let edits = map_model::MapEdits::load(
                &app.primary.map,
                abstutil::path_edits(app.primary.map.get_name(), &edits_name),
                &mut timer,
            )
            .unwrap();
            crate::edit::apply_map_edits(ctx, &mut app, edits);
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
            app.primary.clear_sim();
        }

        let states: Vec<Box<dyn State<App>>> = if title {
            vec![Box::new(TitleScreen::new(ctx, &mut app))]
        } else {
            let mode = maybe_mode
                .unwrap_or_else(|| GameplayMode::Freeform(app.primary.map.get_name().clone()));
            vec![SandboxMode::simple_new(ctx, &mut app, mode)]
        };
        if let Some(ss) = savestate {
            // TODO This is weird, we're left in Freeform mode with the wrong UI. Can't instantiate
            // PlayScenario without clobbering.
            app.primary.sim = ss;
        }

        (app, states)
    }
}

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
