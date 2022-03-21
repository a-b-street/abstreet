use map_gui::tools::percentage_bar;
use map_model::{PathRequest, NORMAL_LANE_THICKNESS};
use widgetry::mapspace::{ToggleZoomed, World};
use widgetry::{EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, Widget};

use crate::per_neighborhood::{FilterableObj, Tab};
use crate::rat_runs::{find_rat_runs, RatRuns};
use crate::{colors, App, Neighborhood, NeighborhoodID, Transition};

pub struct BrowseRatRuns {
    top_panel: Panel,
    left_panel: Panel,
    rat_runs: RatRuns,
    current_idx: usize,

    draw_path: ToggleZoomed,
    world: World<FilterableObj>,
    neighborhood: Neighborhood,
}

impl BrowseRatRuns {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        id: NeighborhoodID,
        start_with_request: Option<PathRequest>,
    ) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::new(ctx, app, id);

        let rat_runs = ctx.loading_screen("find rat runs", |_, timer| {
            find_rat_runs(app, &neighborhood, timer)
        });
        let world = crate::per_neighborhood::make_world(ctx, app, &neighborhood, &rat_runs);

        let mut state = BrowseRatRuns {
            top_panel: crate::common::app_top_panel(ctx, app),
            left_panel: Panel::empty(ctx),
            rat_runs,
            current_idx: 0,
            draw_path: ToggleZoomed::empty(ctx),
            neighborhood,
            world,
        };

        if let Some(req) = start_with_request {
            if let Some(idx) = state
                .rat_runs
                .paths
                .iter()
                .position(|path| path.get_req() == &req)
            {
                state.current_idx = idx;
            }
        }

        state.recalculate(ctx, app);

        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        let (quiet_streets, total_streets) =
            self.rat_runs.quiet_and_total_streets(&self.neighborhood);

        if self.rat_runs.paths.is_empty() {
            self.left_panel = Tab::RatRuns
                .panel_builder(
                    ctx,
                    app,
                    &self.top_panel,
                    percentage_bar(
                        ctx,
                        Text::from(Line(format!(
                            "{} / {} streets have no through-traffic",
                            quiet_streets, total_streets
                        ))),
                        1.0,
                    ),
                )
                .build(ctx);
            return;
        }

        // Optimization to avoid recalculating the whole panel
        if self.left_panel.has_widget("prev/next controls") {
            let controls = self.prev_next_controls(ctx);
            self.left_panel.replace(ctx, "prev/next controls", controls);
        } else {
            self.left_panel = Tab::RatRuns
                .panel_builder(
                    ctx,
                    app,
                    &self.top_panel,
                    Widget::col(vec![
                        percentage_bar(
                            ctx,
                            Text::from(Line(format!(
                                "{} / {} streets have no through-traffic",
                                quiet_streets, total_streets
                            ))),
                            (quiet_streets as f64) / (total_streets as f64),
                        ),
                        self.prev_next_controls(ctx),
                    ]),
                )
                .build(ctx);
        }

        let mut draw_path = ToggleZoomed::builder();
        if let Some(pl) = self.rat_runs.paths[self.current_idx].trace(&app.map) {
            let color = colors::RAT_RUN_PATH;
            let shape = pl.make_polygons(3.0 * NORMAL_LANE_THICKNESS);
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

    fn prev_next_controls(&self, ctx: &EventCtx) -> Widget {
        Widget::row(vec![
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
        ])
        .named("prev/next controls")
    }
}

impl State<App> for BrowseRatRuns {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) = crate::common::handle_top_panel(ctx, app, &mut self.top_panel, help) {
            return t;
        }
        match self.left_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "previous rat run" => {
                    self.current_idx -= 1;
                    self.recalculate(ctx, app);
                }
                "next rat run" => {
                    self.current_idx += 1;
                    self.recalculate(ctx, app);
                }
                x => {
                    if let Some(t) = Tab::RatRuns.handle_action(ctx, app, x, self.neighborhood.id) {
                        return t;
                    }
                    let current_request = if self.rat_runs.paths.is_empty() {
                        None
                    } else {
                        Some(self.rat_runs.paths[self.current_idx].get_req().clone())
                    };
                    return crate::save::AltProposals::handle_action(
                        ctx,
                        app,
                        crate::save::PreserveState::RatRuns(
                            current_request,
                            app.session
                                .partitioning
                                .all_blocks_in_neighborhood(self.neighborhood.id),
                        ),
                        x,
                    )
                    .unwrap();
                }
            },
            _ => {}
        }

        // TODO Bit weird to allow this while showing individual paths, since we don't draw the
        // world
        let world_outcome = self.world.event(ctx);
        if crate::per_neighborhood::handle_world_outcome(ctx, app, world_outcome) {
            // Reset state, but if possible, preserve the current individual rat run.
            let current_request = self.rat_runs.paths[self.current_idx].get_req().clone();
            return Transition::Replace(BrowseRatRuns::new_state(
                ctx,
                app,
                self.neighborhood.id,
                Some(current_request),
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.left_panel.draw(g);

        self.world.draw(g);
        self.draw_path.draw(g);

        g.redraw(&self.neighborhood.fade_irrelevant);
        app.session.draw_all_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }
    }
}

fn help() -> Vec<&'static str> {
    vec![
        "This shows every possible path a driver could take through the neighborhood.",
        "Not all paths may be realistic -- small service roads and alleyways are possible, but unlikely.",
    ]
}
