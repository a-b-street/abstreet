mod filters;
mod one_ways;
mod shortcuts;

use map_model::{IntersectionID, RoadID};
use widgetry::mapspace::{ObjectID, World};
use widgetry::tools::PopupMsg;
use widgetry::{lctrl, EventCtx, Key, Line, Panel, PanelBuilder, TextExt, Widget};

use crate::{after_edit, App, BrowseNeighbourhoods, Neighbourhood, Transition};

pub enum EditMode {
    Filters,
    Oneways,
    // Is a road clicked on right now?
    Shortcuts(Option<shortcuts::FocusedRoad>),
}

pub struct EditNeighbourhood {
    // Only pub for drawing
    pub world: World<Obj>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Obj {
    InteriorRoad(RoadID),
    InteriorIntersection(IntersectionID),
}
impl ObjectID for Obj {}

pub enum EditOutcome {
    Nothing,
    /// Don't recreate the Neighbourhood
    UpdatePanelAndWorld,
    /// Use this with Transition::Recreate to recalculate the Neighbourhood, because it's actually
    /// been edited
    Transition(Transition),
}

impl EditOutcome {
    fn error(ctx: &mut EventCtx, msg: &str) -> Self {
        Self::Transition(Transition::Push(PopupMsg::new_state(
            ctx,
            "Error",
            vec![msg],
        )))
    }
}

impl EditNeighbourhood {
    pub fn temporary() -> Self {
        Self {
            world: World::unbounded(),
        }
    }

    pub fn new(ctx: &mut EventCtx, app: &App, neighbourhood: &Neighbourhood) -> Self {
        Self {
            world: match &app.session.edit_mode {
                EditMode::Filters => filters::make_world(ctx, app, neighbourhood),
                EditMode::Oneways => one_ways::make_world(ctx, app, neighbourhood),
                EditMode::Shortcuts(focus) => shortcuts::make_world(ctx, app, neighbourhood, focus),
            },
        }
    }

    pub fn panel_builder(
        &self,
        ctx: &mut EventCtx,
        app: &App,
        top_panel: &Panel,
        per_tab_contents: Widget,
    ) -> PanelBuilder {
        let contents = Widget::col(vec![
            app.session.alt_proposals.to_widget(ctx, app),
            BrowseNeighbourhoods::button(ctx, app),
            Line("Editing neighbourhood")
                .small_heading()
                .into_widget(ctx),
            Widget::col(vec![
                edit_mode(ctx, &app.session.edit_mode),
                match app.session.edit_mode {
                    EditMode::Filters => filters::widget(ctx),
                    EditMode::Oneways => one_ways::widget(ctx),
                    EditMode::Shortcuts(ref focus) => shortcuts::widget(ctx, app, focus.as_ref()),
                },
            ])
            .section(ctx),
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/undo.svg")
                    .disabled(app.session.modal_filters.previous_version.is_none())
                    .hotkey(lctrl(Key::Z))
                    .build_widget(ctx, "undo"),
                format!(
                    "{} filters added",
                    app.session.modal_filters.roads.len()
                        + app.session.modal_filters.intersections.len()
                )
                .text_widget(ctx)
                .centered_vert(),
            ]),
            {
                let mut row = Vec::new();
                if app.session.consultation.is_none() {
                    row.push(
                        ctx.style()
                            .btn_outline
                            .text("Adjust boundary")
                            .hotkey(Key::B)
                            .build_def(ctx),
                    );
                }
                row.push(crate::route_planner::RoutePlanner::button(ctx));
                Widget::row(row)
            },
            per_tab_contents,
        ]);
        crate::components::LeftPanel::builder(ctx, top_panel, contents)
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        neighbourhood: &Neighbourhood,
    ) -> EditOutcome {
        let outcome = self.world.event(ctx);
        let outcome = match app.session.edit_mode {
            EditMode::Filters => filters::handle_world_outcome(ctx, app, outcome),
            EditMode::Oneways => one_ways::handle_world_outcome(ctx, app, outcome),
            EditMode::Shortcuts(_) => shortcuts::handle_world_outcome(app, outcome, neighbourhood),
        };
        if matches!(outcome, EditOutcome::Transition(_)) {
            self.world.hack_unset_hovering();
        }
        outcome
    }

    pub fn handle_panel_action(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        action: &str,
        neighbourhood: &Neighbourhood,
        panel: &Panel,
    ) -> EditOutcome {
        let id = neighbourhood.id;
        match action {
            "Browse neighbourhoods" => {
                // Recalculate the state to redraw any changed filters
                EditOutcome::Transition(Transition::Replace(BrowseNeighbourhoods::new_state(
                    ctx, app,
                )))
            }
            "Adjust boundary" => EditOutcome::Transition(Transition::Replace(
                crate::select_boundary::SelectBoundary::new_state(ctx, app, id),
            )),
            // Overkill to force all mode-specific code into the module
            "Create filters along a shape" => EditOutcome::Transition(Transition::Push(
                crate::components::FreehandFilters::new_state(
                    ctx,
                    neighbourhood,
                    panel.center_of("Create filters along a shape"),
                ),
            )),
            "undo" => {
                let prev = app.session.modal_filters.previous_version.take().unwrap();
                app.session.modal_filters = prev;
                after_edit(ctx, app);
                // TODO Ideally, preserve panel state (checkboxes and dropdowns)
                if let EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
                    *maybe_focus = None;
                }
                EditOutcome::Transition(Transition::Recreate)
            }
            "Plan a route" => EditOutcome::Transition(Transition::Push(
                crate::route_planner::RoutePlanner::new_state(ctx, app),
            )),
            "Filters" => {
                app.session.edit_mode = EditMode::Filters;
                EditOutcome::UpdatePanelAndWorld
            }
            "One-ways" => {
                app.session.edit_mode = EditMode::Oneways;
                EditOutcome::UpdatePanelAndWorld
            }
            "Shortcuts" => {
                app.session.edit_mode = EditMode::Shortcuts(None);
                EditOutcome::UpdatePanelAndWorld
            }
            "previous shortcut" => {
                if let EditMode::Shortcuts(Some(ref mut focus)) = app.session.edit_mode {
                    focus.current_idx -= 1;
                }
                EditOutcome::UpdatePanelAndWorld
            }
            "next shortcut" => {
                if let EditMode::Shortcuts(Some(ref mut focus)) = app.session.edit_mode {
                    focus.current_idx += 1;
                }
                EditOutcome::UpdatePanelAndWorld
            }
            _ => EditOutcome::Nothing,
        }
    }
}

fn edit_mode(ctx: &mut EventCtx, edit_mode: &EditMode) -> Widget {
    let mut row = Vec::new();
    for (label, key, is_current) in [
        ("Filters", Key::F1, matches!(edit_mode, EditMode::Filters)),
        ("One-ways", Key::F2, matches!(edit_mode, EditMode::Oneways)),
        (
            "Shortcuts",
            Key::F3,
            matches!(edit_mode, EditMode::Shortcuts(_)),
        ),
    ] {
        if is_current {
            row.push(
                ctx.style()
                    .btn_tab
                    .btn()
                    .label_underlined_text(label)
                    .disabled(true)
                    .build_def(ctx),
            );
        } else {
            row.push(ctx.style().btn_tab.text(label).hotkey(key).build_def(ctx));
        }
    }
    Widget::row(row)
}
