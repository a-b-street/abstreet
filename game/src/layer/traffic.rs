use crate::app::App;
use crate::common::{ColorLegend, Colorer};
use crate::layer::Layers;
use ezgui::{
    Btn, Color, Composite, EventCtx, GeomBatch, HorizontalAlignment, TextExt, VerticalAlignment,
    Widget,
};
use geom::{Distance, Duration, PolyLine};
use map_model::IntersectionID;

pub fn delay(ctx: &mut EventCtx, app: &App) -> Layers {
    // TODO explain more
    let mut colorer = Colorer::scaled(
        ctx,
        "Delay (minutes)",
        Vec::new(),
        app.cs.good_to_bad_monochrome_red.to_vec(),
        vec!["0", "1", "5", "15", "longer"],
    );

    let (per_road, per_intersection) = app.primary.sim.worst_delay(&app.primary.map);
    for (r, d) in per_road {
        let color = if d < Duration::minutes(1) {
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
        let color = if d < Duration::minutes(1) {
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

pub fn throughput(ctx: &mut EventCtx, app: &App) -> Layers {
    let mut colorer = Colorer::scaled(
        ctx,
        "Throughput (percentiles)",
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

    Layers::CumulativeThroughput(app.primary.sim.time(), colorer.build_unzoomed(ctx, app))
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
            pl.make_arrow(percent * Distance::meters(5.0)).unwrap(),
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
