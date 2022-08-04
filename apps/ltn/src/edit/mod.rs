mod filters;
mod freehand_filters;
mod one_ways;
mod shortcuts;

use map_model::{IntersectionID, RoadID};
use widgetry::mapspace::{ObjectID, World};
use widgetry::tools::{PolyLineLasso, PopupMsg};
use widgetry::{
    lctrl, Color, ControlState, EventCtx, Key, Line, Panel, PanelBuilder, RewriteColor, TextExt,
    Widget,
};

use crate::{after_edit, App, BrowseNeighbourhoods, FilterType, Neighbourhood, Transition};

pub enum EditMode {
    Filters,
    FreehandFilters(PolyLineLasso),
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
                EditMode::FreehandFilters(_) => World::unbounded(),
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
                edit_mode(ctx, app),
                match app.session.edit_mode {
                    EditMode::Filters => filters::widget(ctx),
                    EditMode::FreehandFilters(_) => freehand_filters::widget(ctx),
                    EditMode::Oneways => one_ways::widget(ctx),
                    EditMode::Shortcuts(ref focus) => shortcuts::widget(ctx, app, focus.as_ref()),
                }
                .named("edit mode contents"),
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
        if let EditMode::FreehandFilters(_) = app.session.edit_mode {
            return freehand_filters::event(ctx, app, neighbourhood);
        }

        let outcome = self.world.event(ctx);
        let outcome = match app.session.edit_mode {
            EditMode::Filters => filters::handle_world_outcome(ctx, app, outcome),
            EditMode::FreehandFilters(_) => unreachable!(),
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
        panel: &mut Panel,
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
            "undo" => {
                let prev = app.session.modal_filters.previous_version.take().unwrap();
                app.session.modal_filters = prev;
                after_edit(ctx, app);
                // TODO Ideally, preserve panel state (checkboxes and dropdowns)
                if let EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
                    *maybe_focus = None;
                }
                if let EditMode::FreehandFilters(_) = app.session.edit_mode {
                    app.session.edit_mode = EditMode::Filters;
                }
                EditOutcome::Transition(Transition::Recreate)
            }
            "Plan a route" => EditOutcome::Transition(Transition::Push(
                crate::route_planner::RoutePlanner::new_state(ctx, app),
            )),
            "Modal filter" => {
                app.session.filter_type = FilterType::NoEntry;
                app.session.edit_mode = EditMode::Filters;
                EditOutcome::UpdatePanelAndWorld
            }
            "Walk/cycle only filter" => {
                app.session.filter_type = FilterType::WalkCycleOnly;
                app.session.edit_mode = EditMode::Filters;
                EditOutcome::UpdatePanelAndWorld
            }
            "Bus gate" => {
                app.session.filter_type = FilterType::BusGate;
                app.session.edit_mode = EditMode::Filters;
                EditOutcome::UpdatePanelAndWorld
            }
            "Freehand filters" => {
                app.session.edit_mode = EditMode::FreehandFilters(PolyLineLasso::new());
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
                // Logically we could do UpdatePanelAndWorld, but we need to be more efficient
                if let EditMode::Shortcuts(ref focus) = app.session.edit_mode {
                    let panel_piece = shortcuts::widget(ctx, app, focus.as_ref());
                    panel.replace(ctx, "edit mode contents", panel_piece);
                    self.world = shortcuts::make_world(ctx, app, neighbourhood, focus);
                }
                EditOutcome::Transition(Transition::Keep)
            }
            "next shortcut" => {
                if let EditMode::Shortcuts(Some(ref mut focus)) = app.session.edit_mode {
                    focus.current_idx += 1;
                }
                if let EditMode::Shortcuts(ref focus) = app.session.edit_mode {
                    let panel_piece = shortcuts::widget(ctx, app, focus.as_ref());
                    panel.replace(ctx, "edit mode contents", panel_piece);
                    self.world = shortcuts::make_world(ctx, app, neighbourhood, focus);
                }
                EditOutcome::Transition(Transition::Keep)
            }
            _ => EditOutcome::Nothing,
        }
    }
}

fn edit_mode(ctx: &mut EventCtx, app: &App) -> Widget {
    let edit_mode = &app.session.edit_mode;
    let filter = |ft: FilterType, hide_color: Color, name: &str| {
        let mut btn = ctx
            .style()
            .btn_solid_primary
            .icon(ft.svg_path())
            .image_color(
                RewriteColor::Change(hide_color, Color::CLEAR),
                ControlState::Default,
            )
            .image_color(
                RewriteColor::Change(hide_color, Color::CLEAR),
                ControlState::Disabled,
            )
            .disabled(matches!(edit_mode, EditMode::Filters) && app.session.filter_type == ft);
        if app.session.filter_type == ft {
            btn = btn.hotkey(Key::F1);
        }
        btn.build_widget(ctx, name)
    };

    Widget::row(vec![
        Widget::row(vec![
            filter(FilterType::NoEntry, Color::RED, "Modal filter"),
            filter(
                FilterType::WalkCycleOnly,
                Color::hex("#0b793a"),
                "Walk/cycle only filter",
            ),
            filter(FilterType::BusGate, Color::hex("#0672B9"), "Bus gate"),
        ])
        .section(ctx),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/select.svg")
            .disabled(matches!(edit_mode, EditMode::FreehandFilters(_)))
            .hotkey(Key::F2)
            .build_widget(ctx, "Freehand filters")
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/one_ways.svg")
            .disabled(matches!(edit_mode, EditMode::Oneways))
            .hotkey(Key::F3)
            .build_widget(ctx, "One-ways")
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/shortcut.svg")
            .disabled(matches!(edit_mode, EditMode::Shortcuts(_)))
            .hotkey(Key::F4)
            .build_widget(ctx, "Shortcuts")
            .centered_vert(),
    ])
}
