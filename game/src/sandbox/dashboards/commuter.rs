use crate::app::{App, ShowEverything};
use crate::common::{ColorLegend, CommonState};
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::checkbox_per_mode;
use crate::render::DrawOptions;
use abstutil::{prettyprint_usize, Counter, MultiMap};
use ezgui::{
    hotkey, AreaSlider, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Distance, Polygon, Time};
use map_model::{BuildingID, BuildingType, IntersectionID, LaneID, RoadID, TurnType};
use maplit::hashset;
use sim::{DontDrawAgents, TripEndpoint, TripInfo, TripMode};
use std::collections::{BTreeSet, HashMap, HashSet};

pub struct CommuterPatterns {
    bldg_to_block: HashMap<BuildingID, BlockID>,
    border_to_block: HashMap<IntersectionID, BlockID>,
    blocks: Vec<Block>,
    selected: Option<CurrentBlock>,
    filter: Filter,

    // Indexed by BlockID
    trips_from_block: Vec<Vec<TripInfo>>,
    trips_to_block: Vec<Vec<TripInfo>>,

    composite: Composite,
    draw_all_blocks: Drawable,
}

struct CurrentBlock {
    id: BlockID,
    draw: Drawable,
    // After clicking a block, show its outline to indicate it's been clicked.
    sticky: Option<Drawable>,
}

// Group many buildings into a single block
struct Block {
    id: BlockID,
    // A block is either some buildings or a single border. Might be worth expressing that more
    // clearly.
    bldgs: HashSet<BuildingID>,
    borders: HashSet<IntersectionID>,
    shape: Polygon,
}

#[derive(PartialEq)]
struct Filter {
    // If false, then trips to this block
    from_block: bool,
    include_borders: bool,
    depart_from: Time,
    depart_until: Time,
    modes: BTreeSet<TripMode>,
}

type BlockID = usize;

impl CommuterPatterns {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let (bldg_to_block, border_to_block, blocks) =
            ctx.loading_screen("group buildings into blocks", |_, _| group_bldgs(app));

        let mut trips_from_block: Vec<Vec<TripInfo>> = std::iter::repeat_with(Vec::new)
            .take(blocks.len())
            .collect();
        let mut trips_to_block: Vec<Vec<TripInfo>> = trips_from_block.clone();
        for (_, trip) in app.primary.sim.all_trip_info() {
            let block1 = match trip.start {
                TripEndpoint::Bldg(b) => bldg_to_block[&b],
                TripEndpoint::Border(i, _) => border_to_block[&i],
            };
            let block2 = match trip.end {
                TripEndpoint::Bldg(b) => bldg_to_block[&b],
                TripEndpoint::Border(i, _) => border_to_block[&i],
            };
            // Totally ignore trips within the same block
            if block1 != block2 {
                trips_from_block[block1].push(trip.clone());
                trips_to_block[block2].push(trip);
            }
        }

        let mut all_blocks = GeomBatch::new();
        for block in &blocks {
            all_blocks.push(Color::YELLOW.alpha(0.5), block.shape.clone());
        }

