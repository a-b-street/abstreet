use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{prettyprint_usize, Counter};
use geom::{Distance, Histogram, Statistic};
use map_model::{IntersectionID, RoadID};
use widgetry::mapspace::{ObjectID, ToggleZoomed, ToggleZoomedBuilder, World};
use widgetry::{Color, EventCtx, GeomBatch, GfxCtx, Key, Line, Text, TextExt, Widget};

use crate::tools::{cmp_count, ColorNetwork, DivergingScale};
use crate::AppLike;

#[derive(Serialize, Deserialize)]
pub struct Counts {
    pub map: MapName,
    // TODO For now, squeeze everything into this -- mode, weekday/weekend, time of day, data
    // source, etc
    pub description: String,
    // TODO Maybe per direction, movement
    pub per_road: Vec<(RoadID, usize)>,
    pub per_intersection: Vec<(IntersectionID, usize)>,
}

pub struct CompareCounts {
    pub layer: Layer,
    pub counts_a: CountsUI,
    pub counts_b: CountsUI,
    world: World<Obj>,
    relative_heatmap: ToggleZoomed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Road(RoadID),
    Intersection(IntersectionID),
}
impl ObjectID for Obj {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Layer {
    A,
    B,
    Compare,
}

pub struct CountsUI {
    // TODO Just embed Counts directly, and make that serialize a Counter?
    map: MapName,
    description: String,
    heatmap: ToggleZoomed,
    per_road: Counter<RoadID>,
    per_intersection: Counter<IntersectionID>,
}

impl CountsUI {
    fn new(ctx: &EventCtx, app: &dyn AppLike, counts: Counts) -> CountsUI {
        let mut per_road = Counter::new();
        for (r, count) in counts.per_road {
            per_road.add(r, count);
        }
        let mut per_intersection = Counter::new();
        for (i, count) in counts.per_intersection {
            per_intersection.add(i, count);
        }

        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(per_road.clone(), &app.cs().good_to_bad_red);
        colorer.ranked_intersections(per_intersection.clone(), &app.cs().good_to_bad_red);
        CountsUI {
            map: counts.map,
            description: counts.description,
            heatmap: colorer.build(ctx),
            per_road,
            per_intersection,
        }
    }

    fn empty(ctx: &EventCtx) -> Self {
        Self {
            map: MapName::new("zz", "place", "holder"),
            description: String::new(),
            heatmap: ToggleZoomed::empty(ctx),
            per_road: Counter::new(),
            per_intersection: Counter::new(),
        }
    }

    pub fn to_counts(&self) -> Counts {
        Counts {
            map: self.map.clone(),
            description: self.description.clone(),
            per_road: self.per_road.clone().consume().into_iter().collect(),
            per_intersection: self
                .per_intersection
                .clone()
                .consume()
                .into_iter()
                .collect(),
        }
    }
}

impl CompareCounts {
    pub fn new(
        ctx: &mut EventCtx,
        app: &dyn AppLike,
        counts_a: Counts,
        counts_b: Counts,
        layer: Layer,
    ) -> CompareCounts {
        let counts_a = CountsUI::new(ctx, app, counts_a);
        let counts_b = CountsUI::new(ctx, app, counts_b);

        let relative_heatmap = calculate_relative_heatmap(ctx, app, &counts_a, &counts_b);

        CompareCounts {
            layer,
            counts_a,
            counts_b,
            world: make_world(ctx, app),
            relative_heatmap,
        }
    }

    /// Start with the relative layer if anything has changed
    pub fn autoselect_layer(&mut self) {
        self.layer = if self.counts_a.per_road == self.counts_b.per_road
            && self.counts_a.per_intersection == self.counts_b.per_intersection
        {
            Layer::A
        } else {
            Layer::Compare
        };
    }

    pub fn recalculate_b(&mut self, ctx: &EventCtx, app: &dyn AppLike, counts_b: Counts) {
        self.counts_b = CountsUI::new(ctx, app, counts_b);
        self.relative_heatmap =
            calculate_relative_heatmap(ctx, app, &self.counts_a, &self.counts_b);
        if self.layer == Layer::A {
            self.autoselect_layer();
        }
    }

    pub fn empty(ctx: &EventCtx) -> CompareCounts {
        CompareCounts {
            layer: Layer::A,
            counts_a: CountsUI::empty(ctx),
            counts_b: CountsUI::empty(ctx),
            world: World::unbounded(),
            relative_heatmap: ToggleZoomed::empty(ctx),
        }
    }

