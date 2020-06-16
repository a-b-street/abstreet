use crate::app::App;
use crate::common::{ColorLegend, Colorer, Scale};
use crate::layer::{Layer, LayerOutcome};
use abstutil::Counter;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Outcome, TextExt, VerticalAlignment, Widget,
};
use geom::{Distance, Duration, Polygon, Time};
use map_model::{IntersectionID, Map, Traversable};
use maplit::btreeset;
use std::collections::BTreeSet;

// TODO Collapse this abstraction
pub struct Dynamic {
    time: Time,
    colorer: Colorer,
    name: &'static str,
}

impl Layer for Dynamic {
    fn name(&self) -> Option<&'static str> {
        Some(self.name)
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = match self.name {
                "backpressure" => Dynamic::backpressure(ctx, app),
                _ => unreachable!(),
            };
        }

        self.colorer.legend.align_above(ctx, minimap);
        if self.colorer.event(ctx) {
            return Some(LayerOutcome::Close);
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.colorer.draw(g, app);
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.colorer.unzoomed);
    }
}

impl Dynamic {
    pub fn backpressure(ctx: &mut EventCtx, app: &App) -> Dynamic {
        // TODO Explain more. Vehicle traffic only!
        // TODO Same caveats as throughput()
        let mut colorer = Colorer::scaled(
            ctx,
            "Backpressure (percentiles)",
            Vec::new(),
            app.cs.good_to_bad.to_vec(),
            vec!["0", "50", "90", "99", "100"],
        );

        let mut cnt_per_r = Counter::new();
        let mut cnt_per_i = Counter::new();

        for path in app.primary.sim.get_all_driving_paths() {
            for step in path.get_steps() {
                match step.as_traversable() {
                    Traversable::Lane(l) => {
                        cnt_per_r.inc(app.primary.map.get_l(l).parent);
                    }
                    Traversable::Turn(t) => {
                        cnt_per_i.inc(t.parent);
                    }
                }
            }
        }

        // TODO dedupe with throughput
        {
            let roads = cnt_per_r.sorted_asc();
            let p50_idx = ((roads.len() as f64) * 0.5) as usize;
            let p90_idx = ((roads.len() as f64) * 0.9) as usize;
            let p99_idx = ((roads.len() as f64) * 0.99) as usize;
            for (idx, list) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    app.cs.good_to_bad[0]
                } else if idx < p90_idx {
                    app.cs.good_to_bad[1]
                } else if idx < p99_idx {
                    app.cs.good_to_bad[2]
                } else {
                    app.cs.good_to_bad[3]
                };
                for r in list {
                    colorer.add_r(r, color, &app.primary.map);
                }
            }
        }
        {
            let intersections = cnt_per_i.sorted_asc();
            let p50_idx = ((intersections.len() as f64) * 0.5) as usize;
            let p90_idx = ((intersections.len() as f64) * 0.9) as usize;
            let p99_idx = ((intersections.len() as f64) * 0.99) as usize;
            for (idx, list) in intersections.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    app.cs.good_to_bad[0]
                } else if idx < p90_idx {
                    app.cs.good_to_bad[1]
                } else if idx < p99_idx {
                    app.cs.good_to_bad[2]
                } else {
                    app.cs.good_to_bad[3]
                };
                for i in list {
                    colorer.add_i(i, color);
                }
            }
        }

        Dynamic {
            time: app.primary.sim.time(),
            colorer: colorer.build(ctx, app),
            name: "backpressure",
        }
    }
}

// TODO Filter by mode
pub struct Throughput {
    time: Time,
    compare: bool,
    unzoomed: Drawable,
    zoomed: Drawable,
    composite: Composite,
}

