use std::collections::BTreeMap;

use abstutil::Timer;
use geom::Distance;
use map_gui::tools::{CityPicker, DrawRoadLabels, Navigator, URLManager};
use map_model::osm::RoadRank;
use map_model::{Block, Perimeter};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

use super::Neighborhood;
use crate::app::{App, Transition};
use crate::ltn::select_boundary::SelectBoundary;

const COLORS: [Color; 6] = [
    Color::BLUE,
    Color::YELLOW,
    Color::GREEN,
    Color::PURPLE,
    Color::PINK,
    Color::ORANGE,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Obj(usize);
impl ObjectID for Obj {}

pub struct BrowseNeighborhoods {
    panel: Panel,
    neighborhoods: BTreeMap<Obj, Block>,
    world: World<Obj>,
    labels: DrawRoadLabels,
}

impl BrowseNeighborhoods {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        URLManager::update_url_map_name(app);

        let (neighborhoods, world) = ctx.loading_screen("calculate neighborhoods", |ctx, timer| {
            detect_neighborhoods(ctx, app, timer)
        });

        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Click a neighborhood".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/search.svg")
                    .hotkey(Key::K)
                    .build_widget(ctx, "search")
                    .align_right(),
            ]),
            // TODO Can customize later, these boundaries are just initial suggestions, see here
            // how they're found...
            ctx.style()
                .btn_outline
                .text("Draw custom boundary")
                .hotkey(Key::B)
                .build_def(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        Box::new(BrowseNeighborhoods {
            panel,
            neighborhoods,
            world,
            labels: DrawRoadLabels::only_major_roads(),
        })
    }
}

impl State<App> for BrowseNeighborhoods {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Home" => {
                    return Transition::Pop;
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
                "Draw custom boundary" => {
                    return Transition::Push(SelectBoundary::new_state(ctx, app, None));
                }
                "search" => {
                    return Transition::Push(Navigator::new_state(ctx, app));
                }
                _ => unreachable!(),
            }
        }

        if let WorldOutcome::ClickedObject(id) = self.world.event(ctx) {
            return Transition::Push(super::connectivity::Viewer::new_state(
                ctx,
                app,
                Neighborhood::new(ctx, app, self.neighborhoods[&id].perimeter.clone()),
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        self.world.draw(g);
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
        }
    }
}

fn detect_neighborhoods(
    ctx: &mut EventCtx,
    app: &App,
    timer: &mut Timer,
) -> (BTreeMap<Obj, Block>, World<Obj>) {
    timer.start("find single blocks");
    let mut single_blocks = Perimeter::find_all_single_blocks(&app.primary.map);
    // TODO Ew! Expensive! But the merged neighborhoods differ widely from blockfinder if we don't.
    single_blocks.retain(|x| x.clone().to_block(&app.primary.map).is_ok());
    timer.stop("find single blocks");

    timer.start("partition");
    let partitions = Perimeter::partition_by_predicate(single_blocks, |r| {
        // "Interior" roads of a neighborhood aren't classified as arterial
        let road = app.primary.map.get_r(r);
        road.get_rank() == RoadRank::Local
    });

    let mut merged = Vec::new();
    for perimeters in partitions {
        // If we got more than one result back, merging partially failed. Oh well?
        merged.extend(Perimeter::merge_all(perimeters, false));
    }

    let mut colors = Perimeter::calculate_coloring(&merged, COLORS.len())
        .unwrap_or_else(|| (0..merged.len()).collect());
    timer.stop("partition");

    timer.start_iter("blockify", merged.len());
    let mut blocks = Vec::new();
    for perimeter in merged {
        timer.next();
        match perimeter.to_block(&app.primary.map) {
            Ok(block) => {
                blocks.push(block);
            }
            Err(err) => {
                warn!("Failed to make a block from a perimeter: {}", err);
                // We assigned a color, so don't let the indices get out of sync!
                colors.remove(blocks.len());
            }
        }
    }

    let mut world = World::bounded(app.primary.map.get_bounds());
    let mut neighborhoods = BTreeMap::new();
    for (block, color_idx) in blocks.into_iter().zip(colors.into_iter()) {
        let id = Obj(neighborhoods.len());
        let color = COLORS[color_idx % COLORS.len()];
        world
            .add(id)
            .hitbox(block.polygon.clone())
            .draw_color(color.alpha(0.5))
            .hover_outline(Color::BLACK, Distance::meters(5.0))
            .clickable()
            .build(ctx);
        neighborhoods.insert(id, block);
    }
    (neighborhoods, world)
}
