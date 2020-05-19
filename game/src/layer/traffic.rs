use crate::app::App;
use crate::common::{ColorLegend, Colorer};
use crate::layer::{Layer, LayerOutcome};
use abstutil::Counter;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key,
    Outcome, TextExt, VerticalAlignment, Widget,
};
use geom::{Duration, Time};
use map_model::Traversable;

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
                "delay" => Dynamic::delay(ctx, app),
                "worst traffic jams" => Dynamic::traffic_jams(ctx, app),
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
    pub fn delay(ctx: &mut EventCtx, app: &App) -> Dynamic {
        // TODO explain more
        let mut colorer = Colorer::scaled(
            ctx,
            "Delay (minutes)",
            Vec::new(),
            app.cs.good_to_bad_monochrome_red.to_vec(),
            vec!["0.5", "1", "5", "15", "longer"],
        );

        let (per_road, per_intersection) = app.primary.sim.worst_delay(&app.primary.map);
        for (r, d) in per_road {
            let color = if d < Duration::seconds(30.0) {
                continue;
            } else if d < Duration::minutes(1) {
                app.cs.good_to_bad_monochrome_red[0]
            } else if d < Duration::minutes(5) {
                app.cs.good_to_bad_monochrome_red[1]
            } else if d < Duration::minutes(15) {
                app.cs.good_to_bad_monochrome_red[2]
            } else {
                app.cs.good_to_bad_monochrome_red[3]
            };
            colorer.add_r(r, color, &app.primary.map);
        }
        for (i, d) in per_intersection {
            let color = if d < Duration::seconds(30.0) {
                continue;
            } else if d < Duration::minutes(1) {
                app.cs.good_to_bad_monochrome_red[0]
            } else if d < Duration::minutes(5) {
                app.cs.good_to_bad_monochrome_red[1]
            } else if d < Duration::minutes(15) {
                app.cs.good_to_bad_monochrome_red[2]
            } else {
                app.cs.good_to_bad_monochrome_red[3]
            };
            colorer.add_i(i, color);
        }

        Dynamic {
            time: app.primary.sim.time(),
            colorer: colorer.build_unzoomed(ctx, app),
            name: "delay",
        }
    }

    pub fn traffic_jams(ctx: &mut EventCtx, app: &App) -> Dynamic {
        let jams = app.primary.sim.delayed_intersections(Duration::minutes(5));

        // TODO Silly colors. Weird way of presenting this information. Epicenter + radius?
        let others = Color::hex("#7FFA4D");
        let early = Color::hex("#F4DA22");
        let earliest = Color::hex("#EB5757");
        let mut colorer = Colorer::discrete(
            ctx,
            format!("{} traffic jams", jams.len()),
            Vec::new(),
            vec![
                ("longest lasting", earliest),
                ("recent problems", early),
                ("others", others),
            ],
        );

        for (idx, (i, _)) in jams.into_iter().enumerate() {
            if idx == 0 {
                colorer.add_i(i, earliest);
            } else if idx <= 5 {
                colorer.add_i(i, early);
            } else {
                colorer.add_i(i, others);
            }
        }

        Dynamic {
            time: app.primary.sim.time(),
            colorer: colorer.build_unzoomed(ctx, app),
            name: "worst traffic jams",
        }
    }

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
            for (idx, r) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    app.cs.good_to_bad[0]
                } else if idx < p90_idx {
                    app.cs.good_to_bad[1]
                } else if idx < p99_idx {
                    app.cs.good_to_bad[2]
                } else {
                    app.cs.good_to_bad[3]
                };
                colorer.add_r(*r, color, &app.primary.map);
            }
        }
        {
            let intersections = cnt_per_i.sorted_asc();
            let p50_idx = ((intersections.len() as f64) * 0.5) as usize;
            let p90_idx = ((intersections.len() as f64) * 0.9) as usize;
            let p99_idx = ((intersections.len() as f64) * 0.99) as usize;
            for (idx, i) in intersections.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    app.cs.good_to_bad[0]
                } else if idx < p90_idx {
                    app.cs.good_to_bad[1]
                } else if idx < p99_idx {
                    app.cs.good_to_bad[2]
                } else {
                    app.cs.good_to_bad[3]
                };
                colorer.add_i(*i, color);
            }
        }

        Dynamic {
            time: app.primary.sim.time(),
            colorer: colorer.build_unzoomed(ctx, app),
            name: "backpressure",
        }
    }
}

