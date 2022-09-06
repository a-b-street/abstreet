mod filters;
mod freehand_filters;
mod one_ways;
mod shortcuts;

use std::collections::BTreeSet;

use geom::Distance;
use map_gui::tools::grey_out_map;
use map_model::{EditRoad, IntersectionID, Road, RoadID};
use street_network::{Direction, LaneSpec};
use widgetry::mapspace::{ObjectID, World};
use widgetry::tools::{PolyLineLasso, PopupMsg};
use widgetry::{
    lctrl, Color, ControlState, DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel,
    PanelBuilder, RewriteColor, State, Text, TextExt, Widget,
};

use crate::{
    after_edit, colors, is_private, App, BrowseNeighbourhoods, FilterType, Neighbourhood,
    RoadFilter, Transition,
};

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
            app.per_map.alt_proposals.to_widget(ctx, app),
            BrowseNeighbourhoods::button(ctx, app),
            {
                let mut row = Vec::new();
                if app.per_map.consultation.is_none() {
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
                    .disabled(app.per_map.edits.previous_version.is_none())
                    .hotkey(lctrl(Key::Z))
                    .build_widget(ctx, "undo"),
                // TODO Only count new filters, not existing
                format!(
                    "{} filters added, {} road directions changed",
                    app.per_map.edits.roads.len() + app.per_map.edits.intersections.len(),
                    app.per_map.edits.one_ways.len()
                )
                .text_widget(ctx)
                .centered_vert(),
            ]),
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
                one_ways::undo_proposal(ctx, app);
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
            "Modal filter - no entry" => {
                app.session.filter_type = FilterType::NoEntry;
                app.session.edit_mode = EditMode::Filters;
                EditOutcome::UpdatePanelAndWorld
            }
            "Modal filter -- walking/cycling only" => {
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
            filter(
                FilterType::WalkCycleOnly,
                Color::hex("#0b793a"),
                "Modal filter -- walking/cycling only",
            ),
            filter(FilterType::NoEntry, Color::RED, "Modal filter - no entry"),
            filter(FilterType::BusGate, *colors::BUS_ROUTE, "Bus gate"),
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

fn road_name(app: &App, road: &Road) -> String {
    let mut name = road.get_name(app.opts.language.as_ref());
    if name == "???" {
        name = "unnamed road".to_string();
    }
    if is_private(road) {
        format!("{name} (private)")
    } else {
        name
    }
}

struct ResolveOneWayAndFilter {
    panel: Panel,
    roads: Vec<RoadID>,
}

impl ResolveOneWayAndFilter {
    fn new_state(ctx: &mut EventCtx, roads: Vec<RoadID>) -> Box<dyn State<App>> {
        let mut txt = Text::new();
        txt.add_line(Line("Error").small_heading());
        txt.add_line("A one-way street can't have a filter");

        let panel = Panel::new_builder(Widget::col(vec![
            txt.into_widget(ctx),
            Widget::row(vec![
                ctx.style().btn_solid.text("OK, do nothing").build_def(ctx),
                ctx.style()
                    .btn_solid_primary
                    .text(if roads.len() == 1 {
                        "Change to a two-way street and add a filter".to_string()
                    } else {
                        format!(
                            "Change {} one-way streets to two-way and add filters",
                            roads.len()
                        )
                    })
                    .build_def(ctx),
            ]),
        ]))
        .build(ctx);

        Box::new(Self { panel, roads })
    }
}

impl State<App> for ResolveOneWayAndFilter {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            if x == "OK, do nothing" {
                return Transition::Pop;
            }

            let driving_side = app.per_map.map.get_config().driving_side;
            let mut edits = app.per_map.map.get_edits().clone();
            for r in &self.roads {
                edits
                    .commands
                    .push(app.per_map.map.edit_road_cmd(*r, |new| {
                        LaneSpec::toggle_road_direction(&mut new.lanes_ltr, driving_side);
                        // Maybe we just flipped a one-way forwards to a one-way backwards. So one more
                        // time to make it two-way
                        if LaneSpec::oneway_for_driving(&new.lanes_ltr) == Some(Direction::Back) {
                            LaneSpec::toggle_road_direction(&mut new.lanes_ltr, driving_side);
                        }
                    }));
            }
            ctx.loading_screen("apply edits", |_, timer| {
                app.per_map.map.must_apply_edits(edits, timer);
            });

            app.per_map.edits.before_edit();

            for r in &self.roads {
                let r = *r;
                let road = app.per_map.map.get_r(r);
                let r_edit = app.per_map.map.get_r_edit(r);
                if r_edit == EditRoad::get_orig_from_osm(road, app.per_map.map.get_config()) {
                    app.per_map.edits.one_ways.remove(&r);
                } else {
                    app.per_map.edits.one_ways.insert(r, r_edit);
                }

                app.per_map.edits.roads.insert(
                    r,
                    RoadFilter::new_by_user(road.length() / 2.0, app.session.filter_type),
                );
            }

            after_edit(ctx, app);

            return Transition::Multi(vec![Transition::Pop, Transition::Recreate]);
        }
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct ResolveBusGate {
    panel: Panel,
    roads: Vec<(RoadID, Distance)>,
}

impl ResolveBusGate {
    fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        roads: Vec<(RoadID, Distance)>,
    ) -> Box<dyn State<App>> {
        app.session.layers.show_bus_routes(ctx, &app.cs);

        let mut txt = Text::new();
        txt.add_line(Line("Warning").small_heading());
        txt.add_line("A regular modal filter would impact bus routes here.");
        txt.add_line("A bus gate uses signage and camera enforcement to only allow buses");
        txt.add_line("");
        txt.add_line("The following bus routes cross this road:");

        let mut routes = BTreeSet::new();
        for (r, _) in &roads {
            routes.extend(app.per_map.map.get_bus_routes_on_road(*r));
        }
        for route in routes {
            txt.add_line(format!("- {route}"));
        }

        let panel = Panel::new_builder(Widget::col(vec![
            txt.into_widget(ctx),
            Widget::row(vec![
                // TODO Just have pictures?
                ctx.style()
                    .btn_solid
                    .text("Place a regular modal filter here")
                    .build_def(ctx),
                ctx.style()
                    .btn_solid_primary
                    .text("Place bus gates")
                    .build_def(ctx),
            ]),
        ]))
        .build(ctx);

        Box::new(Self { panel, roads })
    }
}

impl State<App> for ResolveBusGate {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            app.per_map.edits.before_edit();
            let filter_type = if x == "Place bus gates" {
                FilterType::BusGate
            } else {
                app.session.filter_type
            };

            for (r, dist) in self.roads.drain(..) {
                app.per_map
                    .edits
                    .roads
                    .insert(r, RoadFilter::new_by_user(dist, filter_type));
            }

            after_edit(ctx, app);

            return Transition::Multi(vec![Transition::Pop, Transition::Recreate]);
        }
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
