use crate::app::App;
use crate::info::{header_btns, make_table, Details};
use ezgui::{EventCtx, Line, Widget};
use map_model::AreaID;

pub fn area(ctx: &EventCtx, app: &App, _: &mut Details, id: AreaID) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row2(vec![
        Line(id.to_string()).small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    let a = app.primary.map.get_a(id);
    rows.extend(make_table(
        ctx,
        a.osm_tags
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string())),
    ));

    rows
}
