use crate::app::App;
use crate::info::{header_btns, make_tabs, Details, Tab};
use ezgui::{EventCtx, Line, TextExt, Widget};
use map_model::ParkingLotID;

pub fn info(ctx: &mut EventCtx, app: &App, details: &mut Details, id: ParkingLotID) -> Vec<Widget> {
    let mut rows = header(ctx, details, id, Tab::ParkingLot(id));
    let pl = app.primary.map.get_pl(id);

    if let Some(n) = pl.capacity {
        rows.push(format!("{} spots (from OSM)", n).draw_text(ctx));
    }

    // 250 square feet is around 23 square meters
    rows.push(format!("{} spots (from area)", (pl.polygon.area() / 23.0) as usize).draw_text(ctx));

    rows.push(
        format!(
            "{} spots (from geometry)",
            app.primary.draw_map.get_pl(id).inferred_spots
        )
        .draw_text(ctx),
    );

    rows
}

fn header(ctx: &EventCtx, details: &mut Details, id: ParkingLotID, tab: Tab) -> Vec<Widget> {
    vec![
        Widget::row(vec![
            Line(id.to_string()).small_heading().draw(ctx),
            header_btns(ctx),
        ]),
        make_tabs(
            ctx,
            &mut details.hyperlinks,
            tab,
            vec![("Info", Tab::ParkingLot(id))],
        ),
    ]
}
