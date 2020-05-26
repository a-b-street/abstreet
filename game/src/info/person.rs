use crate::app::App;
use crate::info::{building, header_btns, make_table, make_tabs, trip, Details, OpenTrip, Tab};
use ezgui::{
    hotkey, Btn, Color, EventCtx, GeomBatch, Key, Line, RewriteColor, Text, TextExt, TextSpan,
    Widget,
};
use geom::Duration;
use map_model::Map;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use sim::{
    AgentID, CarID, ParkingSpot, PedestrianID, Person, PersonID, PersonState, TripID, TripMode,
    TripResult, VehicleType,
};
use std::collections::BTreeMap;

pub fn trips(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: PersonID,
    open_trips: &mut BTreeMap<TripID, OpenTrip>,
    is_paused: bool,
) -> Vec<Widget> {
    let mut rows = header(
        ctx,
        app,
        details,
        id,
        Tab::PersonTrips(id, open_trips.clone()),
        is_paused,
    );

    let map = &app.primary.map;
    let sim = &app.primary.sim;
    let person = sim.get_person(id);

    // I'm sorry for bad variable names
    let mut wheres_waldo = true;
    for (idx, t) in person.trips.iter().enumerate() {
        let (trip_status, color, maybe_info) = match sim.trip_to_agent(*t) {
            TripResult::TripNotStarted => {
                if wheres_waldo {
                    wheres_waldo = false;
                    rows.push(current_status(ctx, person, map));
                }
                let start_time = sim.trip_info(*t).0;
                if sim.time() > start_time {
                    (
                        "delayed start",
                        Color::YELLOW,
                        open_trips
                            .get_mut(t)
                            .map(|open_trip| trip::future(ctx, app, *t, open_trip, details)),
                    )
                } else {
                    (
                        "future",
                        Color::hex("#4CA7E9"),
                        open_trips
                            .get_mut(t)
                            .map(|open_trip| trip::future(ctx, app, *t, open_trip, details)),
                    )
                }
            }
            TripResult::Ok(a) => {
                assert!(wheres_waldo);
                wheres_waldo = false;
                (
                    "ongoing",
                    Color::hex("#7FFA4D"),
                    open_trips
                        .get_mut(t)
                        .map(|open_trip| trip::ongoing(ctx, app, *t, a, open_trip, details)),
                )
            }
            TripResult::RemoteTrip => {
                assert!(wheres_waldo);
                wheres_waldo = false;
                (
                    "ongoing (off-map)",
                    Color::hex("#7FFA4D"),
                    // TODO Details about an ongoing remote trip?
                    None,
                )
            }
            TripResult::ModeChange => {
                // TODO No details. Weird case.
                assert!(wheres_waldo);
                wheres_waldo = false;
                (
                    "ongoing",
                    Color::hex("#7FFA4D"),
                    open_trips.get(t).map(|_| Widget::nothing()),
                )
            }
            TripResult::TripDone => {
                assert!(wheres_waldo);
                (
                    "finished",
                    Color::hex("#A3A3A3"),
                    if open_trips.contains_key(t) {
                        Some(trip::finished(ctx, app, id, open_trips, *t, details))
                    } else {
                        None
                    },
                )
            }
            TripResult::TripAborted => {
                // Aborted trips can happen anywhere in the schedule right now
                (
                    "broken",
                    Color::hex("#EB3223"),
                    open_trips.get(t).map(|_| trip::aborted(ctx, app, *t)),
                )
            }
            TripResult::TripDoesntExist => unreachable!(),
        };
        let (_, _, _, trip_mode) = sim.trip_info(*t);

        // TODO Style wrong. Button should be the entire row.
        rows.push(
            Widget::row(vec![
                format!("Trip {} ", idx + 1).draw_text(ctx).margin_right(21),
                Widget::row(vec![
                    Widget::draw_svg_transform(
                        ctx,
                        match trip_mode {
                            TripMode::Walk => "../data/system/assets/meters/pedestrian.svg",
                            TripMode::Bike => "../data/system/assets/meters/bike.svg",
                            TripMode::Drive => "../data/system/assets/meters/car.svg",
                            TripMode::Transit => "../data/system/assets/meters/bus.svg",
                        },
                        RewriteColor::ChangeAll(color),
                    )
                    .margin_right(10),
                    Line(trip_status).small().fg(color).draw(ctx),
                ])
                .fully_rounded()
                .outline(1.0, color)
                .bg(color.alpha(0.2))
                .padding(5)
                .margin_right(21),
                if trip_status == "finished" {
                    if let Some(before) = app
                        .has_prebaked()
                        .and_then(|_| app.prebaked().finished_trip_time(*t))
                    {
                        let (after, _) = app.primary.sim.finished_trip_time(*t).unwrap();
                        Text::from(cmp_duration_shorter(after, before)).draw(ctx)
                    } else {
                        Widget::nothing()
                    }
                } else {
                    Widget::nothing()
                },
                Btn::plaintext(if open_trips.contains_key(t) {
                    "▼"
                } else {
                    "▲"
                })
                .build(
                    ctx,
                    format!(
                        "{} {}",
                        if open_trips.contains_key(t) {
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
            .bg(app.cs.inner_panel)
            .margin_above(if idx == 0 { 0 } else { 16 }),
        );

        if let Some(info) = maybe_info {
            rows.push(
                info.outline(2.0, Color::WHITE)
                    .bg(app.cs.inner_panel)
                    .padding(16),
            );

            let mut new_trips = open_trips.clone();
            new_trips.remove(t);
            details
                .hyperlinks
                .insert(format!("hide {}", t), Tab::PersonTrips(id, new_trips));
        } else {
            let mut new_trips = open_trips.clone();
            new_trips.insert(*t, OpenTrip::new());
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

pub fn bio(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: PersonID,
    is_paused: bool,
) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::PersonBio(id), is_paused);
    let person = app.primary.sim.get_person(id);

    let mut svg_data = Vec::new();
    svg_face::generate_face(&mut svg_data, &mut XorShiftRng::seed_from_u64(id.0 as u64)).unwrap();
    let mut batch = GeomBatch::new();
    batch.add_svg_contents(svg_data);
    batch = batch.autocrop();
    let dims = batch.get_dims();
    batch = batch.scale((200.0 / dims.width).min(200.0 / dims.height));
    rows.push(
        Widget::draw_batch(ctx, batch)
            .centered_horiz()
            .padding(10)
            .outline(5.0, Color::WHITE),
    );

    rows.extend(make_table(
        ctx,
        vec![
            ("Name", "Somebody".to_string()),
            ("Age", "42".to_string()),
            ("Occupation", "classified".to_string()),
            ("Debug ID", format!("{:?}", person.orig_id)),
        ],
    ));
    // TODO Mad libs!
    // - Keeps a collection of ___ at all times
    // - Origin story: accidentally fell into a vat of cheese curds
    // - Superpower: Makes unnervingly realistic squirrel noises
    // - Rides a fixie
    // - Has 17 pinky toe piercings (surprising, considering they're the state champ at
    // barefoot marathons)

    if let Some(p) = app.primary.sim.get_pandemic_model() {
        // TODO add hospitalization/quarantine probably
        let status = if p.is_sane(id) {
            "Susceptible".to_string()
        } else if p.is_exposed(id) {
            format!("Exposed at {}", p.get_time(id).unwrap().ampm_tostring())
        } else if p.is_infectious(id) {
            format!("Infected at {}", p.get_time(id).unwrap().ampm_tostring())
        } else if p.is_recovered(id) {
            format!("Recovered at {}", p.get_time(id).unwrap().ampm_tostring())
        } else if p.is_dead(id) {
            format!("Dead at {}", p.get_time(id).unwrap().ampm_tostring())
        } else {
            // TODO More info here? Make these public too?
            "Other (hospitalized or quarantined)".to_string()
        };
        rows.push(
            Text::from_all(vec![
                Line("Pandemic model state: ").secondary(),
                Line(status),
            ])
            .draw(ctx)
            .margin_below(5),
        );
    }

    let mut has_bike = false;
    for v in &person.vehicles {
        if v.vehicle_type == VehicleType::Bike {
            has_bike = true;
        } else {
            if app.primary.sim.lookup_parked_car(v.id).is_some() {
                rows.push(
                    Btn::text_bg2(format!("Owner of {} (parked)", v.id)).build_def(ctx, None),
                );
                details
                    .hyperlinks
                    .insert(format!("Owner of {} (parked)", v.id), Tab::ParkedCar(v.id));
            } else if let PersonState::Trip(t) = person.state {
                match app.primary.sim.trip_to_agent(t) {
                    TripResult::Ok(AgentID::Car(x)) if x == v.id => {
                        rows.push(format!("Owner of {} (currently driving)", v.id).draw_text(ctx));
                    }
                    _ => {
                        rows.push(format!("Owner of {} (off-map)", v.id).draw_text(ctx));
                    }
                }
            } else {
                rows.push(format!("Owner of {} (off-map)", v.id).draw_text(ctx));
            }
        }
    }
    if has_bike {
        rows.push("Owns a bike".draw_text(ctx));
    }

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
                OpenTrip::single(
                    app.primary
                        .sim
                        .agent_to_trip(AgentID::Pedestrian(*id))
                        .unwrap(),
                ),
            ),
        );
    }

    rows
}

pub fn parked_car(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: CarID,
    is_paused: bool,
) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Parked car #{}", id.0))
            .small_heading()
            .draw(ctx),
        Widget::row(vec![
            // Little indirect, but the handler of this action is actually the ContextualActions
            // for SandboxMode.
            if is_paused {
                Btn::svg_def("../data/system/assets/tools/location.svg")
                    .build(ctx, "follow", hotkey(Key::F))
                    .margin(5)
            } else {
                // TODO Blink
                Btn::svg_def("../data/system/assets/tools/location.svg")
                    .normal_color(RewriteColor::ChangeAll(Color::hex("#7FFA4D")))
                    .build(ctx, "unfollow", hotkey(Key::F))
                    .margin(5)
            },
            Btn::plaintext("X").build(ctx, "close info", hotkey(Key::Escape)),
        ])
        .align_right(),
    ]));

    // TODO how long idle, prev trips, next trips, etc

    let p = app.primary.sim.get_owner_of_car(id).unwrap();
    rows.push(Btn::text_bg2(format!("Owned by {}", p)).build_def(ctx, None));
    details.hyperlinks.insert(
        format!("Owned by {}", p),
        Tab::PersonTrips(p, BTreeMap::new()),
    );

    if let Some(p) = app.primary.sim.lookup_parked_car(id) {
        match p.spot {
            ParkingSpot::Onstreet(_, _) => {
                ctx.canvas.center_on_map_pt(
                    app.primary
                        .sim
                        .canonical_pt_for_agent(AgentID::Car(id), &app.primary.map)
                        .unwrap(),
                );
            }
            ParkingSpot::Offstreet(b, _) => {
                ctx.canvas
                    .center_on_map_pt(app.primary.map.get_b(b).polygon.center());
                rows.push(
                    format!("Parked inside {}", app.primary.map.get_b(b).address).draw_text(ctx),
                );
            }
        }
    } else {
        rows.push("No longer parked".draw_text(ctx));
    }

    rows
}

