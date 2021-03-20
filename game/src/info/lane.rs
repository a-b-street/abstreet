use std::collections::HashSet;

use abstutil::prettyprint_usize;
use map_model::{LaneID, PathConstraints};
use widgetry::{EventCtx, Line, LinePlot, PlotOptions, Series, Text, TextExt, Widget};

use crate::app::App;
use crate::info::{header_btns, make_table, make_tabs, throughput, DataOptions, Details, Tab};

pub fn info(ctx: &EventCtx, app: &App, details: &mut Details, id: LaneID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::LaneInfo(id));
    let map = &app.primary.map;
    let l = map.get_l(id);
    let r = map.get_r(l.parent);

    let mut kv = Vec::new();

    if !l.is_walkable() {
        kv.push(("Type", l.lane_type.describe().to_string()));
    }
    if r.is_private() {
        let mut ban = Vec::new();
        for p in PathConstraints::all() {
            if !r.access_restrictions.allow_through_traffic.contains(p) {
                ban.push(format!("{:?}", p).to_ascii_lowercase());
            }
        }
        if !ban.is_empty() {
            kv.push(("No through-traffic for", ban.join(", ")));
        }
        if let Some(cap) = r.access_restrictions.cap_vehicles_per_hour {
            kv.push((
                "Cap for vehicles this hour",
                format!(
                    "{} / {}",
                    prettyprint_usize(app.primary.sim.get_cap_counter(l.id)),
                    prettyprint_usize(cap)
                ),
            ));
        }
    }

    if l.is_parking() {
        kv.push((
            "Parking",
            format!(
                "{} / {} spots available",
                app.primary.sim.get_free_onstreet_spots(l.id).len(),
                l.number_parking_spots(app.primary.map.get_config())
            ),
        ));
    } else {
        kv.push(("Speed limit", r.speed_limit.to_string(&app.opts.units)));
    }

    kv.push(("Length", l.length().to_string(&app.opts.units)));

    rows.extend(make_table(ctx, kv));

    if l.is_parking() {
        let capacity = l.number_parking_spots(app.primary.map.get_config());
        let mut series = vec![Series {
            label: format!("After \"{}\"", app.primary.map.get_edits().edits_name),
            color: app.cs.after_changes,
            pts: app.primary.sim.get_analytics().parking_lane_availability(
                app.primary.sim.time(),
                l.id,
                capacity,
            ),
        }];
        if app.has_prebaked().is_some() {
            series.push(Series {
                label: format!("Before \"{}\"", app.primary.map.get_edits().edits_name),
                color: app.cs.before_changes.alpha(0.5),
                pts: app.prebaked().parking_lane_availability(
                    app.primary.sim.get_end_of_day(),
                    l.id,
                    capacity,
                ),
            });
        }
        let section = Widget::col(vec![
            Line("Parking spots available")
                .small_heading()
                .into_widget(ctx),
            LinePlot::new(
                ctx,
                series,
                PlotOptions {
                    filterable: false,
                    max_x: None,
                    max_y: Some(capacity),
                    disabled: HashSet::new(),
                },
            ),
        ])
        .padding(10)
        .bg(app.cs.inner_panel_bg)
        .outline(ctx.style().section_outline);
        rows.push(section);
    }

    rows
}

