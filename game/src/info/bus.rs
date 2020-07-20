use crate::app::App;
use crate::helpers::ID;
use crate::info::{header_btns, make_tabs, Details, Tab};
use ezgui::{Btn, Color, EventCtx, GeomBatch, Line, RewriteColor, Text, TextExt, Widget};
use geom::{Circle, Distance, Polygon, Pt2D, Statistic, Time};
use map_model::{BusRoute, BusStopID};
use sim::{AgentID, CarID};
use std::collections::BTreeMap;

pub fn stop(ctx: &mut EventCtx, app: &App, details: &mut Details, id: BusStopID) -> Vec<Widget> {
    let bs = app.primary.map.get_bs(id);
    let mut rows = vec![];

    let sim = &app.primary.sim;

    rows.push(Widget::row(vec![
        Line("Bus stop").small_heading().draw(ctx),
        header_btns(ctx),
    ]));
    rows.push(Line(&bs.name).draw(ctx));

    let all_arrivals = &sim.get_analytics().bus_arrivals;
    for r in app.primary.map.get_routes_serving_stop(id) {
        let buses = app.primary.sim.status_of_buses(r.id);
        if buses.is_empty() {
            rows.push(format!("Route {}: no buses running", r.full_name).draw_text(ctx));
        } else {
            rows.push(Btn::text_fg(format!("Route {}", r.full_name)).build_def(ctx, None));
            details
                .hyperlinks
                .insert(format!("Route {}", r.full_name), Tab::BusStatus(buses[0].0));
        }

        let arrivals: Vec<(Time, CarID)> = all_arrivals
            .iter()
            .filter(|(_, _, route, stop)| r.id == *route && id == *stop)
            .map(|(t, car, _, _)| (*t, *car))
            .collect();
        let mut txt = Text::new();
        if let Some((t, _)) = arrivals.last() {
            // TODO Button to jump to the bus
            txt.add(Line(format!("  Last bus arrived {} ago", sim.time() - *t)).secondary());
        } else {
            txt.add(Line("  No arrivals yet").secondary());
        }
        // TODO Kind of inefficient...
        if let Some(hgram) = sim
            .get_analytics()
            .bus_passenger_delays(sim.time(), r.id)
            .find(|x| x.0 == id)
            .map(|x| x.1)
        {
            txt.add(Line(format!("  Waiting: {}", hgram.describe())).secondary());
        }
        rows.push(txt.draw(ctx));
    }

    // Draw where the bus/train stops
    details.zoomed.push(
        app.cs.bus_body.alpha(0.5),
        Circle::new(bs.driving_pos.pt(&app.primary.map), Distance::meters(2.5)).to_polygon(),
    );

    rows
}

pub fn bus_status(ctx: &mut EventCtx, app: &App, details: &mut Details, id: CarID) -> Vec<Widget> {
    let mut rows = bus_header(ctx, app, details, id, Tab::BusStatus(id));

    let route = app
        .primary
        .map
        .get_br(app.primary.sim.bus_route_id(id).unwrap());

    rows.push(
        Text::from(Line(format!("Serves {}", route.full_name)))
            .wrap_to_pct(ctx, 20)
            .draw(ctx),
    );
    rows.push(
        Line(format!(
            "Currently has {} passengers",
            app.primary.sim.num_transit_passengers(id),
        ))
        .draw(ctx),
    );

    rows.push(passenger_delay(ctx, app, details, route));

    rows
}

fn bus_header(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: CarID,
    tab: Tab,
) -> Vec<Widget> {
    let route = app.primary.sim.bus_route_id(id).unwrap();

    if let Some(pt) = app
        .primary
        .sim
        .canonical_pt_for_agent(AgentID::Car(id), &app.primary.map)
    {
        ctx.canvas.center_on_map_pt(pt);
    }

    let mut rows = vec![];
    rows.push(Widget::row(vec![
        Line(format!(
            "{} (route {})",
            id,
            app.primary.map.get_br(route).short_name
        ))
        .small_heading()
        .draw(ctx),
        header_btns(ctx),
    ]));
    rows.push(make_tabs(
        ctx,
        &mut details.hyperlinks,
        tab,
        vec![("Status", Tab::BusStatus(id))],
    ));

    rows
}

fn passenger_delay(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    route: &BusRoute,
) -> Widget {
    let mut master_col = vec![Line("Passengers waiting").small_heading().draw(ctx)];
    let mut col = Vec::new();

    let mut delay_per_stop = app
        .primary
        .sim
        .get_analytics()
        .bus_passenger_delays(app.primary.sim.time(), route.id)
        .collect::<BTreeMap<_, _>>();
    // TODO I smell an off by one
    for idx in 0..route.stops.len() {
        col.push(Widget::row(vec![
            format!("Stop {}", idx + 1).draw_text(ctx),
            Btn::svg(
                "system/assets/tools/pin.svg",
                RewriteColor::Change(Color::hex("#CC4121"), app.cs.hovering),
            )
            .build(ctx, format!("Stop {}", idx + 1), None),
            if let Some(hgram) = delay_per_stop.remove(&route.stops[idx]) {
                format!(
                    ": {} (avg {})",
                    hgram.count(),
                    hgram.select(Statistic::Mean).unwrap()
                )
                .draw_text(ctx)
            } else {
                ": nobody".draw_text(ctx)
            },
        ]));
        details
            .warpers
            .insert(format!("Stop {}", idx + 1), ID::BusStop(route.stops[idx]));
    }

    let y_len = ctx.default_line_height() * (route.stops.len() as f64);
    let mut batch = GeomBatch::new();
    batch.push(
        Color::CYAN,
        Polygon::rounded_rectangle(15.0, y_len, Some(4.0)),
    );
    for (_, stop_idx, percent_next_stop) in app.primary.sim.status_of_buses(route.id) {
        // TODO This whole function is wrong
        let idx = if let Some(i) = stop_idx {
            i as f64
        } else {
            continue;
        };
        // TODO Line it up right in the middle of the line of text. This is probably a bit
        // wrong.
        let base_percent_y = idx / ((route.stops.len() - 1) as f64);
        batch.push(
            Color::BLUE,
            Circle::new(
                Pt2D::new(
                    7.5,
                    base_percent_y * y_len + percent_next_stop * ctx.default_line_height(),
                ),
                Distance::meters(5.0),
            )
            .to_polygon(),
        );
    }
    let timeline = Widget::draw_batch(ctx, batch);

    master_col.push(Widget::row(vec![timeline, Widget::col(col)]));

    Widget::col(master_col)
}
