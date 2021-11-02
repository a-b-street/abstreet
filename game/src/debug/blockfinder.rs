use std::collections::{BTreeMap, BTreeSet};

use abstutil::Timer;
use geom::Distance;
use map_model::osm::RoadRank;
use map_model::{Block, PathConstraints, Perimeter};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, SimpleState, State,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::debug::polygons;

const COLORS: [Color; 6] = [
    Color::BLUE,
    Color::YELLOW,
    Color::GREEN,
    Color::PURPLE,
    Color::PINK,
    Color::ORANGE,
];
const MODIFIED: Color = Color::RED;
const TO_MERGE: Color = Color::CYAN;

pub struct Blockfinder {
    panel: Panel,
    id_counter: usize,
    blocks: BTreeMap<Obj, Block>,
    world: World<Obj>,
    to_merge: BTreeSet<Obj>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Obj(usize);
impl ObjectID for Obj {}

impl Blockfinder {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Blockfinder {
            panel: make_panel(ctx),
            id_counter: 0,
            blocks: BTreeMap::new(),
            world: World::bounded(app.primary.map.get_bounds()),
            to_merge: BTreeSet::new(),
        };

        ctx.loading_screen("calculate all blocks", |ctx, timer| {
            timer.start("find single blocks");
            let perimeters = Perimeter::find_all_single_blocks(&app.primary.map);
            timer.stop("find single blocks");
            state.add_blocks_with_coloring(ctx, app, perimeters, timer);
        });
        state.world.initialize_hover(ctx);
        Box::new(state)
    }

    fn new_id(&mut self) -> Obj {
        let id = Obj(self.id_counter);
        self.id_counter += 1;
        id
    }

    fn add_block(&mut self, ctx: &mut EventCtx, id: Obj, color: Color, block: Block) {
        let mut obj = self
            .world
            .add(id)
            .hitbox(block.polygon.clone())
            .draw_color(color.alpha(0.5))
            .hover_outline(Color::BLACK, Distance::meters(5.0))
            .clickable();
        if self.to_merge.contains(&id) {
            obj = obj.hotkey(Key::Space, "remove from merge set")
        } else {
            obj = obj.hotkey(Key::Space, "add to merge set")
        }
        obj.build(ctx);
        self.blocks.insert(id, block);
    }

    fn add_blocks_with_coloring(
        &mut self,
        ctx: &mut EventCtx,
        app: &App,
        perimeters: Vec<Perimeter>,
        timer: &mut Timer,
    ) {
        let mut colors = Perimeter::calculate_coloring(&perimeters, COLORS.len())
            .unwrap_or_else(|| (0..perimeters.len()).collect());

        timer.start_iter("blockify", perimeters.len());
        let mut blocks = Vec::new();
        for perimeter in perimeters {
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

        for (block, color_idx) in blocks.into_iter().zip(colors.into_iter()) {
            let id = self.new_id();
            self.add_block(ctx, id, COLORS[color_idx % COLORS.len()], block);
        }
    }
}

impl State<App> for Blockfinder {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Merge" => {
                    // TODO We could update the panel, but meh
                    let mut perimeters = Vec::new();
                    for id in std::mem::take(&mut self.to_merge) {
                        perimeters.push(self.blocks.remove(&id).unwrap().perimeter);
                        // TODO If we happen to be hovering on one, uh oh! It's going to change
                        // ID...
                        self.world.delete(id);
                    }
                    let results = Perimeter::merge_all(perimeters, true);
                    let debug = results.len() > 1;
                    for perimeter in results {
                        let id = self.new_id();
                        let block = perimeter
                            .to_block(&app.primary.map)
                            .expect("Merged perimeter broke the polygon");
                        // To make the one-merge-at-a-time debugging easier, keep these in the
                        // merge set
                        if debug {
                            self.to_merge.insert(id);
                            self.add_block(ctx, id, TO_MERGE, block);
                        } else {
                            self.add_block(ctx, id, MODIFIED, block);
                        }
                    }
                    return Transition::Keep;
                }
                "Collapse dead-ends" => {
                    for id in std::mem::take(&mut self.to_merge) {
                        let mut perimeter = self.blocks.remove(&id).unwrap().perimeter;
                        perimeter.collapse_deadends();
                        let block = perimeter
                            .to_block(&app.primary.map)
                            .expect("collapsing deadends broke the polygon shape");
                        self.world.delete_before_replacement(id);
                        // We'll lose the original coloring, oh well
                        self.add_block(ctx, id, MODIFIED, block);
                    }
                }
                "Classify neighborhoods" | "Auto-merge all neighborhoods" => {
                    let perimeters: Vec<Perimeter> = std::mem::take(&mut self.blocks)
                        .into_iter()
                        .map(|(_, b)| b.perimeter)
                        .collect();
                    let map = &app.primary.map;
                    let partitions = Perimeter::partition_by_predicate(perimeters, |r| {
                        // "Interior" roads of a neighborhood aren't classified as arterial and are
                        // driveable (so existing bike-only connections induce a split)
                        let road = map.get_r(r);
                        road.get_rank() == RoadRank::Local
                            && road
                                .lanes
                                .iter()
                                .any(|l| PathConstraints::Car.can_use(l, map))
                    });

                    // Reset pretty much all of our state
                    self.id_counter = 0;
                    self.world = World::bounded(app.primary.map.get_bounds());
                    self.to_merge.clear();

                    if x == "Auto-merge all neighborhoods" {
                        // Actually merge the partitions
                        let mut merged = Vec::new();
                        for perimeters in partitions {
                            // If we got more than one result back, merging partially failed. Oh
                            // well?
                            merged.extend(Perimeter::merge_all(perimeters, false));
                        }
                        self.add_blocks_with_coloring(ctx, app, merged, &mut Timer::throwaway());
                    } else {
                        // Until we can actually do the merge, just color the partition to show results
                        for (color_idx, perimeters) in partitions.into_iter().enumerate() {
                            let color = COLORS[color_idx % COLORS.len()];
                            for perimeter in perimeters {
                                if let Ok(block) = perimeter.to_block(map) {
                                    let id = self.new_id();
                                    self.add_block(ctx, id, color, block);
                                }
                            }
                        }
                    }
                }
                "Reset" => {
                    return Transition::Replace(Blockfinder::new_state(ctx, app));
                }
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::Keypress("add to merge set", id) => {
                self.to_merge.insert(id);
                let block = self.blocks.remove(&id).unwrap();
                self.world.delete_before_replacement(id);
                self.add_block(ctx, id, TO_MERGE, block);
            }
            WorldOutcome::Keypress("remove from merge set", id) => {
                self.to_merge.remove(&id);
                let block = self.blocks.remove(&id).unwrap();
                self.world.delete_before_replacement(id);
                // We'll lose the original coloring, oh well
                self.add_block(ctx, id, MODIFIED, block);
            }
            WorldOutcome::ClickedObject(id) => {
                return Transition::Push(OneBlock::new_state(ctx, self.blocks[&id].clone()));
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.world.draw(g);
        self.panel.draw(g);
    }
}

