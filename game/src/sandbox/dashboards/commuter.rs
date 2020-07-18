use crate::app::{App, ShowEverything};
use crate::common::ColorLegend;
use crate::game::{DrawBaselayer, State, Transition};
use crate::render::DrawOptions;
use abstutil::{prettyprint_usize, Counter, MultiMap};
use ezgui::{
    hotkey, AreaSlider, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, TextExt, VerticalAlignment, Widget,
};
use geom::{Polygon, Time};
use map_model::{BuildingID, LaneID, TurnType};
use sim::{DontDrawAgents, TripEndpoint};
use std::collections::{BTreeSet, HashMap, HashSet};

pub struct CommuterPatterns {
    bldg_to_block: HashMap<BuildingID, BlockID>,
    blocks: Vec<Block>,
    current_block: Option<BlockID>,
    draw_current_block: Option<Drawable>,

    composite: Composite,
    draw_all_blocks: Drawable,
}

// Group many buildings into a single block
struct Block {
    id: BlockID,
    bldgs: HashSet<BuildingID>,
    shape: Polygon,
}

struct Filters {
    depart_from: Time,
    depart_until: Time,
}

type BlockID = usize;

impl CommuterPatterns {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let (bldg_to_block, blocks) = group_bldgs(app);

        let mut all_blocks = GeomBatch::new();
        for block in &blocks {
            all_blocks.push(Color::YELLOW.alpha(0.5), block.shape.clone());
        }

        Box::new(CommuterPatterns {
            bldg_to_block,
            blocks,
            current_block: None,
            draw_current_block: None,

            draw_all_blocks: ctx.upload(all_blocks),
            composite: make_panel(ctx, app),
        })
    }

    // For all trips from (or to) the base block, how many of them go to all other blocks?
    fn count_per_block(
        &self,
        app: &App,
        base: &Block,
        from: bool,
        filter: Filters,
    ) -> Vec<(&Block, usize)> {
        let mut count: Counter<BlockID> = Counter::new();
        for (_, trip) in app.primary.sim.all_trip_info() {
            if trip.departure < filter.depart_from || trip.departure > filter.depart_until {
                continue;
            }
            if let (TripEndpoint::Bldg(b1), TripEndpoint::Bldg(b2)) = (trip.start, trip.end) {
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

        count
            .consume()
            .into_iter()
            .map(|(id, cnt)| (&self.blocks[id], cnt))
            .collect()
    }
}

impl State for CommuterPatterns {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        // TODO Or if a filter changed!
        if ctx.redo_mouseover() {
            let old_block = self.current_block;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                self.current_block = self
                    .blocks
                    .iter()
                    .find(|b| b.shape.contains_pt(pt))
                    .map(|b| b.id);
            } else {
                self.current_block = None;
            }

            if old_block != self.current_block {
                if let Some(id) = self.current_block {
                    let block = &self.blocks[id];

                    // Show the members of this block
                    let mut batch = GeomBatch::new();
                    for b in &block.bldgs {
                        batch.push(Color::PURPLE, app.primary.map.get_b(*b).polygon.clone());
                    }

                    let from = self.composite.is_checked("from / to this block");
                    let filter =
                        Filters {
                            depart_from: app.primary.sim.get_end_of_day().percent_of(
                                self.composite.area_slider("depart from").get_percent(),
                            ),
                            depart_until: app.primary.sim.get_end_of_day().percent_of(
                                self.composite.area_slider("depart until").get_percent(),
                            ),
                        };
                    let others = self.count_per_block(app, block, from, filter);
                    let mut total_trips = 0;
                    if !others.is_empty() {
                        let max_cnt = others.iter().map(|(_, cnt)| *cnt).max().unwrap() as f64;
                        for (other, cnt) in others {
                            total_trips += cnt;
                            let pct = (cnt as f64) / max_cnt;
                            // TODO Use app.cs.good_to_bad_red or some other color gradient
                            batch.push(
                                app.cs.good_to_bad_red.eval(pct).alpha(0.8),
                                other.shape.clone(),
                            );
                        }
                    }

                    self.draw_current_block = Some(ctx.upload(batch));

                    // TODO Show number of buildings
                    self.composite.replace(
                        ctx,
                        "current",
                        "Something selected".draw_text(ctx).named("current"),
                    );

                    let new_scale = ColorLegend::gradient(
                        ctx,
                        &app.cs.good_to_bad_red,
                        vec![
                            "0".to_string(),
                            format!("{} trips", prettyprint_usize(total_trips)),
                        ],
                    )
                    .named("scale");
                    self.composite.replace(ctx, "scale", new_scale);
                } else {
                    self.draw_current_block = None;
                    self.composite.replace(
                        ctx,
                        "current",
                        "Nothing selected".draw_text(ctx).named("current"),
                    );
                }
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.draw(
            g,
            DrawOptions::new(),
            &DontDrawAgents {},
            &ShowEverything::new(),
        );

        g.redraw(&self.draw_all_blocks);
        if let Some(ref d) = self.draw_current_block {
            g.redraw(d);
        }
        self.composite.draw(g);
    }
}

// This tries to group buildings into neighborhood "blocks". Much of the time, that's a smallish
// region bounded by 4 roads. But there are plenty of places with stranger shapes, or buildings
// near the border of the map. The fallback is currently to just group those buildings that share
// the same sidewalk.
fn group_bldgs(app: &App) -> (HashMap<BuildingID, BlockID>, Vec<Block>) {
    let mut bldg_to_block = HashMap::new();
    let mut blocks = Vec::new();

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
    (bldg_to_block, blocks)
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

            // Chase SharedSidewalkCorners. There should be zero or one new options for corners.
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

fn make_panel(ctx: &mut EventCtx, app: &App) -> Composite {
    Composite::new(Widget::col(vec![
        Widget::row(vec![
            Line("Commute map by block").small_heading().draw(ctx),
            Btn::text_fg("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ]),
        Checkbox::text(ctx, "from / to this block", hotkey(Key::Space), true),
        Widget::row(vec![
            "Departing from:".draw_text(ctx).margin_right(20),
            AreaSlider::new(ctx, 0.25 * ctx.canvas.window_width, 0.0).named("depart from"),
        ]),
        Widget::row(vec![
            "Departing until:".draw_text(ctx).margin_right(20),
            AreaSlider::new(ctx, 0.25 * ctx.canvas.window_width, 1.0).named("depart until"),
        ]),
        "Nothing selected".draw_text(ctx).named("current"),
        ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["0", "0"]).named("scale"),
    ]))
    .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
    .build(ctx)
}
