use std::collections::BTreeSet;

use maplit::btreeset;

use abstutil::{prettyprint_usize, Counter};
use geom::{Circle, Distance, Duration, Polygon, Pt2D, Time};
use map_model::{IntersectionID, Map, Traversable};
use sim::VehicleType;
use widgetry::{
    Btn, Checkbox, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Panel, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::common::{ColorLegend, ColorNetwork, DivergingScale};
use crate::helpers::ID;
use crate::layer::{Layer, LayerOutcome};
use crate::render::unzoomed_agent_radius;

pub struct Backpressure {
    time: Time,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for Backpressure {
    fn name(&self) -> Option<&'static str> {
        Some("backpressure")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = Backpressure::new(ctx, app);
        }

        Layer::simple_event(ctx, minimap, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
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

impl Backpressure {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Backpressure {
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

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Backpressure".draw_text(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", Key::Escape)
                    .align_right(),
            ]),
            Text::from(
                Line("This counts all active trips passing through a road in the future")
                    .secondary(),
            )
            .wrap_to_pct(ctx, 15)
            .draw(ctx),
            ColorLegend::gradient(
                ctx,
                &app.cs.good_to_bad_red,
                vec!["lowest count", "highest"],
            ),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        let mut colorer = ColorNetwork::new(app);
        colorer.pct_roads(cnt_per_r, &app.cs.good_to_bad_red);
        colorer.pct_intersections(cnt_per_i, &app.cs.good_to_bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        Backpressure {
            time: app.primary.sim.time(),
            unzoomed,
            zoomed,
            panel,
        }
    }
}

// TODO Filter by mode
pub struct Throughput {
    time: Time,
    tooltip: Option<Text>,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for Throughput {
    fn name(&self) -> Option<&'static str> {
        Some("throughput")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        let mut recalc_tooltip = false;
        if app.primary.sim.time() != self.time {
            *self = Throughput::new(ctx, app);
            recalc_tooltip = true;
        }

        // Show a tooltip with count, only when unzoomed
        if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            if ctx.redo_mouseover() || recalc_tooltip {
                self.tooltip = None;
                match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    Some(ID::Road(r)) => {
                        let cnt = app.primary.sim.get_analytics().road_thruput.total_for(r);
                        if cnt > 0 {
                            self.tooltip = Some(Text::from(Line(prettyprint_usize(cnt))));
                        }
                    }
                    Some(ID::Intersection(i)) => {
                        let cnt = app
                            .primary
                            .sim
                            .get_analytics()
                            .intersection_thruput
                            .total_for(i);
                        if cnt > 0 {
                            self.tooltip = Some(Text::from(Line(prettyprint_usize(cnt))));
                        }
                    }
                    _ => {}
                }
            }
        } else {
            self.tooltip = None;
        }

        self.panel.align_above(ctx, minimap);
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                return Some(LayerOutcome::Replace(Box::new(CompareThroughput::new(
                    ctx, app,
                ))));
            }
            _ => {}
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl Throughput {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Throughput {
        let stats = &app.primary.sim.get_analytics();
        let road_counter = stats.road_thruput.all_total_counts();
        let intersection_counter = stats.intersection_thruput.all_total_counts();
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Throughput".draw_text(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", Key::Escape)
                    .align_right(),
            ]),
            Text::from(Line("This counts all people crossing since midnight").secondary())
                .wrap_to_pct(ctx, 15)
                .draw(ctx),
            if app.has_prebaked().is_some() {
                Checkbox::switch(ctx, "Compare before proposal", None, false)
            } else {
                Widget::nothing()
            },
            ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["0", "highest"]),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        let mut colorer = ColorNetwork::new(app);
        colorer.ranked_roads(road_counter, &app.cs.good_to_bad_red);
        colorer.ranked_intersections(intersection_counter, &app.cs.good_to_bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        Throughput {
            time: app.primary.sim.time(),
            tooltip: None,
            unzoomed,
            zoomed,
            panel,
        }
    }
}

pub struct CompareThroughput {
    time: Time,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for CompareThroughput {
    fn name(&self) -> Option<&'static str> {
        Some("throughput")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = CompareThroughput::new(ctx, app);
        }

        self.panel.align_above(ctx, minimap);
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                return Some(LayerOutcome::Replace(Box::new(Throughput::new(ctx, app))));
            }
            _ => {}
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
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

