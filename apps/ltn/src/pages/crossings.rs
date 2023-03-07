use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use abstutil::PriorityQueueItem;
use geom::{Circle, Duration};
use map_model::{osm, CrossingType, RoadID};
use widgetry::mapspace::{DrawCustomUnzoomedShapes, ObjectID, PerZoom, World, WorldOutcome};
use widgetry::{
    lctrl, Color, ControlState, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel,
    RewriteColor, State, Text, TextExt, Widget,
};

use crate::components::{AppwidePanel, BottomPanel, Mode};
use crate::render::{colors, Toggle3Zoomed};
use crate::{logic, mut_edits, App, Crossing, Transition};

pub struct Crossings {
    appwide_panel: AppwidePanel,
    bottom_panel: Panel,
    world: World<Obj>,
    draw_porosity: Drawable,
    draw_crossings: Toggle3Zoomed,
    draw_nearest_crossing: Option<Drawable>,
    time_to_nearest_crossing: BTreeMap<RoadID, Duration>,
}

impl Crossings {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let appwide_panel = AppwidePanel::new(ctx, app, Mode::Crossings);
        let contents = make_bottom_panel(ctx, app);
        let bottom_panel = BottomPanel::new(ctx, &appwide_panel, contents);

        // Just force the layers panel to align above the bottom panel
        app.session
            .layers
            .event(ctx, &app.cs, Mode::Crossings, Some(&bottom_panel));

        let mut state = Self {
            appwide_panel,
            bottom_panel,
            world: World::new(),
            draw_porosity: Drawable::empty(ctx),
            draw_crossings: Toggle3Zoomed::empty(ctx),
            draw_nearest_crossing: None,
            time_to_nearest_crossing: BTreeMap::new(),
        };
        state.update(ctx, app);
        Box::new(state)
    }

    pub fn svg_path(ct: CrossingType) -> &'static str {
        match ct {
            CrossingType::Signalized => "system/assets/tools/signalized_crossing.svg",
            CrossingType::Unsignalized => "system/assets/tools/unsignalized_crossing.svg",
        }
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        self.draw_porosity = draw_porosity(ctx, app);
        self.draw_crossings = draw_crossings(ctx, app);
        let contents = make_bottom_panel(ctx, app);
        self.bottom_panel = BottomPanel::new(ctx, &self.appwide_panel, contents);
        self.draw_nearest_crossing = None;
        self.time_to_nearest_crossing.clear();

        if app.session.layers.show_crossing_time {
            let (draw, time) = draw_nearest_crossing(ctx, app);
            self.draw_nearest_crossing = Some(draw);
            self.time_to_nearest_crossing = time;
        }

        self.world = make_world(ctx, app, &self.time_to_nearest_crossing);
    }
}

