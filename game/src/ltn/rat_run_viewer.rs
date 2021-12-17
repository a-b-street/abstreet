use geom::ArrowCap;
use map_model::NORMAL_LANE_THICKNESS;
use widgetry::mapspace::ToggleZoomed;
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use super::rat_runs::{find_rat_runs, RatRun};
use super::Neighborhood;
use crate::app::{App, Transition};

pub struct BrowseRatRuns {
    panel: Panel,
    rat_runs: Vec<RatRun>,
    current_idx: usize,

    draw_paths: ToggleZoomed,
    neighborhood: Neighborhood,
}

impl BrowseRatRuns {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        neighborhood: Neighborhood,
    ) -> Box<dyn State<App>> {
        let rat_runs = find_rat_runs(&app.primary.map, &neighborhood, &app.session.modal_filters);
        let mut state = BrowseRatRuns {
            panel: Panel::empty(ctx),
            rat_runs,
            current_idx: 0,
            draw_paths: ToggleZoomed::empty(ctx),
            neighborhood,
        };
        state.recalculate(ctx, app);
        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.rat_runs.is_empty() {
            self.panel = Panel::new_builder(Widget::col(vec![
                ctx.style()
                    .btn_outline
                    .text("Back to editing modal filters")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
                "No rat runs detected".text_widget(ctx),
            ]))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx);
            return;
        }

        let current = &self.rat_runs[self.current_idx];

        self.panel = Panel::new_builder(Widget::col(vec![
            ctx.style()
                .btn_outline
                .text("Back to editing modal filters")
                .hotkey(Key::Escape)
                .build_def(ctx),
            Line("Warning: placeholder results")
                .fg(Color::RED)
                .into_widget(ctx),
            Widget::row(vec![
                "Rat runs:".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_prev()
                    .disabled(self.current_idx == 0)
                    .hotkey(Key::LeftArrow)
                    .build_widget(ctx, "previous rat run"),
                Text::from(
                    Line(format!("{}/{}", self.current_idx + 1, self.rat_runs.len())).secondary(),
                )
                .into_widget(ctx)
                .centered_vert(),
                ctx.style()
                    .btn_next()
                    .disabled(self.current_idx == self.rat_runs.len() - 1)
                    .hotkey(Key::RightArrow)
                    .build_widget(ctx, "next rat run"),
            ]),
            Text::from_multiline(vec![
                Line(format!("Ratio: {:.2}", current.time_ratio())),
                Line(format!(
                    "Shortcut takes: {}",
                    current.shortcut_path.get_cost()
                )),
                Line(format!(
                    "Fastest path takes: {}",
                    current.fastest_path.get_cost()
                )),
            ])
            .into_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        // TODO Transforming into PathV1 seems like a particularly unnecessary step. Time to come
        // up with a native v2 drawing?
        let mut draw_paths = ToggleZoomed::builder();
        for (path, color) in [
            (current.shortcut_path.clone(), Color::RED),
            (current.fastest_path.clone(), Color::BLUE),
        ] {
            if let Ok(path) = path.into_v1(&app.primary.map) {
                if let Some(pl) = path.trace(&app.primary.map) {
                    // TODO This produces a really buggy shape sometimes!
                    let shape = pl.make_arrow(3.0 * NORMAL_LANE_THICKNESS, ArrowCap::Triangle);
                    draw_paths.unzoomed.push(color.alpha(0.8), shape.clone());
                    draw_paths.zoomed.push(color.alpha(0.5), shape);
                }
            }
        }
        self.draw_paths = draw_paths.build(ctx);
    }
}

impl State<App> for BrowseRatRuns {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Back to editing modal filters" => {
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let state = state.downcast::<BrowseRatRuns>().ok().unwrap();
                        vec![super::viewer::Viewer::new_state(
                            ctx,
                            app,
                            state.neighborhood,
                        )]
                    }));
                }
                "previous rat run" => {
                    self.current_idx -= 1;
                    self.recalculate(ctx, app);
                }
                "next rat run" => {
                    self.current_idx += 1;
                    self.recalculate(ctx, app);
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);

        g.redraw(&self.neighborhood.fade_irrelevant);
        self.neighborhood.draw_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }

        self.draw_paths.draw(g);
    }
}
