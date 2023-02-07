use map_gui::tools::EditPolygon;
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State, TextExt, VerticalAlignment,
    Widget,
};

use crate::{mut_partitioning, App, Neighbourhood, NeighbourhoodID, Transition};

pub struct CustomizeBoundary {
    panel: Panel,
    edit: EditPolygon,
    id: NeighbourhoodID,
}

impl CustomizeBoundary {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighbourhoodID) -> Box<dyn State<App>> {
        let points = Neighbourhood::new(app, id)
            .boundary_polygon
            .into_outer_ring()
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
            edit: EditPolygon::new(ctx, points, false),
        })
    }
}

impl State<App> for CustomizeBoundary {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.edit.event(ctx);

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Save" => {
                    if let Ok(ring) = self.edit.get_ring() {
                        mut_partitioning!(app)
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
