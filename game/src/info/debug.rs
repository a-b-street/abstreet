use crate::app::App;
use crate::info::make_table;
use crate::render::ExtraShapeID;
use ezgui::{EventCtx, Line, Widget};
use map_model::AreaID;

pub fn area(
    ctx: &EventCtx,
    app: &App,
    id: AreaID,
    header_btns: Widget,
    action_btns: Vec<Widget>,
) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Area #{}", id.0)).roboto_bold().draw(ctx),
        header_btns,
    ]));
    rows.extend(action_btns);

    let a = app.primary.map.get_a(id);
    let mut kv = Vec::new();
    for (k, v) in &a.osm_tags {
        kv.push((k.to_string(), v.to_string()));
    }
    rows.extend(make_table(ctx, kv));

    rows
}

pub fn extra_shape(
    ctx: &EventCtx,
    app: &App,
    id: ExtraShapeID,
    header_btns: Widget,
    action_btns: Vec<Widget>,
) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Extra GIS shape #{}", id.0))
            .roboto_bold()
            .draw(ctx),
        header_btns,
    ]));
    rows.extend(action_btns);

    let es = app.primary.draw_map.get_es(id);
    let mut kv = Vec::new();
    for (k, v) in &es.attributes {
        kv.push((k.to_string(), v.to_string()));
    }
    rows.extend(make_table(ctx, kv));

    rows
}
