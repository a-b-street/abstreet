use std::collections::{BTreeSet, HashMap, HashSet};

use maplit::hashset;

use abstutil::{prettyprint_usize, Counter, MultiMap, Timer};
use geom::{Distance, PolyLine, Polygon, Time};
use map_gui::tools::ColorLegend;
use map_model::{osm, BuildingID, BuildingType, IntersectionID, LaneID, Map, RoadID, TurnType};
use sim::{TripEndpoint, TripInfo, TripMode};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    RewriteColor, Slider, State, Text, TextExt, Toggle, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::{checkbox_per_mode, CommonState};
use crate::sandbox::dashboards::DashTab;

pub struct CommuterPatterns {
    bldg_to_block: HashMap<BuildingID, BlockID>,
    border_to_block: HashMap<IntersectionID, BlockID>,
    blocks: Vec<Block>,
    current_block: (BlockSelection, Drawable),
    filter: Filter,

    // Indexed by BlockID
    trips_from_block: Vec<Vec<TripInfo>>,
    trips_to_block: Vec<Vec<TripInfo>>,

    panel: Panel,
    draw_all_blocks: Drawable,
}

#[derive(PartialEq, Clone, Copy)]
enum BlockSelection {
    NothingSelected,
    Unlocked(BlockID),
    Locked {
        base: BlockID,
        compare_to: Option<BlockID>,
    },
}

