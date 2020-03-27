use crate::app::App;
use crate::helpers::rotating_color_map;
use crate::info::{header_btns, make_tabs, throughput, Details, Tab};
use abstutil::prettyprint_usize;
use ezgui::{EventCtx, Line, Plot, PlotOptions, Series, Text, Widget};
use geom::{Duration, Statistic, Time};
use map_model::{IntersectionID, IntersectionType};
use sim::{Analytics, TripEndpoint};
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

    // TODO Rethink
    let trip_lines = app
        .primary
        .sim
        .count_trips(TripEndpoint::Border(id))
        .describe();
    if !trip_lines.is_empty() {
        let mut txt = Text::new();
        for line in trip_lines {
            txt.add(Line(line));
        }
        rows.push(txt.draw(ctx));
    }

    rows
}

pub fn traffic(
    ctx: &EventCtx,
    app: &App,
    details: &mut Details,
    id: IntersectionID,
) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::IntersectionTraffic(id));
    let i = app.primary.map.get_i(id);

    let mut txt = Text::new();

    txt.add(Line("Throughput"));
    txt.add(
        Line(format!(
            "Since midnight: {} agents crossed",
            prettyprint_usize(
                app.primary
                    .sim
                    .get_analytics()
                    .thruput_stats
                    .count_per_intersection
                    .get(id)
            )
        ))
        .secondary(),
    );
    txt.add(Line(format!("In 20 minute buckets:")).secondary());
    rows.push(txt.draw(ctx));

    rows.push(
        throughput(ctx, app, move |a, t| {
            a.throughput_intersection(t, id, Duration::minutes(20))
        })
        .margin(10),
    );

    rows
}

pub fn delay(ctx: &EventCtx, app: &App, details: &mut Details, id: IntersectionID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::IntersectionDelay(id));
    let i = app.primary.map.get_i(id);

    assert!(i.is_traffic_signal());
    let mut txt = Text::from(Line("Delay"));
    txt.add(Line(format!("In 20 minute buckets:")).secondary());
    rows.push(txt.draw(ctx));

    rows.push(delay_plot(ctx, app, id, Duration::minutes(20)).margin(10));

    rows
}

fn delay_plot(ctx: &EventCtx, app: &App, i: IntersectionID, bucket: Duration) -> Widget {
    let get_data = |a: &Analytics, t: Time| {
        let mut series: Vec<(Statistic, Vec<(Time, Duration)>)> = Statistic::all()
            .into_iter()
            .map(|stat| (stat, Vec::new()))
            .collect();
        for (t, distrib) in a.intersection_delays_bucketized(t, i, bucket) {
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
            color: rotating_color_map(idx),
            pts,
        });
    }
    if app.has_prebaked().is_some() {
        for (idx, (stat, pts)) in get_data(app.prebaked(), Time::END_OF_DAY)
            .into_iter()
            .enumerate()
        {
            all_series.push(Series {
                label: format!("{} (baseline)", stat),
                color: rotating_color_map(idx).alpha(0.3),
                pts,
            });
        }
    }

    Plot::new_duration(ctx, all_series, PlotOptions::new())
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
        IntersectionType::StopSign => format!("Intersection #{} (Stop signs)", id.0),
        IntersectionType::TrafficSignal => format!("Intersection #{} (Traffic signals)", id.0),
        IntersectionType::Border => format!("Border #{}", id.0),
        IntersectionType::Construction => format!("Intersection #{} (under construction)", id.0),
    };
    rows.push(Widget::row(vec![
        Line(label).small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    rows.push(make_tabs(ctx, &mut details.hyperlinks, tab, {
        let mut tabs = vec![
            ("Info", Tab::IntersectionInfo(id)),
            ("Traffic", Tab::IntersectionTraffic(id)),
        ];
        if i.is_traffic_signal() {
            tabs.push(("Delay", Tab::IntersectionDelay(id)));
        }
        tabs
    }));

    rows
}
