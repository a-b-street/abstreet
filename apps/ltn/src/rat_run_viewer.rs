use map_gui::tools::{percentage_bar, ColorNetwork};
use map_model::{PathRequest, NORMAL_LANE_THICKNESS};
use widgetry::mapspace::{ToggleZoomed, World};
use widgetry::{
    Color, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, TextExt, Toggle, Widget,
};

use crate::per_neighborhood::{FilterableObj, Tab};
use crate::rat_runs::{find_rat_runs, RatRuns};
use crate::{App, Neighborhood, NeighborhoodID, Transition};

pub struct BrowseRatRuns {
    top_panel: Panel,
    left_panel: Panel,
    rat_runs: RatRuns,
    // When None, show the heatmap of all rat runs
    current_idx: Option<usize>,

    draw_path: ToggleZoomed,
    draw_heatmap: ToggleZoomed,
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
        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(rat_runs.count_per_road.clone(), &app.cs.good_to_bad_red);
        // TODO These two will be on different scales, which'll look really weird!
        colorer.ranked_intersections(
            rat_runs.count_per_intersection.clone(),
            &app.cs.good_to_bad_red,
        );
        let world = make_world(ctx, app, &neighborhood, &rat_runs);

        let mut state = BrowseRatRuns {
            top_panel: crate::common::app_top_panel(ctx, app),
            left_panel: Panel::empty(ctx),
            rat_runs,
            current_idx: None,
            draw_path: ToggleZoomed::empty(ctx),
            draw_heatmap: colorer.build(ctx),
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
                state.current_idx = Some(idx);
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
        if self.left_panel.has_widget("prev/next controls") && self.current_idx.is_some() {
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
                        Widget::row(vec![
                            "Show rat-runs".text_widget(ctx).centered_vert(),
                            Toggle::choice(
                                ctx,
                                "show rat-runs",
                                "all (heatmap)",
                                "individually",
                                Key::R,
                                self.current_idx.is_none(),
                            ),
                        ]),
                        self.prev_next_controls(ctx),
                    ]),
                )
                .build(ctx);
        }

        let mut draw_path = ToggleZoomed::builder();
        if let Some(pl) = self
            .current_idx
            .and_then(|idx| self.rat_runs.paths[idx].trace(&app.map))
        {
            let color = Color::RED;
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
        if let Some(idx) = self.current_idx {
            Widget::row(vec![
                ctx.style()
                    .btn_prev()
                    .disabled(idx == 0)
                    .hotkey(Key::LeftArrow)
                    .build_widget(ctx, "previous rat run"),
                Text::from(Line(format!("{}/{}", idx + 1, self.rat_runs.paths.len())).secondary())
                    .into_widget(ctx)
                    .centered_vert(),
                ctx.style()
                    .btn_next()
                    .disabled(idx == self.rat_runs.paths.len() - 1)
                    .hotkey(Key::RightArrow)
                    .build_widget(ctx, "next rat run"),
            ])
            .named("prev/next controls")
        } else {
            Widget::nothing()
        }
    }
}

impl State<App> for BrowseRatRuns {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) = crate::common::handle_top_panel(ctx, app, &mut self.top_panel) {
            return t;
        }
        match self.left_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "previous rat run" => {
                    for idx in &mut self.current_idx {
                        *idx -= 1;
                    }
                    self.recalculate(ctx, app);
                }
                "next rat run" => {
                    for idx in &mut self.current_idx {
                        *idx += 1;
                    }
                    self.recalculate(ctx, app);
                }
                x => {
                    return Tab::RatRuns
                        .handle_action(ctx, app, x, self.neighborhood.id)
                        .unwrap();
                }
            },
            Outcome::Changed(_) => {
                if self.left_panel.is_checked("show rat-runs") {
                    self.current_idx = None;
                } else {
                    self.current_idx = Some(0);
                }
                self.recalculate(ctx, app);
            }
            _ => {}
        }

        // TODO Bit weird to allow this while showing individual paths, since we don't draw the
        // world
        let world_outcome = self.world.event(ctx);
        if crate::per_neighborhood::handle_world_outcome(ctx, app, world_outcome) {
            // Reset state, but if possible, preserve the current individual rat run.
            let current_request = self
                .current_idx
                .map(|idx| self.rat_runs.paths[idx].get_req().clone());
            return Transition::Replace(BrowseRatRuns::new_state(
                ctx,
                app,
                self.neighborhood.id,
                current_request,
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.left_panel.draw(g);

        if self.current_idx.is_some() {
            self.draw_path.draw(g);
        } else {
            self.draw_heatmap.draw(g);
            self.world.draw(g);
        }

        g.redraw(&self.neighborhood.fade_irrelevant);
        app.session.draw_all_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }
    }
}

fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighborhood: &Neighborhood,
    rat_runs: &RatRuns,
) -> World<FilterableObj> {
    let map = &app.map;
    let mut world = World::bounded(map.get_bounds());

    crate::per_neighborhood::populate_world(ctx, app, neighborhood, &mut world, |id| id, 0);

    // Bit hacky. Go through and fill out tooltips for the objects just added to the world.
    for r in &neighborhood.orig_perimeter.interior {
        assert!(world.override_tooltip(
            &FilterableObj::InteriorRoad(*r),
            Some(Text::from(format!(
                "{} rat-runs cross this street",
                rat_runs.count_per_road.get(*r)
            )))
        ));
    }
    for i in &neighborhood.interior_intersections {
        assert!(world.override_tooltip(
            &FilterableObj::InteriorIntersection(*i),
            Some(Text::from(format!(
                "{} rat-runs cross this intersection",
                rat_runs.count_per_intersection.get(*i)
            )))
        ));
    }

    world.initialize_hover(ctx);

    world
}