pub struct OneBlock {
    block: Block,
}

impl OneBlock {
    pub fn new_state(ctx: &mut EventCtx, block: Block) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Blockfinder").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            ctx.style()
                .btn_outline
                .text("Show perimeter in order")
                .hotkey(Key::O)
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Debug polygon")
                .hotkey(Key::D)
                .build_def(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(OneBlock { block }))
    }
}

impl SimpleState<App> for OneBlock {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            "Show perimeter in order" => {
                let mut items = Vec::new();
                let map = &app.primary.map;
                for road_side in &self.block.perimeter.roads {
                    let lane = road_side.get_outermost_lane(map);
                    items.push(polygons::Item::Polygon(lane.get_thick_polygon()));
                }
                return Transition::Push(polygons::PolygonDebugger::new_state(
                    ctx,
                    "side of road",
                    items,
                    None,
                ));
            }
            "Debug polygon" => {
                return Transition::Push(polygons::PolygonDebugger::new_state(
                    ctx,
                    "pt",
                    self.block
                        .polygon
                        .clone()
                        .into_points()
                        .into_iter()
                        .map(polygons::Item::Point)
                        .collect(),
                    None,
                ));
            }
            _ => unreachable!(),
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.draw_polygon(Color::RED.alpha(0.8), self.block.polygon.clone());
    }
}

fn make_panel(ctx: &mut EventCtx) -> Panel {
    Panel::new_builder(Widget::col(vec![
        Widget::row(vec![
            Line("Blockfinder").small_heading().into_widget(ctx),
            ctx.style().btn_close_widget(ctx),
        ]),
        "Click a block to examine.".text_widget(ctx),
        "Press space to mark/unmark for merging".text_widget(ctx),
        ctx.style()
            .btn_outline
            .text("Merge")
            .hotkey(Key::M)
            .build_def(ctx),
        ctx.style()
            .btn_outline
            .text("Collapse dead-ends")
            .hotkey(Key::C)
            .build_def(ctx),
        ctx.style()
            .btn_outline
            .text("Classify neighborhoods")
            .build_def(ctx),
        ctx.style()
            .btn_outline
            .text("Auto-merge all neighborhoods")
            .hotkey(Key::A)
            .build_def(ctx),
        ctx.style()
            .btn_solid_destructive
            .text("Reset")
            .build_def(ctx),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .build(ctx)
}
