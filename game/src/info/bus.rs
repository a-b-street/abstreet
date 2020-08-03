use crate::app::App;
use crate::common::ColorNetwork;
use crate::helpers::ID;
use crate::info::{header_btns, make_tabs, Details, Tab};
use abstutil::{prettyprint_usize, Counter};
use ezgui::{Btn, Color, EventCtx, Line, RewriteColor, Text, TextExt, Widget};
use geom::{Circle, Distance, Time};
use map_model::{BusRouteID, BusStopID, PathConstraints, PathStep};
use sim::{AgentID, CarID};

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
        // Full names can overlap, so include the ID
        let label = format!("{} ({})", r.full_name, r.id);
        rows.push(Btn::text_fg(format!("Route {}", r.short_name)).build(ctx, &label, None));
        details.hyperlinks.insert(label, Tab::BusRoute(r.id));

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
        rows.push(txt.draw(ctx));
    }

    let mut boardings: Counter<BusRouteID> = Counter::new();
    let mut alightings: Counter<BusRouteID> = Counter::new();
    if let Some(list) = app.primary.sim.get_analytics().passengers_boarding.get(&id) {
        for (_, r, _) in list {
            boardings.inc(*r);
        }
    }
    if let Some(list) = app
        .primary
        .sim
        .get_analytics()
        .passengers_alighting
        .get(&id)
    {
        for (_, r) in list {
            alightings.inc(*r);
        }
    }
    let mut txt = Text::new();
    txt.add(Line("Total"));
    txt.append(
        Line(format!(
            ": {} boardings, {} alightings",
            prettyprint_usize(boardings.sum()),
            prettyprint_usize(alightings.sum())
        ))
        .secondary(),
    );
    for r in app.primary.map.get_routes_serving_stop(id) {
        txt.add(Line(format!("Route {}", r.short_name)));
        txt.append(
            Line(format!(
                ": {} boardings, {} alightings",
                prettyprint_usize(boardings.get(r.id)),
                prettyprint_usize(alightings.get(r.id))
            ))
            .secondary(),
        );
    }
    rows.push(txt.draw(ctx));

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

    rows.push(Btn::text_fg(format!("Serves route {}", route.short_name)).build_def(ctx, None));
    details.hyperlinks.insert(
        format!("Serves route {}", route.short_name),
        Tab::BusRoute(route.id),
    );

    rows.push(
        Line(format!(
            "Currently has {} passengers",
            app.primary.sim.num_transit_passengers(id),
        ))
        .draw(ctx),
    );

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

