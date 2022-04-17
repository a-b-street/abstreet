use abstutil::{prettyprint_usize, Counter};
use geom::{Circle, Distance, Time};
use map_gui::tools::ColorNetwork;
use map_gui::ID;
use map_model::{PathStep, TransitRoute, TransitRouteID, TransitStopID};
use sim::{AgentID, CarID};
use widgetry::{Color, ControlState, EventCtx, Key, Line, RewriteColor, Text, TextExt, Widget};

use crate::app::App;
use crate::info::{header_btns, make_tabs, Details, Tab};

pub fn stop(ctx: &mut EventCtx, app: &App, details: &mut Details, id: TransitStopID) -> Widget {
    let header = Widget::row(vec![
        Line("Bus stop").small_heading().into_widget(ctx),
        header_btns(ctx),
    ]);

    Widget::custom_col(vec![header, stop_body(ctx, app, details, id).tab_body(ctx)])
}

fn stop_body(ctx: &mut EventCtx, app: &App, details: &mut Details, id: TransitStopID) -> Widget {
    let mut rows = vec![];

    let ts = app.primary.map.get_ts(id);
    let sim = &app.primary.sim;

    rows.push(Line(&ts.name).into_widget(ctx));

    let all_arrivals = &sim.get_analytics().bus_arrivals;
    for r in app.primary.map.get_routes_serving_stop(id) {
        // Full names can overlap, so include the ID
        let label = format!("{} ({})", r.long_name, r.id);
        rows.push(
            ctx.style()
                .btn_outline
                .text(format!("Route {}", r.short_name))
                .build_widget(ctx, &label),
        );
        details.hyperlinks.insert(label, Tab::TransitRoute(r.id));

        let arrivals: Vec<(Time, CarID)> = all_arrivals
            .iter()
            .filter(|(_, _, route, stop)| r.id == *route && id == *stop)
            .map(|(t, car, _, _)| (*t, *car))
            .collect();
        let mut txt = Text::new();
        if let Some((t, _)) = arrivals.last() {
            // TODO Button to jump to the bus
            txt.add_line(Line(format!("  Last bus arrived {} ago", sim.time() - *t)).secondary());
        } else {
            txt.add_line(Line("  No arrivals yet").secondary());
        }
        rows.push(txt.into_widget(ctx));
    }

    let mut boardings: Counter<TransitRouteID> = Counter::new();
    let mut alightings: Counter<TransitRouteID> = Counter::new();
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
    txt.add_line("Total");
    txt.append(
        Line(format!(
            ": {} boardings, {} alightings",
            prettyprint_usize(boardings.sum()),
            prettyprint_usize(alightings.sum())
        ))
        .secondary(),
    );
    for r in app.primary.map.get_routes_serving_stop(id) {
        txt.add_line(format!("Route {}", r.short_name));
        txt.append(
            Line(format!(
                ": {} boardings, {} alightings",
                prettyprint_usize(boardings.get(r.id)),
                prettyprint_usize(alightings.get(r.id))
            ))
            .secondary(),
        );
    }
    rows.push(txt.into_widget(ctx));

    // Draw where the bus/train stops
    details.draw_extra.zoomed.push(
        app.cs.bus_body.alpha(0.5),
        Circle::new(ts.driving_pos.pt(&app.primary.map), Distance::meters(2.5)).to_polygon(),
    );

    Widget::col(rows)
}

pub fn bus_status(ctx: &mut EventCtx, app: &App, details: &mut Details, id: CarID) -> Widget {
    Widget::custom_col(vec![
        bus_header(ctx, app, details, id, Tab::TransitVehicleStatus(id)),
        bus_status_body(ctx, app, details, id).tab_body(ctx),
    ])
}

