use std::collections::BTreeSet;

use geom::{Distance, PolyLine};
use map_model::{IntersectionID, PathConstraints, Perimeter, RoadID};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::{open_browser, PolyLineLasso};
use widgetry::{
    lctrl, DrawBaselayer, EventCtx, GfxCtx, Image, Key, Line, Panel, PanelBuilder, ScreenPt, State,
    Text, TextExt, Widget, DEFAULT_CORNER_RADIUS,
};

use crate::shortcuts::Shortcuts;
use crate::{
    after_edit, colors, App, BrowseNeighborhoods, DiagonalFilter, Neighborhood, NeighborhoodID,
    Transition,
};

#[derive(PartialEq)]
pub enum Tab {
    Connectivity,
    Shortcuts,
}

impl Tab {
    pub fn panel_builder(
        self,
        ctx: &mut EventCtx,
        app: &App,
        top_panel: &Panel,
        per_tab_contents: Widget,
    ) -> PanelBuilder {
        let contents = Widget::col(vec![
            app.session.alt_proposals.to_widget(ctx, app),
            ctx.style()
                .btn_back("Browse neighborhoods")
                .hotkey(Key::Escape)
                .build_def(ctx),
            Line("Editing neighborhood")
                .small_heading()
                .into_widget(ctx),
            Widget::col(vec![
                Widget::row(vec![
                    Image::from_path("system/assets/tools/pencil.svg")
                        .into_widget(ctx)
                        .centered_vert(),
                    Text::from(Line(
                        "Click a road or intersection to add or remove a modal filter",
                    ))
                    .wrap_to_pct(ctx, 15)
                    .into_widget(ctx),
                ]),
                ctx.style()
                    .btn_outline
                    .icon_text(
                        "system/assets/tools/select.svg",
                        "Create filters along a shape",
                    )
                    .hotkey(Key::F)
                    .build_def(ctx),
                Widget::row(vec![
                    format!(
                        "{} filters added",
                        app.session.modal_filters.roads.len()
                            + app.session.modal_filters.intersections.len()
                    )
                    .text_widget(ctx)
                    .centered_vert(),
                    ctx.style()
                        .btn_plain
                        .icon("system/assets/tools/undo.svg")
                        .disabled(app.session.modal_filters.previous_version.is_none())
                        .hotkey(lctrl(Key::Z))
                        .build_widget(ctx, "undo"),
                ]),
            ])
            .section(ctx),
            self.make_buttons(ctx),
            per_tab_contents,
        ]);
        crate::common::left_panel_builder(ctx, top_panel, contents)
    }

    pub fn handle_action(
        self,
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
            "Create filters along a shape" => Some(Transition::Push(FreehandFilters::new_state(
                ctx,
                neighborhood,
                panel.center_of("Create filters along a shape"),
                self,
            ))),
            "undo" => {
                let prev = app.session.modal_filters.previous_version.take().unwrap();
                app.session.modal_filters = prev;
                after_edit(ctx, app);
                // Recreate the current state. This will reset any panel state (checkboxes and
                // dropdowns)
                Some(Transition::Replace(match self {
                    Tab::Connectivity => crate::connectivity::Viewer::new_state(ctx, app, id),
                    // TODO Preserve the current shortcut
                    Tab::Shortcuts => {
                        crate::shortcut_viewer::BrowseShortcuts::new_state(ctx, app, id, None)
                    }
                }))
            }
            _ => None,
        }
    }

    fn make_buttons(self, ctx: &mut EventCtx) -> Widget {
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

        Widget::row(row)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilterableObj {
    InteriorRoad(RoadID),
    InteriorIntersection(IntersectionID),
}
impl ObjectID for FilterableObj {}

/// Creates clickable objects for managing filters on roads and intersections. Everything is
/// invisible; the caller is responsible for drawing things.
pub fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighborhood: &Neighborhood,
    shortcuts: &Shortcuts,
) -> World<FilterableObj> {
    let map = &app.map;
    let mut world = World::bounded(map.get_bounds());

    for r in &neighborhood.orig_perimeter.interior {
        let road = map.get_r(*r);
        world
            .add(FilterableObj::InteriorRoad(*r))
            .hitbox(road.get_thick_polygon())
            .drawn_in_master_batch()
            .hover_outline(colors::OUTLINE, Distance::meters(5.0))
            .tooltip(Text::from(format!(
                "{} shortcuts cross {}",
                shortcuts.count_per_road.get(*r),
                road.get_name(app.opts.language.as_ref()),
            )))
            .hotkey(lctrl(Key::D), "debug")
            .clickable()
            .build(ctx);
    }

    for i in &neighborhood.interior_intersections {
        world
            .add(FilterableObj::InteriorIntersection(*i))
            .hitbox(map.get_i(*i).polygon.clone())
            .drawn_in_master_batch()
            .hover_outline(colors::OUTLINE, Distance::meters(5.0))
            .tooltip(Text::from(format!(
                "{} shortcuts cross this intersection",
                shortcuts.count_per_intersection.get(*i)
            )))
            .clickable()
            .hotkey(lctrl(Key::D), "debug")
            .build(ctx);
    }

    world.initialize_hover(ctx);
    world
}

