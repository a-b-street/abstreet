use std::collections::{HashMap, HashSet};

use geom::Distance;
use map_model::osm::RoadRank;
use map_model::{Block, PathConstraints, RoadLoop};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, SimpleState, State,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::debug::polygons;

pub struct Blockfinder {
    panel: Panel,
    id_counter: usize,
    blocks: HashMap<Obj, Block>,
    world: World<Obj>,
    to_merge: HashSet<Obj>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Obj(usize);
impl ObjectID for Obj {}

impl Blockfinder {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Blockfinder {
            panel: make_panel(ctx),
            id_counter: 0,
            blocks: HashMap::new(),
            world: World::bounded(app.primary.map.get_bounds()),
            to_merge: HashSet::new(),
        };

        ctx.loading_screen("calculate all blocks", |ctx, _| {
            for block in Block::find_all_single_blocks(&app.primary.map) {
                let id = state.new_id();
                state.add_block(ctx, id, None, block);
            }
        });
        state.world.initialize_hover(ctx);
        Box::new(state)
    }

    fn new_id(&mut self) -> Obj {
        let id = Obj(self.id_counter);
        self.id_counter += 1;
        id
    }

    fn add_block(&mut self, ctx: &mut EventCtx, id: Obj, color: Option<Color>, block: Block) {
        let color = color.unwrap_or(Color::RED);
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
                    let mut blocks = Vec::new();
                    for id in self.to_merge.drain() {
                        blocks.push(self.blocks.remove(&id).unwrap());
                        // TODO If we happen to be hovering on one, uh oh! It's going to change
                        // ID...
                        self.world.delete(id);
                    }
                    for block in Block::merge_all(&app.primary.map, blocks) {
                        let id = self.new_id();
                        self.add_block(ctx, id, None, block);
                    }
                    return Transition::Keep;
                }
                "Auto-merge all neighborhoods" => {
                    let loops: Vec<RoadLoop> =
                        self.blocks.drain().map(|(_, b)| b.perimeter).collect();
                    let map = &app.primary.map;
                    let partitions = RoadLoop::partition_by_predicate(loops, |r| {
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

                    // Until we can actually do the merge, just color the partition to show results
                    for (color_idx, loops) in partitions.into_iter().enumerate() {
                        let color =
                            [Color::RED, Color::YELLOW, Color::GREEN, Color::PURPLE][color_idx % 4];
                        for perimeter in loops {
                            if let Ok(block) = perimeter.to_block(map) {
                                let id = self.new_id();
                                self.add_block(ctx, id, Some(color), block);
                            }
                        }
                    }
                }
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::Keypress("add to merge set", id) => {
                self.to_merge.insert(id);
                let block = self.blocks.remove(&id).unwrap();
                self.world.delete_before_replacement(id);
                self.add_block(ctx, id, Some(Color::CYAN), block);
            }
            WorldOutcome::Keypress("remove from merge set", id) => {
                self.to_merge.remove(&id);
                let block = self.blocks.remove(&id).unwrap();
                self.world.delete_before_replacement(id);
                self.add_block(ctx, id, None, block);
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
                .build_def(ctx),
            ctx.style().btn_outline.text("Debug polygon").build_def(ctx),
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
            .text("Auto-merge all neighborhoods")
            .hotkey(Key::N)
            .build_def(ctx),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .build(ctx)
}