impl State<App> for Crossings {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) =
            self.appwide_panel
                .event(ctx, app, &crate::save::PreserveState::Crossings, help)
        {
            return t;
        }
        if let Some(t) =
            app.session
                .layers
                .event(ctx, &app.cs, Mode::Crossings, Some(&self.bottom_panel))
        {
            if app.session.layers.show_crossing_time != self.draw_nearest_crossing.is_some() {
                if app.session.layers.show_crossing_time {
                    let (draw, time) = draw_nearest_crossing(ctx, app);
                    self.draw_nearest_crossing = Some(draw);
                    self.time_to_nearest_crossing = time;
                } else {
                    self.draw_nearest_crossing = None;
                    self.time_to_nearest_crossing.clear();
                }
                self.world = make_world(ctx, app, &self.time_to_nearest_crossing);
            }

            return t;
        }
        if let Outcome::Clicked(x) = self.bottom_panel.event(ctx) {
            match x.as_ref() {
                "signalized crossing" => {
                    app.session.crossing_type = CrossingType::Signalized;
                    let contents = make_bottom_panel(ctx, app);
                    self.bottom_panel = BottomPanel::new(ctx, &self.appwide_panel, contents);
                }
                "unsignalized crossing" => {
                    app.session.crossing_type = CrossingType::Unsignalized;
                    let contents = make_bottom_panel(ctx, app);
                    self.bottom_panel = BottomPanel::new(ctx, &self.appwide_panel, contents);
                }
                "undo" => {
                    logic::map_edits::undo_proposal(ctx, app);
                    self.update(ctx, app);
                }
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::ClickedObject(Obj::Road(r)) => {
                let cursor_pt = ctx.canvas.get_cursor_in_map_space().unwrap();
                let road = app.per_map.map.get_r(r);
                let pt_on_line = road.center_pts.project_pt(cursor_pt);
                let (dist, _) = road.center_pts.dist_along_of_point(pt_on_line).unwrap();

                app.per_map.proposals.before_edit();
                let list = mut_edits!(app).crossings.entry(r).or_insert_with(Vec::new);
                list.push(Crossing {
                    kind: app.session.crossing_type,
                    dist,
                    user_modified: true,
                });
                list.sort_by_key(|c| c.dist);
                self.update(ctx, app);
            }
            WorldOutcome::ClickedObject(Obj::Crossing(r, idx)) => {
                // Delete it
                app.per_map.proposals.before_edit();
                let list = mut_edits!(app).crossings.get_mut(&r).unwrap();
                list.remove(idx);
                if list.is_empty() {
                    mut_edits!(app).crossings.remove(&r);
                }
                // We don't need to re-sort
                self.update(ctx, app);
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.appwide_panel.draw(g);
        self.bottom_panel.draw(g);
        app.session.layers.draw(g, app);
        g.redraw(&self.draw_porosity);
        app.per_map.draw_major_road_labels.draw(g);
        app.per_map.draw_poi_icons.draw(g);
        if let Some(ref draw) = self.draw_nearest_crossing {
            g.redraw(draw);
        }
        self.draw_crossings.draw(g);
        // Draw on top of crossings, so hover state is visible
        self.world.draw(g);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

fn help() -> Vec<&'static str> {
    vec![
        "This shows crossings over main roads.",
        "The number of crossings determines the \"porosity\" of areas",
    ]
}

fn main_roads(app: &App) -> BTreeSet<RoadID> {
    let mut result = BTreeSet::new();
    for r in app.per_map.map.all_roads() {
        if r.get_rank() != osm::RoadRank::Local && !r.is_light_rail() {
            result.insert(r.id);
        }
    }
    result
}

fn draw_crossings(ctx: &EventCtx, app: &App) -> Toggle3Zoomed {
    let mut batch = GeomBatch::new();
    let mut low_zoom = DrawCustomUnzoomedShapes::builder();

    let mut icons = BTreeMap::new();
    for ct in [CrossingType::Signalized, CrossingType::Unsignalized] {
        icons.insert(ct, GeomBatch::load_svg(ctx, Crossings::svg_path(ct)));
    }

    for r in main_roads(app) {
        if let Some(list) = app.edits().crossings.get(&r) {
            let road = app.per_map.map.get_r(r);
            for crossing in list {
                let rewrite_color = if crossing.user_modified {
                    RewriteColor::NoOp
                } else {
                    RewriteColor::ChangeAlpha(0.7)
                };

                let icon = &icons[&crossing.kind];
                if let Ok((pt, angle)) = road.center_pts.dist_along(crossing.dist) {
                    let angle = angle.rotate_degs(90.0);
                    batch.append(
                        icon.clone()
                            .scale_to_fit_width(road.get_width().inner_meters())
                            .centered_on(pt)
                            .rotate_around_batch_center(angle)
                            .color(rewrite_color),
                    );

                    // TODO Memory intensive
                    let icon = icon.clone();
                    // TODO They can shrink a bit past their map size
                    low_zoom.add_custom(Box::new(move |batch, thickness| {
                        batch.append(
                            icon.clone()
                                .scale_to_fit_width(30.0 * thickness)
                                .centered_on(pt)
                                .rotate_around_batch_center(angle)
                                .color(rewrite_color),
                        );
                    }));
                }
            }
        }
    }

    let min_zoom_for_detail = 5.0;
    let step_size = 0.1;
    // TODO Ideally we get rid of Toggle3Zoomed and make DrawCustomUnzoomedShapes handle this
    // medium-zoom case.
    Toggle3Zoomed::new(
        batch.build(ctx),
        low_zoom.build(PerZoom::new(min_zoom_for_detail, step_size)),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Obj {
    Road(RoadID),
    // Identify crossings per road by the sorted index. When we make any mutation to a road, we
    // rebuild the world fully, so this works
    Crossing(RoadID, usize),
}

impl ObjectID for Obj {}

fn make_world(
    ctx: &EventCtx,
    app: &App,
    time_to_nearest_crossing: &BTreeMap<RoadID, Duration>,
) -> World<Obj> {
    let mut world = World::new();

    for r in main_roads(app) {
        let road = app.per_map.map.get_r(r);

        if let Some(list) = app.edits().crossings.get(&r) {
            for (idx, crossing) in list.into_iter().enumerate() {
                world
                    .add(Obj::Crossing(r, idx))
                    // The circles change size based on zoom, but for interaction, just use a fixed
                    // multiple of the road's width. It'll be a little weird.
                    .hitbox(
                        Circle::new(
                            road.center_pts.must_dist_along(crossing.dist).0,
                            3.0 * road.get_width() / 2.0,
                        )
                        .to_polygon(),
                    )
                    .drawn_in_master_batch()
                    .hover_color(colors::HOVER)
                    .zorder(1)
                    .clickable()
                    .build(ctx);
            }
        }

        world
            .add(Obj::Road(r))
            .hitbox(road.get_thick_polygon())
            .drawn_in_master_batch()
            .hover_color(colors::HOVER)
            .zorder(0)
            .clickable()
            .maybe_tooltip(if let Some(time) = time_to_nearest_crossing.get(&r) {
                Some(Text::from(Line(format!(
                    "{time} walking to the nearest crossing"
                ))))
            } else {
                None
            })
            .build(ctx);
    }

    world.initialize_hover(ctx);
    world
}

fn draw_porosity(ctx: &EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    for info in app.partitioning().all_neighbourhoods().values() {
        // I haven't seen a single road segment with multiple crossings yet. If it happens, it's
        // likely just a complex intersection and probably shouldn't count as multiple.
        let num_crossings = info
            .block
            .perimeter
            .roads
            .iter()
            .filter(|id| app.edits().crossings.contains_key(&id.road))
            .count();
        let color = if num_crossings == 0 {
            *colors::IMPERMEABLE
        } else if num_crossings == 1 {
            *colors::SEMI_PERMEABLE
        } else {
            *colors::POROUS
        };

        batch.push(color.alpha(0.5), info.block.polygon.clone());
    }
    ctx.upload(batch)
}

fn make_bottom_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    let icon = |ct: CrossingType, key: Key, name: &str| {
        let hide_color = Color::hex("#FDDA06");

        ctx.style()
            .btn_solid_primary
            .icon(Crossings::svg_path(ct))
            .image_color(
                RewriteColor::Change(hide_color, Color::CLEAR),
                ControlState::Default,
            )
            .image_color(
                RewriteColor::Change(hide_color, Color::CLEAR),
                ControlState::Disabled,
            )
            .hotkey(key)
            .disabled(app.session.crossing_type == ct)
            .tooltip_and_disabled({
                let mut txt = Text::new();
                txt.append(Line(name));
                txt.add_line(Line("Click").fg(ctx.style().text_hotkey_color));
                txt.append(Line(" a main road to add or remove a crossing"));
                txt
            })
            .build_widget(ctx, name)
    };

    let main_roads = main_roads(app);
    let mut total_crossings = 0;
    for (r, list) in &app.edits().crossings {
        if main_roads.contains(r) {
            total_crossings += list.len();
        }
    }

    Widget::row(vec![
        icon(CrossingType::Unsignalized, Key::F1, "unsignalized crossing"),
        icon(CrossingType::Signalized, Key::F2, "signalized crossing"),
        Widget::vertical_separator(ctx),
        Widget::row(vec![
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/undo.svg")
                .disabled(app.edits().previous_version.is_none())
                .hotkey(lctrl(Key::Z))
                .build_widget(ctx, "undo"),
            // TODO Only count new crossings
            format!("{total_crossings} crossings",)
                .text_widget(ctx)
                .centered_vert(),
        ]),
    ])
}

fn draw_nearest_crossing(ctx: &EventCtx, app: &App) -> (Drawable, BTreeMap<RoadID, Duration>) {
    // Consider the undirected graph of main roads. Floodfill from each crossing and count the
    // walking time to the nearest crossing, at road segment granularity.
    //
    // Note this is weird -- the nearest crossing might not be in the direction someone wants to
    // go!
    let main_roads = main_roads(app);

    let mut queue: BinaryHeap<PriorityQueueItem<Duration, RoadID>> = BinaryHeap::new();

    for r in &main_roads {
        if app.edits().crossings.contains_key(r) {
            queue.push(PriorityQueueItem {
                cost: Duration::ZERO,
                value: *r,
            });
        }
    }

    let mut cost_per_node: BTreeMap<RoadID, Duration> = BTreeMap::new();
    while let Some(current) = queue.pop() {
        if cost_per_node.contains_key(&current.value) {
            continue;
        }
        cost_per_node.insert(current.value, current.cost);

        // Walk to all main roads connected at either endpoint
        for next in app.per_map.map.get_next_roads(current.value) {
            if main_roads.contains(&next) {
                let cost = app.per_map.map.get_r(next).length() / map_model::MAX_WALKING_SPEED;
                queue.push(PriorityQueueItem {
                    cost: current.cost + cost,
                    value: next,
                });
            }
        }
    }

    let mut drawn_intersections = BTreeSet::new();
    let mut batch = GeomBatch::new();
    for (r, time) in &cost_per_node {
        let scale = if *time < Duration::minutes(1) {
            continue;
        } else if *time < Duration::minutes(2) {
            0.2
        } else if *time < Duration::minutes(3) {
            0.4
        } else if *time < Duration::minutes(4) {
            0.6
        } else if *time < Duration::minutes(5) {
            0.8
        } else {
            1.0
        };
        let color = app.cs.good_to_bad_red.eval(scale);
        let road = app.per_map.map.get_r(*r);
        batch.push(color, road.get_thick_polygon());

        // Color the intersections too, and don't worry if the colors differ. Just be less weird
        // looking.
        for i in [road.src_i, road.dst_i] {
            if drawn_intersections.contains(&i) {
                continue;
            }
            drawn_intersections.insert(i);
            batch.push(color, app.per_map.map.get_i(i).polygon.clone());
        }
    }
    (ctx.upload(batch), cost_per_node)
}
