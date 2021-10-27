use std::collections::{HashMap, HashSet};

use map_model::Block;
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
        let mut id_counter = 0;
        let mut blocks = HashMap::new();
        let mut world = World::bounded(app.primary.map.get_bounds());
        ctx.loading_screen("calculate all blocks", |ctx, _| {
            for block in Block::find_all_single_blocks(&app.primary.map) {
                let id = Obj(id_counter);
                id_counter += 1;
                world
                    .add(id)
                    .hitbox(block.polygon.clone())
                    .draw_color(Color::RED.alpha(0.5))
                    // TODO an outline would be nicer
                    .hover_alpha(0.9)
                    .clickable()
                    .hotkey(Key::Space, "add to merge set")
                    .build(ctx);
                blocks.insert(id, block);
            }
        });
        world.initialize_hover(ctx);

        let panel = Panel::new_builder(Widget::col(vec![
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
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        Box::new(Blockfinder {
            panel,
            id_counter,
            blocks,
            world,
            to_merge: HashSet::new(),
        })
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
                    return Transition::Keep;
                }
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::Keypress("add to merge set", id) => {
                self.to_merge.insert(id);
                let block = &self.blocks[&id];
                self.world.delete_before_replacement(id);
                // TODO Refactor?
                self.world
                    .add(id)
                    .hitbox(block.polygon.clone())
                    .draw_color(Color::CYAN.alpha(0.5))
                    .hover_alpha(0.9)
                    .clickable()
                    .hotkey(Key::Space, "remove from merge set")
                    .build(ctx);
            }
            WorldOutcome::Keypress("remove from merge set", id) => {
                self.to_merge.remove(&id);
                let block = &self.blocks[&id];
                self.world.delete_before_replacement(id);
                self.world
                    .add(id)
                    .hitbox(block.polygon.clone())
                    .draw_color(Color::RED.alpha(0.5))
                    .hover_alpha(0.9)
                    .clickable()
                    .hotkey(Key::Space, "add to merge set")
                    .build(ctx);
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
