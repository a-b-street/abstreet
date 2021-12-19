use geom::ArrowCap;
use map_model::NORMAL_LANE_THICKNESS;
use widgetry::mapspace::ToggleZoomed;
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use super::rat_runs::{find_rat_runs, RatRuns};
use super::Neighborhood;
use crate::app::{App, Transition};

pub struct BrowseRatRuns {
    panel: Panel,
    rat_runs: RatRuns,
    current_idx: usize,

    draw_path: ToggleZoomed,
    neighborhood: Neighborhood,
}

impl BrowseRatRuns {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        neighborhood: Neighborhood,
    ) -> Box<dyn State<App>> {
        let rat_runs = ctx.loading_screen("find rat runs", |_, timer| {
            find_rat_runs(
                &app.primary.map,
                &neighborhood,
                &app.session.modal_filters,
                timer,
            )
        });
        let mut state = BrowseRatRuns {
            panel: Panel::empty(ctx),
            rat_runs,
            current_idx: 0,
            draw_path: ToggleZoomed::empty(ctx),
            neighborhood,
        };
        state.recalculate(ctx, app);
        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.rat_runs.paths.is_empty() {
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

        self.panel = Panel::new_builder(Widget::col(vec![
            ctx.style()
                .btn_outline
                .text("Back to editing modal filters")
                .hotkey(Key::Escape)
                .build_def(ctx),
            Line("Warning: preliminary results")
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
                    Line(format!(
                        "{}/{}",
                        self.current_idx + 1,
                        self.rat_runs.paths.len()
                    ))
                    .secondary(),
                )
                .into_widget(ctx)
                .centered_vert(),
                ctx.style()
                    .btn_next()
                    .disabled(self.current_idx == self.rat_runs.paths.len() - 1)
                    .hotkey(Key::RightArrow)
                    .build_widget(ctx, "next rat run"),
            ]),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        let mut draw_path = ToggleZoomed::builder();
        let color = Color::RED;
        let path = &self.rat_runs.paths[self.current_idx];
        if let Some(pl) = path.trace(&app.primary.map) {
            // TODO This produces a really buggy shape sometimes!
            let shape = pl.make_arrow(3.0 * NORMAL_LANE_THICKNESS, ArrowCap::Triangle);
            draw_path.unzoomed.push(color.alpha(0.8), shape.clone());
            draw_path.zoomed.push(color.alpha(0.5), shape);

            draw_path
                .unzoomed
                .append(map_gui::tools::start_marker(ctx, pl.first_pt(), 2.0));
            draw_path
                .zoomed
                .append(map_gui::tools::start_marker(ctx, pl.first_pt(), 0.5));

            draw_path
                .unzoomed
                .append(map_gui::tools::goal_marker(ctx, pl.last_pt(), 2.0));
            draw_path
                .zoomed
                .append(map_gui::tools::goal_marker(ctx, pl.last_pt(), 0.5));
        }
        self.draw_path = draw_path.build(ctx);
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

        self.draw_path.draw(g);
    }
}
