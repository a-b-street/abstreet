use geom::Distance;
use map_gui::tools::{CityPicker, DrawRoadLabels};
use map_model::{Block, RoadID};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State,
    VerticalAlignment, Widget,
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
            Widget::row(vec![
                Line("LTN tool").small_heading().into_widget(ctx),
                map_gui::tools::change_map_btn(ctx, app)
                    .centered_vert()
                    .align_right(),
            ]),
            ctx.style()
                .btn_outline
                .text("Browse neighborhoods")
                .hotkey(Key::B)
                .build_def(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        let neighborhood = Neighborhood::new(ctx, app, block.perimeter.clone());

        let mut label_roads = neighborhood.perimeter.clone();
        label_roads.extend(neighborhood.orig_perimeter.interior.clone());

        let world = make_world(ctx, app, &neighborhood);

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
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
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
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::ClickedObject(Obj::InteriorRoad(r)) => {
                if app.session.modal_filters.contains(&r) {
                    app.session.modal_filters.remove(&r);
                } else {
                    app.session.modal_filters.insert(r);
                }
                // TODO The cell coloring changes quite spuriously just by toggling a filter, even
                // when it doesn't matter
                self.neighborhood =
                    Neighborhood::new(ctx, app, self.neighborhood.orig_perimeter.clone());
                self.world = make_world(ctx, app, &self.neighborhood);
            }
            _ => {}
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

const COLORS: [Color; 6] = [
    Color::BLUE,
    Color::YELLOW,
    Color::GREEN,
    Color::PURPLE,
    Color::PINK,
    Color::ORANGE,
];

fn make_world(ctx: &mut EventCtx, app: &App, neighborhood: &Neighborhood) -> World<Obj> {
    let map = &app.primary.map;
    let mut world = World::bounded(map.get_bounds());
    let mut draw_intersections = GeomBatch::new();

    for (idx, cells) in neighborhood.cells.iter().enumerate() {
        // TODO It'd be great to use calculate_coloring!
        let color = COLORS[idx % COLORS.len()].alpha(0.9);
        for r in cells {
            world
                .add(Obj::InteriorRoad(*r))
                .hitbox(map.get_r(*r).get_thick_polygon())
                .draw_color(color)
                .hover_outline(Color::BLACK, Distance::meters(5.0))
                .clickable()
                .build(ctx);
        }

        for i in crate::common::intersections_from_roads(cells, map) {
            draw_intersections.push(color, map.get_i(i).polygon.clone());
        }
    }
    world.draw_master_batch(ctx, draw_intersections);
    world.initialize_hover(ctx);

    world
}
