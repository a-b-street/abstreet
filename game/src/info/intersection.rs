use crate::app::App;
use crate::helpers::color_for_agent_type;
use crate::info::{header_btns, make_tabs, throughput, DataOptions, Details, Tab};
use abstutil::prettyprint_usize;
use ezgui::{
    Btn, Checkbox, Color, EventCtx, FanChart, GeomBatch, Line, PlotOptions, ScatterPlot, Series,
    Text, Widget,
};
use geom::{ArrowCap, Distance, Duration, PolyLine, Time};
use map_model::{IntersectionID, IntersectionType};
use sim::AgentType;
use std::collections::{BTreeMap, BTreeSet};

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
    rows.push(txt.draw(ctx));

    if app.opts.dev {
        rows.push(Btn::text_bg1("Open OSM node").build(
            ctx,
            format!("open {}", i.orig_id.osm_node_id),
            None,
        ));
    }

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

    let time = if opts.show_end_of_day {
        app.primary.sim.get_end_of_day()
    } else {
        app.primary.sim.time()
    };
    rows.push(throughput(
        ctx,
        app,
        "Number of crossing trips per hour",
        move |a| {
            if a.intersection_thruput.raw.is_empty() {
                a.intersection_thruput.count_per_hour(id, time)
            } else {
                a.intersection_thruput.raw_throughput(time, id)
            }
        },
        &opts,
    ));

    rows
}

pub fn delay(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    opts: &DataOptions,
    fan_chart: bool,
) -> Vec<Widget> {
    let mut rows = header(
        ctx,
        app,
        details,
        id,
        Tab::IntersectionDelay(id, opts.clone(), fan_chart),
    );
    let i = app.primary.map.get_i(id);

    assert!(i.is_traffic_signal());
    rows.push(opts.to_controls(ctx, app));
    rows.push(Checkbox::toggle(
        ctx,
        "fan chart / scatter plot",
        "fan chart",
        "scatter plot",
        None,
        fan_chart,
    ));

    rows.push(delay_plot(ctx, app, id, opts, fan_chart));

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
            Color::hex("#A3A3A3"),
            pl.make_arrow(percent * Distance::meters(3.0), ArrowCap::Triangle),
        );
        txt_batch.append(
            Text::from(Line(prettyprint_usize(demand)).fg(Color::RED))
                .render_ctx(ctx)
                .scale(0.15 / ctx.get_scale_factor())
                .centered_on(pl.middle()),
        );
    }
    batch.append(txt_batch);
    let batch = batch.translate(-bounds.min_x, -bounds.min_y).scale(zoom);

    let mut txt = Text::from(Line(format!(
        "Active agent demand at {}",
        app.primary.sim.time().ampm_tostring()
    )));
    txt.add(
        Line(format!(
            "Includes all {} active agents anywhere on the map",
            prettyprint_usize(total_demand)
        ))
        .secondary(),
    );

    rows.push(
        Widget::col(vec![txt.draw(ctx), Widget::draw_batch(ctx, batch)])
            .padding(10)
            .bg(app.cs.inner_panel)
            .outline(2.0, Color::WHITE),
    );

    rows
}

pub fn arrivals(
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
        Tab::IntersectionArrivals(id, opts.clone()),
    );

    rows.push(throughput(
        ctx,
        app,
        "Number of in-bound trips from this border",
        move |_| app.primary.sim.all_arrivals_at_border(id),
        opts,
    ));

    rows
}

fn delay_plot(
    ctx: &EventCtx,
    app: &App,
    i: IntersectionID,
    opts: &DataOptions,
    fan_chart: bool,
) -> Widget {
    let data = if opts.show_before {
        app.prebaked()
    } else {
        app.primary.sim.get_analytics()
    };
    let mut by_type: BTreeMap<AgentType, Vec<(Time, Duration)>> = AgentType::all()
        .into_iter()
        .map(|t| (t, Vec::new()))
        .collect();
    let limit = if opts.show_end_of_day {
        app.primary.sim.get_end_of_day()
    } else {
        app.primary.sim.time()
    };
    if let Some(list) = data.intersection_delays.get(&i) {
        for (t, dt, agent_type) in list {
            if *t > limit {
                break;
            }
            by_type.get_mut(agent_type).unwrap().push((*t, *dt));
        }
    }
    let series: Vec<Series<Duration>> = by_type
        .into_iter()
        .map(|(agent_type, pts)| Series {
            label: agent_type.noun().to_string(),
            color: color_for_agent_type(app, agent_type),
            pts,
        })
        .collect();
    let plot_opts = PlotOptions {
        filterable: true,
        max_x: Some(limit),
        max_y: None,
        disabled: opts.disabled_series(),
    };
    Widget::col(vec![
        Line("Delay through intersection").small_heading().draw(ctx),
        if fan_chart {
            FanChart::new(ctx, series, plot_opts)
        } else {
            ScatterPlot::new(ctx, series, plot_opts)
        },
    ])
    .padding(10)
    .bg(app.cs.inner_panel)
    .outline(2.0, Color::WHITE)
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
            ("Traffic", Tab::IntersectionTraffic(id, DataOptions::new())),
        ];
        if i.is_traffic_signal() {
            tabs.push((
                "Delay",
                Tab::IntersectionDelay(id, DataOptions::new(), false),
            ));
            tabs.push(("Current demand", Tab::IntersectionDemand(id)));
        }
        if i.is_incoming_border() {
            tabs.push((
                "Arrivals",
                Tab::IntersectionArrivals(id, DataOptions::new()),
            ));
        }
        tabs
    }));

    rows
}
