use crate::app::App;
use crate::common::{ColorLegend, Colorer};
use crate::layer::Layers;
use abstutil::{prettyprint_usize, Counter};
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, EventCtx, GeomBatch, HorizontalAlignment, Key, Line,
    RewriteColor, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Angle, ArrowCap, Distance, Duration, PolyLine};
use map_model::{IntersectionID, Traversable};

pub fn delay(ctx: &mut EventCtx, app: &App) -> Layers {
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

    Layers::WorstDelay(app.primary.sim.time(), colorer.build_unzoomed(ctx, app))
}

pub fn traffic_jams(ctx: &mut EventCtx, app: &App) -> Layers {
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

    Layers::TrafficJams(app.primary.sim.time(), colorer.build_unzoomed(ctx, app))
}

// TODO Filter by mode
pub fn throughput(ctx: &mut EventCtx, app: &App, compare: bool) -> Layers {
    if compare {
        return compare_throughput(ctx, app);
    }
    let composite = Composite::new(
        Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg").margin_right(10),
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

    let stats = &app.primary.sim.get_analytics().thruput_stats;

    // TODO If there are many duplicate counts, arbitrarily some will look heavier! Find the
    // disribution of counts instead.
    // TODO Actually display the counts at these percentiles
    // TODO Dump the data in debug mode
    {
        let roads = stats.count_per_road.sorted_asc();
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
        let intersections = stats.count_per_intersection.sorted_asc();
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

    Layers::CumulativeThroughput {
        time: app.primary.sim.time(),
        compare: false,
        unzoomed: colorer.build_both(ctx, app).unzoomed,
        composite,
    }
}

fn compare_throughput(ctx: &mut EventCtx, app: &App) -> Layers {
    let now = app.primary.sim.time();
    let after = &app.primary.sim.get_analytics().thruput_stats;
    let before = &app.prebaked().thruput_stats;

    let mut after_road = Counter::new();
    let mut before_road = Counter::new();
    {
        for (_, _, r) in &after.raw_per_road {
            after_road.inc(*r);
        }
        for (t, _, r) in &before.raw_per_road {
            if *t > now {
                break;
            }
            before_road.inc(*r);
        }
    }
    let mut after_intersection = Counter::new();
    let mut before_intersection = Counter::new();
    {
        for (_, _, i) in &after.raw_per_intersection {
            after_intersection.inc(*i);
        }
        for (t, _, i) in &before.raw_per_intersection {
            if *t > now {
                break;
            }
            before_intersection.inc(*i);
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
                Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg").margin_right(10),
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

    Layers::CumulativeThroughput {
        time: app.primary.sim.time(),
        compare: true,
        unzoomed: colorer.build_both(ctx, app).unzoomed,
        composite,
    }
}

pub fn backpressure(ctx: &mut EventCtx, app: &App) -> Layers {
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

    Layers::Backpressure(app.primary.sim.time(), colorer.build_unzoomed(ctx, app))
}

pub fn intersection_demand(ctx: &mut EventCtx, app: &App, i: IntersectionID) -> Layers {
    let mut batch = GeomBatch::new();

    let mut total_demand = 0;
    let mut demand_per_group: Vec<(&PolyLine, usize)> = Vec::new();
    for g in app.primary.map.get_traffic_signal(i).turn_groups.values() {
        let demand = app
            .primary
            .sim
            .get_analytics()
            .thruput_stats
            .demand
            .get(&g.id)
            .cloned()
            .unwrap_or(0);
        if demand > 0 {
            total_demand += demand;
            demand_per_group.push((&g.geom, demand));
        }
    }

    for (pl, demand) in demand_per_group {
        let percent = (demand as f64) / (total_demand as f64);
        batch.push(
            Color::RED,
            pl.make_arrow(percent * Distance::meters(5.0), ArrowCap::Triangle)
                .unwrap(),
        );
        batch.add_transformed(
            Text::from(Line(prettyprint_usize(demand))).render_ctx(ctx),
            pl.middle(),
            0.08,
            Angle::ZERO,
            RewriteColor::NoOp,
        );
    }

    let col = vec![
        Widget::row(vec![
            "intersection demand".draw_text(ctx),
            Btn::svg_def("../data/system/assets/tools/location.svg")
                .build(ctx, "intersection demand", None)
                .margin(5),
            Btn::text_fg("X").build_def(ctx, None).align_right(),
        ]),
        ColorLegend::row(ctx, Color::RED, "current demand"),
    ];

    Layers::IntersectionDemand(
        app.primary.sim.time(),
        i,
        batch.upload(ctx),
        Composite::new(Widget::col(col).bg(app.cs.panel_bg))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
            .build(ctx),
    )
}
