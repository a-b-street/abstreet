use crate::app::App;
use crate::info::{header_btns, make_tabs, throughput, DataOptions, Details, Tab};
use abstutil::prettyprint_usize;
use ezgui::{
    Color, EventCtx, GeomBatch, Line, LinePlot, PlotOptions, RewriteColor, Series, Text, Widget,
};
use geom::{Angle, ArrowCap, Distance, PolyLine};
use geom::{Duration, Statistic, Time};
use map_model::{IntersectionID, IntersectionType};
use sim::Analytics;
use std::collections::BTreeSet;

pub fn info(ctx: &EventCtx, app: &App, details: &mut Details, id: IntersectionID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::IntersectionInfo(id));
    let i = app.primary.map.get_i(id);

    let mut txt = Text::from(Line("Connecting"));
    let mut road_names = BTreeSet::new();
    for r in &i.roads {
        road_names.insert(app.primary.map.get_r(*r).get_name());
    }
    for r in road_names {
        // TODO The spacing is ignored, so use -
        txt.add(Line(format!("- {}", r)));
    }
    if app.opts.dev {
        txt.add(Line(format!("OSM node ID: {}", i.orig_id.osm_node_id)).secondary());
    }
    rows.push(txt.draw(ctx));

    rows
}

pub fn traffic(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    opts: &DataOptions,
) -> Vec<Widget> {
    let mut rows = header(
        ctx,
        app,
        details,
        id,
        Tab::IntersectionTraffic(id, opts.clone()),
    );

    let mut txt = Text::new();

    txt.add(Line(format!(
        "Since midnight: {} agents crossed",
        prettyprint_usize(
            app.primary
                .sim
                .get_analytics()
                .intersection_thruput
                .total_for(id)
        )
    )));
    rows.push(txt.draw(ctx));

    rows.push(opts.to_controls(ctx, app));

    rows.push(
        throughput(
            ctx,
            app,
            move |a| a.intersection_thruput.count_per_hour(id),
            opts.show_before,
        )
        .margin(10),
    );

    rows
}

pub fn delay(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    opts: &DataOptions,
) -> Vec<Widget> {
    let mut rows = header(
        ctx,
        app,
        details,
        id,
        Tab::IntersectionDelay(id, opts.clone()),
    );
    let i = app.primary.map.get_i(id);

    assert!(i.is_traffic_signal());
    rows.push(opts.to_controls(ctx, app));

    rows.push(delay_plot(ctx, app, id, opts).margin(10));

    rows
}

pub fn current_demand(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::IntersectionDemand(id));

    let mut total_demand = 0;
    let mut demand_per_group: Vec<(&PolyLine, usize)> = Vec::new();
    for g in app.primary.map.get_traffic_signal(id).turn_groups.values() {
        let demand = app
            .primary
            .sim
            .get_analytics()
            .demand
            .get(&g.id)
            .cloned()
            .unwrap_or(0);
        if demand > 0 {
            total_demand += demand;
            demand_per_group.push((&g.geom, demand));
        }
    }

    let mut batch = GeomBatch::new();
    let polygon = app.primary.map.get_i(id).polygon.clone();
    let bounds = polygon.get_bounds();
    // Pick a zoom so that we fit a fixed width in pixels
    let zoom = 300.0 / bounds.width();
    batch.push(app.cs.normal_intersection, polygon);

    let mut txt_batch = GeomBatch::new();
    for (pl, demand) in demand_per_group {
        let percent = (demand as f64) / (total_demand as f64);
        batch.push(
            Color::RED,
            pl.make_arrow(percent * Distance::meters(3.0), ArrowCap::Triangle)
                .unwrap(),
        );
        txt_batch.add_transformed(
            Text::from(Line(prettyprint_usize(demand))).render_ctx(ctx),
            pl.middle(),
            0.15 / ctx.get_scale_factor(),
            Angle::ZERO,
            RewriteColor::NoOp,
        );
    }
    batch.append(txt_batch);
    let mut transformed_batch = GeomBatch::new();
    for (color, poly) in batch.consume() {
        transformed_batch.fancy_push(
            color,
            poly.translate(-bounds.min_x, -bounds.min_y).scale(zoom),
        );
    }

    let mut txt = Text::from(Line(format!(
        "How many active trips will cross this intersection?"
    )));
    txt.add(Line(format!("Total: {}", prettyprint_usize(total_demand))).secondary());
    rows.push(txt.draw(ctx));
    rows.push(Widget::draw_batch(ctx, transformed_batch));

    rows
}

fn delay_plot(ctx: &EventCtx, app: &App, i: IntersectionID, opts: &DataOptions) -> Widget {
    let get_data = |a: &Analytics, t: Time| {
        let mut series: Vec<(Statistic, Vec<(Time, Duration)>)> = Statistic::all()
            .into_iter()
            .map(|stat| (stat, Vec::new()))
            .collect();
        for (t, distrib) in a.intersection_delays_bucketized(t, i, Duration::hours(1)) {
            for (stat, pts) in series.iter_mut() {
                if distrib.count() == 0 {
                    pts.push((t, Duration::ZERO));
                } else {
                    pts.push((t, distrib.select(*stat)));
                }
            }
        }
        series
    };

    let mut all_series = Vec::new();
    for (idx, (stat, pts)) in get_data(app.primary.sim.get_analytics(), app.primary.sim.time())
        .into_iter()
        .enumerate()
    {
        all_series.push(Series {
            label: stat.to_string(),
            color: app.cs.rotating_color_plot(idx),
            pts,
        });
    }
    if opts.show_before {
        for (idx, (stat, pts)) in get_data(app.prebaked(), app.primary.sim.get_end_of_day())
            .into_iter()
            .enumerate()
        {
            all_series.push(Series {
                label: format!("{} (before changes)", stat),
                color: app.cs.rotating_color_plot(idx).alpha(0.3),
                pts,
            });
        }
    }

    LinePlot::new(ctx, "delay", all_series, PlotOptions::new())
}

fn header(
    ctx: &EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    tab: Tab,
) -> Vec<Widget> {
    let mut rows = vec![];

    let i = app.primary.map.get_i(id);

    let label = match i.intersection_type {
        IntersectionType::StopSign => format!("{} (Stop signs)", id),
        IntersectionType::TrafficSignal => format!("{} (Traffic signals)", id),
        IntersectionType::Border => format!("Border #{}", id.0),
        IntersectionType::Construction => format!("{} (under construction)", id),
    };
    rows.push(Widget::row(vec![
        Line(label).small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    rows.push(make_tabs(ctx, &mut details.hyperlinks, tab, {
        let mut tabs = vec![
            ("Info", Tab::IntersectionInfo(id)),
            (
                "Traffic",
                Tab::IntersectionTraffic(id, DataOptions::new(app)),
            ),
        ];
        if i.is_traffic_signal() {
            tabs.push(("Delay", Tab::IntersectionDelay(id, DataOptions::new(app))));
            tabs.push(("Current demand", Tab::IntersectionDemand(id)));
        }
        tabs
    }));

    rows
}
