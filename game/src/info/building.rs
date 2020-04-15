use crate::app::App;
use crate::info::{header_btns, make_table, make_tabs, Details, Tab};
use crate::render::DrawPedestrian;
use ezgui::{Btn, Color, EventCtx, Line, Text, TextExt, Widget};
use geom::{Angle, Circle, Time};
use map_model::{BuildingID, LaneID, Traversable, SIDEWALK_THICKNESS};
use sim::{DrawPedestrianInput, PedestrianID, PersonID, TripMode, TripResult};
use std::collections::BTreeMap;

pub fn info(ctx: &mut EventCtx, app: &App, details: &mut Details, id: BuildingID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::BldgInfo(id));
    let b = app.primary.map.get_b(id);

    let mut kv = Vec::new();

    kv.push(("Address", b.just_address(&app.primary.map)));
    if let Some(name) = b.just_name() {
        kv.push(("Name", name.to_string()));
    }

    if let Some(ref p) = b.parking {
        kv.push(("Parking", format!("{} spots via {}", p.num_stalls, p.name)));
    } else {
        kv.push(("Parking", "None".to_string()));
    }

    rows.extend(make_table(ctx, kv));

    let mut txt = Text::new();

    if !b.amenities.is_empty() {
        txt.add(Line(""));
        if b.amenities.len() > 1 {
            txt.add(Line(format!("{} amenities:", b.amenities.len())));
        }
        for (name, amenity) in &b.amenities {
            txt.add(Line(format!("- {} (a {})", name, amenity)));
        }
    }

    let cars = app.primary.sim.get_offstreet_parked_cars(id);
    if !cars.is_empty() {
        txt.add(Line(""));
        txt.add(Line(format!("{} cars parked inside right now", cars.len())));
    }

    if !txt.is_empty() {
        rows.push(txt.draw(ctx))
    }

    rows
}

pub fn debug(ctx: &mut EventCtx, app: &App, details: &mut Details, id: BuildingID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::BldgDebug(id));
    let b = app.primary.map.get_b(id);

    rows.extend(make_table(
        ctx,
        vec![(
            "Dist along sidewalk",
            b.front_path.sidewalk.dist_along().to_string(),
        )],
    ));
    rows.push("Raw OpenStreetMap data".draw_text(ctx));
    rows.extend(make_table(ctx, b.osm_tags.clone().into_iter().collect()));

    rows
}

pub fn people(ctx: &mut EventCtx, app: &App, details: &mut Details, id: BuildingID) -> Vec<Widget> {
    let mut rows = header(ctx, app, details, id, Tab::BldgPeople(id));

    let mut ppl: Vec<(Time, Widget)> = Vec::new();
    for p in app.primary.sim.bldg_to_people(id) {
        let person = app.primary.sim.get_person(p);

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
                TripResult::TripDone | TripResult::TripAborted => {}
                TripResult::TripDoesntExist => unreachable!(),
            }
        }

        details
            .hyperlinks
            .insert(p.to_string(), Tab::PersonTrips(p, BTreeMap::new()));
        let widget = Widget::col(vec![
            Btn::text_bg1(p.to_string()).build_def(ctx, None),
            if let Some((t, mode)) = next_trip {
                format!(
                    "Leaving in {} to {}",
                    t - app.primary.sim.time(),
                    mode.verb()
                )
                .draw_text(ctx)
            } else {
                "Staying inside".draw_text(ctx)
            },
        ]);
        ppl.push((
            next_trip
                .map(|(t, _)| t)
                .unwrap_or(app.primary.sim.get_end_of_day()),
            widget,
        ));
    }
    // Sort by time to next trip
    ppl.sort_by_key(|(t, _)| *t);
    if ppl.is_empty() {
        rows.push("Nobody's inside right now".draw_text(ctx));
    } else {
        for (_, w) in ppl {
            rows.push(w);
        }
    }

    rows
}

fn header(
    ctx: &EventCtx,
    app: &App,
    details: &mut Details,
    id: BuildingID,
    tab: Tab,
) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(id.to_string()).small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    rows.push(make_tabs(
        ctx,
        &mut details.hyperlinks,
        tab,
        vec![
            ("Info", Tab::BldgInfo(id)),
            ("Debug", Tab::BldgDebug(id)),
            ("People", Tab::BldgPeople(id)),
        ],
    ));

    draw_occupants(details, app, id, None);
    // TODO Draw cars parked inside?

    rows
}

pub fn draw_occupants(details: &mut Details, app: &App, id: BuildingID, focus: Option<PersonID>) {
    // TODO Lots of fun ideas here. Have a deterministic simulation based on building ID and time
    // to have people "realistically" move around. Draw little floor plans.

    let mut ppl = app.primary.sim.bldg_to_people(id);
    let num_rows_cols = (ppl.len() as f64).sqrt().ceil() as usize;

    let ped_len = SIDEWALK_THICKNESS.inner_meters() / 2.0;
    let separation = ped_len * 1.5;

    let total_width_height = (num_rows_cols as f64) * (ped_len + separation);
    let top_left = app
        .primary
        .map
        .get_b(id)
        .label_center
        .offset(-total_width_height / 2.0, -total_width_height / 2.0);

    // TODO Current thing is inefficient and can easily wind up outside the building.

    'OUTER: for x in 0..num_rows_cols {
        for y in 0..num_rows_cols {
            let person = if let Some(p) = ppl.pop() {
                p
            } else {
                break 'OUTER;
            };
            let pos = top_left.offset(
                (x as f64) * (ped_len + separation),
                (y as f64) * (ped_len + separation),
            );

            if Some(person) == focus {
                details.zoomed.push(
                    Color::YELLOW.alpha(0.8),
                    Circle::new(pos, SIDEWALK_THICKNESS).to_polygon(),
                );
            }

            DrawPedestrian::geometry(
                &mut details.zoomed,
                &app.cs,
                &DrawPedestrianInput {
                    // Lies
                    id: PedestrianID(person.0),
                    pos,
                    facing: Angle::new_degs(90.0),
                    waiting_for_turn: None,
                    preparing_bike: false,
                    // Both hands and feet!
                    waiting_for_bus: true,
                    on: Traversable::Lane(LaneID(0)),
                },
                0,
            );
        }
    }
}
