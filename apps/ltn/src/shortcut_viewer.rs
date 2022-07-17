use map_gui::tools::percentage_bar;
use map_model::{PathRequest, NORMAL_LANE_THICKNESS};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, TextExt, Widget};

use crate::edit::{EditNeighbourhood, EditOutcome, Tab};
use crate::{colors, App, Neighbourhood, NeighbourhoodID, Transition};

pub struct BrowseShortcuts {
    top_panel: Panel,
    left_panel: Panel,
    current_idx: usize,

    draw_path: ToggleZoomed,
    edit: EditNeighbourhood,
    neighbourhood: Neighbourhood,
}

impl BrowseShortcuts {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        id: NeighbourhoodID,
        start_with_request: Option<PathRequest>,
    ) -> Box<dyn State<App>> {
        let neighbourhood = Neighbourhood::new(ctx, app, id);
        let edit = EditNeighbourhood::new(ctx, app, &neighbourhood);

        let mut state = BrowseShortcuts {
            top_panel: crate::components::TopPanel::panel(ctx, app),
            left_panel: Panel::empty(ctx),
            current_idx: 0,
            draw_path: ToggleZoomed::empty(ctx),
            neighbourhood,
            edit,
        };

        if let Some(req) = start_with_request {
            if let Some(idx) = state
                .neighbourhood
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
        let (quiet_streets, total_streets) = self
            .neighbourhood
            .shortcuts
            .quiet_and_total_streets(&self.neighbourhood);

        if self.neighbourhood.shortcuts.paths.is_empty() {
            self.left_panel = self
                .edit
                .panel_builder(
                    ctx,
                    app,
                    Tab::Shortcuts,
                    &self.top_panel,
                    &self.neighbourhood,
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
                    &self.neighbourhood,
                    Widget::col(vec![
                        percentage_bar(
                            ctx,
                            Text::from(Line(format!(
                                "{} / {} streets have no shortcuts",
                                quiet_streets, total_streets
                            ))),
                            (quiet_streets as f64) / (total_streets as f64),
                        ),
                        "Browse possible shortcuts through the neighbourhood.".text_widget(ctx),
                        self.prev_next_controls(ctx),
                    ]),
                )
                .build(ctx);
        }

        let mut draw_path = ToggleZoomed::builder();
        if let Some(pl) = self.neighbourhood.shortcuts.paths[self.current_idx].trace(&app.map) {
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
                    self.neighbourhood.shortcuts.paths.len()
                ))
                .secondary(),
            )
            .into_widget(ctx)
            .centered_vert(),
            ctx.style()
                .btn_next()
                .disabled(self.current_idx == self.neighbourhood.shortcuts.paths.len() - 1)
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
                        &self.neighbourhood,
                        &self.left_panel,
                    ) {
                        return t;
                    }
                    let current_request = if self.neighbourhood.shortcuts.paths.is_empty() {
                        None
                    } else {
                        Some(
                            self.neighbourhood.shortcuts.paths[self.current_idx]
                                .get_req()
                                .clone(),
                        )
                    };
                    return crate::save::AltProposals::handle_action(
                        ctx,
                        app,
                        crate::save::PreserveState::Shortcuts(
                            current_request,
                            app.session
                                .partitioning
                                .all_blocks_in_neighbourhood(self.neighbourhood.id),
                        ),
                        x,
                    )
                    .unwrap();
                }
            },
            _ => {}
        }

        match self.edit.event(ctx, app) {
            EditOutcome::Nothing => {}
            EditOutcome::Recalculate => {
                // Reset state, but if possible, preserve the current individual shortcut.
                let current_request = self.neighbourhood.shortcuts.paths[self.current_idx]
                    .get_req()
                    .clone();
                return Transition::Replace(BrowseShortcuts::new_state(
                    ctx,
                    app,
                    self.neighbourhood.id,
                    Some(current_request),
                ));
            }
            EditOutcome::Transition(t) => {
                return t;
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.left_panel.draw(g);

        self.edit.world.draw(g);
        self.draw_path.draw(g);

        g.redraw(&self.neighbourhood.fade_irrelevant);
        app.session.draw_all_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.neighbourhood.labels.draw(g, app);
        }
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let current_request = if self.neighbourhood.shortcuts.paths.is_empty() {
            None
        } else {
            Some(
                self.neighbourhood.shortcuts.paths[self.current_idx]
                    .get_req()
                    .clone(),
            )
        };
        Self::new_state(ctx, app, self.neighbourhood.id, current_request)
    }
}

fn help() -> Vec<&'static str> {
    vec![
        "This shows every possible path a driver could take through the neighbourhood.",
        "Not all paths may be realistic -- small service roads and alleyways are possible, but unlikely.",
    ]
}