impl Layer for Throughput {
    fn name(&self) -> Option<&'static str> {
        Some("throughput")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = Throughput::new(ctx, app, self.compare);
        }

        self.composite.align_above(ctx, minimap);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            None => {
                let new_compare = self.composite.has_widget("Compare before edits")
                    && self.composite.is_checked("Compare before edits");
                if new_compare != self.compare {
                    *self = Throughput::new(ctx, app, new_compare);
                    self.composite.align_above(ctx, minimap);
                }
            }
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl Throughput {
    pub fn new(ctx: &mut EventCtx, app: &App, compare: bool) -> Throughput {
        if compare {
            return Throughput::compare_throughput(ctx, app);
        }
        let composite = Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg")
                        .margin_right(10),
                    "Throughput (percentiles)".draw_text(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                if app.has_prebaked().is_some() {
                    Checkbox::text(ctx, "Compare before edits", None, false).margin_below(5)
                } else {
                    Widget::nothing()
                },
                ColorLegend::scale(
                    ctx,
                    app.cs.good_to_bad.to_vec(),
                    vec!["0%", "40%", "70%", "90%", "100%"],
                ),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        let mut colorer = Colorer::scaled(
            ctx,
            "",
            Vec::new(),
            app.cs.good_to_bad.to_vec(),
            vec!["0", "50", "90", "99", "100"],
        );

        let stats = &app.primary.sim.get_analytics();

        // TODO Actually display the counts at these percentiles
        {
            let cnt = stats.road_thruput.all_total_counts();
            let roads = cnt.sorted_asc();
            let p50_idx = ((roads.len() as f64) * 0.5) as usize;
            let p90_idx = ((roads.len() as f64) * 0.9) as usize;
            let p99_idx = ((roads.len() as f64) * 0.99) as usize;
            for (idx, list) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    app.cs.good_to_bad[0]
                } else if idx < p90_idx {
                    app.cs.good_to_bad[1]
                } else if idx < p99_idx {
                    app.cs.good_to_bad[2]
                } else {
                    app.cs.good_to_bad[3]
                };
                for r in list {
                    colorer.add_r(r, color, &app.primary.map);
                }
            }
        }
        // TODO dedupe
        {
            let cnt = stats.intersection_thruput.all_total_counts();
            let intersections = cnt.sorted_asc();
            let p50_idx = ((intersections.len() as f64) * 0.5) as usize;
            let p90_idx = ((intersections.len() as f64) * 0.9) as usize;
            let p99_idx = ((intersections.len() as f64) * 0.99) as usize;
            for (idx, list) in intersections.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    app.cs.good_to_bad[0]
                } else if idx < p90_idx {
                    app.cs.good_to_bad[1]
                } else if idx < p99_idx {
                    app.cs.good_to_bad[2]
                } else {
                    app.cs.good_to_bad[3]
                };
                for i in list {
                    colorer.add_i(i, color);
                }
            }
        }
        let colorer = colorer.build(ctx, app);

        Throughput {
            time: app.primary.sim.time(),
            compare: false,
            unzoomed: colorer.unzoomed,
            zoomed: colorer.zoomed,
            composite,
        }
    }

    fn compare_throughput(ctx: &mut EventCtx, app: &App) -> Throughput {
        let map = &app.primary.map;
        let after = app.primary.sim.get_analytics();
        let before = app.prebaked();
        let hour = app.primary.sim.time().get_parts().0;

        let mut after_road = Counter::new();
        let mut before_road = Counter::new();
        {
            for ((r, _, _), count) in &after.road_thruput.counts {
                after_road.add(*r, *count);
            }
            // TODO ew. lerp?
            for ((r, _, hr), count) in &before.road_thruput.counts {
                if *hr <= hour {
                    before_road.add(*r, *count);
                }
            }
        }
        let mut after_intersection = Counter::new();
        let mut before_intersection = Counter::new();
        {
            for ((i, _, _), count) in &after.intersection_thruput.counts {
                after_intersection.add(*i, *count);
            }
            // TODO ew. lerp?
            for ((i, _, hr), count) in &before.intersection_thruput.counts {
                if *hr <= hour {
                    before_intersection.add(*i, *count);
                }
            }
        }

        let mut unzoomed = GeomBatch::new();
        unzoomed.push(app.cs.fade_map_dark, map.get_boundary_polygon().clone());
        let mut zoomed = GeomBatch::new();

        let scale = Scale::diverging(Color::hex("#A32015"), Color::WHITE, Color::hex("#5D9630"))
            .range(0.0, 2.0)
            .ignore(0.9, 1.1);

        for (r, before, after) in before_road.compare(after_road) {
            if let Some(c) = scale.eval((after as f64) / (before as f64)) {
                unzoomed.push(c, map.get_r(r).get_thick_polygon(map).unwrap());
                zoomed.push(c.alpha(0.4), map.get_r(r).get_thick_polygon(map).unwrap());
            }
        }
        for (i, before, after) in before_intersection.compare(after_intersection) {
            if let Some(c) = scale.eval((after as f64) / (before as f64)) {
                unzoomed.push(c, map.get_i(i).polygon.clone());
                zoomed.push(c.alpha(0.4), map.get_i(i).polygon.clone());
            }
        }

        let composite = Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg")
                        .margin_right(10),
                    "Throughput (percent change)".draw_text(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Checkbox::text(ctx, "Compare before edits", None, true).margin_below(5),
                scale.make_legend(ctx, vec!["less", "-50%", "0%", "50%", "more"]),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        Throughput {
            time: app.primary.sim.time(),
            compare: true,
            unzoomed: ctx.upload(unzoomed),
            zoomed: ctx.upload(zoomed),
            composite,
        }
    }
}

