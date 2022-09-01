use std::collections::{BTreeMap, BTreeSet};

use abstutil::prettyprint_usize;
use geom::{ArrowCap, Distance, Duration, PolyLine, Polygon, Tessellation, Time};
use map_gui::options::TrafficSignalStyle;
use map_gui::render::traffic_signal::draw_signal_stage;
use map_model::{IntersectionID, IntersectionType, StageType};
use sim::AgentType;
use widgetry::{
    Color, DrawWithTooltips, EventCtx, FanChart, GeomBatch, Line, PlotOptions, ScatterPlot, Series,
    Text, Toggle, Widget,
};

use crate::app::App;
use crate::common::color_for_agent_type;
use crate::info::{
    header_btns, make_tabs, problem_count, throughput, DataOptions, Details, ProblemOptions, Tab,
};

pub fn info(ctx: &EventCtx, app: &App, details: &mut Details, id: IntersectionID) -> Widget {
    Widget::custom_col(vec![
        header(ctx, app, details, id, Tab::IntersectionInfo(id)),
        info_body(ctx, app, id).tab_body(ctx),
    ])
}

fn info_body(ctx: &EventCtx, app: &App, id: IntersectionID) -> Widget {
    let mut rows = vec![];

    let i = app.primary.map.get_i(id);

    let mut txt = Text::from("Connecting");
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
        txt.add_line(format!("  {}", r));
    }
    rows.push(txt.into_widget(ctx));

    if app.opts.dev {
        rows.push(
            ctx.style()
                .btn_outline
                .text("Open OSM node")
                .build_widget(ctx, format!("open {}", i.orig_id)),
        );
    }

    Widget::col(rows)
}

pub fn traffic(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    opts: &DataOptions,
) -> Widget {
    Widget::custom_col(vec![
        header(
            ctx,
            app,
            details,
            id,
            Tab::IntersectionTraffic(id, opts.clone()),
        ),
        traffic_body(ctx, app, id, opts).tab_body(ctx),
    ])
}

fn traffic_body(ctx: &mut EventCtx, app: &App, id: IntersectionID, opts: &DataOptions) -> Widget {
    let mut rows = vec![];
    let mut txt = Text::new();

    txt.add_line(format!(
        "Since midnight: {} commuters and vehicles crossed",
        prettyprint_usize(
            app.primary
                .sim
                .get_analytics()
                .intersection_thruput
                .total_for(id)
        )
    ));
    rows.push(txt.into_widget(ctx));

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
        opts,
    ));

    Widget::col(rows)
}

pub fn delay(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    opts: &DataOptions,
    fan_chart: bool,
) -> Widget {
    Widget::custom_col(vec![
        header(
            ctx,
            app,
            details,
            id,
            Tab::IntersectionDelay(id, opts.clone(), fan_chart),
        ),
        delay_body(ctx, app, id, opts, fan_chart).tab_body(ctx),
    ])
}

fn delay_body(
    ctx: &mut EventCtx,
    app: &App,
    id: IntersectionID,
    opts: &DataOptions,
    fan_chart: bool,
) -> Widget {
    let mut rows = vec![];
    let i = app.primary.map.get_i(id);

    assert!(i.is_traffic_signal());
    rows.push(opts.to_controls(ctx, app));
    rows.push(Toggle::choice(
        ctx,
        "fan chart / scatter plot",
        "fan chart",
        "scatter plot",
        None,
        fan_chart,
    ));

    rows.push(delay_plot(ctx, app, id, opts, fan_chart));

    Widget::col(rows)
}

pub fn current_demand(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
) -> Widget {
    Widget::custom_col(vec![
        header(ctx, app, details, id, Tab::IntersectionDemand(id)),
        current_demand_body(ctx, app, id).tab_body(ctx),
    ])
}