// TODO Filter by mode
pub struct Throughput {
    time: Time,
    compare: bool,
    unzoomed: Drawable,
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

        // TODO If there are many duplicate counts, arbitrarily some will look heavier! Find the
        // disribution of counts instead.
        // TODO Actually display the counts at these percentiles
        // TODO Dump the data in debug mode
        {
            let cnt = stats.road_thruput.all_total_counts();
            let roads = cnt.sorted_asc();
            let p50_idx = ((roads.len() as f64) * 0.5) as usize;
            let p90_idx = ((roads.len() as f64) * 0.9) as usize;
            let p99_idx = ((roads.len() as f64) * 0.99) as usize;
            for (idx, r) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    app.cs.good_to_bad[0]
                } else if idx < p90_idx {
                    app.cs.good_to_bad[1]
                } else if idx < p99_idx {
                    app.cs.good_to_bad[2]
                } else {
                    app.cs.good_to_bad[3]
                };
                colorer.add_r(*r, color, &app.primary.map);
            }
        }
        // TODO dedupe
        {
            let cnt = stats.intersection_thruput.all_total_counts();
            let intersections = cnt.sorted_asc();
            let p50_idx = ((intersections.len() as f64) * 0.5) as usize;
            let p90_idx = ((intersections.len() as f64) * 0.9) as usize;
            let p99_idx = ((intersections.len() as f64) * 0.99) as usize;
            for (idx, i) in intersections.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    app.cs.good_to_bad[0]
                } else if idx < p90_idx {
                    app.cs.good_to_bad[1]
                } else if idx < p99_idx {
                    app.cs.good_to_bad[2]
                } else {
                    app.cs.good_to_bad[3]
                };
                colorer.add_i(*i, color);
            }
        }

        Throughput {
            time: app.primary.sim.time(),
            compare: false,
            unzoomed: colorer.build_both(ctx, app).unzoomed,
            composite,
        }
    }

    fn compare_throughput(ctx: &mut EventCtx, app: &App) -> Throughput {
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

        // Diverging
        let gradient = colorous::RED_YELLOW_GREEN;
        let num_colors = 4;
        let mut colors: Vec<Color> = (0..num_colors)
            .map(|i| {
                let c = gradient.eval_rational(i, num_colors);
                Color::rgb(c.r as usize, c.g as usize, c.b as usize)
            })
            .collect();
        colors.reverse();
        // TODO But the yellow is confusing. Two greens, two reds
        colors[2] = Color::hex("#F27245");
        let mut colorer = Colorer::scaled(
            ctx,
            "",
            Vec::new(),
            colors.clone(),
            vec!["", "", "", "", ""],
        );

        for (r, before, after) in before_road.compare(after_road) {
            let pct_change = (after as f64) / (before as f64);
            let color = if pct_change < 0.5 {
                colors[0]
            } else if pct_change < 0.1 {
                colors[1]
            } else if pct_change < 1.1 {
                // Just filter it out
                continue;
            } else if pct_change < 1.5 {
                colors[2]
            } else {
                colors[3]
            };
            colorer.add_r(r, color, &app.primary.map);
        }
        for (i, before, after) in before_intersection.compare(after_intersection) {
            let pct_change = (after as f64) / (before as f64);
            let color = if pct_change < 0.5 {
                colors[0]
            } else if pct_change < 0.1 {
                colors[1]
            } else if pct_change < 1.1 {
                // Just filter it out
                continue;
            } else if pct_change < 1.5 {
                colors[2]
            } else {
                colors[3]
            };
            colorer.add_i(i, color);
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
                ColorLegend::scale(ctx, colors, vec!["less", "-50%", "0%", "50%", "more"]),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        Throughput {
            time: app.primary.sim.time(),
            compare: true,
            unzoomed: colorer.build_both(ctx, app).unzoomed,
            composite,
        }
    }
}
