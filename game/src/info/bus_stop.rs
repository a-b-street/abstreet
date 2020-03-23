use crate::app::App;
use ezgui::{EventCtx, Line, Text, Widget};
use geom::Time;
use map_model::BusStopID;
use sim::CarID;

pub fn info(
    ctx: &EventCtx,
    app: &App,
    id: BusStopID,
    header_btns: Widget,
    action_btns: Vec<Widget>,
) -> Vec<Widget> {
    let mut rows = vec![];

    let sim = &app.primary.sim;

    rows.push(Widget::row(vec![
        Line("Bus stop").roboto_bold().draw(ctx),
        header_btns,
    ]));
    rows.extend(action_btns);

    let mut txt = Text::new();
    txt.add(Line(format!(
        "On {}",
        app.primary.map.get_parent(id.sidewalk).get_name()
    )));
    let all_arrivals = &sim.get_analytics().bus_arrivals;
    for r in app.primary.map.get_routes_serving_stop(id) {
        txt.add(Line(format!("- Route {}", r.name)).roboto_bold());
        let arrivals: Vec<(Time, CarID)> = all_arrivals
            .iter()
            .filter(|(_, _, route, stop)| r.id == *route && id == *stop)
            .map(|(t, car, _, _)| (*t, *car))
            .collect();
        if let Some((t, _)) = arrivals.last() {
            // TODO Button to jump to the bus
            txt.add(Line(format!("  Last bus arrived {} ago", sim.time() - *t)));
        } else {
            txt.add(Line("  No arrivals yet"));
        }
        // TODO Kind of inefficient...
        if let Some(hgram) = sim
            .get_analytics()
            .bus_passenger_delays(sim.time(), r.id)
            .remove(&id)
        {
            txt.add(Line(format!("  Waiting: {}", hgram.describe())));
        }
    }
    rows.push(txt.draw(ctx));

    rows
}
