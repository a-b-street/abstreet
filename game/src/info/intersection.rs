use crate::app::App;
use crate::info::{header_btns, make_tabs, throughput, DataOptions, Details, Tab};
use abstutil::prettyprint_usize;
use ezgui::{
    Color, EventCtx, GeomBatch, Line, PlotOptions, RewriteColor, ScatterPlotV2, Series, Text,
    Widget,
};
use geom::{Angle, ArrowCap, Distance, PolyLine};
use map_model::{IntersectionID, IntersectionType};
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

// TODO a fan chart might be nicer
fn delay_plot(ctx: &EventCtx, app: &App, i: IntersectionID, opts: &DataOptions) -> Widget {
    let series = if opts.show_before {
        Series {
            label: "Delay through intersection (before changes)".to_string(),
            color: Color::BLUE.alpha(0.9),
            pts: app
                .prebaked()
                .intersection_delays
                .get(&i)
                .cloned()
                .unwrap_or_else(Vec::new),
        }
    } else {
        Series {
            label: "Delay through intersection (after changes)".to_string(),
            color: Color::RED.alpha(0.9),
            pts: app
                .primary
                .sim
                .get_analytics()
                .intersection_delays
                .get(&i)
                .cloned()
                .unwrap_or_else(Vec::new),
        }
    };

    ScatterPlotV2::new(ctx, series, PlotOptions::new())
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
            tabs.push((
                "Delay",
                Tab::IntersectionDelay(id, DataOptions { show_before: false }),
            ));
            tabs.push(("Current demand", Tab::IntersectionDemand(id)));
        }
        tabs
    }));

    rows
}
