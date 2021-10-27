use geom::Distance;
use map_gui::tools::PopupMsg;
use map_gui::ID;
use map_model::Block;
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Panel, SimpleState,
    State, TextExt, Toggle, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::debug::polygons;

pub struct Blockfinder {
    draw_all_blocks: Option<Drawable>,
}

impl Blockfinder {
    pub fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Blockfinder").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Toggle::checkbox(ctx, "Draw all blocks", None, false),
            "Click a lane to find one block".text_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        <dyn SimpleState<_>>::new_state(
            panel,
            Box::new(Blockfinder {
                draw_all_blocks: None,
            }),
        )
    }
}

impl SimpleState<App> for Blockfinder {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            _ => unreachable!(),
        }
    }

    fn panel_changed(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut Panel,
    ) -> Option<Transition> {
        if self.draw_all_blocks.is_some() {
            self.draw_all_blocks = None;
        } else {
            let mut batch = GeomBatch::new();
            for block in Block::find_all_single_blocks(&app.primary.map) {
                batch.push(Color::RED.alpha(0.5), block.polygon.clone());
                if let Ok(outline) = block.polygon.to_outline(Distance::meters(3.0)) {
                    batch.push(Color::BLACK, outline);
                }
            }
            self.draw_all_blocks = Some(batch.upload(ctx));
        }
        None
    }

    fn on_mouseover(&mut self, ctx: &mut EventCtx, app: &mut App) {
        app.recalculate_current_selection(ctx);
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if let Some(ID::Lane(l)) = app.primary.current_selection {
            if app.per_obj.left_click(ctx, "trace this block") {
                app.primary.current_selection = None;
                return Transition::Push(match Block::single_block(&app.primary.map, l) {
                    Ok(block) => OneBlock::new_state(ctx, block),
                    Err(err) => {
                        // Rendering the error message is breaking
                        error!("Blockfinding failed: {}", err);
                        PopupMsg::new_state(ctx, "Error", vec!["See console"])
                    }
                });
            }
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        if let Some(ref draw) = self.draw_all_blocks {
            g.redraw(draw);
        }
    }
}

struct OneBlock {
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
