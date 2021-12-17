use std::collections::HashSet;

use geom::Distance;
use map_gui::tools::CityPicker;
use map_model::{IntersectionID, RoadID};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, TextExt,
    Toggle, VerticalAlignment, Widget,
};

use super::{BrowseNeighborhoods, DiagonalFilter, Neighborhood};
use crate::app::{App, Transition};

pub struct Viewer {
    panel: Panel,
    neighborhood: Neighborhood,
    world: World<Obj>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    InteriorRoad(RoadID),
    InteriorIntersection(IntersectionID),
}
impl ObjectID for Obj {}

impl Viewer {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        neighborhood: Neighborhood,
    ) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
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
            ctx.style()
                .btn_outline
                .text("Browse rat-runs")
                .hotkey(Key::R)
                .disabled(true)
                .disabled_tooltip("Still being prototyped")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Pathfind")
                .hotkey(Key::P)
                .build_def(ctx),
            Widget::row(vec![
                "Draw traffic cells as".text_widget(ctx).centered_vert(),
                Toggle::choice(ctx, "draw cells", "areas", "streets", Key::C, true),
            ]),
            "Click a road to add or remove a modal filter".text_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        let world = make_world(ctx, app, &neighborhood, panel.is_checked("draw cells"));

        Box::new(Viewer {
            panel,
            neighborhood,
            world,
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Home" => {
                    return Transition::Clear(vec![crate::pregame::TitleScreen::new_state(
                        ctx, app,
                    )]);
                }
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))
                        }),
                    ));
                }
                "Browse neighborhoods" => {
                    return Transition::Pop;
                }
                "Adjust boundary" => {
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let perimeter = state
                            .downcast::<Viewer>()
                            .ok()
                            .unwrap()
                            .neighborhood
                            .orig_perimeter;
                        vec![super::select_boundary::SelectBoundary::new_state(
                            ctx,
                            app,
                            Some(perimeter),
                        )]
                    }));
                }
                "Browse rat-runs" => {
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let state = state.downcast::<Viewer>().ok().unwrap();
                        vec![super::rat_run_viewer::BrowseRatRuns::new_state(
                            ctx,
                            app,
                            state.neighborhood,
                        )]
                    }));
                }
                "Pathfind" => {
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let state = state.downcast::<Viewer>().ok().unwrap();
                        vec![super::route::RoutePlanner::new_state(
                            ctx,
                            app,
                            state.neighborhood,
                        )]
                    }));
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                self.world = make_world(
                    ctx,
                    app,
                    &self.neighborhood,
                    self.panel.is_checked("draw cells"),
                );
            }
            _ => {}
        }

        match self.world.event(ctx) {
            WorldOutcome::ClickedObject(Obj::InteriorRoad(r)) => {
                if app.session.modal_filters.roads.remove(&r).is_none() {
                    // Place the filter on the part of the road that was clicked
                    let road = app.primary.map.get_r(r);
                    // These calls shouldn't fail -- since we clicked a road, the cursor must be in
                    // map-space. And project_pt returns a point that's guaranteed to be on the
                    // polyline.
                    let cursor_pt = ctx.canvas.get_cursor_in_map_space().unwrap();
                    let pt_on_line = road.center_pts.project_pt(cursor_pt);
                    let (distance, _) = road.center_pts.dist_along_of_point(pt_on_line).unwrap();

                    app.session.modal_filters.roads.insert(r, distance);
                }
                // TODO The cell coloring changes quite spuriously just by toggling a filter, even
                // when it doesn't matter
                self.neighborhood =
                    Neighborhood::new(ctx, app, self.neighborhood.orig_perimeter.clone());
                self.world = make_world(
                    ctx,
                    app,
                    &self.neighborhood,
                    self.panel.is_checked("draw cells"),
                );
            }
            WorldOutcome::ClickedObject(Obj::InteriorIntersection(i)) => {
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

                self.neighborhood =
                    Neighborhood::new(ctx, app, self.neighborhood.orig_perimeter.clone());
                self.world = make_world(
                    ctx,
                    app,
                    &self.neighborhood,
                    self.panel.is_checked("draw cells"),
                );
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        g.redraw(&self.neighborhood.fade_irrelevant);
        self.world.draw(g);
        self.neighborhood.draw_filters.draw(g);
        // TODO Since we cover such a small area, treating multiple segments of one road as the
        // same might be nice. And we should seed the quadtree with the locations of filters and
        // arrows, possibly.
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }
    }
}

fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighborhood: &Neighborhood,
    draw_cells_as_areas: bool,
) -> World<Obj> {
    let map = &app.primary.map;
    let mut world = World::bounded(map.get_bounds());

    // Could refactor this, but I suspect we'll settle on one drawing style or another. Toggling
    // between the two is temporary.
    if draw_cells_as_areas {
        for r in &neighborhood.orig_perimeter.interior {
            world
                .add(Obj::InteriorRoad(*r))
                .hitbox(map.get_r(*r).get_thick_polygon())
                .drawn_in_master_batch()
                .hover_outline(Color::BLACK, Distance::meters(5.0))
                .clickable()
                .build(ctx);
        }

        world.draw_master_batch(ctx, super::draw_cells::draw_cells(map, neighborhood));
    } else {
        let mut draw_intersections = GeomBatch::new();
        let mut seen_roads = HashSet::new();
        for (idx, cell) in neighborhood.cells.iter().enumerate() {
            let color = super::draw_cells::COLORS[idx % super::draw_cells::COLORS.len()].alpha(0.9);
            for r in cell.roads.keys() {
                // TODO Roads with a filter belong to two cells. Avoid adding them to the world
                // twice. But the drawn form (and the intersections included) needs to be adjusted
                // to use two colors.
                if seen_roads.contains(r) {
                    continue;
                }
                seen_roads.insert(*r);

                world
                    .add(Obj::InteriorRoad(*r))
                    .hitbox(map.get_r(*r).get_thick_polygon())
                    .draw_color(color)
                    .hover_outline(Color::BLACK, Distance::meters(5.0))
                    .clickable()
                    .build(ctx);
            }
            for i in
                crate::common::intersections_from_roads(&cell.roads.keys().cloned().collect(), map)
            {
                draw_intersections.push(color, map.get_i(i).polygon.clone());
            }
        }
        world.draw_master_batch(ctx, draw_intersections);
    }

    for i in &neighborhood.interior_intersections {
        world
            .add(Obj::InteriorIntersection(*i))
            .hitbox(map.get_i(*i).polygon.clone())
            .drawn_in_master_batch()
            .hover_outline(Color::BLACK, Distance::meters(5.0))
            .clickable()
            .build(ctx);
    }

    world.initialize_hover(ctx);

    world
}