    pub fn get_panel_widget(&self, ctx: &EventCtx) -> Widget {
        Widget::col(vec![
            "Show which traffic counts?".text_widget(ctx),
            // TODO Maybe tab style
            Widget::row(vec![
                ctx.style()
                    .btn_solid_primary
                    .text(&self.counts_a.description)
                    .disabled(self.layer == Layer::A)
                    .hotkey(Key::Num1)
                    .build_widget(ctx, "A counts"),
                ctx.style()
                    .btn_solid_primary
                    .text(&self.counts_b.description)
                    .disabled(self.layer == Layer::B)
                    .hotkey(Key::Num2)
                    .build_widget(ctx, "B counts"),
                ctx.style()
                    .btn_solid_primary
                    .text("Compare")
                    .disabled(self.layer == Layer::Compare)
                    .hotkey(Key::Num3)
                    .build_def(ctx),
            ]),
            ctx.style().btn_outline.text("Swap A<->B").build_def(ctx),
        ])
        .section(ctx)
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self.layer {
            Layer::A => {
                self.counts_a.heatmap.draw(g);
            }
            Layer::B => {
                self.counts_b.heatmap.draw(g);
            }
            Layer::Compare => {
                self.relative_heatmap.draw(g);
            }
        }

        // Manually generate tooltips last-minute
        if let Some(id) = self.world.get_hovering() {
            let count = match id {
                Obj::Road(r) => match self.layer {
                    Layer::A => self.counts_a.per_road.get(r),
                    Layer::B => self.counts_b.per_road.get(r),
                    Layer::Compare => {
                        g.draw_mouse_tooltip(self.relative_road_tooltip(r));
                        return;
                    }
                },
                Obj::Intersection(i) => match self.layer {
                    Layer::A => self.counts_a.per_intersection.get(i),
                    Layer::B => self.counts_b.per_intersection.get(i),
                    Layer::Compare => {
                        return;
                    }
                },
            };
            g.draw_mouse_tooltip(Text::from(Line(prettyprint_usize(count))));
        }
    }

    fn relative_road_tooltip(&self, r: RoadID) -> Text {
        let a = self.counts_a.per_road.get(r);
        let b = self.counts_b.per_road.get(r);
        let ratio = (b as f64) / (a as f64);

        let mut txt = Text::from_multiline(vec![
            Line(format!(
                "{}: {}",
                self.counts_a.description,
                prettyprint_usize(a)
            )),
            Line(format!(
                "{}: {}",
                self.counts_b.description,
                prettyprint_usize(b)
            )),
        ]);
        cmp_count(&mut txt, a, b);
        txt.add_line(Line(format!(
            "{}/{}: {:.2}",
            self.counts_b.description, self.counts_a.description, ratio
        )));
        txt
    }

    pub fn other_event(&mut self, ctx: &mut EventCtx) {
        // Just trigger hovering
        let _ = self.world.event(ctx);
    }

    /// If a button owned by this was clicked, returns the new widget to replace
    pub fn on_click(&mut self, ctx: &EventCtx, app: &dyn AppLike, x: &str) -> Option<Widget> {
        self.layer = match x {
            "A counts" => Layer::A,
            "B counts" => Layer::B,
            "Compare" => Layer::Compare,
            "Swap A<->B" => {
                std::mem::swap(&mut self.counts_a, &mut self.counts_b);
                self.relative_heatmap =
                    calculate_relative_heatmap(ctx, app, &self.counts_a, &self.counts_b);
                self.layer
            }
            _ => {
                return None;
            }
        };
        Some(self.get_panel_widget(ctx))
    }
}

fn calculate_relative_heatmap(
    ctx: &EventCtx,
    app: &dyn AppLike,
    counts_a: &CountsUI,
    counts_b: &CountsUI,
) -> ToggleZoomed {
    // First just understand the counts...
    let mut hgram_before = Histogram::new();
    for (_, cnt) in counts_a.per_road.borrow() {
        hgram_before.add(*cnt);
    }
    let mut hgram_after = Histogram::new();
    for (_, cnt) in counts_b.per_road.borrow() {
        hgram_after.add(*cnt);
    }
    info!("Road counts before: {}", hgram_before.describe());
    info!("Road counts after: {}", hgram_after.describe());

    // What's physical road width look like?
    let mut hgram_width = Histogram::new();
    for r in app.map().all_roads() {
        hgram_width.add(r.get_width());
    }
    info!("Physical road widths: {}", hgram_width.describe());

    // TODO This is still a bit arbitrary
    let scale = DivergingScale::new(Color::hex("#5D9630"), Color::WHITE, Color::hex("#A32015"))
        .range(0.0, 2.0);

    // Draw road width based on the count before
    // TODO unwrap will crash on an empty demand model
    let min_count = hgram_before.select(Statistic::Min).unwrap();
    let max_count = hgram_before.select(Statistic::Max).unwrap();

    let mut draw_roads = GeomBatch::new();
    for (r, before, after) in counts_a.per_road.clone().compare(counts_b.per_road.clone()) {
        let ratio = (after as f64) / (before as f64);
        let color = if let Some(c) = scale.eval(ratio) {
            c
        } else {
            continue;
        };

        // TODO Refactor histogram helpers
        let pct_count = (before - min_count) as f64 / (max_count - min_count) as f64;
        // TODO Pretty arbitrary. Ideally we'd hide roads and intersections underneath...
        let width = Distance::meters(2.0) + pct_count * Distance::meters(10.0);

        draw_roads.push(color, app.map().get_r(r).center_pts.make_polygons(width));
    }
    ToggleZoomedBuilder::from(draw_roads).build(ctx)
}

fn make_world(ctx: &mut EventCtx, app: &dyn AppLike) -> World<Obj> {
    let mut world = World::bounded(app.map().get_bounds());
    for r in app.map().all_roads() {
        world
            .add(Obj::Road(r.id))
            .hitbox(r.get_thick_polygon())
            .drawn_in_master_batch()
            .invisibly_hoverable()
            .build(ctx);
    }
    for i in app.map().all_intersections() {
        world
            .add(Obj::Intersection(i.id))
            .hitbox(i.polygon.clone())
            .drawn_in_master_batch()
            .invisibly_hoverable()
            .build(ctx);
    }
    world
}
