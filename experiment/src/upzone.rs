use std::collections::HashSet;

use map_gui::load::MapLoader;
use map_gui::{SimpleApp, ID};
use map_model::BuildingID;
use widgetry::{
    Btn, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State,
    TextExt, Transition, VerticalAlignment, Widget,
};

use crate::buildings::{BldgState, Buildings};
use crate::game::Game;
use crate::levels::Level;
use crate::vehicles::Vehicle;

const ZOOM: f64 = 2.0;

pub struct Picker {
    panel: Panel,
    level: Level,
    bldgs: Buildings,
    current_picks: HashSet<BuildingID>,
}

impl Picker {
    pub fn new(ctx: &mut EventCtx, app: &SimpleApp, level: Level) -> Box<dyn State<SimpleApp>> {
        MapLoader::new(
            ctx,
            app,
            level.map.clone(),
            Box::new(move |ctx, app| {
                ctx.canvas.cam_zoom = ZOOM;
                ctx.canvas.center_on_map_pt(app.map.get_bounds().center());

                // Just start playing immediately
                if level.num_upzones == 0 {
                    // TODO Maybe we still want to choose a vehicle
                    let vehicle = Vehicle::get(level.vehicles[0]);
                    return Transition::Replace(Game::new(
                        ctx,
                        app,
                        level,
                        vehicle,
                        HashSet::new(),
                    ));
                }

                let bldgs = Buildings::new(ctx, app, HashSet::new());

                Transition::Replace(Box::new(Picker {
                    panel: Panel::new(Widget::col(vec![
                        Line("Upzone").small_heading().draw(ctx),
                        format!(
                            "You can select {} houses to transform into stores",
                            level.num_upzones
                        )
                        .draw_text(ctx),
                        Widget::dropdown(
                            ctx,
                            "vehicle",
                            level.vehicles[0].to_string(),
                            Choice::strings(level.vehicles.clone()),
                        ),
                        Btn::text_bg2("Start game").build_def(ctx, Key::Enter),
                    ]))
                    .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
                    .build(ctx),
                    level,
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
                } else if self.current_picks.len() < self.level.num_upzones {
                    self.current_picks.insert(b);
                }
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Start game" => {
                    app.current_selection = None;
                    let vehicle: String = self.panel.dropdown_value("vehicle");
                    return Transition::Replace(Game::new(
                        ctx,
                        app,
                        self.level.clone(),
                        Vehicle::get(&vehicle),
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
