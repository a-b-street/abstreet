use crate::app::App;
use crate::info::{header_btns, make_table, Details};
use ezgui::{EventCtx, Line, Text, Widget};
use geom::Time;
use map_model::BusStopID;
use sim::CarID;

// TODO Needs much more work
pub fn stop(ctx: &EventCtx, app: &App, details: &mut Details, id: BusStopID) -> Vec<Widget> {
    let mut rows = vec![];

    let sim = &app.primary.sim;

    rows.push(Widget::row(vec![
        Line("Bus stop").small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    let mut txt = Text::new();
    txt.add(Line(format!(
        "On {}",
        app.primary.map.get_parent(id.sidewalk).get_name()
    )));
    let all_arrivals = &sim.get_analytics().bus_arrivals;
    for r in app.primary.map.get_routes_serving_stop(id) {
        txt.add(Line(format!("- Route {}", r.name)));
        let arrivals: Vec<(Time, CarID)> = all_arrivals
            .iter()
            .filter(|(_, _, route, stop)| r.id == *route && id == *stop)
            .map(|(t, car, _, _)| (*t, *car))
            .collect();
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
            .remove(&id)
        {
            txt.add(Line(format!("  Waiting: {}", hgram.describe())).secondary());
        }
    }
    rows.push(txt.draw(ctx));

    rows
}

// TODO Likewise
pub fn bus(ctx: &EventCtx, app: &App, details: &mut Details, id: CarID) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Bus #{}", id.0)).small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    let (kv, extra) = app.primary.sim.car_properties(id, &app.primary.map);
    rows.extend(make_table(ctx, kv));
    if !extra.is_empty() {
        let mut txt = Text::from(Line(""));
        for line in extra {
            txt.add(Line(line));
        }
        rows.push(txt.draw(ctx));
    }

    rows
}
