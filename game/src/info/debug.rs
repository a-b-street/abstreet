use crate::app::App;
use crate::info::{header_btns, make_table, Details};
use map_model::AreaID;
use widgetry::{EventCtx, Line, Widget};

pub fn area(ctx: &EventCtx, app: &App, _: &mut Details, id: AreaID) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(id.to_string()).small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    let a = app.primary.map.get_a(id);
    rows.extend(make_table(
        ctx,
        a.osm_tags
            .inner()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    ));

    rows
}
