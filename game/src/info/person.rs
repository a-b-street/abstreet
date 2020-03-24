use crate::app::App;
use crate::helpers::ID;
use crate::info::InfoTab;
use ezgui::{Btn, EventCtx, Line, TextExt, Widget};
use sim::{PersonID, PersonState};
use std::collections::HashMap;

pub fn info(
    ctx: &EventCtx,
    app: &App,
    id: PersonID,
    // If None, then the panel is embedded
    header_btns: Option<Widget>,
    action_btns: Vec<Widget>,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
) -> Vec<Widget> {
    let mut rows = vec![];

    // Header
    let standalone = header_btns.is_some();
    if let Some(btns) = header_btns {
        rows.push(Widget::row(vec![
            Line(format!("Person #{}", id.0)).roboto_bold().draw(ctx),
            btns,
        ]));
    } else {
        rows.push(Line(format!("Person #{}", id.0)).roboto_bold().draw(ctx));
    }
    rows.extend(action_btns);

    let person = app.primary.sim.get_person(id);

    // TODO Redundant to say they're inside when the panel is embedded. But... if the person leaves
    // while we have the panel open, then it IS relevant.
    if standalone {
        // TODO Point out where the person is now, relative to schedule...
        rows.push(match person.state {
            PersonState::Inside(b) => {
                // TODO not the best tooltip, but disambiguous:(
                hyperlinks.insert(
                    format!("examine Building #{}", b.0),
                    (ID::Building(b), InfoTab::Nil),
                );
                Btn::text_bg1(format!(
                    "Currently inside {}",
                    app.primary.map.get_b(b).just_address(&app.primary.map)
                ))
                .build(ctx, format!("examine Building #{}", b.0), None)
            }
            PersonState::Trip(t) => format!("Currently doing Trip #{}", t.0).draw_text(ctx),
            PersonState::OffMap => "Currently outside the map boundaries".draw_text(ctx),
            PersonState::Limbo => "Currently in limbo -- they broke out of the Matrix! Woops. (A \
                                   bug occurred)"
                .draw_text(ctx),
        });
    }

    rows.push(Line("Schedule").roboto_bold().draw(ctx));
    for t in &person.trips {
        let start_time = app.primary.sim.trip_start_time(*t);
        hyperlinks.insert(
            format!("examine Trip #{}", t.0),
            (ID::Trip(*t), InfoTab::Nil),
        );
        rows.push(Widget::row(vec![
            format!("{}: ", start_time.ampm_tostring()).draw_text(ctx),
            Btn::text_bg1(format!("Trip #{}", t.0))
                .build(ctx, format!("examine Trip #{}", t.0), None)
                .margin(5),
        ]));
    }

    // TODO All the colorful side info

    rows
}
