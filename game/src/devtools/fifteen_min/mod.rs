//! This is a tool to experiment with the concept of 15-minute neighborhoods. Can you access your
//! daily needs (like groceries, a cafe, a library) within a 15-minute walk, bike ride, or public
//! transit ride of your home?
//!
//! See https://github.com/dabreegster/abstreet/issues/393 for more context.

use rand::seq::SliceRandom;

use geom::Pt2D;
use map_model::BuildingID;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome, Panel,
    RewriteColor, State, Text, VerticalAlignment, Widget,
};

use self::isochrone::Isochrone;
use crate::app::App;
use crate::game::Transition;
use crate::helpers::ID;

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
        for (amenity, buildings) in isochrone.amenities_reachable.borrow() {
            txt.add(Line(format!("{}: {}", amenity, buildings.len())));
        }
        rows.push(txt.draw(ctx));

        let panel = Panel::new(Widget::col(rows))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);

        // Draw a star on the start building.
        let highlight_start = draw_star(start.polygon.center(), ctx);

        Box::new(Viewer {
            panel,
            highlight_start: ctx.upload(highlight_start),
            isochrone,
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // Allow panning and zooming
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        if ctx.input.left_mouse_button_pressed() {
            if let Some(ID::Building(building_id)) = app.primary.current_selection.clone() {
                let building = app.primary.map.get_b(building_id);
                debug!("clicked on building: {:?}", building);
                self.isochrone = Isochrone::new(ctx, app, building_id);
                let star = draw_star(building.polygon.center(), ctx);
                self.highlight_start = ctx.upload(star);
            }
        }

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

fn draw_star(center: Pt2D, ctx: &mut EventCtx) -> GeomBatch {
    GeomBatch::load_svg(ctx.prerender, "system/assets/tools/star.svg")
        .centered_on(center)
        .color(RewriteColor::ChangeAll(Color::YELLOW))
}
