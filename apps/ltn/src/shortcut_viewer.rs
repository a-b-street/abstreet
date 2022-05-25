use map_gui::tools::percentage_bar;
use map_model::{PathRequest, NORMAL_LANE_THICKNESS};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, TextExt, Widget};

use crate::edit::{EditNeighborhood, Tab};
use crate::shortcuts::{find_shortcuts, Shortcuts};
use crate::{colors, App, Neighborhood, NeighborhoodID, Transition};

pub struct BrowseShortcuts {
    top_panel: Panel,
    left_panel: Panel,
    shortcuts: Shortcuts,
    current_idx: usize,

    draw_path: ToggleZoomed,
    edit: EditNeighborhood,
    neighborhood: Neighborhood,
}

impl BrowseShortcuts {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        id: NeighborhoodID,
        start_with_request: Option<PathRequest>,
    ) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::new(ctx, app, id);

        let shortcuts = ctx.loading_screen("find shortcuts", |_, timer| {
            find_shortcuts(app, &neighborhood, timer)
        });
        let edit = EditNeighborhood::new(ctx, app, &neighborhood, &shortcuts);

        let mut state = BrowseShortcuts {
            top_panel: crate::components::TopPanel::panel(ctx, app),
            left_panel: Panel::empty(ctx),
            shortcuts,
            current_idx: 0,
            draw_path: ToggleZoomed::empty(ctx),
            neighborhood,
            edit,
        };

        if let Some(req) = start_with_request {
            if let Some(idx) = state
                .shortcuts
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
            self.shortcuts.quiet_and_total_streets(&self.neighborhood);

        if self.shortcuts.paths.is_empty() {
            self.left_panel = self
                .edit
                .panel_builder(
                    ctx,
                    app,
                    Tab::Shortcuts,
                    &self.top_panel,
                    percentage_bar(
                        ctx,
                        Text::from(Line(format!(
                            "{} / {} streets have no shortcuts",
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
            self.left_panel = self
                .edit
                .panel_builder(
                    ctx,
                    app,
                    Tab::Shortcuts,
                    &self.top_panel,
                    Widget::col(vec![
                        percentage_bar(
                            ctx,
                            Text::from(Line(format!(
                                "{} / {} streets have no shortcuts",
                                quiet_streets, total_streets
                            ))),
                            (quiet_streets as f64) / (total_streets as f64),
                        ),
                        "Browse possible shortcuts through the neighborhood.".text_widget(ctx),
                        self.prev_next_controls(ctx),
                    ]),
                )
                .build(ctx);
        }

        let mut draw_path = ToggleZoomed::builder();
        if let Some(pl) = self.shortcuts.paths[self.current_idx].trace(&app.map) {
            let color = colors::SHORTCUT_PATH;
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
                .build_widget(ctx, "previous shortcut"),
            Text::from(
                Line(format!(
                    "{}/{}",
                    self.current_idx + 1,
                    self.shortcuts.paths.len()
                ))
                .secondary(),
            )
            .into_widget(ctx)
            .centered_vert(),
            ctx.style()
                .btn_next()
                .disabled(self.current_idx == self.shortcuts.paths.len() - 1)
                .hotkey(Key::RightArrow)
                .build_widget(ctx, "next shortcut"),
        ])
        .named("prev/next controls")
    }
}

impl State<App> for BrowseShortcuts {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) = crate::components::TopPanel::event(ctx, app, &mut self.top_panel, help) {
            return t;
        }
        match self.left_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "previous shortcut" => {
                    self.current_idx -= 1;
                    self.recalculate(ctx, app);
                }
                "next shortcut" => {
                    self.current_idx += 1;
                    self.recalculate(ctx, app);
                }
                x => {
                    if let Some(t) = self.edit.handle_panel_action(
                        ctx,
                        app,
                        x,
                        &self.neighborhood,
                        &self.left_panel,
                    ) {
                        return t;
                    }
                    let current_request = if self.shortcuts.paths.is_empty() {
                        None
                    } else {
                        Some(self.shortcuts.paths[self.current_idx].get_req().clone())
                    };
                    return crate::save::AltProposals::handle_action(
                        ctx,
                        app,
                        crate::save::PreserveState::Shortcuts(
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

        if self.edit.event(ctx, app) {
            // Reset state, but if possible, preserve the current individual shortcut.
            let current_request = self.shortcuts.paths[self.current_idx].get_req().clone();
            return Transition::Replace(BrowseShortcuts::new_state(
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

        self.edit.world.draw(g);
        self.draw_path.draw(g);

        g.redraw(&self.neighborhood.fade_irrelevant);
        app.session.draw_all_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let current_request = if self.shortcuts.paths.is_empty() {
            None
        } else {
            Some(self.shortcuts.paths[self.current_idx].get_req().clone())
        };
        Self::new_state(ctx, app, self.neighborhood.id, current_request)
    }
}

fn help() -> Vec<&'static str> {
    vec![
        "This shows every possible path a driver could take through the neighborhood.",
        "Not all paths may be realistic -- small service roads and alleyways are possible, but unlikely.",
    ]
}
