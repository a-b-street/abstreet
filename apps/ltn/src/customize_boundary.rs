use geom::Ring;
use map_gui::tools::EditPolygon;
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State, TextExt, VerticalAlignment,
    Widget,
};

use crate::{App, NeighbourhoodID, Transition};

pub struct CustomizeBoundary {
    panel: Panel,
    edit: EditPolygon,
    id: NeighbourhoodID,
}

impl CustomizeBoundary {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighbourhoodID) -> Box<dyn State<App>> {
        let points = app
            .session
            .partitioning
            .neighbourhood_boundary_polygon(app, id)
            .get_outer_ring()
            .into_points();
        Box::new(Self {
            id,
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Customize boundary").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                "For drawing only. You can't edit roads outside the normal boundary"
                    .text_widget(ctx),
                ctx.style().btn_solid_primary.text("Save").build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            edit: EditPolygon::new(points),
        })
    }
}

impl State<App> for CustomizeBoundary {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.edit.event(ctx, app);

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Save" => {
                    let mut pts = self.edit.get_points().to_vec();
                    pts.push(pts[0]);
                    if let Ok(ring) = Ring::new(pts) {
                        app.session
                            .partitioning
                            .override_neighbourhood_boundary_polygon(self.id, ring.into_polygon());
                        return Transition::Multi(vec![Transition::Pop, Transition::Recreate]);
                    }
                    // Silently stay here so the user can try to fix
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
