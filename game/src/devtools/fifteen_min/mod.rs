//! This is a tool to experiment with the concept of 15-minute neighborhoods. Can you access your
//! daily needs (like groceries, a cafe, a library) within a 15-minute walk, bike ride, or public
//! transit ride of your home?
//!
//! See https://github.com/dabreegster/abstreet/issues/393 for more context.

use map_model::BuildingID;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome, Panel,
    RewriteColor, State, Text, VerticalAlignment, Widget,
};

use self::isochrone::Isochrone;
use crate::app::App;
use crate::game::Transition;

mod isochrone;

/// This is the UI state for exploring the isochrone/walkshed from a single building.
pub struct Viewer {
    panel: Panel,
    highlight_start: Drawable,
    isochrone: Isochrone,
}

impl Viewer {
    pub fn new(ctx: &mut EventCtx, app: &App, start: BuildingID) -> Box<dyn State<App>> {
        let start = app.primary.map.get_b(start);

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("15-minute neighborhood explorer")
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            Text::from_all(vec![
                Line("Starting from: ").secondary(),
                Line(&start.address),
            ])
            .draw(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);

        // Draw a star on the start building.
        let highlight_start = GeomBatch::load_svg(ctx.prerender, "system/assets/tools/star.svg")
            .centered_on(start.polygon.center())
            .color(RewriteColor::ChangeAll(Color::YELLOW));

        Box::new(Viewer {
            panel,
            highlight_start: ctx.upload(highlight_start),
            isochrone: Isochrone::new(ctx, app, start.id),
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        // Allow panning and zooming
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.isochrone.draw);
        g.redraw(&self.highlight_start);
        self.panel.draw(g);
    }
}