fn current_demand_body(ctx: &mut EventCtx, app: &App, id: IntersectionID) -> Widget {
    let mut rows = vec![];
    let mut total_demand = 0;
    let mut demand_per_movement: Vec<(&PolyLine, usize)> = Vec::new();
    for m in app.primary.map.get_i(id).movements.values() {
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
    let mut polygon = Tessellation::from(app.primary.map.get_i(id).polygon.clone());
    let bounds = polygon.get_bounds();
    // Pick a zoom so that we fit a fixed width in pixels
    let zoom = (0.25 * ctx.canvas.window_width) / bounds.width();
    polygon.translate(-bounds.min_x, -bounds.min_y);
    polygon.scale(zoom);
    batch.push(app.cs.normal_intersection, polygon);

    let mut tooltips = Vec::new();
    let mut outlines = Vec::new();
    for (pl, demand) in demand_per_movement {
        let percent = (demand as f64) / (total_demand as f64);
        if let Ok(arrow) = pl
            .make_arrow(percent * Distance::meters(3.0), ArrowCap::Triangle)
            .translate(-bounds.min_x, -bounds.min_y)
            .scale(zoom)
        {
            if let Ok(p) = arrow.to_outline(Distance::meters(1.0)) {
                outlines.push(p);
            }
            batch.push(Color::hex("#A3A3A3"), arrow.clone());
            tooltips.push((arrow, Text::from(prettyprint_usize(demand)), None));
        }
    }
    batch.extend(Color::WHITE, outlines);

    let mut txt = Text::from(format!(
        "Active agent demand at {}",
        app.primary.sim.time().ampm_tostring()
    ));
    txt.add_line(
        Line(format!(
            "Includes all {} active agents anywhere on the map",
            prettyprint_usize(total_demand)
        ))
        .secondary(),
    );

    rows.push(
        Widget::col(vec![
            txt.into_widget(ctx),
            DrawWithTooltips::new_widget(
                ctx,
                batch,
                tooltips,
                Box::new(|arrow| {
                    let mut batch = GeomBatch::from(vec![(Color::hex("#EE702E"), arrow.clone())]);
                    if let Ok(p) = arrow.to_outline(Distance::meters(1.0)) {
                        batch.push(Color::WHITE, p);
                    }
                    batch
                }),
            ),
        ])
        .padding(10)
        .bg(app.cs.inner_panel_bg)
        .outline(ctx.style().section_outline),
    );
    rows.push(
        ctx.style()
            .btn_outline
            .text("Explore demand across all traffic signals")
            .build_def(ctx),
    );
    if app.opts.dev {
        rows.push(
            ctx.style()
                .btn_outline
                .text("Where are these agents headed?")
                .build_widget(ctx, format!("routes across {}", id)),
        );
    }

    Widget::col(rows)
}

pub fn arrivals(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    opts: &DataOptions,
) -> Widget {
    Widget::custom_col(vec![
        header(
            ctx,
            app,
            details,
            id,
            Tab::IntersectionArrivals(id, opts.clone()),
        ),
        throughput(
            ctx,
            app,
            "Number of in-bound trips from this border",
            move |_| app.primary.sim.all_arrivals_at_border(id),
            opts,
        )
        .tab_body(ctx),
    ])
}

pub fn traffic_signal(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
) -> Widget {
    Widget::custom_col(vec![
        header(ctx, app, details, id, Tab::IntersectionTrafficSignal(id)),
        traffic_signal_body(ctx, app, id).tab_body(ctx),
    ])
}

fn traffic_signal_body(ctx: &mut EventCtx, app: &App, id: IntersectionID) -> Widget {
    let mut rows = vec![];
    // Slightly inaccurate -- the turn rendering may slightly exceed the intersection polygon --
    // but this is close enough.
    let bounds = app.primary.map.get_i(id).polygon.get_bounds();
    // Pick a zoom so that we fit a fixed width in pixels
    let zoom = 150.0 / bounds.width();
    let bbox = Polygon::rectangle(zoom * bounds.width(), zoom * bounds.height());

    let signal = app.primary.map.get_traffic_signal(id);
    {
        let mut txt = Text::new();
        txt.add_line(Line(format!("{} stages", signal.stages.len())).small_heading());
        txt.add_line(format!("Signal offset: {}", signal.offset));
        {
            let mut total = Duration::ZERO;
            for s in &signal.stages {
                total += s.stage_type.simple_duration();
            }
            // TODO Say "normally" or something?
            txt.add_line(format!("One cycle lasts {}", total));
        }
        rows.push(txt.into_widget(ctx));
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
            .into_widget(ctx),
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

            rows.push(normal.into_widget(ctx));
        }
    }

    Widget::col(rows)
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
    let series: Vec<Series<Time, Duration>> = by_type
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
        dims: None,
    };
    Widget::col(vec![
        Line("Delay through intersection")
            .small_heading()
            .into_widget(ctx),
        if fan_chart {
            FanChart::new_widget(ctx, series, plot_opts, app.opts.units)
        } else {
            ScatterPlot::new_widget(ctx, series, plot_opts, app.opts.units)
        },
    ])
    .padding(10)
    .bg(app.cs.inner_panel_bg)
    .outline(ctx.style().section_outline)
}

pub fn problems(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    opts: &ProblemOptions,
) -> Widget {
    Widget::custom_col(vec![
        header(
            ctx,
            app,
            details,
            id,
            Tab::IntersectionProblems(id, opts.clone()),
        ),
        problems_body(ctx, app, id, opts).tab_body(ctx),
    ])
}

fn problems_body(
    ctx: &mut EventCtx,
    app: &App,
    id: IntersectionID,
    opts: &ProblemOptions,
) -> Widget {
    let mut rows = vec![];

    rows.push(opts.to_controls(ctx, app));

    let time = if opts.show_end_of_day {
        app.primary.sim.get_end_of_day()
    } else {
        app.primary.sim.time()
    };
    rows.push(problem_count(
        ctx,
        app,
        "Number of problems per 15 minutes",
        move |a| a.problems_per_intersection(time, id),
        opts,
    ));

    Widget::col(rows)
}

fn header(
    ctx: &EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
    tab: Tab,
) -> Widget {
    let mut rows = vec![];

    let i = app.primary.map.get_i(id);

    let label = match i.intersection_type {
        IntersectionType::StopSign | IntersectionType::Uncontrolled => {
            format!("{} (Stop signs)", id)
        }
        IntersectionType::TrafficSignal => format!("{} (Traffic signals)", id),
        IntersectionType::Border => format!("Border #{}", id.0),
        IntersectionType::Construction => format!("{} (under construction)", id),
    };
    rows.push(Widget::row(vec![
        Line(label).small_heading().into_widget(ctx),
        header_btns(ctx),
    ]));

    rows.push(make_tabs(ctx, &mut details.hyperlinks, tab, {
        let mut tabs = vec![
            ("Info", Tab::IntersectionInfo(id)),
            ("Traffic", Tab::IntersectionTraffic(id, DataOptions::new())),
            (
                "Problems",
                Tab::IntersectionProblems(id, ProblemOptions::new()),
            ),
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

    Widget::custom_col(rows)
}
