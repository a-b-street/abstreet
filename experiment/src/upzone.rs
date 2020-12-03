use std::collections::HashSet;

use map_gui::load::MapLoader;
use map_gui::{SimpleApp, ID};
use map_model::BuildingID;
use widgetry::{
    Btn, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, TextExt,
    Transition, VerticalAlignment, Widget,
};

use crate::buildings::{BldgState, Buildings};
use crate::game::Game;
use crate::levels::Config;

const ZOOM: f64 = 2.0;

pub struct Picker {
    panel: Panel,
    config: Config,
    bldgs: Buildings,
    current_picks: HashSet<BuildingID>,
}

impl Picker {
    pub fn new(ctx: &mut EventCtx, app: &SimpleApp, config: Config) -> Box<dyn State<SimpleApp>> {
        MapLoader::new(
            ctx,
            app,
            config.map.clone(),
            Box::new(move |ctx, app| {
                ctx.canvas.cam_zoom = ZOOM;
                ctx.canvas.center_on_map_pt(app.map.get_bounds().center());

                // Just start playing immediately
                if config.num_upzones == 0 {
                    return Transition::Replace(Game::new(ctx, app, config, HashSet::new()));
                }

                let bldgs = Buildings::new(ctx, app, HashSet::new());

                Transition::Replace(Box::new(Picker {
                    panel: Panel::new(Widget::col(vec![
                        Line("Upzone").small_heading().draw(ctx),
                        format!(
                            "You can select {} houses to transform into stores",
                            config.num_upzones
                        )
                        .draw_text(ctx),
                        Btn::text_bg2("Start game").build_def(ctx, Key::Enter),
                    ]))
                    .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
                    .build(ctx),
                    config,
                    bldgs,
                    current_picks: HashSet::new(),
                }))
            }),
        )
    }
}

impl State<SimpleApp> for Picker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut SimpleApp) -> Transition<SimpleApp> {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.current_selection = app.mouseover_unzoomed_buildings(ctx).filter(|id| {
                match self.bldgs.buildings[&id.as_building()] {
                    BldgState::Undelivered(_) => true,
                    _ => false,
                }
            });
        }
        if let Some(ID::Building(b)) = app.current_selection {
            if ctx.normal_left_click() {
                if self.current_picks.contains(&b) {
                    self.current_picks.remove(&b);
                } else if self.current_picks.len() < self.config.num_upzones {
                    self.current_picks.insert(b);
                }
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Start game" => {
                    app.current_selection = None;
                    return Transition::Replace(Game::new(
                        ctx,
                        app,
                        self.config.clone(),
                        self.current_picks.clone(),
                    ));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &SimpleApp) {
        self.panel.draw(g);
        g.redraw(&self.bldgs.draw_all);
        for b in &self.current_picks {
            g.draw_polygon(Color::PINK, app.map.get_b(*b).polygon.clone());
        }
        // This covers up the current selection, so...
        if let Some(ID::Building(b)) = app.current_selection {
            g.draw_polygon(app.cs.selected, app.map.get_b(b).polygon.clone());
        }
    }
}
