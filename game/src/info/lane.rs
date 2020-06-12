use crate::app::App;
use crate::info::{header_btns, make_table, make_tabs, throughput, DataOptions, Details, Tab};
use abstutil::prettyprint_usize;
use ezgui::{Btn, EventCtx, Line, LinePlot, PlotOptions, Series, Text, TextExt, Widget};
use map_model::{LaneID, OriginalLane};

pub fn info(ctx: &EventCtx, app: &App, details: &mut Details, id: LaneID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::LaneInfo(id));
    let map = &app.primary.map;
    let l = map.get_l(id);
    let r = map.get_r(l.parent);

    let mut kv = Vec::new();

    if !l.is_sidewalk() {
        kv.push(("Type", l.lane_type.describe().to_string()));
    }

    if l.is_parking() {
        kv.push((
            "Parking",
            format!(
                "{} / {} spots available",
                app.primary.sim.get_free_onstreet_spots(l.id).len(),
                l.number_parking_spots()
            ),
        ));
    } else {
        kv.push(("Speed limit", r.speed_limit.to_string()));
    }

    kv.push(("Length", l.length().describe_rounded()));

    rows.extend(make_table(ctx, kv));

    if l.is_parking() {
        let capacity = l.number_parking_spots();
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
        rows.push("Parking spots available".draw_text(ctx).margin_above(10));
        rows.push(LinePlot::new(
            ctx,
            "parking spots available",
            series,
            PlotOptions {
                max_x: None,
                max_y: Some(capacity),
            },
        ));
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

    if l.is_driving() {
        kv.push((
            "Parking blackhole redirect".to_string(),
            format!("{:?}", l.parking_blackhole),
        ));
    }

    if let Some(types) = l.get_turn_restrictions(r) {
        kv.push(("Turn restrictions".to_string(), format!("{:?}", types)));
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
        format!("{:.1}%", l.percent_grade(map) * 100.0),
    ));
    kv.push((
        "Elevation details".to_string(),
        format!(
            "{} over {}",
            map.get_i(l.dst_i).elevation - map.get_i(l.src_i).elevation,
            l.length()
        ),
    ));

    rows.extend(make_table(ctx, kv));

    rows.push(Widget::row(vec![
        "Copy OriginalLane to clipboard: "
            .draw_text(ctx)
            .margin_right(15),
        Btn::svg_def("../data/system/assets/tools/clipboard.svg").build(
            ctx,
            "copy OriginalLane",
            None,
        ),
    ]));

    let mut txt = Text::from(Line(""));
    txt.add(Line("Raw OpenStreetMap data"));
    rows.push(txt.draw(ctx));

    rows.extend(make_table(ctx, r.osm_tags.clone().into_iter().collect()));

    rows
}

pub fn copy_orig_lane(app: &App, id: LaneID) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use clipboard::{ClipboardContext, ClipboardProvider};

        let mut cb: ClipboardContext = ClipboardProvider::new().unwrap();
        cb.set_contents(format!(
            "{:?}",
            OriginalLane::to_permanent(id, &app.primary.map)
        ))
        .unwrap();
    }
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
    let mut txt = Text::from(Line("Traffic over entire road, not just this lane"));
    txt.add(Line(format!(
        "Since midnight: {} agents crossed",
        prettyprint_usize(app.primary.sim.get_analytics().road_thruput.total_for(r.id))
    )));
    rows.push(txt.draw(ctx));

    rows.push(opts.to_controls(ctx, app).margin_below(10));

    let r = map.get_l(id).parent;
    rows.push(throughput(
        ctx,
        app,
        move |a| a.road_thruput.count_per_hour(r),
        opts.show_before,
    ));

    rows
}

fn header(ctx: &EventCtx, app: &App, details: &mut Details, id: LaneID, tab: Tab) -> Vec<Widget> {
    let mut rows = vec![];

    let map = &app.primary.map;
    let l = map.get_l(id);
    let r = map.get_r(l.parent);

    let label = if l.is_sidewalk() { "Sidewalk" } else { "Lane" };
    rows.push(Widget::row(vec![
        Line(format!("{} #{}", label, id.0))
            .small_heading()
            .draw(ctx),
        header_btns(ctx),
    ]));
    rows.push(format!("@ {}", r.get_name()).draw_text(ctx));

    let mut tabs = vec![
        ("Info", Tab::LaneInfo(id)),
        ("Traffic", Tab::LaneTraffic(id, DataOptions::new(app))),
    ];
    if app.opts.dev {
        tabs.push(("Debug", Tab::LaneDebug(id)));
    }
    rows.push(make_tabs(ctx, &mut details.hyperlinks, tab, tabs));

    rows
}
