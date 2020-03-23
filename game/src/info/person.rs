use crate::app::App;
use ezgui::{Btn, EventCtx, Line, TextExt, Widget};
use sim::{PersonID, PersonState};

pub fn info(
    ctx: &EventCtx,
    app: &App,
    id: PersonID,
    // If None, then the panel is embedded
    header_btns: Option<Widget>,
    action_btns: Vec<Widget>,
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
            // TODO not the best tooltip, but easy to parse :(
            PersonState::Inside(b) => Btn::text_bg1(format!(
                "Currently inside {}",
                app.primary.map.get_b(b).just_address(&app.primary.map)
            ))
            .build(ctx, format!("examine Building #{}", b.0), None),
            PersonState::Trip(t) => format!("Currently doing Trip #{}", t.0).draw_text(ctx),
            PersonState::OffMap => "Currently outside the map boundaries".draw_text(ctx),
            PersonState::Limbo => "Currently in limbo -- they broke out of the Matrix! Woops. (A \
                                   bug occurred)"
                .draw_text(ctx),
        });
    }

    rows.push(Line("Schedule").roboto_bold().draw(ctx));
    for t in &person.trips {
        // TODO Still maybe unsafe? Check if trip has actually started or not
        // TODO Say where the trip goes, no matter what?
        let start_time = app.primary.sim.trip_start_time(*t);
        if app.primary.sim.time() < start_time {
            rows.push(
                format!("{}: Trip #{} will start", start_time.ampm_tostring(), t.0).draw_text(ctx),
            );
        } else {
            rows.push(Widget::row(vec![
                format!("{}: ", start_time.ampm_tostring()).draw_text(ctx),
                Btn::text_bg1(format!("Trip #{}", t.0))
                    .build(ctx, format!("examine Trip #{}", t.0), None)
                    .margin(5),
            ]));
        }
    }

    // TODO All the colorful side info

    rows
}
