use geom::ArrowCap;
use map_gui::tools::ColorNetwork;
use map_model::NORMAL_LANE_THICKNESS;
use widgetry::mapspace::{ToggleZoomed, World};
use widgetry::{
    Color, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, TextExt, Toggle, Widget,
};

use super::per_neighborhood::{FilterableObj, Tab, TakeNeighborhood};
use super::rat_runs::{find_rat_runs, RatRuns};
use super::Neighborhood;
use crate::app::{App, Transition};

pub struct BrowseRatRuns {
    panel: Panel,
    rat_runs: RatRuns,
    current_idx: usize,

    draw_path: ToggleZoomed,
    draw_heatmap: ToggleZoomed,
    world: World<FilterableObj>,
    neighborhood: Neighborhood,
}

impl TakeNeighborhood for BrowseRatRuns {
    fn take_neighborhood(self) -> Neighborhood {
        self.neighborhood
    }
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
        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(rat_runs.count_per_road.clone(), &app.cs.good_to_bad_red);
        // TODO These two will be on different scales, which'll look really weird!
        colorer.ranked_intersections(
            rat_runs.count_per_intersection.clone(),
            &app.cs.good_to_bad_red,
        );
        let world = make_world(ctx, app, &neighborhood, &rat_runs);

        let mut state = BrowseRatRuns {
            panel: Panel::empty(ctx),
            rat_runs,
            current_idx: 0,
            draw_path: ToggleZoomed::empty(ctx),
            draw_heatmap: colorer.build(ctx),
            neighborhood,
            world,
        };
        state.recalculate(ctx, app);
        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.rat_runs.paths.is_empty() {
            self.panel = Tab::RatRuns
                .panel_builder(ctx, app, "No rat runs detected".text_widget(ctx))
                .build(ctx);
            return;
        }

        self.panel = Tab::RatRuns
            .panel_builder(
                ctx,
                app,
                Widget::col(vec![
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
                    // TODO This should disable the individual path controls, or maybe even be a different
                    // state entirely...
                    Toggle::checkbox(
                        ctx,
                        "show heatmap of all rat-runs",
                        Key::R,
                        self.panel
                            .maybe_is_checked("show heatmap of all rat-runs")
                            .unwrap_or(true),
                    ),
                ]),
            )
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
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "previous rat run" => {
                    self.current_idx -= 1;
                    self.panel
                        .set_checked("show heatmap of all rat-runs", false);
                    self.recalculate(ctx, app);
                }
                "next rat run" => {
                    self.current_idx += 1;
                    self.panel
                        .set_checked("show heatmap of all rat-runs", false);
                    self.recalculate(ctx, app);
                }
                x => {
                    return Tab::Connectivity.must_handle_action::<BrowseRatRuns>(ctx, app, x);
                }
            }
        }

        // TODO Bit weird to allow this while showing individual paths, since we don't draw the
        // world
        let world_outcome = self.world.event(ctx);
        if super::per_neighborhood::handle_world_outcome(ctx, app, world_outcome) {
            // TODO We could be a bit more efficient here, but simplest to just start over with a
            // new state
            return Transition::Replace(BrowseRatRuns::new_state(
                ctx,
                app,
                Neighborhood::new(ctx, app, self.neighborhood.orig_perimeter.clone()),
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);

        if self.panel.is_checked("show heatmap of all rat-runs") {
            self.draw_heatmap.draw(g);
            self.world.draw(g);
        } else {
            self.draw_path.draw(g);
        }

        g.redraw(&self.neighborhood.fade_irrelevant);
        self.neighborhood.draw_filters.draw(g);
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
    let map = &app.primary.map;
    let mut world = World::bounded(map.get_bounds());

    super::per_neighborhood::populate_world(ctx, app, neighborhood, &mut world, |id| id, 0);

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
