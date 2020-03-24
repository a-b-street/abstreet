use crate::app::App;
use crate::colors;
use crate::helpers::ID;
use crate::info::{make_table, person, InfoTab};
use ezgui::{hotkey, Btn, EventCtx, GeomBatch, Key, Line, Text, TextExt, Widget};
use map_model::BuildingID;

pub fn info(
    ctx: &EventCtx,
    app: &App,
    id: BuildingID,
    tab: InfoTab,
    header_btns: Widget,
    action_btns: Vec<Widget>,
    batch: &mut GeomBatch,
) -> Vec<Widget> {
    let mut rows = vec![];

    let b = app.primary.map.get_b(id);

    rows.push(Widget::row(vec![
        Line(format!("Building #{}", id.0)).roboto_bold().draw(ctx),
        header_btns,
    ]));
    rows.extend(action_btns);

    // Properties
    {
        let mut kv = Vec::new();

        kv.push(("Address".to_string(), b.just_address(&app.primary.map)));
        if let Some(name) = b.just_name() {
            kv.push(("Name".to_string(), name.to_string()));
        }

        if let Some(ref p) = b.parking {
            kv.push((
                "Parking".to_string(),
                format!("{} spots via {}", p.num_stalls, p.name),
            ));
        } else {
            kv.push(("Parking".to_string(), "None".to_string()));
        }

        if app.opts.dev {
            kv.push((
                "Dist along sidewalk".to_string(),
                b.front_path.sidewalk.dist_along().to_string(),
            ));

            for (k, v) in &b.osm_tags {
                kv.push((k.to_string(), v.to_string()));
            }
        }

        rows.extend(make_table(ctx, kv));
    }

    let mut txt = Text::new();
    let trip_lines = app.primary.sim.count_trips_involving_bldg(id).describe();
    if !trip_lines.is_empty() {
        txt.add(Line(""));
        for line in trip_lines {
            txt.add(Line(line));
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

    if !b.amenities.is_empty() {
        txt.add(Line(""));
        if b.amenities.len() > 1 {
            txt.add(Line(format!("{} amenities:", b.amenities.len())));
        }
        for (name, amenity) in &b.amenities {
            txt.add(Line(format!("- {} (a {})", name, amenity)));
        }
    }

    if !txt.is_empty() {
        rows.push(txt.draw(ctx))
    }

    match tab {
        InfoTab::Nil => {
            let num = app.primary.sim.bldg_to_people(id).len();
            if num > 0 {
                rows.push(
                    Btn::text_bg1(format!("{} people inside", num))
                        .build(ctx, "examine people inside", None)
                        .margin(5),
                );
            }
        }
        InfoTab::BldgPeople(ppl, idx) => {
            let mut inner = vec![
                // TODO Keys are weird! But left/right for speed
                Widget::row(vec![
                    Btn::text_fg("<")
                        .build(ctx, "previous", hotkey(Key::UpArrow))
                        .margin(5),
                    format!("Occupant {}/{}", idx + 1, ppl.len()).draw_text(ctx),
                    Btn::text_fg(">")
                        .build(ctx, "next", hotkey(Key::DownArrow))
                        .margin(5),
                    Btn::text_fg("X")
                        .build(ctx, "close occupants panel", None)
                        .align_right(),
                ])
                .centered(),
            ];
            inner.extend(person::info(ctx, app, ppl[idx], None, Vec::new()));
            rows.push(Widget::col(inner).bg(colors::INNER_PANEL_BG));
        }
        _ => unreachable!(),
    }

    for p in app.primary.sim.get_parked_cars_by_owner(id) {
        batch.push(
            app.cs.get("something associated with something else"),
            app.primary
                .draw_map
                .get_obj(
                    ID::Car(p.vehicle.id),
                    app,
                    &mut app.primary.draw_map.agents.borrow_mut(),
                    ctx.prerender,
                )
                .unwrap()
                .get_outline(&app.primary.map),
        );
    }

    rows
}
