use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use map_model::osm::OsmID;
use map_model::BuildingID;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Panel, RewriteColor,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::layer::{Layer, LayerOutcome};

/// A set of buildings that the player has starred, persisted as player data.
#[derive(Serialize, Deserialize)]
pub struct Favorites {
    pub buildings: BTreeSet<OsmID>,
}

impl Favorites {
    fn load(app: &App) -> Favorites {
        abstio::maybe_read_json::<Favorites>(Favorites::path(app), &mut Timer::throwaway())
            .unwrap_or_else(|_| Favorites {
                buildings: BTreeSet::new(),
            })
    }

    fn path(app: &App) -> String {
        let name = app.primary.map.get_name();
        abstio::path_player(format!("favorites/{}/{}.json", name.city, name.map))
    }

    pub fn contains(app: &App, b: BuildingID) -> bool {
        Favorites::load(app)
            .buildings
            .contains(&app.primary.map.get_b(b).orig_id)
    }

    pub fn add(app: &App, b: BuildingID) {
        let mut faves = Favorites::load(app);
        faves.buildings.insert(app.primary.map.get_b(b).orig_id);
        abstio::write_json(Favorites::path(app), &faves);
    }

    pub fn remove(app: &App, b: BuildingID) {
        let mut faves = Favorites::load(app);
        faves.buildings.remove(&app.primary.map.get_b(b).orig_id);
        abstio::write_json(Favorites::path(app), &faves);
    }
}

pub struct ShowFavorites {
    panel: Panel,
    draw: Drawable,
}

impl Layer for ShowFavorites {
    fn name(&self) -> Option<&'static str> {
        Some("favorites")
    }
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App, minimap: &Panel) -> Option<LayerOutcome> {
        Layer::simple_event(ctx, minimap, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        g.redraw(&self.draw);
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw);
    }
}

impl ShowFavorites {
    pub fn new(ctx: &mut EventCtx, app: &App) -> ShowFavorites {
        let mut batch = GeomBatch::new();
        for orig_id in Favorites::load(app).buildings.into_iter() {
            if let Some(b) = app.primary.map.find_b_by_osm_id(orig_id) {
                batch.append(
                    GeomBatch::load_svg(ctx, "system/assets/tools/star.svg")
                        .centered_on(app.primary.map.get_b(b).polygon.center())
                        .color(RewriteColor::ChangeAll(Color::YELLOW)),
                );
            }
        }

        let panel = Panel::new(Widget::row(vec![
            Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
            "Your favorite buildings".draw_text(ctx),
            Btn::close(ctx),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        ShowFavorites {
            panel,
            draw: ctx.upload(batch),
        }
    }
}