/// If true, the neighborhood has changed and the caller should recalculate stuff, including the
/// panel
pub fn handle_world_outcome(
    ctx: &mut EventCtx,
    app: &mut App,
    outcome: WorldOutcome<FilterableObj>,
) -> bool {
    let map = &app.map;
    match outcome {
        WorldOutcome::ClickedObject(FilterableObj::InteriorRoad(r)) => {
            let road = map.get_r(r);
            // Filtering on a road that's already marked bike-only doesn't make sense
            if !PathConstraints::Car.can_use_road(road, map) {
                return true;
            }

            app.session.modal_filters.before_edit();
            if app.session.modal_filters.roads.remove(&r).is_none() {
                // Place the filter on the part of the road that was clicked
                // These calls shouldn't fail -- since we clicked a road, the cursor must be in
                // map-space. And project_pt returns a point that's guaranteed to be on the
                // polyline.
                let cursor_pt = ctx.canvas.get_cursor_in_map_space().unwrap();
                let pt_on_line = road.center_pts.project_pt(cursor_pt);
                let (distance, _) = road.center_pts.dist_along_of_point(pt_on_line).unwrap();

                app.session.modal_filters.roads.insert(r, distance);
            }
            after_edit(ctx, app);
            true
        }
        WorldOutcome::ClickedObject(FilterableObj::InteriorIntersection(i)) => {
            DiagonalFilter::cycle_through_alternatives(ctx, app, i);
            true
        }
        WorldOutcome::Keypress("debug", FilterableObj::InteriorIntersection(i)) => {
            open_browser(app.map.get_i(i).orig_id.to_string());
            false
        }
        WorldOutcome::Keypress("debug", FilterableObj::InteriorRoad(r)) => {
            open_browser(app.map.get_r(r).orig_id.osm_way_id.to_string());
            false
        }
        _ => false,
    }
}

struct FreehandFilters {
    lasso: PolyLineLasso,
    id: NeighborhoodID,
    perimeter: Perimeter,
    interior_intersections: BTreeSet<IntersectionID>,
    instructions: Text,
    instructions_at: ScreenPt,
    tab: Tab,
}

impl FreehandFilters {
    fn new_state(
        ctx: &EventCtx,
        neighborhood: &Neighborhood,
        instructions_at: ScreenPt,
        tab: Tab,
    ) -> Box<dyn State<App>> {
        Box::new(Self {
            lasso: PolyLineLasso::new(),
            id: neighborhood.id,
            perimeter: neighborhood.orig_perimeter.clone(),
            interior_intersections: neighborhood.interior_intersections.clone(),
            instructions_at,
            instructions: Text::from_all(vec![
                Line("Click and drag").fg(ctx.style().text_hotkey_color),
                Line(" across the roads you want to filter"),
            ]),
            tab,
        })
    }

    fn make_filters_along_path(&self, ctx: &mut EventCtx, app: &mut App, path: PolyLine) {
        app.session.modal_filters.before_edit();
        for r in &self.perimeter.interior {
            if app.session.modal_filters.roads.contains_key(r) {
                continue;
            }
            let road = app.map.get_r(*r);
            if let Some((pt, _)) = road.center_pts.intersection(&path) {
                let dist = road
                    .center_pts
                    .dist_along_of_point(pt)
                    .map(|pair| pair.0)
                    .unwrap_or(road.center_pts.length() / 2.0);
                app.session.modal_filters.roads.insert(*r, dist);
            }
        }
        for i in &self.interior_intersections {
            if app.map.get_i(*i).polygon.intersects_polyline(&path) {
                // We probably won't guess the right one, but make an attempt
                DiagonalFilter::cycle_through_alternatives(ctx, app, *i);
            }
        }
        after_edit(ctx, app);
    }
}

impl State<App> for FreehandFilters {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(pl) = self.lasso.event(ctx) {
            self.make_filters_along_path(ctx, app, pl);
            return Transition::Multi(vec![
                Transition::Pop,
                Transition::Replace(match self.tab {
                    Tab::Connectivity => crate::connectivity::Viewer::new_state(ctx, app, self.id),
                    // TODO Preserve the current shortcut
                    Tab::Shortcuts => {
                        crate::shortcut_viewer::BrowseShortcuts::new_state(ctx, app, self.id, None)
                    }
                }),
            ]);
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.lasso.draw(g);
        // Hacky, but just draw instructions over the other panel
        g.draw_tooltip_at(self.instructions.clone(), self.instructions_at);
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }
}
