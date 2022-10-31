pub mod filters;
pub mod freehand_filters;
pub mod one_ways;
pub mod shortcuts;

use std::collections::BTreeSet;

use geom::{Distance, Polygon};
use map_gui::tools::grey_out_map;
use map_model::{EditRoad, IntersectionID, Road, RoadID};
use osm2streets::{Direction, LaneSpec};
use widgetry::mapspace::{ObjectID, World};
use widgetry::tools::{PolyLineLasso, PopupMsg};
use widgetry::{
    Color, ControlState, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel,
    RewriteColor, State, Text, Texture, Widget,
};

use crate::{
    is_private, mut_edits, redraw_all_filters, App, FilterType, Neighbourhood, RoadFilter,
    Transition,
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
            "Modal filter - no entry" | "Modal filter -- walking/cycling only" | "Bus gate" => {
                app.session.edit_mode = EditMode::Filters;
                EditOutcome::UpdatePanelAndWorld
            }
            "Change modal filter" => {
                EditOutcome::Transition(Transition::Push(ChangeFilterType::new_state(ctx, app)))
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
        txt.add_line(Line("Warning").small_heading());
        txt.add_line("A modal filter cannot be placed on a one-way street.");
        txt.add_line("");
        txt.add_line("You can make the street two-way first, then place a filter.");

        let panel = Panel::new_builder(Widget::col(vec![
            txt.into_widget(ctx),
            Widget::row(vec![
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
                ctx.style().btn_outline.text("Cancel").build_def(ctx),
            ]),
        ]))
        .build(ctx);

        Box::new(Self { panel, roads })
    }
}

impl State<App> for ResolveOneWayAndFilter {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            if x == "Cancel" {
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

            app.per_map.proposals.before_edit();

            for r in &self.roads {
                let r = *r;
                let road = app.per_map.map.get_r(r);
                let r_edit = app.per_map.map.get_r_edit(r);
                if r_edit == EditRoad::get_orig_from_osm(road, app.per_map.map.get_config()) {
                    mut_edits!(app).one_ways.remove(&r);
                } else {
                    mut_edits!(app).one_ways.insert(r, r_edit);
                }

                mut_edits!(app).roads.insert(
                    r,
                    RoadFilter::new_by_user(road.length() / 2.0, app.session.filter_type),
                );
            }

            redraw_all_filters(ctx, app);

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
        // TODO This'll mess up the panel, but we don't have easy access to the panel here
        app.session.layers.show_bus_routes(ctx, &app.cs, None);

        let mut txt = Text::new();
        txt.add_line(Line("Warning").small_heading());
        txt.add_line("The following bus routes cross this road. Adding a regular modal filter would block them.");
        txt.add_line("");

        let mut routes = BTreeSet::new();
        for (r, _) in &roads {
            routes.extend(app.per_map.map.get_bus_routes_on_road(*r));
        }
        for route in routes {
            txt.add_line(format!("- {route}"));
        }

        txt.add_line("");
        txt.add_line("You can use a bus gate instead.");

        let panel = Panel::new_builder(Widget::col(vec![
            txt.into_widget(ctx),
            Widget::row(vec![
                // TODO Just have pictures?
                ctx.style()
                    .btn_solid_primary
                    .text("Place bus gates")
                    .build_def(ctx),
                ctx.style().btn_outline.text("Cancel").build_def(ctx),
            ]),
        ]))
        .build(ctx);

        Box::new(Self { panel, roads })
    }
}

impl State<App> for ResolveBusGate {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            if x == "Place bus gates" {
                app.per_map.proposals.before_edit();
                for (r, dist) in self.roads.drain(..) {
                    mut_edits!(app)
                        .roads
                        .insert(r, RoadFilter::new_by_user(dist, FilterType::BusGate));
                }
                redraw_all_filters(ctx, app);
            }

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

struct ChangeFilterType {
    panel: Panel,
}

impl ChangeFilterType {
    fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let filter = |ft: FilterType, hotkey: Key, name: &str| {
            ctx.style()
                .btn_solid_primary
                .icon_text(ft.svg_path(), name)
                .image_color(
                    RewriteColor::Change(ft.hide_color(), Color::CLEAR),
                    ControlState::Default,
                )
                .image_color(
                    RewriteColor::Change(ft.hide_color(), Color::CLEAR),
                    ControlState::Disabled,
                )
                .disabled(app.session.filter_type == ft)
                .hotkey(hotkey)
                .build_def(ctx)
        };

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Choose a modal filter to place on streets")
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![
                Widget::col(vec![
                    filter(
                        FilterType::WalkCycleOnly,
                        Key::Num1,
                        "Walking/cycling only",
                    ),
                    filter(FilterType::NoEntry, Key::Num2, "No entry"),
                    filter(FilterType::BusGate, Key::Num3, "Bus gate"),
                ]),
                Widget::vertical_separator(ctx),
                Widget::col(vec![
                    GeomBatch::from(vec![
                        (match app.session.filter_type {
                            FilterType::WalkCycleOnly => Texture(1),
                            FilterType::NoEntry => Texture(2),
                            FilterType::BusGate => Texture(3),
                            // The rectangle size must match the base image, otherwise it'll be
                            // repeated (tiled) or cropped -- not scaled.
                        }, Polygon::rectangle(crate::SPRITE_WIDTH as f64, crate::SPRITE_HEIGHT as f64))
                    ]).into_widget(ctx),
                    // TODO Ambulances, etc
                    Text::from(Line(match app.session.filter_type {
                        FilterType::WalkCycleOnly => "A physical barrier that only allows people walking, cycling, and rolling to pass. Often planters or bollards. Larger vehicles cannot enter.",
                        FilterType::NoEntry => "An alternative sign to indicate vehicles are not allowed to enter the street. Only people walking, cycling, and rolling may pass through.",
                        FilterType::BusGate => "A bus gate sign and traffic cameras are installed to allow buses, pedestrians, and cyclists to pass. There is no physical barrier.",
                    })).wrap_to_pixels(ctx, crate::SPRITE_WIDTH as f64).into_widget(ctx),
                ]),
            ]),
            ctx.style().btn_solid_primary.text("OK").hotkey(Key::Enter).build_def(ctx).centered_horiz(),
        ]))
        .build(ctx);
        Box::new(Self { panel })
    }
}

impl State<App> for ChangeFilterType {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            return match x.as_ref() {
                "No entry" => {
                    app.session.filter_type = FilterType::NoEntry;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "Walking/cycling only" => {
                    app.session.filter_type = FilterType::WalkCycleOnly;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "Bus gate" => {
                    app.session.filter_type = FilterType::BusGate;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "close" | "OK" => Transition::Multi(vec![Transition::Pop, Transition::Recreate]),
                _ => unreachable!(),
            };
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