fn bus_status_body(ctx: &mut EventCtx, app: &App, details: &mut Details, id: CarID) -> Widget {
    let mut rows = vec![];

    let route = app
        .primary
        .map
        .get_tr(app.primary.sim.bus_route_id(id).unwrap());

    rows.push(
        ctx.style()
            .btn_outline
            .text(format!("Serves route {}", route.short_name))
            .build_def(ctx),
    );
    details.hyperlinks.insert(
        format!("Serves route {}", route.short_name),
        Tab::TransitRoute(route.id),
    );

    rows.push(
        Line(format!(
            "Currently has {} passengers",
            app.primary.sim.num_transit_passengers(id),
        ))
        .into_widget(ctx),
    );

    Widget::col(rows)
}

fn bus_header(ctx: &mut EventCtx, app: &App, details: &mut Details, id: CarID, tab: Tab) -> Widget {
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
            app.primary.map.get_tr(route).short_name
        ))
        .small_heading()
        .into_widget(ctx),
        header_btns(ctx),
    ]));
    rows.push(make_tabs(
        ctx,
        &mut details.hyperlinks,
        tab,
        vec![("Status", Tab::TransitVehicleStatus(id))],
    ));

    Widget::custom_col(rows)
}

pub fn route(ctx: &mut EventCtx, app: &App, details: &mut Details, id: TransitRouteID) -> Widget {
    let header = {
        let map = &app.primary.map;
        let route = map.get_tr(id);

        Widget::row(vec![
            Line(format!("Route {}", route.short_name))
                .small_heading()
                .into_widget(ctx),
            header_btns(ctx),
        ])
    };

    Widget::custom_col(vec![
        header,
        route_body(ctx, app, details, id).tab_body(ctx),
    ])
}

