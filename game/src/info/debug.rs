use map_model::AreaID;
use widgetry::{Btn, EventCtx, Line, Widget};

use crate::app::App;
use crate::info::{header_btns, make_table, Details};

pub fn area(ctx: &EventCtx, app: &App, _: &mut Details, id: AreaID) -> Vec<Widget> {
    let mut rows = vec![];

    rows.push(Widget::row(vec![
        Line(id.to_string()).small_heading().draw(ctx),
        header_btns(ctx),
    ]));

    let area = app.primary.map.get_a(id);

    if let Some(osm_id) = area.osm_id {
        rows.push(Btn::text_bg1("Open in OSM").build(ctx, format!("open {}", osm_id), None));
    }

    rows.extend(make_table(
        ctx,
        area.osm_tags
            .inner()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    ));

    rows
}