struct PanelState<'a> {
    building_counts: Vec<(&'a str, u32)>,
    max_count: usize,
    total_trips: usize,
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
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let (bldg_to_block, border_to_block, blocks) = ctx
            .loading_screen("group buildings into blocks", |_, timer| {
                group_bldgs(app, timer)
            });

        let mut trips_from_block: Vec<Vec<TripInfo>> = std::iter::repeat_with(Vec::new)
            .take(blocks.len())
            .collect();
        let mut trips_to_block: Vec<Vec<TripInfo>> = trips_from_block.clone();
        for (_, trip) in app.primary.sim.all_trip_info() {
            let block1 = match trip.start {
                TripEndpoint::Bldg(b) => bldg_to_block[&b],
                TripEndpoint::Border(i) => border_to_block[&i],
                TripEndpoint::SuddenlyAppear(_) => continue,
            };
            let block2 = match trip.end {
                TripEndpoint::Bldg(b) => bldg_to_block[&b],
                TripEndpoint::Border(i) => border_to_block[&i],
                TripEndpoint::SuddenlyAppear(_) => continue,
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

        let depart_until = app.primary.sim.get_end_of_day();

        assert!(app.primary.suspended_sim.is_none());
        app.primary.suspended_sim = Some(app.primary.clear_sim());

        Box::new(CommuterPatterns {
            bldg_to_block,
            border_to_block,
            blocks,
            current_block: (BlockSelection::NothingSelected, Drawable::empty(ctx)),
            trips_from_block,
            trips_to_block,
            filter: Filter {
                from_block: true,
                include_borders: true,
                depart_from: Time::START_OF_DAY,
                depart_until,
                modes: TripMode::all().into_iter().collect(),
            },

            draw_all_blocks: ctx.upload(all_blocks),
            panel: make_panel(ctx, app),
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
                    TripEndpoint::Border(i) => {
                        if self.filter.include_borders {
                            count.inc(self.border_to_block[&i]);
                        }
                    }
                    TripEndpoint::SuddenlyAppear(_) => {}
                }
            } else {
                match trip.start {
                    TripEndpoint::Bldg(b) => {
                        count.inc(self.bldg_to_block[&b]);
                    }
                    TripEndpoint::Border(i) => {
                        if self.filter.include_borders {
                            count.inc(self.border_to_block[&i]);
                        }
                    }
                    TripEndpoint::SuddenlyAppear(_) => {}
                }
            }
        }

        count
            .consume()
            .into_iter()
            .map(|(id, cnt)| (&self.blocks[id], cnt))
            .collect()
    }

    fn build_block_drawable<'a>(
        &self,
        block_selection: BlockSelection,
        ctx: &EventCtx,
        app: &App,
    ) -> (Drawable, Option<PanelState<'a>>) {
        let mut batch = GeomBatch::new();

        let base_block_id = match block_selection {
            BlockSelection::Unlocked(id) => Some(id),
            BlockSelection::Locked { base, .. } => Some(base),
            BlockSelection::NothingSelected => None,
        };

        match base_block_id {
            None => (ctx.upload(batch), None),
            Some(base_block_id) => {
                let base_block = &self.blocks[base_block_id];

                // Show the members of this block
                let mut building_counts: Vec<(&'a str, u32)> = vec![
                    ("Residential", 0),
                    ("Residential/Commercial", 0),
                    ("Commercial", 0),
                    ("Empty", 0),
                ];
                for b in &base_block.bldgs {
                    let b = app.primary.map.get_b(*b);
                    batch.push(Color::PURPLE, b.polygon.clone());
                    match b.bldg_type {
                        BuildingType::Residential { .. } => building_counts[0].1 += 1,
                        BuildingType::ResidentialCommercial(_, _) => building_counts[1].1 += 1,
                        BuildingType::Commercial(_) => building_counts[2].1 += 1,
                        BuildingType::Empty => building_counts[3].1 += 1,
                    }
                }
                for i in &base_block.borders {
                    batch.push(Color::PURPLE, app.primary.map.get_i(*i).polygon.clone());
                }

                batch.push(Color::BLACK.alpha(0.5), base_block.shape.clone());

                // Draw outline for Locked Selection
                if let BlockSelection::Locked { .. } = block_selection {
                    let outline = base_block.shape.to_outline(Distance::meters(10.0)).unwrap();
                    batch.push(Color::BLACK, outline);
                };

                {
                    // Indicate direction over current block
                    let (icon_name, icon_scale) = if self.filter.from_block {
                        ("outward.svg", 1.2)
                    } else {
                        ("inward.svg", 1.0)
                    };

                    let center = base_block.shape.polylabel();
                    let icon = GeomBatch::load_svg(
                        ctx.prerender,
                        format!("system/assets/tools/{}", icon_name),
                    )
                    .scale(icon_scale)
                    .centered_on(center)
                    .color(RewriteColor::ChangeAll(Color::WHITE));

                    batch.append(icon);
                }

                let others = self.count_per_block(base_block);

                let mut total_trips = 0;
                let max_count = others.iter().map(|(_, cnt)| *cnt).max().unwrap_or(0);
                for (other, cnt) in &others {
                    total_trips += cnt;
                    let pct = (*cnt as f64) / (max_count as f64);
                    batch.push(
                        app.cs.good_to_bad_red.eval(pct).alpha(0.8),
                        other.shape.clone(),
                    );
                }

                // While selection is locked, draw an overlay with compare_to information for the
                // hovered block
                if let BlockSelection::Locked {
                    base: _,
                    compare_to: Some(compare_to),
                } = block_selection
                {
                    let compare_to_block = &self.blocks[compare_to];

                    let border = compare_to_block
                        .shape
                        .to_outline(Distance::meters(10.0))
                        .unwrap();
                    batch.push(Color::WHITE.alpha(0.8), border);

                    let count = others
                        .into_iter()
                        .find(|(b, _)| b.id == compare_to)
                        .map(|(_, count)| count)
                        .unwrap_or(0);
                    let label_text = abstutil::prettyprint_usize(count);
                    let label = Text::from(Line(label_text).fg(Color::BLACK))
                        .render_autocropped(ctx)
                        .scale(2.0)
                        .centered_on(compare_to_block.shape.polylabel());

                    let dims = label.get_dims();
                    let label_bg = Polygon::pill(dims.width + 70.0, dims.height + 20.0);
                    let bg = GeomBatch::from(vec![(Color::WHITE, label_bg)])
                        .centered_on(compare_to_block.shape.polylabel());
                    batch.append(bg);
                    batch.append(label);
                };
                let panel_data = PanelState {
                    building_counts,
                    max_count,
                    total_trips,
                };
                (ctx.upload(batch), Some(panel_data))
            }
        }
    }

    fn redraw_panel(&mut self, state: Option<&PanelState>, ctx: &mut EventCtx, app: &App) {
        if let Some(state) = state {
            let mut txt = Text::new();
            txt.add_line(format!(
                "Total: {} trips",
                abstutil::prettyprint_usize(state.total_trips)
            ));

            for (name, cnt) in &state.building_counts {
                if *cnt != 0 {
                    txt.add_line(format!("{}: {}", name, cnt));
                }
            }

            self.panel.replace(ctx, "current", txt.into_widget(ctx));

            let new_scale = ColorLegend::gradient(
                ctx,
                &app.cs.good_to_bad_red,
                vec![
                    "0".to_string(),
                    format!("{} trips", prettyprint_usize(state.max_count)),
                ],
            );
            self.panel.replace(ctx, "scale", new_scale);
        } else {
            self.panel
                .replace(ctx, "current", "None selected".text_widget(ctx));
        }
    }
}

