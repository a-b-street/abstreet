use std::collections::HashSet;

use geom::Distance;
use map_gui::tools::{CityPicker, DrawRoadLabels};
use map_model::{Block, RoadID};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, TextExt,
    Toggle, VerticalAlignment, Widget,
};

use super::{BrowseNeighborhoods, Neighborhood};
use crate::app::{App, Transition};

pub struct Viewer {
    panel: Panel,
    neighborhood: Neighborhood,
    world: World<Obj>,
    labels: DrawRoadLabels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    InteriorRoad(RoadID),
}
impl ObjectID for Obj {}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App, block: &Block) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            ctx.style()
                .btn_outline
                .text("Browse neighborhoods")
                .hotkey(Key::Escape)
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Browse rat-runs")
                .hotkey(Key::R)
                .build_def(ctx),
            Widget::row(vec![
                "Draw traffic cells as".text_widget(ctx).centered_vert(),
                Toggle::choice(ctx, "draw cells", "areas", "streets", Key::C, true),
            ]),
            "Click a road to add or remove a modal filter".text_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        let neighborhood = Neighborhood::new(ctx, app, block.perimeter.clone());

        let mut label_roads = neighborhood.perimeter.clone();
        label_roads.extend(neighborhood.orig_perimeter.interior.clone());

        let world = make_world(ctx, app, &neighborhood, panel.is_checked("draw cells"));

        Box::new(Viewer {
            panel,
            neighborhood,
            world,
            labels: DrawRoadLabels::new(Box::new(move |r| label_roads.contains(&r.id))),
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
                "Browse rat-runs" => {
                    return Transition::Push(super::rat_run_viewer::BrowseRatRuns::new_state(
                        ctx,
                        app,
                        &self.neighborhood,
                    ));
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

        if let WorldOutcome::ClickedObject(Obj::InteriorRoad(r)) = self.world.event(ctx) {
            if app.session.modal_filters.roads.contains_key(&r) {
                app.session.modal_filters.roads.remove(&r);
            } else {
                let road = app.primary.map.get_r(r);
                // If this road touches a border, place it closer to that intersection. If it's an
                // inner neighborhood split, then stick to the middle of that road. If it touches
                // two borders, also choose the middle.
                let near_start = self.neighborhood.borders.contains(&road.src_i);
                let near_end = self.neighborhood.borders.contains(&road.dst_i);
                let pct_along = if near_start && !near_end {
                    0.1
                } else if near_end && !near_start {
                    0.9
                } else {
                    0.5
                };
                app.session
                    .modal_filters
                    .roads
                    .insert(r, pct_along * road.length());
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

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        g.redraw(&self.neighborhood.fade_irrelevant);
        self.world.draw(g);
        g.redraw(&self.neighborhood.draw_filters);
        // TODO Since we cover such a small area, treating multiple segments of one road as the
        // same might be nice. And we should seed the quadtree with the locations of filters and
        // arrows, possibly.
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
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

    world.initialize_hover(ctx);

    world
}
