use crate::app::App;
use crate::helpers::ID;
use crate::info::trip::trip_details;
use crate::info::{make_table, make_tabs, person, InfoTab, TripDetails};
use crate::render::Renderable;
use ezgui::{Color, EventCtx, GeomBatch, Line, Text, Widget};
use sim::{AgentID, CarID, PedestrianID, PersonID, VehicleType};
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
pub enum Tab {
    Person(PersonID),
}

pub fn car_info(
    ctx: &mut EventCtx,
    app: &App,
    id: CarID,
    tab: InfoTab,
    header_btns: Widget,
    action_btns: Vec<Widget>,
    batch: &mut GeomBatch,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
) -> (Vec<Widget>, Option<TripDetails>) {
    let mut rows = vec![];

    let label = match id.1 {
        VehicleType::Car => "Car",
        VehicleType::Bike => "Bike",
        VehicleType::Bus => "Bus",
    };
    rows.push(Widget::row(vec![
        Line(format!("{} #{}", label, id.0)).roboto_bold().draw(ctx),
        header_btns,
    ]));

    rows.push(make_tabs(ctx, hyperlinks, ID::Car(id), tab.clone(), {
        let mut tabs = vec![("Info", InfoTab::Nil)];
        if let Some(p) = app
            .primary
            .sim
            .agent_to_trip(AgentID::Car(id))
            .and_then(|t| app.primary.sim.trip_to_person(t))
        {
            tabs.push(("Schedule", InfoTab::Agent(Tab::Person(p))));
        }
        tabs
    }));

    let mut details: Option<TripDetails> = None;

    match tab {
        InfoTab::Nil => {
            rows.extend(action_btns);

            let (kv, extra) = app.primary.sim.car_properties(id, &app.primary.map);
            rows.extend(make_table(ctx, kv));
            if !extra.is_empty() {
                let mut txt = Text::from(Line(""));
                for line in extra {
                    txt.add(Line(line));
                }
                rows.push(txt.draw(ctx));
            }

            let trip = if id.1 == VehicleType::Bus {
                None
            } else {
                app.primary.sim.agent_to_trip(AgentID::Car(id))
            };
            details = trip.map(|t| {
                let (more, details) = trip_details(
                    ctx,
                    app,
                    t,
                    app.primary.sim.progress_along_path(AgentID::Car(id)),
                );
                rows.push(more);
                details
            });
        }
        InfoTab::Agent(Tab::Person(p)) => {
            rows.extend(person::info(ctx, app, p, None, Vec::new(), hyperlinks));
        }
        _ => unreachable!(),
    }

    if let Some(b) = app.primary.sim.get_owner_of_car(id) {
        // TODO Mention this, with a warp tool
        batch.push(
            app.cs
                .get_def("something associated with something else", Color::PURPLE),
            app.primary.draw_map.get_b(b).get_outline(&app.primary.map),
        );
    }

    (rows, details)
}

pub fn ped_info(
    ctx: &mut EventCtx,
    app: &App,
    id: PedestrianID,
    tab: InfoTab,
    header_btns: Widget,
    action_btns: Vec<Widget>,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
) -> (Vec<Widget>, Option<TripDetails>) {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Pedestrian #{}", id.0))
            .roboto_bold()
            .draw(ctx),
        header_btns,
    ]));

    let trip = app
        .primary
        .sim
        .agent_to_trip(AgentID::Pedestrian(id))
        .unwrap();

    rows.push(make_tabs(
        ctx,
        hyperlinks,
        ID::Pedestrian(id),
        tab.clone(),
        vec![
            ("Info", InfoTab::Nil),
            (
                "Schedule",
                InfoTab::Agent(Tab::Person(app.primary.sim.trip_to_person(trip).unwrap())),
            ),
        ],
    ));

    let mut details: Option<TripDetails> = None;

    match tab {
        InfoTab::Nil => {
            rows.extend(action_btns);

            let (kv, extra) = app.primary.sim.ped_properties(id, &app.primary.map);
            rows.extend(make_table(ctx, kv));
            if !extra.is_empty() {
                let mut txt = Text::from(Line(""));
                for line in extra {
                    txt.add(Line(line));
                }
                rows.push(txt.draw(ctx));
            }

            let (more, trip_details) = trip_details(
                ctx,
                app,
                app.primary
                    .sim
                    .agent_to_trip(AgentID::Pedestrian(id))
                    .unwrap(),
                app.primary.sim.progress_along_path(AgentID::Pedestrian(id)),
            );
            rows.push(more);
            details = Some(trip_details);
        }
        InfoTab::Agent(Tab::Person(p)) => {
            rows.extend(person::info(ctx, app, p, None, Vec::new(), hyperlinks));
        }
        _ => unreachable!(),
    }

    (rows, details)
}

// TODO Embedded panel is perfect here
pub fn crowd_info(
    ctx: &EventCtx,
    _app: &App,
    members: Vec<PedestrianID>,
    header_btns: Widget,
    action_btns: Vec<Widget>,
) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line("Pedestrian crowd").roboto_bold().draw(ctx),
        header_btns,
    ]));
    rows.extend(action_btns);

    let mut txt = Text::new();
    txt.add(Line(format!("Crowd of {}", members.len())));
    rows.push(txt.draw(ctx));

    rows
}
