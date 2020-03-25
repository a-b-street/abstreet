use crate::app::App;
use crate::colors;
use crate::helpers::ID;
use crate::info::{make_browser, make_table, make_tabs, person, InfoTab};
use ezgui::{EventCtx, GeomBatch, Line, Text, TextExt, Widget};
use map_model::BuildingID;
use sim::{PersonID, TripEndpoint};
use std::collections::HashMap;

#[derive(Clone)]
pub enum Tab {
    // If we're live updating, the people inside could change! We're choosing to freeze the list
    // here.
    People(Vec<PersonID>, usize),
    Debug,
}
impl std::cmp::PartialEq for Tab {
    fn eq(&self, other: &Tab) -> bool {
        match (self, other) {
            (Tab::People(_, _), Tab::People(_, _)) => true,
            (Tab::Debug, Tab::Debug) => true,
            _ => false,
        }
    }
}

pub fn info(
    ctx: &mut EventCtx,
    app: &App,
    id: BuildingID,
    tab: InfoTab,
    header_btns: Widget,
    action_btns: Vec<Widget>,
    batch: &mut GeomBatch,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
    warpers: &mut HashMap<String, ID>,
) -> Vec<Widget> {
    let mut rows = vec![];

    let b = app.primary.map.get_b(id);

    rows.push(Widget::row(vec![
        Line(format!("Building #{}", id.0)).roboto_bold().draw(ctx),
        header_btns,
    ]));

    rows.push(make_tabs(ctx, hyperlinks, ID::Building(id), tab.clone(), {
        let mut tabs = vec![("Info", InfoTab::Nil), ("Debug", InfoTab::Bldg(Tab::Debug))];

        let ppl = app.primary.sim.bldg_to_people(id);
        if !ppl.is_empty() {
            tabs.push(("People", InfoTab::Bldg(Tab::People(ppl, 0))));
        }

        tabs
    }));

    match tab {
        InfoTab::Nil => {
            rows.extend(action_btns);

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

            // TODO Rethink this
            let trip_lines = app
                .primary
                .sim
                .count_trips(TripEndpoint::Bldg(id))
                .describe();
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

            if !txt.is_empty() {
                rows.push(txt.draw(ctx))
            }
        }
        InfoTab::Bldg(Tab::Debug) => {
            rows.extend(make_table(
                ctx,
                vec![(
                    "Dist along sidewalk",
                    b.front_path.sidewalk.dist_along().to_string(),
                )],
            ));
            rows.push("Raw OpenStreetMap data".draw_text(ctx));
            rows.extend(make_table(ctx, b.osm_tags.clone().into_iter().collect()));
        }
        InfoTab::Bldg(Tab::People(ppl, idx)) => {
            let mut inner = vec![make_browser(
                ctx,
                hyperlinks,
                "Occupant",
                ppl.len(),
                idx,
                |n| (ID::Building(id), InfoTab::Bldg(Tab::People(ppl.clone(), n))),
            )];
            inner.extend(person::info(
                ctx,
                app,
                ppl[idx],
                None,
                Vec::new(),
                hyperlinks,
                warpers,
            ));
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
