use crate::app::App;
use crate::helpers::{ColorScheme, ID};
use crate::info::{header_btns, make_table, make_tabs, Details, Tab};
use crate::render::DrawPedestrian;
use ezgui::{Btn, EventCtx, Line, Text, TextExt, Widget};
use geom::{Angle, Time};
use map_model::{Building, BuildingID, LaneID, Traversable, SIDEWALK_THICKNESS};
use sim::{DrawPedestrianInput, PedestrianID, TripMode, TripResult};

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

    let cars = app.primary.sim.get_parked_cars_by_owner(id);
    if !cars.is_empty() {
        txt.add(Line(""));
        txt.add(Line(format!(
            "{} parked cars owned by this building",
            cars.len()
        )));
        // TODO Jump to it or see status
        for p in cars {
            txt.add(Line(format!("- {}", p.vehicle.id)));
        }
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
    // TODO Sort/group better
    // Show minimal info: ID, next departure time, type of that trip
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
                TripResult::TripDone => {}
                TripResult::TripDoesntExist => unreachable!(),
            }
        }

        let label = format!("Person #{}", p.0);
        details
            .hyperlinks
            .insert(label.clone(), Tab::PersonTrips(p));
        rows.push(Widget::col(vec![
            Btn::text_bg1(label).build_def(ctx, None),
            if let Some((t, mode)) = next_trip {
                format!("Leaving in {} to {}", t - app.primary.sim.time(), mode).draw_text(ctx)
            } else {
                "Staying inside".draw_text(ctx)
            },
        ]));
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
        Line(format!("Building #{}", id.0))
            .small_heading()
            .draw(ctx),
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

    // TODO On every tab?
    for p in app.primary.sim.get_parked_cars_by_owner(id) {
        let shape = app
            .primary
            .draw_map
            .get_obj(
                ID::Car(p.vehicle.id),
                app,
                &mut app.primary.draw_map.agents.borrow_mut(),
                ctx.prerender,
            )
            .unwrap()
            .get_outline(&app.primary.map);
        details.unzoomed.push(
            app.cs.get("something associated with something else"),
            shape.clone(),
        );
        details.zoomed.push(
            app.cs.get("something associated with something else"),
            shape,
        );
    }

    draw_occupants(
        details,
        &app.cs,
        app.primary.map.get_b(id),
        app.primary.sim.bldg_to_people(id).len(),
    );

    rows
}

fn draw_occupants(details: &mut Details, cs: &ColorScheme, bldg: &Building, num_ppl: usize) {
    // TODO Lots of fun ideas here. Have a deterministic simulation based on building ID and time
    // to have people "realistically" move around. Draw little floor plans.

    let num_rows_cols = (num_ppl as f64).sqrt().ceil() as usize;

    let ped_len = SIDEWALK_THICKNESS.inner_meters() / 2.0;
    let separation = ped_len * 1.5;

    let total_width_height = (num_rows_cols as f64) * (ped_len + separation);
    let top_left = bldg
        .label_center
        .offset(-total_width_height / 2.0, -total_width_height / 2.0);

    // TODO Current thing is inefficient and can easily wind up outside the building.

    let mut cnt = 0;
    'OUTER: for x in 0..num_rows_cols {
        for y in 0..num_rows_cols {
            DrawPedestrian::geometry(
                &mut details.zoomed,
                cs,
                &DrawPedestrianInput {
                    id: PedestrianID(cnt),
                    pos: top_left.offset(
                        (x as f64) * (ped_len + separation),
                        (y as f64) * (ped_len + separation),
                    ),
                    facing: Angle::new_degs(90.0),
                    waiting_for_turn: None,
                    preparing_bike: false,
                    // Both hands and feet!
                    waiting_for_bus: true,
                    on: Traversable::Lane(LaneID(0)),
                },
                0,
            );

            cnt += 1;
            if cnt == num_ppl {
                break 'OUTER;
            }
        }
    }
}
