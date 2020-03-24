use crate::app::App;
use crate::helpers::ID;
use crate::info::{make_table, throughput, InfoTab};
use abstutil::prettyprint_usize;
use ezgui::{Btn, EventCtx, Line, Text, TextExt, Widget};
use geom::Duration;
use map_model::LaneID;
use std::collections::HashMap;

#[derive(Clone)]
pub enum Tab {
    OSM,
    Debug,
    Throughput,
}

pub fn info(
    ctx: &EventCtx,
    app: &App,
    id: LaneID,
    tab: InfoTab,
    header_btns: Widget,
    action_btns: Vec<Widget>,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
) -> Vec<Widget> {
    let mut rows = vec![];

    let map = &app.primary.map;
    let l = map.get_l(id);
    let r = map.get_r(l.parent);

    let label = if l.is_sidewalk() { "Sidewalk" } else { "Lane" };
    rows.push(Widget::row(vec![
        Line(format!("{} #{}", label, id.0)).roboto_bold().draw(ctx),
        header_btns,
    ]));
    rows.push(format!("@ {}", r.get_name()).draw_text(ctx));

    // TODO Inactive
    // TODO Naming, style...
    rows.push(Widget::row(vec![
        Btn::text_bg2("Main").build_def(ctx, None),
        Btn::text_bg2("OpenStreetMap").build_def(ctx, None),
        Btn::text_bg2("Debug").build_def(ctx, None),
        Btn::text_bg2("Traffic").build_def(ctx, None),
    ]));
    hyperlinks.insert(
        "OpenStreetMap".to_string(),
        (ID::Lane(id), InfoTab::Lane(Tab::OSM)),
    );
    hyperlinks.insert(
        "Debug".to_string(),
        (ID::Lane(id), InfoTab::Lane(Tab::Debug)),
    );
    hyperlinks.insert(
        "Traffic".to_string(),
        (ID::Lane(id), InfoTab::Lane(Tab::Throughput)),
    );

    match tab {
        InfoTab::Nil => {
            rows.extend(action_btns);

            let mut kv = Vec::new();

            if !l.is_sidewalk() {
                kv.push(("Type".to_string(), l.lane_type.describe().to_string()));
            }

            if l.is_parking() {
                kv.push((
                    "Parking".to_string(),
                    format!("{} spots, parallel parking", l.number_parking_spots()),
                ));
            } else {
                kv.push(("Speed limit".to_string(), r.get_speed_limit().to_string()));
            }

            kv.push(("Length".to_string(), l.length().describe_rounded()));

            rows.extend(make_table(ctx, kv));
        }
        InfoTab::Lane(Tab::OSM) => {
            rows.extend(make_table(ctx, r.osm_tags.clone().into_iter().collect()));
        }
        InfoTab::Lane(Tab::Debug) => {
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
        }
        InfoTab::Lane(Tab::Throughput) => {
            // Since this applies to the entire road, ignore lane type.
            let mut txt = Text::from(Line(""));
            txt.add(Line("Throughput (entire road)").roboto_bold());
            txt.add(Line(format!(
                "Since midnight: {} agents crossed",
                prettyprint_usize(
                    app.primary
                        .sim
                        .get_analytics()
                        .thruput_stats
                        .count_per_road
                        .get(r.id)
                )
            )));
            txt.add(Line(format!("In 20 minute buckets:")));
            rows.push(txt.draw(ctx));

            let r = app.primary.map.get_l(id).parent;
            rows.push(
                throughput(ctx, app, move |a, t| {
                    a.throughput_road(t, r, Duration::minutes(20))
                })
                .margin(10),
            );
        }
        _ => unreachable!(),
    }

    rows
}
