use geom::Distance;
use map_gui::tools::CityPicker;
use map_model::{IntersectionID, PathConstraints, RoadID};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, HorizontalAlignment, Key, Panel, PanelBuilder, State, TextExt,
    VerticalAlignment, Widget, DEFAULT_CORNER_RADIUS,
};

use super::{BrowseNeighborhoods, DiagonalFilter, Neighborhood};
use crate::app::{App, Transition};

#[derive(PartialEq)]
pub enum Tab {
    Connectivity,
    RatRuns,
    Pathfinding,
}

pub trait TakeNeighborhood {
    fn take_neighborhood(self) -> Neighborhood;
}

impl Tab {
    pub fn panel_builder(
        self,
        ctx: &mut EventCtx,
        app: &App,
        per_tab_contents: Widget,
    ) -> PanelBuilder {
        Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                ctx.style()
                    .btn_outline
                    .text("Browse neighborhoods")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
                ctx.style()
                    .btn_outline
                    .text("Adjust boundary")
                    .hotkey(Key::B)
                    .build_def(ctx),
            ]),
            self.make_buttons(ctx),
            "Click a road or intersection to add or remove a modal filter".text_widget(ctx),
            per_tab_contents.tab_body(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    }

    pub fn handle_action<T: TakeNeighborhood + State<App>>(
        self,
        ctx: &mut EventCtx,
        app: &mut App,
        action: &str,
    ) -> Option<Transition> {
        Some(match action {
            "Home" => Transition::Clear(vec![crate::pregame::TitleScreen::new_state(ctx, app)]),
            "change map" => Transition::Push(CityPicker::new_state(
                ctx,
                app,
                Box::new(|ctx, app| Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))),
            )),
            "Browse neighborhoods" => Transition::Pop,
            "Adjust boundary" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![super::select_boundary::SelectBoundary::new_state(
                    ctx,
                    app,
                    Some(state.take_neighborhood().orig_perimeter),
                )]
            })),
            "Connectivity" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![super::connectivity::Viewer::new_state(
                    ctx,
                    app,
                    state.take_neighborhood(),
                )]
            })),
            "Rat runs" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![super::rat_run_viewer::BrowseRatRuns::new_state(
                    ctx,
                    app,
                    state.take_neighborhood(),
                )]
            })),
            "Pathfinding" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![super::pathfinding::RoutePlanner::new_state(
                    ctx,
                    app,
                    state.take_neighborhood(),
                )]
            })),
            _ => {
                return None;
            }
        })
    }

    fn make_buttons(self, ctx: &mut EventCtx) -> Widget {
        let mut row = Vec::new();
        for (tab, label, key) in [
            (Tab::Connectivity, "Connectivity", Key::Num1),
            (Tab::RatRuns, "Rat runs", Key::Num2),
            (Tab::Pathfinding, "Pathfinding", Key::Num3),
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
        Widget::row(row)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilterableObj {
    InteriorRoad(RoadID),
    InteriorIntersection(IntersectionID),
}
impl ObjectID for FilterableObj {}

/// Adds clickable objects for managing filters on roads and intersections. The caller is
/// responsible for base drawing behavior, initialize_hover, etc.
pub fn populate_world<T: ObjectID, F: Fn(FilterableObj) -> T>(
    ctx: &mut EventCtx,
    app: &App,
    neighborhood: &Neighborhood,
    world: &mut World<T>,
    wrap_id: F,
    zorder: usize,
) {
    let map = &app.primary.map;

    for r in &neighborhood.orig_perimeter.interior {
        world
            .add(wrap_id(FilterableObj::InteriorRoad(*r)))
            .hitbox(map.get_r(*r).get_thick_polygon())
            .zorder(zorder)
            .drawn_in_master_batch()
            .hover_outline(Color::BLACK, Distance::meters(5.0))
            .clickable()
            .build(ctx);
    }

    for i in &neighborhood.interior_intersections {
        world
            .add(wrap_id(FilterableObj::InteriorIntersection(*i)))
            .hitbox(map.get_i(*i).polygon.clone())
            .zorder(zorder)
            .drawn_in_master_batch()
            .hover_outline(Color::BLACK, Distance::meters(5.0))
            .clickable()
            .build(ctx);
    }
}

/// If true, the neighborhood has changed and the caller should recalculate stuff
pub fn handle_world_outcome(
    ctx: &mut EventCtx,
    app: &mut App,
    outcome: WorldOutcome<FilterableObj>,
) -> bool {
    let map = &app.primary.map;
    match outcome {
        WorldOutcome::ClickedObject(FilterableObj::InteriorRoad(r)) => {
            let road = map.get_r(r);
            // Filtering on a road that's already marked bike-only doesn't make sense
            if !PathConstraints::Car.can_use_road(road, map) {
                return true;
            }

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
            true
        }
        WorldOutcome::ClickedObject(FilterableObj::InteriorIntersection(i)) => {
            if map.get_i(i).roads.len() != 4 {
                // Misleading. Nothing changes, but we'll "fall through" to other cases without
                // this
                return true;
            }

            // Toggle through all possible filters
            let mut all = DiagonalFilter::filters_for(app, i);
            if let Some(current) = app.session.modal_filters.intersections.get(&i) {
                let idx = all.iter().position(|x| x == current).unwrap();
                if idx == all.len() - 1 {
                    app.session.modal_filters.intersections.remove(&i);
                } else {
                    app.session
                        .modal_filters
                        .intersections
                        .insert(i, all.remove(idx + 1));
                }
            } else if !all.is_empty() {
                app.session
                    .modal_filters
                    .intersections
                    .insert(i, all.remove(0));
            }
            true
        }
        _ => false,
    }
}