impl State<App> for CommuterPatterns {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    app.primary.sim = app.primary.suspended_sim.take().unwrap();
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                if let Some(tab) = DashTab::CommuterPatterns.tab_changed(app, &self.panel) {
                    app.primary.sim = app.primary.suspended_sim.take().unwrap();
                    return Transition::Replace(tab.launch(ctx, app));
                }
            }
            _ => {}
        }

        let block_selection = if let Some(Some(b)) = ctx
            .canvas
            .get_cursor_in_map_space()
            .map(|pt| self.blocks.iter().find(|b| b.shape.contains_pt(pt)))
        {
            if app.per_obj.left_click(ctx, "clicked block") {
                match self.current_block.0 {
                    BlockSelection::Locked { base: old_base, .. } => {
                        if old_base == b.id {
                            BlockSelection::Unlocked(b.id)
                        } else {
                            BlockSelection::Locked {
                                base: b.id,
                                compare_to: None,
                            }
                        }
                    }
                    _ => BlockSelection::Locked {
                        base: b.id,
                        compare_to: None,
                    },
                }
            } else {
                // Hovering over block
                match self.current_block.0 {
                    BlockSelection::Locked { base, .. } => {
                        if base == b.id {
                            BlockSelection::Locked {
                                base,
                                compare_to: None,
                            }
                        } else {
                            BlockSelection::Locked {
                                base,
                                compare_to: Some(b.id),
                            }
                        }
                    }
                    BlockSelection::Unlocked(_) => BlockSelection::Unlocked(b.id),
                    BlockSelection::NothingSelected => BlockSelection::Unlocked(b.id),
                }
            }
        } else {
            // cursor not over any block
            match self.current_block.0 {
                BlockSelection::NothingSelected | BlockSelection::Unlocked(_) => {
                    BlockSelection::NothingSelected
                }
                BlockSelection::Locked { base, .. } => BlockSelection::Locked {
                    base,
                    compare_to: None,
                },
            }
        };

        let mut filter = Filter {
            from_block: self.panel.is_checked("from / to this block"),
            include_borders: self.panel.is_checked("include borders"),
            depart_from: app
                .primary
                .sim
                .get_end_of_day()
                .percent_of(self.panel.slider("depart from").get_percent()),
            depart_until: app
                .primary
                .sim
                .get_end_of_day()
                .percent_of(self.panel.slider("depart until").get_percent()),
            modes: BTreeSet::new(),
        };
        for m in TripMode::all() {
            if self.panel.is_checked(m.ongoing_verb()) {
                filter.modes.insert(m);
            }
        }

        if filter != self.filter || block_selection != self.current_block.0 {
            self.filter = filter;
            let (drawable, per_block_counts) = self.build_block_drawable(block_selection, ctx, app);
            self.redraw_panel(per_block_counts.as_ref(), ctx, app);
            self.current_block = (block_selection, drawable);
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw_all_blocks);
        g.redraw(&self.current_block.1);

        self.panel.draw(g);
        CommonState::draw_osd(g, app);
    }
}

// This tries to group buildings into neighborhood "blocks". Much of the time, that's a smallish
// region bounded by 4 roads. But there are plenty of places with stranger shapes, or buildings
// near the border of the map. The fallback is currently to just group those buildings that share
// the same sidewalk.
fn group_bldgs(
    app: &App,
    timer: &mut Timer,
) -> (
    HashMap<BuildingID, BlockID>,
    HashMap<IntersectionID, BlockID>,
    Vec<Block>,
) {
    let mut bldg_to_block = HashMap::new();
    let mut blocks = Vec::new();

    let map = &app.primary.map;
    for block in timer.parallelize(
        "draw neighborhoods",
        partition_sidewalk_loops(app)
            .into_iter()
            .enumerate()
            .collect(),
        |(block_id, group)| {
            let mut hull_points = Vec::new();
            let mut lanes = HashSet::new();
            for b in &group.bldgs {
                let bldg = map.get_b(*b);
                if group.proper {
                    lanes.insert(bldg.sidewalk());
                }
                hull_points.append(&mut bldg.polygon.points().clone());
            }
            if group.proper {
                // TODO Even better, glue the loop of sidewalks together and fill that area.
                for l in lanes {
                    let lane_line = map
                        .get_l(l)
                        .lane_center_pts
                        .interpolate_points(Distance::meters(20.0));
                    hull_points.append(&mut lane_line.points().clone());
                }
            }
            Block {
                id: block_id,
                bldgs: group.bldgs,
                borders: HashSet::new(),
                shape: Polygon::concave_hull(hull_points, 10),
            }
        },
    ) {
        for b in &block.bldgs {
            bldg_to_block.insert(*b, block.id);
        }
        blocks.push(block);
    }

    let mut border_to_block = HashMap::new();
    for i in app.primary.map.all_incoming_borders() {
        let id = blocks.len();
        border_to_block.insert(i.id, id);
        blocks.push(Block {
            id,
            bldgs: HashSet::new(),
            borders: hashset! { i.id },
            shape: build_shape_for_border(i, BorderType::Incoming, &app.primary.map),
        });
    }
    for i in app.primary.map.all_outgoing_borders() {
        if let Some(incoming_border_id) = border_to_block.get(&i.id) {
            let two_way_border = &mut blocks[*incoming_border_id];
            two_way_border.shape = build_shape_for_border(i, BorderType::Both, &app.primary.map);
            continue;
        }
        let id = blocks.len();
        border_to_block.insert(i.id, id);

        blocks.push(Block {
            id,
            bldgs: HashSet::new(),
            borders: hashset! { i.id },
            shape: build_shape_for_border(i, BorderType::Outgoing, &app.primary.map),
        });
    }

    (bldg_to_block, border_to_block, blocks)
}

