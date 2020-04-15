use crate::app::App;
use crate::info::{header_btns, make_tabs, throughput, DataOptions, Details, Tab};
use abstutil::prettyprint_usize;
use ezgui::{EventCtx, Line, LinePlot, PlotOptions, Series, Text, Widget};
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
                .thruput_stats
                .count_per_intersection
                .get(id)
        )
    )));
    rows.push(txt.draw(ctx));

    rows.push(opts.to_controls(ctx, app));

    rows.push(
        throughput(
            ctx,
            app,
            move |a, t| a.throughput_intersection(t, id, opts.bucket_size),
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

fn delay_plot(ctx: &EventCtx, app: &App, i: IntersectionID, opts: &DataOptions) -> Widget {
    let get_data = |a: &Analytics, t: Time| {
        let mut series: Vec<(Statistic, Vec<(Time, Duration)>)> = Statistic::all()
            .into_iter()
            .map(|stat| (stat, Vec::new()))
            .collect();
        for (t, distrib) in a.intersection_delays_bucketized(t, i, opts.bucket_size) {
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
        }
        tabs
    }));

    rows
}
