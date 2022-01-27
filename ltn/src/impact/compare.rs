// TODO Some of this may warrant a standalone tool, or being in game/devtools

use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{prettyprint_usize, Counter};
use geom::{Distance, Histogram, Statistic};
use map_gui::tools::{cmp_count, ColorNetwork, DivergingScale};
use map_model::{IntersectionID, RoadID};
use widgetry::mapspace::{ObjectID, ToggleZoomed, ToggleZoomedBuilder, World};
use widgetry::{Choice, Color, EventCtx, GeomBatch, GfxCtx, Line, Panel, Text, TextExt, Widget};

use super::App;

// TODO
// 1) Just make this as something that can be embedded in a UI
// 2) Refactor the impact prediction to use this
// 3) Make a new UI with a file picker and CLI shortcuts
// 4) See if we can dedupe requests in the impact prediction -- using this tool to validate
// 5) Download the sensor data and get it in this format (and maybe filter simulated data to only
//    match roads we have)

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
    layer: Layer,
    counts_a: CountsUI,
    counts_b: CountsUI,
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
enum Layer {
    A,
    B,
    Compare,
}

struct CountsUI {
    description: String,
    heatmap: ToggleZoomed,
    per_road: Counter<RoadID>,
    per_intersection: Counter<IntersectionID>,
}

impl CountsUI {
    fn new(ctx: &EventCtx, app: &App, counts: Counts) -> CountsUI {
        let mut per_road = Counter::new();
        for (r, count) in counts.per_road {
            per_road.add(r, count);
        }
        let mut per_intersection = Counter::new();
        for (i, count) in counts.per_intersection {
            per_intersection.add(i, count);
        }

        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(per_road.clone(), &app.cs.good_to_bad_red);
        colorer.ranked_intersections(per_intersection.clone(), &app.cs.good_to_bad_red);
        CountsUI {
            description: counts.description,
            heatmap: colorer.build(ctx),
            per_road,
            per_intersection,
        }
    }

    fn tooltip(&self, id: Obj) -> Text {
        Text::from(Line(prettyprint_usize(match id {
            Obj::Road(r) => self.per_road.get(r),
            Obj::Intersection(i) => self.per_intersection.get(i),
        })))
    }
}

impl CompareCounts {
    pub fn new(ctx: &mut EventCtx, app: &App, counts_a: Counts, counts_b: Counts) -> CompareCounts {
        let counts_a = CountsUI::new(ctx, app, counts_a);
        let counts_b = CountsUI::new(ctx, app, counts_b);

        // Start with the relative layer if anything has changed
        let layer = {
            if counts_a.per_road == counts_b.per_road
                && counts_a.per_intersection == counts_b.per_intersection
            {
                Layer::A
            } else {
                Layer::Compare
            }
        };
        let relative_heatmap = calculate_relative_heatmap(ctx, app, &counts_a, &counts_b);

        CompareCounts {
            layer,
            counts_a,
            counts_b,
            world: make_world(ctx, app),
            relative_heatmap,
        }
    }

    pub fn get_panel_widget(&self, ctx: &EventCtx) -> Widget {
        Widget::row(vec![
            "Show counts:"
                .text_widget(ctx)
                .centered_vert()
                .margin_right(20),
            Widget::dropdown(
                ctx,
                "layer",
                self.layer,
                vec![
                    Choice::new(&self.counts_a.description, Layer::A),
                    Choice::new(&self.counts_b.description, Layer::B),
                    Choice::new("compare", Layer::Compare),
                ],
            ),
        ])
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
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
        let before = self.counts_a.per_road.get(r);
        let after = self.counts_b.per_road.get(r);
        let ratio = (after as f64) / (before as f64);

        let mut txt = Text::from_multiline(vec![
            Line(format!("Before: {}", prettyprint_usize(before))),
            Line(format!("After: {}", prettyprint_usize(after))),
        ]);
        cmp_count(&mut txt, before, after);
        txt.add_line(Line(format!("After/before: {:.2}", ratio)));
        txt
    }

    pub fn other_event(&mut self, ctx: &mut EventCtx) {
        // Just trigger hovering
        let _ = self.world.event(ctx);
    }

    /// True if the change was for controls owned by CompareCounts
    pub fn panel_changed(&mut self, panel: &Panel) -> bool {
        let layer = panel.dropdown_value("layer");
        if layer != self.layer {
            self.layer = layer;
            return true;
        }
        false
    }
}

fn calculate_relative_heatmap(
    ctx: &EventCtx,
    app: &App,
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
    for r in app.map.all_roads() {
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

        draw_roads.push(color, app.map.get_r(r).center_pts.make_polygons(width));
    }
    ToggleZoomedBuilder::from(draw_roads).build(ctx)
}

fn make_world(ctx: &mut EventCtx, app: &App) -> World<Obj> {
    let mut world = World::bounded(app.map.get_bounds());
    for r in app.map.all_roads() {
        world
            .add(Obj::Road(r.id))
            .hitbox(r.get_thick_polygon())
            .drawn_in_master_batch()
            .invisibly_hoverable()
            .build(ctx);
    }
    for i in app.map.all_intersections() {
        world
            .add(Obj::Intersection(i.id))
            .hitbox(i.polygon.clone())
            .drawn_in_master_batch()
            .invisibly_hoverable()
            .build(ctx);
    }
    world
}