        Box::new(CommuterPatterns {
            bldg_to_block,
            border_to_block,
            blocks,
            selected: None,
            trips_from_block,
            trips_to_block,
            filter: Filter {
                from_block: true,
                include_borders: true,
                depart_from: Time::START_OF_DAY,
                depart_until: app.primary.sim.get_end_of_day(),
                modes: TripMode::all().into_iter().collect(),
            },

            draw_all_blocks: ctx.upload(all_blocks),
            composite: make_panel(ctx, app),
        })
    }

    // For all trips from (or to) the base block, how many of them go to all other blocks?
    fn count_per_block(&self, base: &Block) -> Vec<(&Block, usize)> {
        let candidates = if self.filter.from_block {
            &self.trips_from_block[base.id]
        } else {
            &self.trips_to_block[base.id]
        };
        let mut count: Counter<BlockID> = Counter::new();
        for trip in candidates {
            if trip.departure < self.filter.depart_from || trip.departure > self.filter.depart_until
            {
                continue;
            }
            if !self.filter.modes.contains(&trip.mode) {
                continue;
            }
            if self.filter.from_block {
                match trip.end {
                    TripEndpoint::Bldg(b) => {
                        count.inc(self.bldg_to_block[&b]);
                    }
                    TripEndpoint::Border(i, _) => {
                        if self.filter.include_borders {
                            count.inc(self.border_to_block[&i]);
                        }
                    }
                }
            } else {
                match trip.start {
                    TripEndpoint::Bldg(b) => {
                        count.inc(self.bldg_to_block[&b]);
                    }
                    TripEndpoint::Border(i, _) => {
                        if self.filter.include_borders {
                            count.inc(self.border_to_block[&i]);
                        }
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

        let old_block = self.selected.as_ref().map(|b| b.id);
        if let Some(ref mut current) = self.selected {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if app.per_obj.left_click(ctx, "change current block") {
                    if let Some(b) = self.blocks.iter().find(|b| b.shape.contains_pt(pt)) {
                        current.id = b.id;
                        current.sticky = Some(ctx.upload(GeomBatch::from(vec![(
                                Color::BLACK,
                                self.blocks[current.id]
                                    .shape
                                    .to_outline(Distance::meters(10.0))
                                    .unwrap(),
                            )])));
                    } else {
                        self.selected = None;
                    }
                }
            }
        }
        if self
            .selected
            .as_ref()
            .map(|b| b.sticky.is_none())
            .unwrap_or(true)
            && ctx.redo_mouseover()
        {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if let Some(b) = self.blocks.iter().find(|b| b.shape.contains_pt(pt)) {
                    if old_block != Some(b.id) {
                        self.selected = Some(CurrentBlock {
                            id: b.id,
                            draw: ctx.upload(GeomBatch::new()),
                            sticky: None,
                        });
                    }
                } else {
                    self.selected = None;
                }
            }
        }

        let mut filter = Filter {
            from_block: self.composite.is_checked("from / to this block"),
            include_borders: self.composite.is_checked("include borders"),
            depart_from: app
                .primary
                .sim
                .get_end_of_day()
                .percent_of(self.composite.area_slider("depart from").get_percent()),
            depart_until: app
                .primary
                .sim
                .get_end_of_day()
                .percent_of(self.composite.area_slider("depart until").get_percent()),
            modes: BTreeSet::new(),
        };
        for m in TripMode::all() {
            if self.composite.is_checked(m.ongoing_verb()) {
                filter.modes.insert(m);
            }
        }

        if old_block != self.selected.as_ref().map(|b| b.id) || filter != self.filter {
            self.filter = filter;
            if let Some(id) = self.selected.as_ref().map(|b| b.id) {
                let block = &self.blocks[id];

                // Show the members of this block
                let mut batch = GeomBatch::new();
                let mut building_counts: Vec<(&str, u32)> = vec![
                    ("Residential", 0),
                    ("Residential/Commercial", 0),
                    ("Commercial", 0),
                    ("Empty", 0),
                ];
                for b in &block.bldgs {
                    let b = app.primary.map.get_b(*b);
                    batch.push(Color::PURPLE, b.polygon.clone());
                    match b.bldg_type {
                        BuildingType::Residential(_) => building_counts[0].1 += 1,
                        BuildingType::ResidentialCommercial(_) => building_counts[1].1 += 1,
                        BuildingType::Commercial => building_counts[2].1 += 1,
                        BuildingType::Empty => building_counts[3].1 += 1,
                    }
                }
                for i in &block.borders {
                    batch.push(Color::PURPLE, app.primary.map.get_i(*i).polygon.clone());
                }

                let others = self.count_per_block(block);
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

                self.selected.as_mut().unwrap().draw = ctx.upload(batch);

                let mut txt = Text::new();
                for (name, cnt) in building_counts {
                    if cnt != 0 {
                        txt.add(Line(format!("{}: {}", name, cnt)));
                    }
                }
                self.composite
                    .replace(ctx, "current", txt.draw(ctx).named("current"));

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
                self.composite.replace(
                    ctx,
                    "current",
                    "None selected".draw_text(ctx).named("current"),
                );
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
        if let Some(ref b) = self.selected {
            g.redraw(&b.draw);
            if let Some(ref d) = b.sticky {
                g.redraw(d);
            }
        }
        self.composite.draw(g);
        CommonState::draw_osd(g, app);
    }
}

// This tries to group buildings into neighborhood "blocks". Much of the time, that's a smallish
// region bounded by 4 roads. But there are plenty of places with stranger shapes, or buildings
// near the border of the map. The fallback is currently to just group those buildings that share
// the same sidewalk.
fn group_bldgs(
    app: &App,
) -> (
    HashMap<BuildingID, BlockID>,
    HashMap<IntersectionID, BlockID>,
    Vec<Block>,
) {
    let mut bldg_to_block = HashMap::new();
    let mut blocks = Vec::new();

    for group in partition_sidewalk_loops(app) {
        let block_id = blocks.len();
        let mut polygons = Vec::new();
        let mut lanes = HashSet::new();
        for b in &group.bldgs {
            bldg_to_block.insert(*b, block_id);
            let bldg = app.primary.map.get_b(*b);
            if group.proper {
                lanes.insert(bldg.sidewalk());
            } else {
                polygons.push(bldg.polygon.clone());
            }
        }
        if group.proper {
            // TODO Even better, glue the loop of sidewalks together and fill that area.
            for l in lanes {
                polygons.push(app.primary.draw_map.get_l(l).polygon.clone());
            }
        }
        blocks.push(Block {
            id: block_id,
            bldgs: group.bldgs,
            borders: HashSet::new(),
            shape: Polygon::convex_hull(polygons),
        });
    }

    let mut border_to_block = HashMap::new();
    for i in app.primary.map.all_incoming_borders() {
        let id = blocks.len();
        border_to_block.insert(i.id, id);
        blocks.push(Block {
            id,
            bldgs: HashSet::new(),
            borders: hashset! { i.id },
            shape: i.polygon.clone(),
        });
    }
    for i in app.primary.map.all_outgoing_borders() {
        if border_to_block.contains_key(&i.id) {
            continue;
        }
        let id = blocks.len();
        border_to_block.insert(i.id, id);
        blocks.push(Block {
            id,
            bldgs: HashSet::new(),
            borders: hashset! { i.id },
            shape: i.polygon.clone(),
        });
    }

    (bldg_to_block, border_to_block, blocks)
}

struct Loop {
    bldgs: HashSet<BuildingID>,
    // True if it's a "proper" block, false if it's a hack.
    proper: bool,
    roads: HashSet<RoadID>,
}

fn partition_sidewalk_loops(app: &App) -> Vec<Loop> {
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
            groups.push(Loop {
                bldgs,
                proper: true,
                roads: sidewalks.into_iter().map(|l| map.get_l(l).parent).collect(),
            });
        } else {
            remainder.extend(bldgs);
        }
    }

    // Merge adjacent residential blocks
    loop {
        // Find a pair of blocks that have at least one residential road in common.
        // Rank comes from OSM highway type; < 6 means residential.
        let mut any = false;
        for mut idx1 in 0..groups.len() {
            for mut idx2 in 0..groups.len() {
                // This is O(n^3) on original groups.len(). In practice, it's fine, as long as we
                // don't start the search over from scratch after making a single merge. Starting
                // over is really wasteful, because it's guaranteed that nothing there has changed.
                if idx1 >= groups.len() || idx2 >= groups.len() {
                    break;
                }

                if idx1 != idx2
                    && groups[idx1]
                        .roads
                        .intersection(&groups[idx2].roads)
                        .any(|r| map.get_r(*r).get_rank() < 6)
                {
                    // Indexing gets messed up, so remove the larger one
                    if idx1 > idx2 {
                        std::mem::swap(&mut idx1, &mut idx2);
                    }
                    let merge = groups.remove(idx2);
                    groups[idx1].bldgs.extend(merge.bldgs);
                    groups[idx1].roads.extend(merge.roads);
                    any = true;
                }
            }
        }
        if !any {
            break;
        }
    }

    // For all the weird remainders, just group them based on sidewalk.
    let mut per_sidewalk: MultiMap<LaneID, BuildingID> = MultiMap::new();
    for b in remainder {
        per_sidewalk.insert(map.get_b(b).sidewalk(), b);
    }
    for (_, bldgs) in per_sidewalk.consume() {
        let r = map
            .get_l(map.get_b(*bldgs.iter().next().unwrap()).sidewalk())
            .parent;
        groups.push(Loop {
            bldgs: bldgs.into_iter().collect(),
            proper: false,
            roads: hashset! { r },
        });
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
        Checkbox::text(ctx, "include borders", None, true),
        Widget::row(vec![
            "Departing from:".draw_text(ctx).margin_right(20),
            AreaSlider::new(ctx, 0.15 * ctx.canvas.window_width, 0.0).named("depart from"),
        ]),
        Widget::row(vec![
            "Departing until:".draw_text(ctx).margin_right(20),
            AreaSlider::new(ctx, 0.15 * ctx.canvas.window_width, 1.0).named("depart until"),
        ]),
        checkbox_per_mode(ctx, app, &TripMode::all().into_iter().collect()),
        ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["0", "0"]).named("scale"),
        "None selected".draw_text(ctx).named("current"),
    ]))
    .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
    .build(ctx)
}
