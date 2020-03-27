use crate::app::App;
use crate::colors;
use crate::helpers::ID;
use crate::info::trip::trip_details;
use crate::info::{make_table, make_tabs, InfoTab};
use ezgui::{Btn, EventCtx, Line, TextExt, Widget};
use geom::Time;
use map_model::Map;
use sim::{Person, PersonID, PersonState, TripMode, TripResult};
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
pub enum Tab {
    Bio,
}

pub fn info(
    ctx: &mut EventCtx,
    app: &App,
    id: PersonID,
    tab: InfoTab,
    // If None, then the panel is embedded
    header_btns: Option<Widget>,
    action_btns: Vec<Widget>,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
    warpers: &mut HashMap<String, ID>,
) -> Vec<Widget> {
    let mut rows = vec![];

    // Header
    if let Some(btns) = header_btns {
        rows.push(Widget::row(vec![
            Line(format!("Person #{}", id.0)).small_heading().draw(ctx),
            btns,
        ]));
    } else {
        rows.push(Line(format!("Person #{}", id.0)).small_heading().draw(ctx));
    }

    rows.push(make_tabs(
        ctx,
        hyperlinks,
        ID::Person(id),
        tab.clone(),
        vec![("Trips", InfoTab::Nil), ("Bio", InfoTab::Person(Tab::Bio))],
    ));

    match tab {
        InfoTab::Nil => {
            // TODO None of these right now
            rows.extend(action_btns);

            let map = &app.primary.map;
            let sim = &app.primary.sim;
            let person = sim.get_person(id);

            // I'm sorry for bad variable names
            let mut wheres_waldo = true;
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
                rows.push(
                    Widget::col(vec![
                        Line(format!("Trip #{}", t.0)).small_heading().draw(ctx),
                        trip_details(ctx, app, *t, None, warpers).0,
                    ])
                    .bg(colors::SECTION_BG)
                    .margin(10),
                );
            }
            if wheres_waldo {
                rows.push(current_status(ctx, person, map));
            }
        }
        InfoTab::Person(Tab::Bio) => {
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
        }
        _ => unreachable!(),
    }

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

pub fn summary(
    ctx: &EventCtx,
    app: &App,
    id: PersonID,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
) -> Widget {
    let person = app.primary.sim.get_person(id);

    let mut next_trip: Option<(Time, TripMode)> = None;
    for t in &person.trips {
        match app.primary.sim.trip_to_agent(*t) {
            TripResult::TripNotStarted => {
                let (start_time, _, _, mode) = app.primary.sim.trip_info(*t);
                next_trip = Some((start_time, mode));
                break;
            }
            TripResult::Ok(_) | TripResult::ModeChange => {
                // TODO What to do here? This is meant for building callers right now
                break;
            }
            TripResult::TripDone => {}
            TripResult::TripDoesntExist => unreachable!(),
        }
    }

    let label = format!("Person #{}", id.0);
    hyperlinks.insert(label.clone(), (ID::Person(id), InfoTab::Nil));
    Widget::col(vec![
        Btn::text_bg1(label).build_def(ctx, None),
        if let Some((t, mode)) = next_trip {
            format!("Leaving in {} to {}", t - app.primary.sim.time(), mode).draw_text(ctx)
        } else {
            "Staying inside".draw_text(ctx)
        },
    ])
}