pub struct Delay {
    time: Time,
    compare: bool,
    unzoomed: Drawable,
    zoomed: Drawable,
    composite: Composite,
}

impl Layer for Delay {
    fn name(&self) -> Option<&'static str> {
        Some("delay")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = Delay::new(ctx, app, self.compare);
        }

        self.composite.align_above(ctx, minimap);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            None => {
                let new_compare = self.composite.has_widget("Compare before edits")
                    && self.composite.is_checked("Compare before edits");
                if new_compare != self.compare {
                    *self = Delay::new(ctx, app, new_compare);
                    self.composite.align_above(ctx, minimap);
                }
            }
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl Delay {
    pub fn new(ctx: &mut EventCtx, app: &App, compare: bool) -> Delay {
        if compare {
            return Delay::compare_delay(ctx, app);
        }

        let map = &app.primary.map;
        let mut unzoomed = GeomBatch::new();
        unzoomed.push(app.cs.fade_map_dark, map.get_boundary_polygon().clone());
        let mut zoomed = GeomBatch::new();

        let (per_road, per_intersection) = app.primary.sim.worst_delay(&app.primary.map);
        for (r, d) in per_road {
            if d < Duration::minutes(1) {
                continue;
            }
            let color = app.cs.good_to_bad_monochrome_red[0].lerp(
                *app.cs.good_to_bad_monochrome_red.last().unwrap(),
                ((d - Duration::minutes(1)) / Duration::minutes(15)).min(1.0),
            );
            unzoomed.push(color, map.get_r(r).get_thick_polygon(map).unwrap());
            zoomed.push(
                color.alpha(0.4),
                map.get_r(r).get_thick_polygon(map).unwrap(),
            );
        }
        for (i, d) in per_intersection {
            if d < Duration::minutes(1) {
                continue;
            }
            let color = app.cs.good_to_bad_monochrome_red[0].lerp(
                *app.cs.good_to_bad_monochrome_red.last().unwrap(),
                ((d - Duration::minutes(1)) / Duration::minutes(15)).min(1.0),
            );
            unzoomed.push(color, map.get_i(i).polygon.clone());
            zoomed.push(color.alpha(0.4), map.get_i(i).polygon.clone());
        }

        let composite = Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg")
                        .margin_right(10),
                    "Delay (minutes)".draw_text(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                if app.has_prebaked().is_some() {
                    Checkbox::text(ctx, "Compare before edits", None, false).margin_below(5)
                } else {
                    Widget::nothing()
                },
                ColorLegend::gradient(
                    ctx,
                    vec![
                        app.cs.good_to_bad_monochrome_red[0],
                        *app.cs.good_to_bad_monochrome_red.last().unwrap(),
                    ],
                    vec!["1", "5", "10", "15+"],
                ),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        Delay {
            time: app.primary.sim.time(),
            compare: false,
            unzoomed: ctx.upload(unzoomed),
            zoomed: ctx.upload(zoomed),
            composite,
        }
    }

    // TODO Needs work.
    fn compare_delay(ctx: &mut EventCtx, app: &App) -> Delay {
        let map = &app.primary.map;
        let mut unzoomed = GeomBatch::new();
        unzoomed.push(app.cs.fade_map_dark, map.get_boundary_polygon().clone());
        let mut zoomed = GeomBatch::new();
        let red = Color::hex("#A32015");
        let green = Color::hex("#5D9630");

        let results = app
            .primary
            .sim
            .get_analytics()
            .compare_delay(app.primary.sim.time(), app.prebaked());
        if !results.is_empty() {
            let fastest = results.iter().min_by_key(|(_, dt)| *dt).unwrap().1;
            let slowest = results.iter().max_by_key(|(_, dt)| *dt).unwrap().1;

            for (i, dt) in results {
                let color = if dt < Duration::ZERO {
                    green.lerp(Color::WHITE, 1.0 - (dt / fastest))
                } else {
                    Color::WHITE.lerp(red, dt / slowest)
                };
                unzoomed.push(color, map.get_i(i).polygon.clone());
                zoomed.push(color.alpha(0.4), map.get_i(i).polygon.clone());
            }
        }

        let composite = Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg")
                        .margin_right(10),
                    "Delay".draw_text(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Checkbox::text(ctx, "Compare before edits", None, true).margin_below(5),
                ColorLegend::gradient(
                    ctx,
                    vec![green, Color::WHITE, red],
                    vec!["faster", "same", "slower"],
                ),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        Delay {
            time: app.primary.sim.time(),
            compare: true,
            unzoomed: ctx.upload(unzoomed),
            zoomed: ctx.upload(zoomed),
            composite,
        }
    }
}

pub struct TrafficJams {
    time: Time,
    unzoomed: Drawable,
    zoomed: Drawable,
    composite: Composite,
}

impl Layer for TrafficJams {
    fn name(&self) -> Option<&'static str> {
        Some("traffic jams")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = TrafficJams::new(ctx, app);
        }

        self.composite.align_above(ctx, minimap);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            None => {}
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl TrafficJams {
    pub fn new(ctx: &mut EventCtx, app: &App) -> TrafficJams {
        // TODO Use cached delayed_intersections?
        let mut unzoomed = GeomBatch::new();
        unzoomed.push(
            app.cs.fade_map_dark,
            app.primary.map.get_boundary_polygon().clone(),
        );
        let mut zoomed = GeomBatch::new();
        let mut cnt = 0;
        // TODO Maybe look for intersections with delay > 5m, then expand out while roads have
        // delay of at least 1m?
        for (epicenter, boundary) in cluster_jams(
            &app.primary.map,
            app.primary.sim.delayed_intersections(Duration::minutes(5)),
        ) {
            cnt += 1;
            unzoomed.push(Color::RED, boundary.to_outline(Distance::meters(5.0)));
            unzoomed.push(Color::RED.alpha(0.7), boundary.clone());
            unzoomed.push(Color::WHITE, epicenter.clone());

            zoomed.push(
                Color::RED.alpha(0.4),
                boundary.to_outline(Distance::meters(5.0)),
            );
            zoomed.push(Color::RED.alpha(0.3), boundary);
            zoomed.push(Color::WHITE.alpha(0.4), epicenter);
        }

        let composite = Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg")
                        .margin_right(10),
                    "Traffic jams".draw_text(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                format!("{} jams detected", cnt).draw_text(ctx),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        TrafficJams {
            time: app.primary.sim.time(),
            unzoomed: ctx.upload(unzoomed),
            zoomed: ctx.upload(zoomed),
            composite,
        }
    }
}

struct Jam {
    epicenter: IntersectionID,
    members: BTreeSet<IntersectionID>,
}

// (Epicenter, entire shape)
fn cluster_jams(map: &Map, problems: Vec<(IntersectionID, Time)>) -> Vec<(Polygon, Polygon)> {
    let mut jams: Vec<Jam> = Vec::new();
    // The delay itself doesn't matter, as long as they're sorted.
    for (i, _) in problems {
        // Is this connected to an existing problem?
        if let Some(ref mut jam) = jams.iter_mut().find(|j| j.adjacent_to(map, i)) {
            jam.members.insert(i);
        } else {
            jams.push(Jam {
                epicenter: i,
                members: btreeset! { i },
            });
        }
    }

    jams.into_iter()
        .map(|jam| {
            (
                map.get_i(jam.epicenter).polygon.clone(),
                Polygon::convex_hull(
                    jam.members
                        .into_iter()
                        .map(|i| map.get_i(i).polygon.clone())
                        .collect(),
                ),
            )
        })
        .collect()
}

impl Jam {
    fn adjacent_to(&self, map: &Map, i: IntersectionID) -> bool {
        for r in &map.get_i(i).roads {
            let r = map.get_r(*r);
            if self.members.contains(&r.src_i) || self.members.contains(&r.dst_i) {
                return true;
            }
        }
        false
    }
}