fn route_body(ctx: &mut EventCtx, app: &App, details: &mut Details, id: TransitRouteID) -> Widget {
    let mut rows = vec![];

    let map = &app.primary.map;
    let route = map.get_tr(id);
    rows.push(
        Text::from(&route.long_name)
            .wrap_to_pct(ctx, 20)
            .into_widget(ctx),
    );

    let buses = app.primary.sim.status_of_buses(id, map);
    let mut bus_locations = Vec::new();
    if buses.is_empty() {
        rows.push(format!("No {} running", route.plural_noun()).text_widget(ctx));
    } else {
        for (bus, _, _, pt) in buses {
            rows.push(ctx.style().btn_outline.text(bus.to_string()).build_def(ctx));
            details
                .hyperlinks
                .insert(bus.to_string(), Tab::TransitVehicleStatus(bus));
            bus_locations.push(pt);
        }
    }

    let mut boardings: Counter<TransitStopID> = Counter::new();
    let mut alightings: Counter<TransitStopID> = Counter::new();
    let mut waiting: Counter<TransitStopID> = Counter::new();
    for ts in &route.stops {
        if let Some(list) = app.primary.sim.get_analytics().passengers_boarding.get(ts) {
            for (_, r, _) in list {
                if *r == id {
                    boardings.inc(*ts);
                }
            }
        }
        if let Some(list) = app.primary.sim.get_analytics().passengers_alighting.get(ts) {
            for (_, r) in list {
                if *r == id {
                    alightings.inc(*ts);
                }
            }
        }

        for (_, r, _, _) in app.primary.sim.get_people_waiting_at_stop(*ts) {
            if *r == id {
                waiting.inc(*ts);
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
        .into_widget(ctx),
    );

    rows.push(format!("{} stops", route.stops.len()).text_widget(ctx));
    {
        let i = map.get_i(map.get_l(route.start).src_i);
        let name = format!("Starts at {}", i.name(app.opts.language.as_ref(), map));
        rows.push(Widget::row(vec![
            ctx.style()
                .btn_plain
                .icon("system/assets/timeline/start_pos.svg")
                .image_color(RewriteColor::NoOp, ControlState::Default)
                .build_widget(ctx, &name),
            name.clone().text_widget(ctx),
        ]));
        details.warpers.insert(name, ID::Intersection(i.id));
    }
    for (idx, ts) in route.stops.iter().enumerate() {
        let ts = map.get_ts(*ts);
        let name = format!("Stop {}: {}", idx + 1, ts.name);
        rows.push(Widget::row(vec![
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/pin.svg")
                .build_widget(ctx, &name),
            Text::from_all(vec![
                Line(&ts.name),
                Line(format!(
                    ": {} boardings, {} alightings, {} currently waiting",
                    prettyprint_usize(boardings.get(ts.id)),
                    prettyprint_usize(alightings.get(ts.id)),
                    prettyprint_usize(waiting.get(ts.id))
                ))
                .secondary(),
            ])
            .into_widget(ctx),
        ]));
        details.warpers.insert(name, ID::TransitStop(ts.id));
    }
    if let Some(l) = route.end_border {
        let i = map.get_i(map.get_l(l).dst_i);
        let name = format!("Ends at {}", i.name(app.opts.language.as_ref(), map));
        rows.push(Widget::row(vec![
            ctx.style()
                .btn_plain
                .icon("system/assets/timeline/goal_pos.svg")
                .image_color(RewriteColor::NoOp, ControlState::Default)
                .build_widget(ctx, &name),
            name.clone().text_widget(ctx),
        ]));
        details.warpers.insert(name, ID::Intersection(i.id));
    }

    // TODO Soon it'll be time to split into tabs
    {
        rows.push(
            ctx.style()
                .btn_outline
                .text("Edit schedule")
                .hotkey(Key::E)
                .build_widget(ctx, format!("edit {}", route.id)),
        );
        rows.push(describe_schedule(route).into_widget(ctx));
    }

    // Draw the route, label stops, and show location of buses
    {
        let mut colorer = ColorNetwork::new(app);
        for path in route.all_paths(map).unwrap() {
            for step in path.get_steps() {
                if let PathStep::Lane(l) = step {
                    colorer.add_l(*l, app.cs.unzoomed_bus);
                }
            }
        }
        details.draw_extra.append(colorer.draw);

        for pt in bus_locations {
            details.draw_extra.unzoomed.push(
                Color::BLUE,
                Circle::new(pt, Distance::meters(20.0)).to_polygon(),
            );
            details.draw_extra.zoomed.push(
                Color::BLUE.alpha(0.5),
                Circle::new(pt, Distance::meters(5.0)).to_polygon(),
            );
        }

        for (idx, ts) in route.stops.iter().enumerate() {
            let ts = map.get_ts(*ts);
            details.draw_extra.unzoomed.append(
                Text::from(format!("{}) {}", idx + 1, ts.name))
                    .bg(app.cs.bus_layer)
                    .render_autocropped(ctx)
                    .centered_on(ts.sidewalk_pos.pt(map)),
            );
            details.draw_extra.zoomed.append(
                Text::from(format!("{}) {}", idx + 1, ts.name))
                    .bg(app.cs.bus_layer)
                    .render_autocropped(ctx)
                    .scale(0.1)
                    .centered_on(ts.sidewalk_pos.pt(map)),
            );
        }
    }

    Widget::col(rows)
}

// TODO Unit test
fn describe_schedule(route: &TransitRoute) -> Text {
    let mut txt = Text::new();
    txt.add_line(format!(
        "{} {}s run this route daily",
        route.spawn_times.len(),
        route.plural_noun()
    ));

    if false {
        // Compress the times
        let mut start = route.spawn_times[0];
        let mut last = None;
        let mut dt = None;
        for t in route.spawn_times.iter().skip(1) {
            if let Some(l) = last {
                let new_dt = *t - l;
                if Some(new_dt) == dt {
                    last = Some(*t);
                } else {
                    txt.add_line(format!(
                        "Every {} from {} to {}",
                        dt.unwrap(),
                        start.ampm_tostring(),
                        l.ampm_tostring()
                    ));
                    start = l;
                    last = Some(*t);
                    dt = Some(new_dt);
                }
            } else {
                last = Some(*t);
                dt = Some(*t - start);
            }
        }
        // Handle end
        txt.add_line(format!(
            "Every {} from {} to {}",
            dt.unwrap(),
            start.ampm_tostring(),
            last.unwrap().ampm_tostring()
        ));
    } else {
        // Just list the times
        for t in &route.spawn_times {
            txt.add_line(t.ampm_tostring());
        }
    }
    txt
}