pub fn route(ctx: &mut EventCtx, app: &App, details: &mut Details, id: BusRouteID) -> Vec<Widget> {
    let map = &app.primary.map;
    let route = map.get_br(id);
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Route {}", route.short_name))
            .small_heading()
            .draw(ctx),
        header_btns(ctx),
    ]));
    rows.push(
        Text::from(Line(&route.full_name))
            .wrap_to_pct(ctx, 20)
            .draw(ctx),
    );

    let buses = app.primary.sim.status_of_buses(id, map);
    let mut bus_locations = Vec::new();
    if buses.is_empty() {
        if route.route_type == PathConstraints::Bus {
            rows.push("No buses running".draw_text(ctx));
        } else {
            rows.push("No trains running".draw_text(ctx));
        }
    } else {
        for (bus, _, _, pt) in buses {
            rows.push(Btn::text_fg(bus.to_string()).build_def(ctx, None));
            details
                .hyperlinks
                .insert(bus.to_string(), Tab::BusStatus(bus));
            bus_locations.push(pt);
        }
    }

    let mut boardings: Counter<BusStopID> = Counter::new();
    let mut alightings: Counter<BusStopID> = Counter::new();
    let mut waiting: Counter<BusStopID> = Counter::new();
    for bs in &route.stops {
        if let Some(list) = app.primary.sim.get_analytics().passengers_boarding.get(bs) {
            for (_, r, _) in list {
                if *r == id {
                    boardings.inc(*bs);
                }
            }
        }
        if let Some(list) = app.primary.sim.get_analytics().passengers_alighting.get(bs) {
            for (_, r) in list {
                if *r == id {
                    alightings.inc(*bs);
                }
            }
        }

        for (_, r, _, _) in app.primary.sim.get_people_waiting_at_stop(*bs) {
            if *r == id {
                waiting.inc(*bs);
            }
        }
    }

    rows.push(
        Text::from_all(vec![
            Line("Total"),
            Line(format!(
                ": {} boardings, {} alightings, {} currently waiting",
                prettyprint_usize(boardings.sum()),
                prettyprint_usize(alightings.sum()),
                prettyprint_usize(waiting.sum())
            ))
            .secondary(),
        ])
        .draw(ctx),
    );

    rows.push(format!("{} stops", route.stops.len()).draw_text(ctx));
    {
        let i = map.get_i(map.get_l(route.start).src_i);
        let name = format!("Starts at {}", i.name(map));
        rows.push(Widget::row(vec![
            Btn::svg(
                "system/assets/timeline/goal_pos.svg",
                RewriteColor::Change(Color::WHITE, app.cs.hovering),
            )
            .build(ctx, &name, None),
            name.clone().draw_text(ctx),
        ]));
        details.warpers.insert(name, ID::Intersection(i.id));
    }
    for (idx, bs) in route.stops.iter().enumerate() {
        let bs = map.get_bs(*bs);
        let name = format!("Stop {}: {}", idx + 1, bs.name);
        rows.push(Widget::row(vec![
            Btn::svg(
                "system/assets/tools/pin.svg",
                RewriteColor::Change(Color::hex("#CC4121"), app.cs.hovering),
            )
            .build(ctx, &name, None),
            Text::from_all(vec![
                Line(&bs.name),
                Line(format!(
                    ": {} boardings, {} alightings, {} currently waiting",
                    prettyprint_usize(boardings.get(bs.id)),
                    prettyprint_usize(alightings.get(bs.id)),
                    prettyprint_usize(waiting.get(bs.id))
                ))
                .secondary(),
            ])
            .draw(ctx),
        ]));
        details.warpers.insert(name, ID::BusStop(bs.id));
    }
    if let Some(l) = route.end_border {
        let i = map.get_i(map.get_l(l).dst_i);
        let name = format!("Ends at {}", i.name(map));
        rows.push(Widget::row(vec![
            Btn::svg(
                "system/assets/timeline/goal_pos.svg",
                RewriteColor::Change(Color::WHITE, app.cs.hovering),
            )
            .build(ctx, &name, None),
            name.clone().draw_text(ctx),
        ]));
        details.warpers.insert(name, ID::Intersection(i.id));
    }

    // Draw the route, label stops, and show location of buses
    {
        let mut colorer = ColorNetwork::new(app);
        for req in route.all_steps(map) {
            for step in map.pathfind(req).unwrap().get_steps() {
                if let PathStep::Lane(l) = step {
                    colorer.add_l(*l, app.cs.unzoomed_bus);
                }
            }
        }
        details.unzoomed.append(colorer.unzoomed);
        details.zoomed.append(colorer.zoomed);

        for pt in bus_locations {
            details.unzoomed.push(
                Color::BLUE,
                Circle::new(pt, Distance::meters(20.0)).to_polygon(),
            );
            details.zoomed.push(
                Color::BLUE.alpha(0.5),
                Circle::new(pt, Distance::meters(5.0)).to_polygon(),
            );
        }

        for bs in &route.stops {
            let bs = map.get_bs(*bs);
            details.unzoomed.append(
                Text::from(Line(&bs.name))
                    .with_bg()
                    .render_to_batch(ctx.prerender)
                    .centered_on(bs.sidewalk_pos.pt(map)),
            );
            details.zoomed.append(
                Text::from(Line(&bs.name))
                    .with_bg()
                    .render_to_batch(ctx.prerender)
                    .scale(0.1)
                    .centered_on(bs.sidewalk_pos.pt(map)),
            );
        }
    }

    rows
}
