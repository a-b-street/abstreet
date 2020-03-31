use crate::app::App;
use crate::info::{building, header_btns, make_table, make_tabs, trip, Details, Tab, Text};
use crate::render::Renderable;
use ezgui::{Btn, Color, EventCtx, Line, TextExt, Widget};
use map_model::Map;
use sim::{
    AgentID, CarID, PedestrianID, Person, PersonID, PersonState, TripID, TripResult, VehicleType,
};
use std::collections::BTreeSet;

pub fn status(ctx: &mut EventCtx, app: &App, details: &mut Details, id: PersonID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::PersonStatus(id));

    let map = &app.primary.map;
    let sim = &app.primary.sim;

    match sim.get_person(id).state {
        PersonState::Inside(b) => {
            // TODO hyperlink
            rows.push(
                format!("Currently inside {}", map.get_b(b).just_address(map)).draw_text(ctx),
            );
        }
        PersonState::OffMap => {
            rows.push("Currently outside the map boundaries".draw_text(ctx));
        }
        PersonState::Limbo => {
            rows.push(
                "Currently in limbo -- they broke out of the Matrix! Woops. (A bug occurred)"
                    .draw_text(ctx),
            );
        }
        PersonState::Trip(t) => {
            if let Some(a) = sim.trip_to_agent(t).ok() {
                let (kv, extra) = match a {
                    AgentID::Car(c) => sim.car_properties(c, map),
                    AgentID::Pedestrian(p) => sim.ped_properties(p, map),
                };
                rows.extend(make_table(ctx, kv));
                if !extra.is_empty() {
                    let mut txt = Text::from(Line(""));
                    for line in extra {
                        txt.add(Line(line));
                    }
                    rows.push(txt.draw(ctx));
                }
            }

            let mut open_trips = BTreeSet::new();
            open_trips.insert(t);
            rows.push(trip::details(ctx, app, t, details, id, open_trips));
        }
    }

    rows
}

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
    // TODO Color by future/ongoing/done
    // TODO Do we need to echo current status here?
    for t in &person.trips {
        match sim.trip_to_agent(*t) {
            TripResult::TripNotStarted => {
                if wheres_waldo {
                    wheres_waldo = false;
                    rows.push(current_status(ctx, person, map));
                }
            }
            TripResult::Ok(_) | TripResult::ModeChange => {
                // ongoing
                assert!(wheres_waldo);
                wheres_waldo = false;
            }
            TripResult::TripDone => {
                assert!(wheres_waldo);
            }
            TripResult::TripDoesntExist => unreachable!(),
        }
        if open_trips.contains(t) {
            rows.push(trip::details(ctx, app, *t, details, id, open_trips.clone()));
        } else {
            // TODO Style wrong. Button should be the entire row.
            rows.push(
                Widget::row(vec![
                    format!("Trip #{}", t.0).draw_text(ctx),
                    Btn::text_fg("▼")
                        .build(ctx, format!("show Trip #{}", t.0), None)
                        .align_right(),
                ])
                .outline(2.0, Color::WHITE),
            );
            let mut new_trips = open_trips.clone();
            new_trips.insert(*t);
            details.hyperlinks.insert(
                format!("show Trip #{}", t.0),
                Tab::PersonTrips(id, new_trips),
            );
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
            Btn::text_fg(format!("Person #{}", person.0)).build_def(ctx, None),
        ]));
        details
            .hyperlinks
            .insert(format!("Person #{}", person.0), Tab::PersonStatus(person));
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

    let (kv, extra) = app.primary.sim.car_properties(id, &app.primary.map);
    rows.extend(make_table(ctx, kv));
    if !extra.is_empty() {
        let mut txt = Text::from(Line(""));
        for line in extra {
            txt.add(Line(line));
        }
        rows.push(txt.draw(ctx));
    }

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
        Line(format!("Person #{} ({})", id.0, descr))
            .small_heading()
            .draw(ctx),
        header_btns(ctx),
    ]));

    let mut open_trips = BTreeSet::new();
    if let Some(t) = current_trip {
        open_trips.insert(t);
    }
    rows.push(make_tabs(
        ctx,
        &mut details.hyperlinks,
        tab,
        vec![
            ("Status", Tab::PersonStatus(id)),
            ("Trips", Tab::PersonTrips(id, open_trips)),
            ("Bio", Tab::PersonBio(id)),
        ],
    ));

    rows
}

fn current_status(ctx: &EventCtx, person: &Person, map: &Map) -> Widget {
    match person.state {
        PersonState::Inside(b) => {
            // TODO hyperlink
            format!("Currently inside {}", map.get_b(b).just_address(map)).draw_text(ctx)
        }
        PersonState::Trip(_) => unreachable!(),
        PersonState::OffMap => "Currently outside the map boundaries".draw_text(ctx),
        PersonState::Limbo => "Currently in limbo -- they broke out of the Matrix! Woops. (A bug \
                               occurred)"
            .draw_text(ctx),
    }
}
