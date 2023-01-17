use geom::{LonLat, Ring};
use map_gui::tools::EditPolygon;
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, VerticalAlignment,
    Widget,
};

use crate::app::App;
use crate::app::Transition;

pub struct PolygonEditor {
    panel: Panel,
    name: String,
    edit: EditPolygon,
}

impl PolygonEditor {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        name: String,
        points: Vec<LonLat>,
    ) -> Box<dyn State<App>> {
        let points = app.primary.map.get_gps_bounds().convert(&points);
        Box::new(Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Polygon editor").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                ctx.style()
                    .btn_outline
                    .text("export as a GeoJSON boundary")
                    // TODO Disable based on number of points
                    .hotkey(Key::X)
                    .build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            name,
            edit: EditPolygon::new(ctx, app, points, true),
        })
    }
}

impl State<App> for PolygonEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.edit.event(ctx, app);

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "export as a GeoJSON boundary" => {
                    if let Ok(ring) = self.edit.get_ring() {
                        let polygon = ring
                            .into_polygon()
                            .to_geojson(Some(app.primary.map.get_gps_bounds()));
                        let gj = geom::geometries_with_properties_to_geojson(vec![(
                            polygon,
                            serde_json::Map::new(),
                        )]);
                        abstio::write_file(
                            format!("{}.geojson", self.name),
                            abstutil::to_json(&gj),
                        )
                        .unwrap();
                    }
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.edit.draw(g);
    }
}