enum BorderType {
    Incoming,
    Outgoing,
    Both,
}

fn build_shape_for_border(
    border: &map_model::Intersection,
    border_type: BorderType,
    map: &Map,
) -> Polygon {
    let start = border.polygon.center();

    let road = map.get_r(*border.roads.iter().next().unwrap());
    let center_line = road.get_dir_change_pl(map);
    let angle = if road.src_i == border.id {
        center_line.first_line().angle().opposite()
    } else {
        center_line.first_line().angle()
    };

    let length = Distance::meters(150.0);
    let thickness = Distance::meters(30.0);
    let end = start.project_away(length, angle);

    match border_type {
        BorderType::Incoming => {
            PolyLine::must_new(vec![end, start]).make_arrow(thickness, geom::ArrowCap::Triangle)
        }
        BorderType::Outgoing => {
            PolyLine::must_new(vec![start, end]).make_arrow(thickness, geom::ArrowCap::Triangle)
        }
        BorderType::Both => PolyLine::must_new(vec![start, end])
            .make_double_arrow(thickness, geom::ArrowCap::Triangle),
    }
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

    let mut sidewalk_to_bldgs = MultiMap::new();
    for b in map.all_buildings() {
        sidewalk_to_bldgs.insert(b.sidewalk(), b.id);
    }

    while !todo_bldgs.is_empty() {
        let mut sidewalks = HashSet::new();
        let mut bldgs = HashSet::new();
        let mut current_l = map.get_b(*todo_bldgs.iter().next().unwrap()).sidewalk();
        let mut current_i = map.get_l(current_l).src_i;

        let ok = loop {
            sidewalks.insert(current_l);
            for b in sidewalk_to_bldgs.get(current_l) {
                bldgs.insert(*b);
                // TODO I wanted to assert that we haven't assigned this one yet, but...
                todo_bldgs.remove(b);
            }

            // Chase SharedSidewalkCorners. There should be zero or one new options for corners.
            let turns = map
                .get_next_turns_and_lanes(current_l)
                .into_iter()
                .map(|(t, _)| t)
                .filter(|t| {
                    t.turn_type == TurnType::SharedSidewalkCorner && t.id.parent != current_i
                })
                .collect::<Vec<_>>();
            if turns.is_empty() {
                // TODO If we're not a loop, maybe toss this out. It's arbitrary that we didn't go
                // look the other way.
                break false;
            } else if turns.len() == 1 {
                current_l = if turns[0].id.dst != current_l {
                    turns[0].id.dst
                } else {
                    turns[0].id.src
                };
                current_i = turns[0].id.parent;
                if sidewalks.contains(&current_l) {
                    // Loop closed!
                    break true;
                }
            } else {
                // Around pedestrian-only roads, there'll be many SharedSidewalkCorners. Just give
                // up.
                break false;
            };
        };

        if ok {
            groups.push(Loop {
                bldgs,
                proper: true,
                roads: sidewalks.into_iter().map(|l| l.road).collect(),
            });
        } else {
            remainder.extend(bldgs);
        }
    }

    // Merge adjacent residential blocks
    loop {
        // Find a pair of blocks that have at least one residential road in common.
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
                        .any(|r| map.get_r(*r).get_rank() == osm::RoadRank::Local)
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
        let r = map.get_b(*bldgs.iter().next().unwrap()).sidewalk().road;
        groups.push(Loop {
            bldgs: bldgs.into_iter().collect(),
            proper: false,
            roads: hashset! { r },
        });
    }

    groups
}

fn make_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    Panel::new_builder(Widget::col(vec![
        DashTab::CommuterPatterns.picker(ctx, app),
        Toggle::choice(ctx, "from / to this block", "from", "to", Key::Space, true),
        Toggle::switch(ctx, "include borders", None, true),
        Widget::row(vec![
            "Departing from:".text_widget(ctx).margin_right(20),
            Slider::area(ctx, 0.15 * ctx.canvas.window_width, 0.0, "depart from"),
        ]),
        Widget::row(vec![
            "Departing until:".text_widget(ctx).margin_right(20),
            Slider::area(ctx, 0.15 * ctx.canvas.window_width, 1.0, "depart until"),
        ]),
        checkbox_per_mode(ctx, app, &TripMode::all().into_iter().collect()),
        ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["0", "0"]).named("scale"),
        "None selected".text_widget(ctx).named("current"),
    ]))
    .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
    .build(ctx)
}
