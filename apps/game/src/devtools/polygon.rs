use geom::LonLat;
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
                    .text("export as an Osmosis polygon filter")
                    // TODO Disable based on number of points
                    .hotkey(Key::X)
                    .build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            name,
            edit: EditPolygon::new(points),
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
                "export as an Osmosis polygon filter" => {
                    if self.edit.get_points().len() >= 3 {
                        let mut pts = app
                            .primary
                            .map
                            .get_gps_bounds()
                            .convert_back(self.edit.get_points());
                        // Have to repeat the first point
                        pts.push(pts[0]);
                        LonLat::write_osmosis_polygon(&format!("{}.poly", self.name), &pts)
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
