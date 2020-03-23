use crate::app::App;
use crate::info::{make_table, throughput};
use abstutil::prettyprint_usize;
use ezgui::{EventCtx, Line, Text, TextExt, Widget};
use geom::Duration;
use map_model::LaneID;

pub fn info(
    ctx: &EventCtx,
    app: &App,
    id: LaneID,
    header_btns: Widget,
    action_btns: Vec<Widget>,
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
    rows.extend(action_btns);

    // Properties
    {
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

        if app.opts.dev {
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

            for (k, v) in &r.osm_tags {
                kv.push((k.to_string(), v.to_string()));
            }
        }

        rows.extend(make_table(ctx, kv));
    }

    if !l.is_parking() {
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

    rows
}
