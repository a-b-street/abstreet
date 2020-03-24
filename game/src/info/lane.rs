use crate::app::App;
use crate::helpers::ID;
use crate::info::{make_table, make_tabs, throughput, InfoTab};
use abstutil::prettyprint_usize;
use ezgui::{EventCtx, Line, Text, TextExt, Widget};
use geom::Duration;
use map_model::LaneID;
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
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

    // TODO Naming still weird
    rows.push(make_tabs(
        ctx,
        hyperlinks,
        ID::Lane(id),
        tab.clone(),
        vec![
            ("Main", InfoTab::Nil),
            ("OpenStreetMap", InfoTab::Lane(Tab::OSM)),
            ("Debug", InfoTab::Lane(Tab::Debug)),
            ("Traffic", InfoTab::Lane(Tab::Throughput)),
        ],
    ));

    match tab {
        InfoTab::Nil => {
            rows.extend(action_btns);

            let mut kv = Vec::new();

            if !l.is_sidewalk() {
                kv.push(("Type", l.lane_type.describe().to_string()));
            }

            if l.is_parking() {
                kv.push((
                    "Parking",
                    format!("{} spots, parallel parking", l.number_parking_spots()),
                ));
            } else {
                kv.push(("Speed limit", r.get_speed_limit().to_string()));
            }

            kv.push(("Length", l.length().describe_rounded()));

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
