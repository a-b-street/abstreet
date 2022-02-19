use map_model::AreaID;
use widgetry::{EventCtx, Line, Widget};

use crate::app::App;
use crate::info::{header_btns, make_table, Details};

pub fn area(ctx: &EventCtx, app: &App, _: &mut Details, id: AreaID) -> Widget {
    let header = Widget::row(vec![
        Line(id.to_string()).small_heading().into_widget(ctx),
        header_btns(ctx),
    ]);

    Widget::custom_col(vec![header, area_body(ctx, app, id).tab_body(ctx)])
}

fn area_body(ctx: &EventCtx, app: &App, id: AreaID) -> Widget {
    let mut rows = vec![];
    let area = app.primary.map.get_a(id);

    if let Some(osm_id) = area.osm_id {
        rows.push(
            ctx.style()
                .btn_outline
                .text("Open in OSM")
                .build_widget(ctx, format!("open {}", osm_id)),
        );
    }

    rows.extend(make_table(
        ctx,
        area.osm_tags
            .inner()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    ));

    Widget::col(rows)
}