pub fn debug(ctx: &EventCtx, app: &App, details: &mut Details, id: LaneID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::LaneDebug(id));
    let map = &app.primary.map;
    let l = map.get_l(id);
    let r = map.get_r(l.parent);

    let mut kv = Vec::new();

    kv.push(("Parent".to_string(), r.id.to_string()));

    if l.lane_type.is_for_moving_vehicles() {
        kv.push((
            "Driving blackhole".to_string(),
            l.driving_blackhole.to_string(),
        ));
        kv.push((
            "Biking blackhole".to_string(),
            l.biking_blackhole.to_string(),
        ));
    }

    if let Some(types) = l.get_turn_restrictions(r) {
        kv.push((
            "Turn restrictions".to_string(),
            format!("{:?}", types.into_iter().collect::<Vec<_>>()),
        ));
    }
    for (restriction, to) in &r.turn_restrictions {
        kv.push((
            format!("Restriction from this road to {}", to),
            format!("{:?}", restriction),
        ));
    }

    // TODO Simplify and expose everywhere after there's better data
    kv.push((
        "Elevation change".to_string(),
        format!(
            "{} to {}",
            map.get_i(l.src_i).elevation,
            map.get_i(l.dst_i).elevation
        ),
    ));
    kv.push((
        "Incline / grade".to_string(),
        format!("{:.1}%", r.percent_incline(map) * 100.0),
    ));
    kv.push((
        "Elevation details".to_string(),
        format!(
            "{} over {}",
            map.get_i(l.dst_i).elevation - map.get_i(l.src_i).elevation,
            l.length()
        ),
    ));
    kv.push((
        "Dir and offset".to_string(),
        format!("{}, {}", r.dir(l.id), r.offset(l.id)),
    ));
    if let Some((reserved, total)) = app.primary.sim.debug_queue_lengths(l.id) {
        kv.push((
            "Queue (reserved, total) length".to_string(),
            format!("{}, {}", reserved, total),
        ));
    }

    rows.extend(make_table(ctx, kv));

    rows.push(
        ctx.style()
            .btn_outline
            .text("Open OSM way")
            .build_widget(ctx, format!("open {}", r.orig_id.osm_way_id)),
    );

    let mut txt = Text::from("");
    txt.add_line("Raw OpenStreetMap data");
    rows.push(txt.into_widget(ctx));

    rows.extend(make_table(
        ctx,
        r.osm_tags
            .inner()
            .iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect(),
    ));

    rows
}

pub fn traffic(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: LaneID,
    opts: &DataOptions,
) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::LaneTraffic(id, opts.clone()));
    let map = &app.primary.map;
    let l = map.get_l(id);
    let r = map.get_r(l.parent);

    // Since this applies to the entire road, ignore lane type.
    let mut txt = Text::from("Traffic over entire road, not just this lane");
    txt.add_line(format!(
        "Since midnight: {} commuters and vehicles crossed",
        prettyprint_usize(app.primary.sim.get_analytics().road_thruput.total_for(r.id))
    ));
    rows.push(txt.into_widget(ctx));

    rows.push(opts.to_controls(ctx, app));

    let r = map.get_l(id).parent;
    let time = if opts.show_end_of_day {
        app.primary.sim.get_end_of_day()
    } else {
        app.primary.sim.time()
    };
    // TODO This conflates commuters and vehicles, so we should maybe split it into different plots.
    rows.push(throughput(
        ctx,
        app,
        "Number of commuters and vehicles per hour",
        move |a| {
            if a.road_thruput.raw.is_empty() {
                a.road_thruput.count_per_hour(r, time)
            } else {
                a.road_thruput.raw_throughput(time, r)
            }
        },
        &opts,
    ));

    rows
}

fn header(ctx: &EventCtx, app: &App, details: &mut Details, id: LaneID, tab: Tab) -> Vec<Widget> {
    let mut rows = vec![];

    let map = &app.primary.map;
    let l = map.get_l(id);
    let r = map.get_r(l.parent);

    let label = if l.is_shoulder() {
        "Shoulder"
    } else if l.is_sidewalk() {
        "Sidewalk"
    } else {
        "Lane"
    };
    rows.push(Widget::row(vec![
        Line(format!("{} #{}", label, id.0))
            .small_heading()
            .into_widget(ctx),
        header_btns(ctx),
    ]));
    rows.push(format!("@ {}", r.get_name(app.opts.language.as_ref())).text_widget(ctx));

    let mut tabs = vec![("Info", Tab::LaneInfo(id))];
    if !l.is_parking() {
        tabs.push(("Traffic", Tab::LaneTraffic(id, DataOptions::new())));
    }
    if app.opts.dev {
        tabs.push(("Debug", Tab::LaneDebug(id)));
    }
    rows.push(make_tabs(ctx, &mut details.hyperlinks, tab, tabs));

    rows
}
