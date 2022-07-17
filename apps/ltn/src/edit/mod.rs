mod filters;
mod one_ways;
mod shortcuts;

use map_model::{IntersectionID, RoadID};
use widgetry::mapspace::{ObjectID, World};
use widgetry::tools::PopupMsg;
use widgetry::{EventCtx, Key, Line, Panel, PanelBuilder, Widget, DEFAULT_CORNER_RADIUS};

use crate::{after_edit, App, BrowseNeighbourhoods, Neighbourhood, Transition};

// TODO This is only used for styling now
#[derive(PartialEq)]
pub enum Tab {
    Connectivity,
    Shortcuts,
}

impl Tab {
    fn make_buttons(self, ctx: &mut EventCtx, app: &App) -> Widget {
        let mut row = Vec::new();
        for (tab, label, key) in [
            (Tab::Connectivity, "Connectivity", Key::F1),
            (Tab::Shortcuts, "Shortcuts (old)", Key::F2),
        ] {
            // TODO Match the TabController styling
            row.push(
                ctx.style()
                    .btn_tab
                    .text(label)
                    .corner_rounding(geom::CornerRadii {
                        top_left: DEFAULT_CORNER_RADIUS,
                        top_right: DEFAULT_CORNER_RADIUS,
                        bottom_left: 0.0,
                        bottom_right: 0.0,
                    })
                    .hotkey(key)
                    // We abuse "disabled" to denote "currently selected"
                    .disabled(self == tab)
                    .build_def(ctx),
            );
        }
        if app.session.consultation.is_none() {
            // TODO The 3rd doesn't really act like a tab
            row.push(
                ctx.style()
                    .btn_tab
                    .text("Adjust boundary")
                    .corner_rounding(geom::CornerRadii {
                        top_left: DEFAULT_CORNER_RADIUS,
                        top_right: DEFAULT_CORNER_RADIUS,
                        bottom_left: 0.0,
                        bottom_right: 0.0,
                    })
                    .hotkey(Key::B)
                    .build_def(ctx),
            );
        }

        Widget::row(row)
    }
}

// TODO This will replace Tab soon
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
    /// The neighbourhood has changed and the caller should recalculate stuff, including the panel
    Recalculate,
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
        tab: Tab,
        top_panel: &Panel,
        per_tab_contents: Widget,
    ) -> PanelBuilder {
        let contents = Widget::col(vec![
            app.session.alt_proposals.to_widget(ctx, app),
            BrowseNeighbourhoods::button(ctx, app),
            Line("Editing neighbourhood")
                .small_heading()
                .into_widget(ctx),
            edit_mode(ctx, &app.session.edit_mode),
            match app.session.edit_mode {
                EditMode::Filters => filters::widget(ctx, app),
                EditMode::Oneways => one_ways::widget(ctx),
                EditMode::Shortcuts(ref focus) => shortcuts::widget(ctx, app, focus.as_ref()),
            }
            .section(ctx),
            tab.make_buttons(ctx, app),
            per_tab_contents,
            crate::route_planner::RoutePlanner::button(ctx),
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
    ) -> Option<Transition> {
        let id = neighbourhood.id;
        match action {
            "Browse neighbourhoods" => {
                // Recalculate the state to redraw any changed filters
                Some(Transition::Replace(BrowseNeighbourhoods::new_state(
                    ctx, app,
                )))
            }
            "Adjust boundary" => Some(Transition::Replace(
                crate::select_boundary::SelectBoundary::new_state(ctx, app, id),
            )),
            "Connectivity" => Some(Transition::Replace(crate::connectivity::Viewer::new_state(
                ctx, app, id,
            ))),
            "Shortcuts (old)" => Some(Transition::Replace(
                crate::shortcut_viewer::BrowseShortcuts::new_state(ctx, app, id, None),
            )),
            // Overkill to force all mode-specific code into the module
            "Create filters along a shape" => Some(Transition::Push(
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
                Some(Transition::Recreate)
            }
            "Plan a route" => Some(Transition::Push(
                crate::route_planner::RoutePlanner::new_state(ctx, app),
            )),
            "Filters" => {
                app.session.edit_mode = EditMode::Filters;
                Some(Transition::Recreate)
            }
            "One-ways" => {
                app.session.edit_mode = EditMode::Oneways;
                Some(Transition::Recreate)
            }
            "Shortcuts" => {
                app.session.edit_mode = EditMode::Shortcuts(None);
                Some(Transition::Recreate)
            }
            "previous shortcut" => {
                if let EditMode::Shortcuts(Some(ref mut focus)) = app.session.edit_mode {
                    focus.current_idx -= 1;
                }
                Some(Transition::Recreate)
            }
            "next shortcut" => {
                if let EditMode::Shortcuts(Some(ref mut focus)) = app.session.edit_mode {
                    focus.current_idx += 1;
                }
                Some(Transition::Recreate)
            }
            _ => None,
        }
    }
}

fn edit_mode(ctx: &mut EventCtx, edit_mode: &EditMode) -> Widget {
    let mut row = Vec::new();
    for (label, is_current) in [
        ("Filters", matches!(edit_mode, EditMode::Filters)),
        ("One-ways", matches!(edit_mode, EditMode::Oneways)),
        ("Shortcuts", matches!(edit_mode, EditMode::Shortcuts(_))),
    ] {
        row.push(
            ctx.style()
                .btn_tab
                .text(label)
                .corner_rounding(geom::CornerRadii {
                    top_left: DEFAULT_CORNER_RADIUS,
                    top_right: DEFAULT_CORNER_RADIUS,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                })
                .disabled(is_current)
                .build_def(ctx),
        );
    }
    Widget::row(row)
}
