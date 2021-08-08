use geom::{Bounds, Pt2D};
use map_gui::render::DrawOptions;
use map_gui::ID;
use widgetry::{
    Color, EventCtx, Filler, GfxCtx, HorizontalAlignment, Line, Panel, ScreenDims, ScreenPt, Text,
    VerticalAlignment, Widget,
};

use crate::app::App;

// TODO We could stylize more by fading the map out, then adding a circle of light with an outline

pub struct MagnifyingGlass {
    panel: Panel,
}

impl MagnifyingGlass {
    pub fn new(ctx: &mut EventCtx) -> MagnifyingGlass {
        MagnifyingGlass {
            panel: Panel::new_builder(
                Widget::col(vec![
                    Filler::fixed_dims(ScreenDims::new(300.0, 300.0)).named("glass"),
                    Text::new().into_widget(ctx).named("label"),
                ])
                .padding(16)
                .outline((4.0, Color::BLACK)),
            )
            .aligned(HorizontalAlignment::LeftInset, VerticalAlignment::TopInset)
            .build(ctx),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &App) {
        if ctx.redo_mouseover() {
            let mut label = Text::new();
            if let Some(ID::Road(r)) = app.mouseover_unzoomed_roads_and_intersections(ctx) {
                let road = app.primary.map.get_r(r);
                label.add_line(Line(road.get_name(app.opts.language.as_ref())).small_heading());
                // TODO Indicate which direction is uphill
                label.add_line(Line(format!(
                    "{}% incline",
                    (road.percent_incline.abs() * 100.0).round()
                )));
            } else {
                // TODO Jittery panel
            }
            let label = label.into_widget(ctx);
            self.panel.replace(ctx, "label", label);
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        let rect = self.panel.rect_of("glass");
        let zoom = 8.0;

        if let Some(pt) = g.canvas.get_cursor_in_map_space() {
            // This is some of the math from screen_to_map and center_on_map_pt.
            let mut bounds = Bounds::new();
            let cam_x = (pt.x() * zoom) - (rect.width() / 2.0);
            let cam_y = (pt.y() * zoom) - (rect.height() / 2.0);
            // Top-left
            bounds.update(Pt2D::new(cam_x / zoom, cam_y / zoom));
            // Bottom-right
            bounds.update(Pt2D::new(
                (rect.width() + cam_x) / zoom,
                (rect.height() + cam_y) / zoom,
            ));

            g.fork(
                Pt2D::new(bounds.min_x, bounds.min_y),
                ScreenPt::new(rect.x1, rect.y1),
                zoom,
                None,
            );
            g.enable_clipping(rect.clone());

            g.redraw(&app.primary.draw_map.boundary_polygon);
            g.redraw(&app.primary.draw_map.draw_all_areas);
            g.redraw(&app.primary.draw_map.draw_all_buildings);

            let opts = DrawOptions::new();
            for obj in app
                .primary
                .draw_map
                .get_renderables_back_to_front(bounds, &app.primary.map)
            {
                obj.draw(g, app, &opts);
            }

            g.disable_clipping();
            g.unfork();
        }
    }
}
