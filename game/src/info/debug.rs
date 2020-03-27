use crate::app::App;
use crate::info::{header_btns, make_table, Details};
use crate::render::ExtraShapeID;
use ezgui::{EventCtx, Line, Widget};
use map_model::AreaID;

pub fn area(ctx: &EventCtx, app: &App, _: &mut Details, id: AreaID) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Area #{}", id.0)).small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    let a = app.primary.map.get_a(id);
    let mut kv = Vec::new();
    for (k, v) in &a.osm_tags {
        kv.push((k.to_string(), v.to_string()));
    }
    rows.extend(make_table(ctx, kv));

    rows
}

pub fn extra_shape(ctx: &EventCtx, app: &App, _: &mut Details, id: ExtraShapeID) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(format!("Extra GIS shape #{}", id.0))
            .small_heading()
            .draw(ctx),
        header_btns(ctx),
    ]));

    let es = app.primary.draw_map.get_es(id);
    let mut kv = Vec::new();
    for (k, v) in &es.attributes {
        kv.push((k.to_string(), v.to_string()));
    }
    rows.extend(make_table(ctx, kv));

    rows
}