fn header(
    ctx: &mut EventCtx,
    app: &App,
    details: &mut Details,
    id: PersonID,
    tab: Tab,
    is_paused: bool,
) -> Vec<Widget> {
    let mut rows = vec![];

    let (current_trip, (descr, maybe_icon)) = match app.primary.sim.get_person(id).state {
        PersonState::Inside(b) => {
            ctx.canvas
                .center_on_map_pt(app.primary.map.get_b(b).label_center);
            building::draw_occupants(details, app, b, Some(id));
            (
                None,
                ("indoors", Some("../data/system/assets/tools/home.svg")),
            )
        }
        PersonState::Trip(t) => (
            Some(t),
            if let Some(a) = app.primary.sim.trip_to_agent(t).ok() {
                if let Some(pt) = app.primary.sim.canonical_pt_for_agent(a, &app.primary.map) {
                    ctx.canvas.center_on_map_pt(pt);
                }
                match a {
                    AgentID::Pedestrian(_) => (
                        "walking",
                        Some("../data/system/assets/meters/pedestrian.svg"),
                    ),
                    AgentID::Car(c) => match c.1 {
                        VehicleType::Car => {
                            ("driving", Some("../data/system/assets/meters/car.svg"))
                        }
                        VehicleType::Bike => {
                            ("biking", Some("../data/system/assets/meters/bike.svg"))
                        }
                        VehicleType::Bus => unreachable!(),
                    },
                }
            } else {
                // TODO Really should clean up the TripModeChange issue
                ("...", None)
            },
        ),
        PersonState::OffMap => (None, ("off map", None)),
    };

    rows.push(Widget::row(vec![
        Line(format!("{}", id)).small_heading().draw(ctx),
        if let Some(icon) = maybe_icon {
            Widget::draw_svg_transform(ctx, icon, RewriteColor::ChangeAll(Color::hex("#A3A3A3")))
                .margin_left(28)
        } else {
            Widget::nothing()
        },
        Line(format!("{}", descr))
            .small_heading()
            .fg(Color::hex("#A3A3A3"))
            .draw(ctx)
            .margin_horiz(10),
        Widget::row(vec![
            // Little indirect, but the handler of this action is actually the ContextualActions
            // for SandboxMode.
            if is_paused {
                Btn::svg_def("../data/system/assets/tools/location.svg")
                    .build(ctx, "follow", hotkey(Key::F))
                    .margin(5)
            } else {
                // TODO Blink
                Btn::svg_def("../data/system/assets/tools/location.svg")
                    .normal_color(RewriteColor::ChangeAll(Color::hex("#7FFA4D")))
                    .build(ctx, "unfollow", hotkey(Key::F))
                    .margin(5)
            },
            Btn::plaintext("X").build(ctx, "close info", hotkey(Key::Escape)),
        ])
        .align_right(),
    ]));

    let open_trips = if let Some(t) = current_trip {
        OpenTrip::single(t)
    } else {
        BTreeMap::new()
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
            format!("Currently inside {}", map.get_b(b).address).draw_text(ctx)
        }
        PersonState::Trip(_) => unreachable!(),
        PersonState::OffMap => "Currently outside the map boundaries".draw_text(ctx),
    })
    .margin_vert(16)
}

// TODO Dedupe with the version in helpers
fn cmp_duration_shorter(after: Duration, before: Duration) -> TextSpan {
    if after.epsilon_eq(before) {
        Line("no change").small()
    } else if after < before {
        Line(format!("{} faster", before - after))
            .small()
            .fg(Color::GREEN)
    } else if after > before {
        Line(format!("{} slower", after - before))
            .small()
            .fg(Color::RED)
    } else {
        unreachable!()
    }
}
