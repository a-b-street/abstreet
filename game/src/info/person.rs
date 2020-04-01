use crate::app::App;
use crate::colors;
use crate::info::{building, header_btns, make_table, make_tabs, trip, Details, Tab};
use crate::render::Renderable;
use ezgui::{Btn, Color, EventCtx, Line, TextExt, Widget};
use map_model::Map;
use maplit::btreeset;
use sim::{
    AgentID, CarID, PedestrianID, Person, PersonID, PersonState, TripID, TripResult, VehicleType,
};
use std::collections::BTreeSet;

pub fn trips(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: PersonID,
    open_trips: &BTreeSet<TripID>,
) -> Vec<Widget> {
    let mut rows = header(
        ctx,
        app,
        details,
        id,
        Tab::PersonTrips(id, open_trips.clone()),
    );

    let map = &app.primary.map;
    let sim = &app.primary.sim;
    let person = sim.get_person(id);

    // I'm sorry for bad variable names
    let mut wheres_waldo = true;
    let mut is_first = true;
    for t in &person.trips {
        let trip_status = match sim.trip_to_agent(*t) {
            TripResult::TripNotStarted => {
                if wheres_waldo {
                    wheres_waldo = false;
                    rows.push(current_status(ctx, person, map));
                }
                "future"
            }
            TripResult::Ok(_) | TripResult::ModeChange => {
                // ongoing
                assert!(wheres_waldo);
                wheres_waldo = false;
                "ongoing"
            }
            TripResult::TripDone => {
                assert!(wheres_waldo);
                "finished"
            }
            TripResult::TripDoesntExist => unreachable!(),
        };

        // TODO Style wrong. Button should be the entire row.
        rows.push(
            Widget::row(vec![
                t.to_string().draw_text(ctx),
                if trip_status == "ongoing" {
                    // TODO Padding doesn't work without wrapping in a row
                    Widget::row(vec![Line(trip_status)
                        .small()
                        .fg(Color::hex("#7FFA4D"))
                        .draw(ctx)])
                    .rounder_outline(1.0, 10.0, Color::hex("#7FFA4D"))
                    .bg(Color::rgba(127, 250, 77, 0.2))
                    .padding(5)
                } else {
                    Line(trip_status).small().draw(ctx)
                }
                .margin_horiz(15)
                .margin_vert(5),
                Btn::plaintext(if open_trips.contains(t) { "▲" } else { "▼" })
                    .build(
                        ctx,
                        format!(
                            "{} {}",
                            if open_trips.contains(t) {
                                "hide"
                            } else {
                                "show"
                            },
                            t
                        ),
                        None,
                    )
                    .align_right(),
            ])
            .outline(2.0, Color::WHITE)
            .padding(16)
            .bg(colors::INNER_PANEL)
            .margin_above(if is_first { 0 } else { 16 }),
        );
        is_first = false;

        if open_trips.contains(t) {
            rows.push(
                trip::details(ctx, app, *t, details)
                    .outline(2.0, Color::WHITE)
                    .bg(colors::INNER_PANEL)
                    .padding(16),
            );

            let mut new_trips = open_trips.clone();
            new_trips.remove(t);
            details
                .hyperlinks
                .insert(format!("hide {}", t), Tab::PersonTrips(id, new_trips));
        } else {
            let mut new_trips = open_trips.clone();
            new_trips.insert(*t);
            details
                .hyperlinks
                .insert(format!("show {}", t), Tab::PersonTrips(id, new_trips));
        }
    }
    if wheres_waldo {
        rows.push(current_status(ctx, person, map));
    }

    rows
}

pub fn bio(ctx: &EventCtx, app: &App, details: &mut Details, id: PersonID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::PersonBio(id));

    // TODO A little picture
    rows.extend(make_table(
        ctx,
        vec![
            ("Name", "Somebody".to_string()),
            ("Age", "42".to_string()),
            ("Occupation", "classified".to_string()),
        ],
    ));
    // TODO Mad libs!
    // - Keeps a collection of ___ at all times
    // - Origin story: accidentally fell into a vat of cheese curds
    // - Superpower: Makes unnervingly realistic squirrel noises
    // - Rides a fixie
    // - Has 17 pinky toe piercings (surprising, considering they're the state champ at
    // barefoot marathons)

    rows
}

