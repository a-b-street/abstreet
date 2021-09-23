use std::collections::BTreeSet;

use map_gui::tools::ColorDiscrete;
use map_gui::ID;
use map_model::{IntersectionID, RoadID};
use widgetry::{
    Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Line, Panel, SimpleState, State,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

mod algorithms;

pub struct Viewer {
    neighborhood: Neighborhood,
    draw: Drawable,
}

struct Neighborhood {
    interior: BTreeSet<RoadID>,
    perimeter: BTreeSet<RoadID>,
    borders: BTreeSet<IntersectionID>,
}

impl Viewer {
    pub fn start_from_road(ctx: &mut EventCtx, app: &App, start: RoadID) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::from_road(&app.primary.map, start);
        let (draw, legend) = neighborhood.render(ctx, app);

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("LTN tool").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            legend,
            "Click a road to re-center".text_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(Viewer { neighborhood, draw }))
    }
}

impl SimpleState<App> for Viewer {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            _ => unreachable!(),
        }
    }

    fn on_mouseover(&mut self, ctx: &mut EventCtx, app: &mut App) {
        app.primary.current_selection = match app.mouseover_unzoomed_roads_and_intersections(ctx) {
            x @ Some(ID::Road(_)) => x,
            Some(ID::Lane(l)) => Some(ID::Road(l.road)),
            _ => None,
        };
    }
    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if let Some(ID::Road(r)) = app.primary.current_selection {
            if ctx.normal_left_click() {
                return Transition::Replace(Viewer::start_from_road(ctx, app, r));
            }
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw);
    }
}

impl Neighborhood {
    // Also a legend
    fn render(&self, ctx: &mut EventCtx, app: &App) -> (Drawable, Widget) {
        let interior = Color::BLUE;
        let perimeter = Color::RED;
        let border = Color::CYAN;
        let mut colorer = ColorDiscrete::new(
            app,
            vec![
                ("interior", interior),
                ("perimeter", perimeter),
                ("border", border),
            ],
        );
        for r in &self.interior {
            colorer.add_r(*r, "interior");
        }
        for r in &self.perimeter {
            colorer.add_r(*r, "perimeter");
        }
        for i in &self.borders {
            colorer.add_i(*i, "border");
        }
        let (unzoomed, _, legend) = colorer.build(ctx);
        (unzoomed, legend)
    }
}
