use std::collections::HashSet;

use anyhow::Result;

use abstutil::wraparound_get;
use geom::{Distance, Polygon, Ring};
use map_gui::tools::PopupMsg;
use map_gui::ID;
use map_model::{LaneID, Map, RoadSideID, SideOfRoad};
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
            for block in Block::find_all(&app.primary.map) {
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
                return Transition::Push(match Block::new(&app.primary.map, l) {
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
                    items.push(polygons::Item::Polygon(
                        lane.lane_center_pts.make_polygons(lane.width),
                    ));
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

/// A sequence of directed roads in order, beginning and ending at the same place.
// TODO Handle map borders
struct RoadLoop {
    // The first and last are the same
    roads: Vec<RoadSideID>,
}

impl RoadLoop {
    // TODO No need for result?
    fn new(map: &Map, start: LaneID) -> Result<RoadLoop> {
        let mut roads = Vec::new();
        let start_road_side = map.get_l(start).get_nearest_side_of_road(map);
        let mut current_road_side = start_road_side;
        let mut current_intersection = map.get_l(start).dst_i;
        loop {
            println!(
                "at {:?} pointing to {}",
                current_road_side, current_intersection
            );
            let i = map.get_i(current_intersection);
            let sorted_roads = i.get_road_sides_sorted_by_incoming_angle(map);
            // Find this one
            let idx = sorted_roads
                .iter()
                .position(|x| *x == current_road_side)
                .unwrap() as isize;
            println!("  idx {} in sorted {:?}", idx, sorted_roads);
            // Do we go clockwise or counter-clockwise? Well, unless we're at a dead-end, we want
            // to avoid the other side of the same road.
            let mut next = *wraparound_get(&sorted_roads, idx + 1);
            assert_ne!(next, current_road_side);
            if next.id == current_road_side.id {
                next = *wraparound_get(&sorted_roads, idx - 1);
                assert_ne!(next, current_road_side);
                if next.id == current_road_side.id {
                    // A dead-end then
                    assert_eq!(2, sorted_roads.len());
                }
            }
            roads.push(current_road_side);
            current_road_side = next;
            current_intersection = map
                .get_r(current_road_side.id)
                .other_endpt(current_intersection);

            if current_road_side == start_road_side {
                roads.push(start_road_side);
                break;
            }
        }
        assert_eq!(roads[0], *roads.last().unwrap());
        Ok(RoadLoop { roads })
    }
}

struct Block {
    perimeter: RoadLoop,
    polygon: Polygon,
    // TODO Interior stuff
}

impl Block {
    fn new(map: &Map, start: LaneID) -> Result<Block> {
        let perimeter = RoadLoop::new(map, start)?;

        let mut pts = Vec::new();
        let mut reversed_last = false;
        for pair in perimeter.roads.windows(2) {
            let lane1 = pair[0].get_outermost_lane(map);
            let lane2 = pair[1].get_outermost_lane(map);
            if lane1.id == lane2.id {
                bail!(
                    "Perimeter road has duplicate adjacent. {:?}",
                    perimeter.roads
                );
            }
            let pl = match pair[0].side {
                SideOfRoad::Right => lane1.lane_center_pts.must_shift_right(lane1.width / 2.0),
                SideOfRoad::Left => lane1.lane_center_pts.must_shift_left(lane1.width / 2.0),
            };
            if pair[0].id == pair[1].id {
                // We're doubling back at a dead-end
                if reversed_last {
                    pts.extend(pl.into_points());
                    reversed_last = false;
                } else {
                    pts.extend(pl.into_points());
                    reversed_last = true;
                }
            } else {
                match lane1.common_endpt(lane2) {
                    Some(i) => {
                        if i == lane1.dst_i {
                            pts.extend(pl.into_points());
                            reversed_last = false;
                        } else {
                            pts.extend(pl.reversed().into_points());
                            reversed_last = true;
                        }
                    }
                    None => {
                        // Two different roads link the same two intersections. I don't think we
                        // can decide the order of points other than seeing which endpoint is
                        // closest to our last point.
                        if let Some(last) = pts.last() {
                            if last.dist_to(pl.first_pt()) < last.dist_to(pl.last_pt()) {
                                pts.extend(pl.into_points());
                                reversed_last = false;
                            } else {
                                pts.extend(pl.reversed().into_points());
                                reversed_last = true;
                            }
                        } else {
                            // Doesn't matter
                            pts.extend(pl.into_points());
                            reversed_last = false;
                        }
                    }
                }
            }
        }
        // TODO Depending where we start, this sometimes misses the SharedSidewalkCorner?
        pts.push(pts[0]);
        pts.dedup();
        let polygon = Ring::new(pts)?.into_polygon();

        Ok(Block { perimeter, polygon })
    }

    // TODO Doesn't handle leftovers yet
    fn find_all(map: &Map) -> Vec<Block> {
        let mut seen = HashSet::new();
        let mut blocks = Vec::new();
        for lane in map.all_lanes() {
            let side = lane.get_nearest_side_of_road(map);
            if seen.contains(&side) {
                continue;
            }
            match Block::new(map, lane.id) {
                Ok(block) => {
                    seen.extend(block.perimeter.roads.clone());
                    blocks.push(block);
                }
                Err(err) => {
                    warn!("Failed from {}: {}", lane.id, err);
                    // Don't try again
                    seen.insert(side);
                }
            }
        }
        blocks
    }
}