pub fn crowd(
    ctx: &EventCtx,
    app: &App,
    details: &mut Details,
    members: &Vec<PedestrianID>,
) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line("Pedestrian crowd").small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    for (idx, id) in members.into_iter().enumerate() {
        let person = app
            .primary
            .sim
            .agent_to_person(AgentID::Pedestrian(*id))
            .unwrap();
        // TODO What other info is useful to summarize?
        rows.push(Widget::row(vec![
            format!("{})", idx + 1).draw_text(ctx),
            Btn::text_fg(person.to_string()).build_def(ctx, None),
        ]));
        details.hyperlinks.insert(
            person.to_string(),
            Tab::PersonTrips(
                person,
                btreeset! {app.primary.sim.agent_to_trip(AgentID::Pedestrian(*id)).unwrap()},
            ),
        );
    }

    rows
}

pub fn parked_car(ctx: &EventCtx, app: &App, details: &mut Details, id: CarID) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Parked car #{}", id.0))
            .small_heading()
            .draw(ctx),
        header_btns(ctx),
    ]));

    let kv = app.primary.sim.car_properties(id, &app.primary.map);
    rows.extend(make_table(ctx, kv));

    if let Some(b) = app.primary.sim.get_owner_of_car(id) {
        // TODO Mention this, with a warp tool
        details.unzoomed.push(
            app.cs
                .get_def("something associated with something else", Color::PURPLE),
            app.primary.draw_map.get_b(b).get_outline(&app.primary.map),
        );
        details.zoomed.push(
            app.cs.get("something associated with something else"),
            app.primary.draw_map.get_b(b).get_outline(&app.primary.map),
        );
    }

    rows
}

fn header(ctx: &EventCtx, app: &App, details: &mut Details, id: PersonID, tab: Tab) -> Vec<Widget> {
    let mut rows = vec![];

    let (current_trip, descr) = match app.primary.sim.get_person(id).state {
        PersonState::Inside(b) => {
            building::draw_occupants(details, app, b, Some(id));
            (None, "inside")
        }
        PersonState::Trip(t) => (
            Some(t),
            match app.primary.sim.trip_to_agent(t).ok() {
                Some(AgentID::Pedestrian(_)) => "on foot",
                Some(AgentID::Car(c)) => match c.1 {
                    VehicleType::Car => "in a car",
                    VehicleType::Bike => "on a bike",
                    VehicleType::Bus => unreachable!(),
                },
                // TODO Really should clean up the TripModeChange issue
                None => "...",
            },
        ),
        PersonState::OffMap => (None, "off map"),
        PersonState::Limbo => (None, "in limbo"),
    };

    rows.push(Widget::row(vec![
        Line(format!("{} ({})", id, descr))
            .small_heading()
            .draw(ctx),
        header_btns(ctx),
    ]));

    let open_trips = if let Some(t) = current_trip {
        btreeset! {t}
    } else {
        BTreeSet::new()
    };
    rows.push(make_tabs(
        ctx,
        &mut details.hyperlinks,
        tab,
        vec![
            ("Trips", Tab::PersonTrips(id, open_trips)),
            ("Bio", Tab::PersonBio(id)),
        ],
    ));

    rows
}

fn current_status(ctx: &EventCtx, person: &Person, map: &Map) -> Widget {
    (match person.state {
        PersonState::Inside(b) => {
            // TODO hyperlink
            format!("Currently inside {}", map.get_b(b).just_address(map)).draw_text(ctx)
        }
        PersonState::Trip(_) => unreachable!(),
        PersonState::OffMap => "Currently outside the map boundaries".draw_text(ctx),
        PersonState::Limbo => "Currently in limbo -- they broke out of the Matrix! Woops. (A bug \
                               occurred)"
            .draw_text(ctx),
    })
    .outline(2.0, Color::WHITE)
    .margin_vert(16)
}
