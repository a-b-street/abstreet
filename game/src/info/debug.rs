use map_model::AreaID;
use widgetry::{EventCtx, Line, StyledButtons, Widget};

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
        rows.push(
            ctx.style()
                .btn_solid_light_text("Open in OSM")
                .build_widget(ctx, &format!("open {}", osm_id)),
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

    rows
}
