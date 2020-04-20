use crate::app::App;
use crate::game::{State, Transition};
use abstutil::{Counter, MultiMap};
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, VerticalAlignment, Widget,
};
use geom::{Distance, PolyLine, Polygon};
use map_model::{BuildingID, LaneID, Map, TurnType};
use sim::{Scenario, TripEndpoint};
use std::collections::{BTreeSet, HashMap, HashSet};

// TODO Handle borders too

pub struct BlockMap {
    bldg_to_block: HashMap<BuildingID, usize>,
    blocks: Vec<Block>,
    scenario: Scenario,

    composite: Composite,
    draw_all_blocks: Drawable,
}

struct Block {
    id: usize,
    bldgs: HashSet<BuildingID>,
    shape: Polygon,
}

impl BlockMap {
    pub fn new(ctx: &mut EventCtx, app: &App, scenario: Scenario) -> BlockMap {
        let mut bldg_to_block = HashMap::new();
        let mut blocks = Vec::new();

        // Really dumb assignment to start with
        for (bldgs, proper) in partition_sidewalk_loops(app) {
            let block_id = blocks.len();
            let mut polygons = Vec::new();
            let mut lanes = HashSet::new();
            for b in &bldgs {
                bldg_to_block.insert(*b, block_id);
                let bldg = app.primary.map.get_b(*b);
                if proper {
                    lanes.insert(bldg.sidewalk());
                } else {
                    polygons.push(bldg.polygon.clone());
                }
            }
            if proper {
                // TODO Even better, glue the loop of sidewalks together and fill that area.
                for l in lanes {
                    polygons.push(app.primary.draw_map.get_l(l).polygon.clone());
                }
            }
            blocks.push(Block {
                id: block_id,
                bldgs,
                shape: Polygon::convex_hull(polygons),
            });
        }

        let mut all_blocks = GeomBatch::new();
        for block in &blocks {
            all_blocks.push(Color::YELLOW.alpha(0.5), block.shape.clone());
        }

        BlockMap {
            bldg_to_block,
            blocks,
            scenario,

            draw_all_blocks: ctx.upload(all_blocks),
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Commute map by block")
                            .small_heading()
                            .draw(ctx)
                            .margin_right(10),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Checkbox::text(ctx, "from / to this block", hotkey(Key::Space), true),
                    Checkbox::text(ctx, "arrows / heatmap", hotkey(Key::H), true),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        }
    }

    fn count_per_block(&self, base: &Block, from: bool, map: &Map) -> Vec<(&Block, usize)> {
        let mut count: Counter<usize> = Counter::new();
        for p in &self.scenario.people {
            for trip in &p.trips {
                if let (TripEndpoint::Bldg(b1), TripEndpoint::Bldg(b2)) =
                    (trip.trip.start(map), trip.trip.end())
                {
                    let block1 = self.bldg_to_block[&b1];
                    let block2 = self.bldg_to_block[&b2];
                    if block1 == block2 {
                        continue;
                    }
                    if from && block1 == base.id {
                        count.inc(block2);
                    }
                    if !from && block2 == base.id {
                        count.inc(block1);
                    }
                }
            }
        }

        count
            .consume()
            .into_iter()
            .map(|(id, cnt)| (&self.blocks[id], cnt))
            .collect()
    }
}

impl State for BlockMap {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw_all_blocks);
        self.composite.draw(g);

        // TODO Expensive!
        if let Some(pt) = g.get_cursor_in_map_space() {
            for block in &self.blocks {
                if block.shape.contains_pt(pt) {
                    let mut batch = GeomBatch::new();
                    for b in &block.bldgs {
                        batch.push(Color::PURPLE, app.primary.map.get_b(*b).polygon.clone());
                    }

                    let from = self.composite.is_checked("from / to this block");
                    let arrows = self.composite.is_checked("arrows / heatmap");
                    let others = self.count_per_block(block, from, &app.primary.map);
                    if !others.is_empty() {
                        let max_cnt = others.iter().map(|(_, cnt)| *cnt).max().unwrap() as f64;
                        for (other, cnt) in others {
                            let pct = (cnt as f64) / max_cnt;
                            if arrows {
                                batch.push(
                                    Color::hex("#A32015").alpha(0.7),
                                    PolyLine::new(if from {
                                        vec![block.shape.center(), other.shape.center()]
                                    } else {
                                        vec![other.shape.center(), block.shape.center()]
                                    })
                                    .make_arrow(Distance::meters(15.0) * pct)
                                    .unwrap(),
                                );
                            } else {
                                batch.push(Color::RED.alpha(pct as f32), other.shape.clone());
                            }
                        }
                    }

                    let draw = g.upload(batch);
                    g.redraw(&draw);
                    return;
                }
            }
        }
    }
}

// True if it's a "proper" block, false if it's a hack.
fn partition_sidewalk_loops(app: &App) -> Vec<(HashSet<BuildingID>, bool)> {
    let map = &app.primary.map;

    let mut groups = Vec::new();
    let mut todo_bldgs: BTreeSet<BuildingID> = map.all_buildings().iter().map(|b| b.id).collect();
    let mut remainder = HashSet::new();

    while !todo_bldgs.is_empty() {
        let mut sidewalks = HashSet::new();
        let mut bldgs = HashSet::new();
        let mut current_l = map.get_b(*todo_bldgs.iter().next().unwrap()).sidewalk();
        let mut current_i = map.get_l(current_l).src_i;

        let ok = loop {
            sidewalks.insert(current_l);
            for b in &map.get_l(current_l).building_paths {
                bldgs.insert(*b);
                // TODO I wanted to assert that we haven't assigned this one yet, but...
                todo_bldgs.remove(b);
            }

            // Chase SharedSidewalkCorners. There should be zero or one new option for corners.
            let turns = map
                .get_turns_from_lane(current_l)
                .into_iter()
                .filter(|t| {
                    t.turn_type == TurnType::SharedSidewalkCorner && t.id.parent != current_i
                })
                .collect::<Vec<_>>();
            if turns.is_empty() {
                // TODO If we're not a loop, maybe toss this out. It's arbitrary that we didn't go
                // look the other way.
                break false;
            } else if turns.len() == 1 {
                current_l = turns[0].id.dst;
                current_i = turns[0].id.parent;
                if sidewalks.contains(&current_l) {
                    // Loop closed!
                    break true;
                }
            } else {
                panic!(
                    "Too many SharedSidewalkCorners from ({}, {})",
                    current_l, current_i
                );
            };
        };

        if ok {
            groups.push((bldgs, true));
        } else {
            remainder.extend(bldgs);
        }
    }

    // For all the weird remainders, just group them based on sidewalk.
    let mut per_sidewalk: MultiMap<LaneID, BuildingID> = MultiMap::new();
    for b in remainder {
        per_sidewalk.insert(map.get_b(b).sidewalk(), b);
    }
    for (_, bldgs) in per_sidewalk.consume() {
        groups.push((bldgs.into_iter().collect(), false));
    }

    groups
}
