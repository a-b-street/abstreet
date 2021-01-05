use std::collections::{BTreeMap, BTreeSet};

use abstutil::prettyprint_usize;
use geom::{ArrowCap, Distance, Duration, PolyLine, Polygon, Time};
use map_gui::options::TrafficSignalStyle;
use map_gui::render::traffic_signal::draw_signal_stage;
use map_model::{IntersectionID, IntersectionType, StageType};
use sim::AgentType;
use widgetry::{
    Btn, Checkbox, Color, DrawWithTooltips, EventCtx, FanChart, GeomBatch, Line, PlotOptions,
    ScatterPlot, Series, Text, Widget,
};

use crate::app::App;
use crate::common::color_for_agent_type;
use crate::info::{header_btns, make_tabs, throughput, DataOptions, Details, Tab};

pub fn info(ctx: &EventCtx, app: &App, details: &mut Details, id: IntersectionID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::IntersectionInfo(id));
    let i = app.primary.map.get_i(id);

    let mut txt = Text::from(Line("Connecting"));
    let mut road_names = BTreeSet::new();
    for r in &i.roads {
        road_names.insert(
            app.primary
                .map
                .get_r(*r)
                .get_name(app.opts.language.as_ref()),
        );
    }
    for r in road_names {
        txt.add(Line(format!("  {}", r)));
    }
    rows.push(txt.draw(ctx));

    if app.opts.dev {
        rows.push(Btn::text_bg1("Open OSM node").build(ctx, format!("open {}", i.orig_id), None));
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
        "Since midnight: {} commuters and vehicles crossed",
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
        "Number of commuters and vehicles per hour",
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
    let mut demand_per_movement: Vec<(&PolyLine, usize)> = Vec::new();
    for m in app.primary.map.get_traffic_signal(id).movements.values() {
        let demand = app
            .primary
            .sim
            .get_analytics()
            .demand
            .get(&m.id)
            .cloned()
            .unwrap_or(0);
        if demand > 0 {
            total_demand += demand;
            demand_per_movement.push((&m.geom, demand));
        }
    }

    let mut batch = GeomBatch::new();
    let polygon = app.primary.map.get_i(id).polygon.clone();
    let bounds = polygon.get_bounds();
    // Pick a zoom so that we fit a fixed width in pixels
    let zoom = (0.25 * ctx.canvas.window_width) / bounds.width();
    batch.push(
        app.cs.normal_intersection,
        polygon.translate(-bounds.min_x, -bounds.min_y).scale(zoom),
    );

    let mut tooltips: Vec<(Polygon, Text)> = Vec::new();
    let mut outlines = Vec::new();
    for (pl, demand) in demand_per_movement {
        let percent = (demand as f64) / (total_demand as f64);
        let arrow = pl
            .make_arrow(percent * Distance::meters(3.0), ArrowCap::Triangle)
            .translate(-bounds.min_x, -bounds.min_y)
            .scale(zoom);
        if let Ok(p) = arrow.to_outline(Distance::meters(1.0)) {
            outlines.push(p);
        }
        batch.push(Color::hex("#A3A3A3"), arrow.clone());
        tooltips.push((arrow, Text::from(Line(prettyprint_usize(demand)))));
    }
    batch.extend(Color::WHITE, outlines);

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
        Widget::col(vec![
            txt.draw(ctx),
            DrawWithTooltips::new(
                ctx,
                batch,
                tooltips,
                Box::new(|arrow| {
                    let mut list = vec![(Color::hex("#EE702E"), arrow.clone())];
                    if let Ok(p) = arrow.to_outline(Distance::meters(1.0)) {
                        list.push((Color::WHITE, p));
                    }
                    GeomBatch::from(list)
                }),
            ),
        ])
        .padding(10)
        .bg(app.cs.inner_panel)
        .outline(2.0, Color::WHITE),
    );
    rows.push(Btn::text_fg("Explore demand across all traffic signals").build_def(ctx, None));
    if app.opts.dev {
        rows.push(Btn::text_fg("Where are these agents headed?").build(
            ctx,
            format!("routes across {}", id),
            None,
        ));
    }

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

pub fn traffic_signal(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::IntersectionTrafficSignal(id));

    // Slightly inaccurate -- the turn rendering may slightly exceed the intersection polygon --
    // but this is close enough.
    let bounds = app.primary.map.get_i(id).polygon.get_bounds();
    // Pick a zoom so that we fit a fixed width in pixels
    let zoom = 150.0 / bounds.width();
    let bbox = Polygon::rectangle(zoom * bounds.width(), zoom * bounds.height());

    let signal = app.primary.map.get_traffic_signal(id);
    {
        let mut txt = Text::new();
        txt.add(Line(format!("{} stages", signal.stages.len())).small_heading());
        txt.add(Line(format!("Signal offset: {}", signal.offset)));
        {
            let mut total = Duration::ZERO;
            for s in &signal.stages {
                total += s.stage_type.simple_duration();
            }
            // TODO Say "normally" or something?
            txt.add(Line(format!("One cycle lasts {}", total)));
        }
        rows.push(txt.draw(ctx));
    }

    for (idx, stage) in signal.stages.iter().enumerate() {
        rows.push(
            match stage.stage_type {
                StageType::Fixed(d) => Line(format!("Stage {}: {}", idx + 1, d)),
                StageType::Variable(min, delay, additional) => Line(format!(
                    "Stage {}: {}, {}, {} (variable)",
                    idx + 1,
                    min,
                    delay,
                    additional
                )),
            }
            .draw(ctx),
        );

        {
            let mut orig_batch = GeomBatch::new();
            draw_signal_stage(
                ctx.prerender,
                stage,
                idx,
                id,
                None,
                &mut orig_batch,
                app,
                TrafficSignalStyle::Yuwen,
            );

            let mut normal = GeomBatch::new();
            normal.push(Color::BLACK, bbox.clone());
            normal.append(
                orig_batch
                    .translate(-bounds.min_x, -bounds.min_y)
                    .scale(zoom),
            );

            rows.push(Widget::draw_batch(ctx, normal));
        }
    }

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
        for (_, t, dt, agent_type) in list {
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
            tabs.push(("Signal", Tab::IntersectionTrafficSignal(id)));
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