impl CompareThroughput {
    pub fn new(ctx: &mut EventCtx, app: &App) -> CompareThroughput {
        let after = app.primary.sim.get_analytics();
        let before = app.prebaked();
        let hour = app.primary.sim.time().get_hours();

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

        let mut colorer = ColorNetwork::new(app);

        let scale = DivergingScale::new(Color::hex("#5D9630"), Color::WHITE, Color::hex("#A32015"))
            .range(0.0, 2.0)
            .ignore(0.7, 1.3);

        for (r, before, after) in before_road.compare(after_road) {
            if let Some(c) = scale.eval((after as f64) / (before as f64)) {
                colorer.add_r(r, c);
            }
        }
        for (i, before, after) in before_intersection.compare(after_intersection) {
            if let Some(c) = scale.eval((after as f64) / (before as f64)) {
                colorer.add_i(i, c);
            }
        }

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Relative Throughput".draw_text(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", Key::Escape)
                    .align_right(),
            ]),
            Checkbox::switch(ctx, "Compare before proposal", None, true),
            scale.make_legend(ctx, vec!["less traffic", "same", "more"]),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);
        let (unzoomed, zoomed) = colorer.build(ctx);

        CompareThroughput {
            time: app.primary.sim.time(),
            unzoomed,
            zoomed,
            panel,
        }
    }
}

pub struct TrafficJams {
    time: Time,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for TrafficJams {
    fn name(&self) -> Option<&'static str> {
        Some("traffic jams")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = TrafficJams::new(ctx, app);
        }

        Layer::simple_event(ctx, minimap, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
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
        for (epicenter, boundary) in cluster_jams(
            &app.primary.map,
            app.primary.sim.delayed_intersections(Duration::minutes(5)),
        ) {
            cnt += 1;
            unzoomed.push(
                Color::RED,
                boundary.to_outline(Distance::meters(5.0)).unwrap(),
            );
            unzoomed.push(Color::RED.alpha(0.5), boundary.clone());
            unzoomed.push(Color::WHITE, epicenter.clone());

            zoomed.push(
                Color::RED.alpha(0.4),
                boundary.to_outline(Distance::meters(5.0)).unwrap(),
            );
            zoomed.push(Color::RED.alpha(0.3), boundary);
            zoomed.push(Color::WHITE.alpha(0.4), epicenter);
        }

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Traffic jams".draw_text(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", Key::Escape)
                    .align_right(),
            ]),
            Text::from(
                Line("A jam starts when delay exceeds 5 mins, then spreads out").secondary(),
            )
            .wrap_to_pct(ctx, 15)
            .draw(ctx),
            format!("{} jams detected", cnt).draw_text(ctx),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        TrafficJams {
            time: app.primary.sim.time(),
            unzoomed: ctx.upload(unzoomed),
            zoomed: ctx.upload(zoomed),
            panel,
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
                Polygon::convex_hull(jam.all_polygons(map)),
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

    fn all_polygons(self, map: &Map) -> Vec<Polygon> {
        let mut polygons = Vec::new();
        for i in self.members {
            polygons.push(map.get_i(i).polygon.clone());
        }
        polygons
    }
}

// Shows how long each agent has been waiting in one spot.
pub struct Delay {
    time: Time,
    unzoomed: Drawable,
    panel: Panel,
}

impl Layer for Delay {
    fn name(&self) -> Option<&'static str> {
        Some("delay")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = Delay::new(ctx, app);
        }

        self.panel.align_above(ctx, minimap);
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl Delay {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Delay {
        let mut delays = app.primary.sim.all_waiting_people();
        let mut unzoomed = GeomBatch::new();
        unzoomed.push(
            app.cs.fade_map_dark,
            app.primary.map.get_boundary_polygon().clone(),
        );
        // A bit of copied code from draw_unzoomed_agents
        let car_circle = Circle::new(
            Pt2D::new(0.0, 0.0),
            unzoomed_agent_radius(Some(VehicleType::Car)),
        )
        .to_polygon();
        let ped_circle = Circle::new(Pt2D::new(0.0, 0.0), unzoomed_agent_radius(None)).to_polygon();
        for agent in app.primary.sim.get_unzoomed_agents(&app.primary.map) {
            if let Some(delay) = agent.person.and_then(|p| delays.remove(&p)) {
                let color = app
                    .cs
                    .good_to_bad_red
                    .eval((delay / Duration::minutes(15)).min(1.0));
                if agent.vehicle_type.is_some() {
                    unzoomed.push(color, car_circle.translate(agent.pos.x(), agent.pos.y()));
                } else {
                    unzoomed.push(color, ped_circle.translate(agent.pos.x(), agent.pos.y()));
                }
            }
        }

        Delay {
            time: app.primary.sim.time(),
            unzoomed: ctx.upload(unzoomed),
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                    "Delay per agent (minutes)".draw_text(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["0", "5", "10", "15+"]),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
            .build(ctx),
        }
    }
}
