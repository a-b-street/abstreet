//! This is a tool to experiment with the concept of 15-minute neighborhoods. Can you access your
//! daily needs (like groceries, a cafe, a library) within a 15-minute walk, bike ride, or public
//! transit ride of your home?
//!
//! See https://github.com/dabreegster/abstreet/issues/393 for more context.

use rand::seq::SliceRandom;

use map_model::BuildingID;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome, Panel,
    RewriteColor, State, Text, VerticalAlignment, Widget,
};

use self::isochrone::Isochrone;
use crate::app::App;
use crate::game::{Transition, PopupMsg};

mod isochrone;

/// This is the UI state for exploring the isochrone/walkshed from a single building.
pub struct Viewer {
    panel: Panel,
    highlight_start: Drawable,
    isochrone: Isochrone,
}

impl Viewer {
    /// Start with a random building
    pub fn random_start(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut rng = app.primary.current_flags.sim_flags.make_rng();
        let start = app.primary.map.all_buildings().choose(&mut rng).unwrap().id;
        Viewer::new(ctx, app, start)
    }

    pub fn new(ctx: &mut EventCtx, app: &App, start: BuildingID) -> Box<dyn State<App>> {
        let start = app.primary.map.get_b(start);
        let isochrone = Isochrone::new(ctx, app, start.id);

        let mut rows = Vec::new();
        
        rows.push(Widget::row(vec![
            Line("15-minute neighborhood explorer")
                .small_heading()
                .draw(ctx),
            Btn::close(ctx),
        ]));
        
        let mut txt = Text::from_all(vec![
            Line("Starting from: ").secondary(),
            Line(&start.address),
        ]);
        
        let about_button = Widget::row(vec![
            Btn::text_fg("About").build_def(ctx, None),
        ]);

        for (amenity, buildings) in isochrone.amenities_reachable.borrow() {
            txt.add(Line(format!("{}: {}", amenity, buildings.len())));
        }
        rows.push(txt.draw(ctx));
        rows.push(about_button);

        let panel = Panel::new(Widget::col(rows))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);

        // Draw a star on the start building.
        let highlight_start = GeomBatch::load_svg(ctx.prerender, "system/assets/tools/star.svg")
            .centered_on(start.polygon.center())
            .color(RewriteColor::ChangeAll(Color::YELLOW));

        Box::new(Viewer {
            panel,
            highlight_start: ctx.upload(highlight_start),
            isochrone,
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
                },
                "About" => {
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "15 minute neighborhoods",
                        vec![
                            "What if you could access most of your daily needs with a 15-minute walk or bike ride from your house?",
                            "Wouldn't it be nice to not rely on a climate unfriendly motor vehicle and get stuck in traffic for these simple errands?", 
                            "Different cities around the world are talking about what design and policy changes could lead to 15 minute neighborhoods.", 
                            "This tool lets you see what commercial amenities are near you right now, using data from OpenStreetMap.",
                        ],
                    ));
                },
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
