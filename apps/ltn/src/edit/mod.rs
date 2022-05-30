mod filters;
mod one_ways;

use map_model::{IntersectionID, RoadID};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{EventCtx, Key, Line, Panel, PanelBuilder, Widget, DEFAULT_CORNER_RADIUS};

use crate::shortcuts::Shortcuts;
use crate::{after_edit, App, BrowseNeighborhoods, Neighborhood, Transition};

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
            (Tab::Shortcuts, "Shortcuts", Key::F2),
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

pub struct EditNeighborhood {
    // Only pub for drawing
    pub world: World<Obj>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Obj {
    InteriorRoad(RoadID),
    InteriorIntersection(IntersectionID),
}
impl ObjectID for Obj {}

impl EditNeighborhood {
    pub fn temporary() -> Self {
        Self {
            world: World::unbounded(),
        }
    }

    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        neighborhood: &Neighborhood,
        shortcuts: &Shortcuts,
    ) -> Self {
        Self {
            world: if app.session.edit_filters {
                filters::make_world(ctx, app, neighborhood, shortcuts)
            } else {
                one_ways::make_world(ctx, app, neighborhood)
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
            BrowseNeighborhoods::button(ctx, app),
            Line("Editing neighborhood")
                .small_heading()
                .into_widget(ctx),
            edit_mode(ctx, app.session.edit_filters),
            if app.session.edit_filters {
                filters::widget(ctx, app)
            } else {
                one_ways::widget(ctx)
            }
            .section(ctx),
            tab.make_buttons(ctx, app),
            per_tab_contents,
            crate::route_planner::RoutePlanner::button(ctx),
        ]);
        crate::components::LeftPanel::builder(ctx, top_panel, contents)
    }

    /// If true, the neighborhood has changed and the caller should recalculate stuff, including
    /// the panel
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> bool {
        let outcome = self.world.event(ctx);
        if app.session.edit_filters {
            filters::handle_world_outcome(ctx, app, outcome)
        } else {
            one_ways::handle_world_outcome(ctx, app, outcome)
        }
    }

    pub fn handle_panel_action(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        action: &str,
        neighborhood: &Neighborhood,
        panel: &Panel,
    ) -> Option<Transition> {
        let id = neighborhood.id;
        match action {
            "Browse neighborhoods" => {
                // Recalculate the state to redraw any changed filters
                Some(Transition::Replace(BrowseNeighborhoods::new_state(
                    ctx, app,
                )))
            }
            "Adjust boundary" => Some(Transition::Replace(
                crate::select_boundary::SelectBoundary::new_state(ctx, app, id),
            )),
            "Connectivity" => Some(Transition::Replace(crate::connectivity::Viewer::new_state(
                ctx, app, id,
            ))),
            "Shortcuts" => Some(Transition::Replace(
                crate::shortcut_viewer::BrowseShortcuts::new_state(ctx, app, id, None),
            )),
            // Overkill to force all mode-specific code into the module
            "Create filters along a shape" => Some(Transition::Push(
                crate::components::FreehandFilters::new_state(
                    ctx,
                    neighborhood,
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
                app.session.edit_filters = true;
                Some(Transition::Recreate)
            }
            "One-ways" => {
                app.session.edit_filters = false;
                Some(Transition::Recreate)
            }
            _ => None,
        }
    }
}

fn edit_mode(ctx: &mut EventCtx, filters: bool) -> Widget {
    let mut row = Vec::new();
    row.push(
        ctx.style()
            .btn_tab
            .text("Filters")
            .corner_rounding(geom::CornerRadii {
                top_left: DEFAULT_CORNER_RADIUS,
                top_right: DEFAULT_CORNER_RADIUS,
                bottom_left: 0.0,
                bottom_right: 0.0,
            })
            .disabled(filters)
            .build_def(ctx),
    );
    row.push(
        ctx.style()
            .btn_tab
            .text("One-ways")
            .corner_rounding(geom::CornerRadii {
                top_left: DEFAULT_CORNER_RADIUS,
                top_right: DEFAULT_CORNER_RADIUS,
                bottom_left: 0.0,
                bottom_right: 0.0,
            })
            .disabled(!filters)
            .build_def(ctx),
    );
    Widget::row(row)
}
