use crate::app::App;
use crate::game::{State, Transition};
use abstutil::Counter;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, VerticalAlignment, Widget,
};
use geom::{Distance, PolyLine, Polygon};
use map_model::{BuildingID, LaneType};
use sim::Scenario;
use std::collections::{HashMap, HashSet};

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
        let map = &app.primary.map;
        for r in map.all_roads() {
            let mut bldgs = HashSet::new();
            for (l, lt) in r
                .children_forwards
                .iter()
                .chain(r.children_backwards.iter())
            {
                if *lt == LaneType::Sidewalk {
                    bldgs.extend(map.get_l(*l).building_paths.clone());
                }
            }

            if !bldgs.is_empty() {
                let block_id = blocks.len();
                let mut polygons = Vec::new();
                for b in &bldgs {
                    bldg_to_block.insert(*b, block_id);
                    polygons.push(map.get_b(*b).polygon.clone());
                }
                blocks.push(Block {
                    id: block_id,
                    bldgs,
                    shape: Polygon::convex_hull(polygons),
                });
            }
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
                        Line("Commute map by block").small_heading().draw(ctx),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Checkbox::text(ctx, "trips from this block", hotkey(Key::Space), true),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        }
    }

    fn count_per_block(&self, base: &Block, from: bool) -> Vec<(&Block, usize)> {
        let mut count: Counter<usize> = Counter::new();
        for p in &self.scenario.people {
            for trip in &p.trips {
                if let (Some(b1), Some(b2)) = (trip.trip.start_from_bldg(), trip.trip.end_at_bldg())
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

        // TODO Expensive!
        if let Some(pt) = g.get_cursor_in_map_space() {
            for block in &self.blocks {
                if block.shape.contains_pt(pt) {
                    let mut batch = GeomBatch::new();
                    for b in &block.bldgs {
                        batch.push(Color::PURPLE, app.primary.map.get_b(*b).polygon.clone());
                    }

                    let from = self.composite.is_checked("trips from this block");
                    let others = self.count_per_block(block, from);
                    if !others.is_empty() {
                        let max_cnt = others.iter().map(|(_, cnt)| *cnt).max().unwrap() as f64;
                        for (other, cnt) in others {
                            batch.push(
                                Color::hex("#A32015").alpha(0.7),
                                PolyLine::new(if from {
                                    vec![block.shape.center(), other.shape.center()]
                                } else {
                                    vec![other.shape.center(), block.shape.center()]
                                })
                                .make_arrow(Distance::meters(15.0) * (cnt as f64) / max_cnt)
                                .unwrap(),
                            );
                        }
                    }

                    let draw = g.upload(batch);
                    g.redraw(&draw);
                    break;
                }
            }
        }

        self.composite.draw(g);
    }
}
