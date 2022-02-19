//! Generic UI tools. Some of this should perhaps be lifted to widgetry.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use anyhow::Result;

use abstutil::prettyprint_usize;
use geom::{Distance, Duration, Polygon};
use synthpop::TripMode;
use widgetry::tools::FutureLoader;
use widgetry::{
    Choice, Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Line, Menu, Outcome, Panel,
    State, Text, TextBox, Toggle, Transition, Widget,
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

pub struct FilePicker;

impl FilePicker {
    pub fn new_state<A: 'static + AppLike>(
        ctx: &mut EventCtx,
        start_dir: Option<String>,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, Result<Option<String>>) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
        let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
        FutureLoader::<A, Option<String>>::new_state(
            ctx,
            Box::pin(async move {
                let mut builder = rfd::AsyncFileDialog::new();
                if let Some(dir) = start_dir {
                    builder = builder.set_directory(&dir);
                }
                let result = builder.pick_file().await.map(|x| {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        x.path().display().to_string()
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        format!("TODO rfd on wasm: {:?}", x)
                    }
                });
                let wrap: Box<dyn Send + FnOnce(&A) -> Option<String>> =
                    Box::new(move |_: &A| result);
                Ok(wrap)
            }),
            outer_progress_rx,
            inner_progress_rx,
            "Waiting for a file to be chosen",
            on_load,
        )
    }
}

pub fn percentage_bar(ctx: &EventCtx, txt: Text, pct_green: f64) -> Widget {
    let bad_color = Color::RED;
    let good_color = Color::GREEN;

    let total_width = 450.0;
    let height = 32.0;
    let radius = 4.0;

    let mut batch = GeomBatch::new();
    // Background
    batch.push(
        bad_color,
        Polygon::rounded_rectangle(total_width, height, radius),
    );
    // Foreground
    if let Some(poly) = Polygon::maybe_rounded_rectangle(pct_green * total_width, height, radius) {
        batch.push(good_color, poly);
    }
    // Text
    let label = txt.render_autocropped(ctx);
    let dims = label.get_dims();
    batch.append(label.translate(10.0, height / 2.0 - dims.height / 2.0));
    batch.into_widget(ctx)
}

/// Shorter is better
pub fn cmp_dist(txt: &mut Text, app: &dyn AppLike, dist: Distance, shorter: &str, longer: &str) {
    match dist.cmp(&Distance::ZERO) {
        Ordering::Less => {
            txt.add_line(
                Line(format!(
                    "{} {}",
                    (-dist).to_string(&app.opts().units),
                    shorter
                ))
                .fg(Color::GREEN),
            );
        }
        Ordering::Greater => {
            txt.add_line(
                Line(format!("{} {}", dist.to_string(&app.opts().units), longer)).fg(Color::RED),
            );
        }
        Ordering::Equal => {}
    }
}

/// Shorter is better
pub fn cmp_duration(
    txt: &mut Text,
    app: &dyn AppLike,
    duration: Duration,
    shorter: &str,
    longer: &str,
) {
    match duration.cmp(&Duration::ZERO) {
        Ordering::Less => {
            txt.add_line(
                Line(format!(
                    "{} {}",
                    (-duration).to_string(&app.opts().units),
                    shorter
                ))
                .fg(Color::GREEN),
            );
        }
        Ordering::Greater => {
            txt.add_line(
                Line(format!(
                    "{} {}",
                    duration.to_string(&app.opts().units),
                    longer
                ))
                .fg(Color::RED),
            );
        }
        Ordering::Equal => {}
    }
}

/// Less is better
pub fn cmp_count(txt: &mut Text, before: usize, after: usize) {
    match after.cmp(&before) {
        std::cmp::Ordering::Equal => {
            txt.add_line(Line("same"));
        }
        std::cmp::Ordering::Less => {
            txt.add_appended(vec![
                Line(prettyprint_usize(before - after)).fg(Color::GREEN),
                Line(" less"),
            ]);
        }
        std::cmp::Ordering::Greater => {
            txt.add_appended(vec![
                Line(prettyprint_usize(after - before)).fg(Color::RED),
                Line(" more"),
            ]);
        }
    }
}

pub fn color_for_mode(app: &dyn AppLike, m: TripMode) -> Color {
    match m {
        TripMode::Walk => app.cs().unzoomed_pedestrian,
        TripMode::Bike => app.cs().unzoomed_bike,
        TripMode::Transit => app.cs().unzoomed_bus,
        TripMode::Drive => app.cs().unzoomed_car,
    }
}

pub fn checkbox_per_mode(
    ctx: &mut EventCtx,
    app: &dyn AppLike,
    current_state: &BTreeSet<TripMode>,
) -> Widget {
    let mut filters = Vec::new();
    for m in TripMode::all() {
        filters.push(
            Toggle::colored_checkbox(
                ctx,
                m.ongoing_verb(),
                color_for_mode(app, m),
                current_state.contains(&m),
            )
            .margin_right(24),
        );
    }
    Widget::custom_row(filters)
}
